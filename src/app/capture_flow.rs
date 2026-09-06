//! Park / restore the studio window for capture without `Visible(false)`.
//!
//! `Visible(false)` destroys child viewports (region overlay, REC bar) on Windows.
//! Capture hide parks the main window off-screen and keeps it ordered-in.

use eframe::egui::{Context, Pos2, UserAttentionType, Vec2, ViewportCommand};

/// Record current outer geometry, then park far off-screen.
pub fn park_offscreen(ctx: &Context, pre_outer: &mut Option<Pos2>, pre_size: &mut Option<Vec2>) {
    let outer = ctx.input(|i| i.viewport().outer_rect);
    if let Some(rect) = outer {
        *pre_outer = Some(rect.min);
        *pre_size = Some(rect.size());
    } else if pre_outer.is_none() {
        *pre_outer = Some(Pos2::new(120.0, 80.0));
        *pre_size = Some(Vec2::new(760.0, 640.0));
    }
    ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Pos2::new(-12_000.0, -12_000.0)));
    ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(120.0, 80.0)));
    ctx.request_repaint();
}

/// Restore parked geometry and focus the studio.
pub fn restore_parked(ctx: &Context, pre_outer: &mut Option<Pos2>, pre_size: &mut Option<Vec2>) {
    if let (Some(pos), Some(size)) = (pre_outer.take(), pre_size.take()) {
        let size = Vec2::new(size.x.max(640.0), size.y.max(480.0));
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
}
