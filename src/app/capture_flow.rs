//! Hide / restore the studio for capture without dropping the taskbar button.
//!
//! Windows: `Visible(false)` kills child viewports (region overlay, REC bar)
//! and removes the taskbar ("quickbar") entry. Off-screen park is clamped back
//! onto the desktop, so the studio appears in gdigrab. Minimize instead.
//!
//! macOS/Linux: park off-screen (minimize/orderOut is hard to reverse on winit).

use eframe::egui::{Context, Pos2, UserAttentionType, Vec2, ViewportCommand};

/// Snapshot geometry only when the window still looks like the studio, not a park.
pub fn should_snapshot_geometry(already_parked: bool, size: Option<Vec2>) -> bool {
    if already_parked {
        return false;
    }
    match size {
        Some(s) => s.x >= 400.0 && s.y >= 300.0,
        None => true,
    }
}

/// Record current outer geometry, then hide so it is not in the shot.
pub fn park_offscreen(ctx: &Context, pre_outer: &mut Option<Pos2>, pre_size: &mut Option<Vec2>) {
    let outer = ctx.input(|i| i.viewport().outer_rect);
    let size = outer.map(|r| r.size());
    if should_snapshot_geometry(pre_outer.is_some(), size) {
        if let Some(rect) = outer {
            *pre_outer = Some(rect.min);
            *pre_size = Some(rect.size());
        } else if pre_outer.is_none() {
            *pre_outer = Some(Pos2::new(120.0, 80.0));
            *pre_size = Some(Vec2::new(1160.0, 800.0));
        }
    }

    #[cfg(windows)]
    {
        // Keep Visible(true) + a taskbar button. Minimize removes pixels from gdigrab.
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        crate::platform::minimize_studio();
        ctx.request_repaint();
        return;
    }

    #[cfg(not(windows))]
    {
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Pos2::new(-12_000.0, -12_000.0)));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(120.0, 80.0)));
        ctx.request_repaint();
    }
}

/// Restore parked geometry and focus the studio (taskbar + foreground).
pub fn restore_parked(ctx: &Context, pre_outer: &mut Option<Pos2>, pre_size: &mut Option<Vec2>) {
    if let (Some(pos), Some(size)) = (pre_outer.take(), pre_size.take()) {
        let size = Vec2::new(size.x.max(640.0), size.y.max(480.0));
        // Reject leftover park coords if a second hide overwrote them.
        let pos = if pos.x < -2000.0 || pos.y < -2000.0 {
            Pos2::new(80.0, 60.0)
        } else {
            pos
        };
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
    }
    ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(ViewportCommand::Focus);
    ctx.send_viewport_cmd(ViewportCommand::RequestUserAttention(
        UserAttentionType::Informational,
    ));
    crate::platform::activate_own_app();
    ctx.request_repaint();
}

/// True while a capture hide should still own `pre_capture_outer`.
#[cfg(debug_assertions)]
pub fn capture_in_flight(
    screenshot: bool,
    recording: bool,
    arming: bool,
    selecting_region: bool,
    region_snap_pending: bool,
) -> bool {
    screenshot || recording || arming || selecting_region || region_snap_pending
}

#[cfg(debug_assertions)]
pub fn assert_unparked(pre_outer: &Option<Pos2>, in_flight: bool) {
    if !in_flight {
        debug_assert!(
            pre_outer.is_none(),
            "studio still parked after capture ended — restore every error/cancel/success path"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_flight_covers_region_snap() {
        assert!(capture_in_flight(false, false, false, false, true));
        assert!(!capture_in_flight(false, false, false, false, false));
    }

    #[test]
    fn snapshot_skips_already_parked_and_tiny_windows() {
        assert!(!should_snapshot_geometry(true, Some(Vec2::new(1160.0, 800.0))));
        assert!(!should_snapshot_geometry(false, Some(Vec2::new(120.0, 80.0))));
        assert!(should_snapshot_geometry(false, Some(Vec2::new(1160.0, 800.0))));
    }
}
