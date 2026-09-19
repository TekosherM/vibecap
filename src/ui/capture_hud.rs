//! Capture HUD family — region selector with thirds grid, handles, W×H, cursor loupe.
//! Extracted from main so Phase 3 chrome stays out of the eframe loop body.

use eframe::egui;
use egui::{
    Align2, Color32, FontId, Frame, Pos2, Rect, RichText, Sense, Stroke, Vec2, ViewportBuilder,
    ViewportClass, ViewportId,
};

use crate::ui::theme;

/// Outcome of one frame of the region-select overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionHudResult {
    /// Still selecting.
    Continue,
    /// User finished a drag — `selected` is the rect in overlay coords.
    Confirmed { selected: Rect, overlay: Rect },
    /// Esc / cancel.
    Cancelled,
}

/// Map a selection in overlay-local points onto the backdrop image in pixels.
///
/// `sel` and `overlay` are in the same egui coordinate space. Returns `(w,h,x,y)`
/// evened for yuv420p.
pub fn overlay_rect_to_pixels(
    sel: Rect,
    overlay: Rect,
    img_w: u32,
    img_h: u32,
) -> (i32, i32, i32, i32) {
    if overlay.width() < 1.0 || overlay.height() < 1.0 || img_w == 0 || img_h == 0 {
        return (2, 2, 0, 0);
    }
    let nx0 = ((sel.min.x - overlay.min.x) / overlay.width()).clamp(0.0, 1.0);
    let ny0 = ((sel.min.y - overlay.min.y) / overlay.height()).clamp(0.0, 1.0);
    let nx1 = ((sel.max.x - overlay.min.x) / overlay.width()).clamp(0.0, 1.0);
    let ny1 = ((sel.max.y - overlay.min.y) / overlay.height()).clamp(0.0, 1.0);
    let x0 = (nx0 * img_w as f32).floor() as i32;
    let y0 = (ny0 * img_h as f32).floor() as i32;
    let x1 = (nx1 * img_w as f32).ceil() as i32;
    let y1 = (ny1 * img_h as f32).ceil() as i32;
    crate::platform::even_screen_rect(x0, y0, (x1 - x0).max(2), (y1 - y0).max(2)).as_whxy()
}

/// Inverse of [`overlay_rect_to_pixels`] — map a stored pixel crop onto overlay points.
pub fn pixels_to_overlay_rect(
    crop_whxy: (i32, i32, i32, i32),
    overlay: Rect,
    img_w: u32,
    img_h: u32,
) -> Rect {
    let (w, h, x, y) = crop_whxy;
    if overlay.width() < 1.0 || overlay.height() < 1.0 || img_w == 0 || img_h == 0 {
        return overlay;
    }
    let nx0 = (x as f32 / img_w as f32).clamp(0.0, 1.0);
    let ny0 = (y as f32 / img_h as f32).clamp(0.0, 1.0);
    let nx1 = ((x + w) as f32 / img_w as f32).clamp(0.0, 1.0);
    let ny1 = ((y + h) as f32 / img_h as f32).clamp(0.0, 1.0);
    Rect::from_min_max(
        Pos2::new(
            overlay.min.x + nx0 * overlay.width(),
            overlay.min.y + ny0 * overlay.height(),
        ),
        Pos2::new(
            overlay.min.x + nx1 * overlay.width(),
            overlay.min.y + ny1 * overlay.height(),
        ),
    )
}

/// Paint fullscreen region selector. Caller owns start/end state.
///
/// Always a dedicated immediate viewport — never the main window. Painting the
/// overlay as the app's own `CentralPanel` (and maximizing) is what made region
/// pick feel like the studio hijacked itself.
///
/// `last_region` — optional ghost of the previous selection (session memory).
/// `backdrop` — frozen desktop still. Used on Windows/Linux where a *transparent*
/// overlay does not composite; the viewport itself stays opaque in that case.
/// `window_pick` — when `Some`, the overlay is a click-to-pick target: it
/// highlights `(title, x, y, w, h)` in OS pixels and a click confirms that
/// rect instead of a drag.
/// `aspect_lock` — `Some(w/h)` pins the drag to a ratio (toolbar chips);
/// Shift/Alt remain momentary overrides.
/// `backdrop_stale` — the shown backdrop is last pick's snap; stamps
/// "refreshing…" until the fresh one lands (caller blocks confirms).
/// `window_pick_cycle` — scroll-wheel index into overlapping windows.
#[allow(clippy::too_many_arguments)]
pub fn show_region_selector(
    ctx: &egui::Context,
    region_start: &mut Option<Pos2>,
    region_end: &mut Option<Pos2>,
    last_pixels: Option<(i32, i32, i32, i32)>,
    last_region: Option<Rect>,
    backdrop: Option<&egui::TextureHandle>,
    backdrop_rgba: Option<&(u32, u32, Vec<u8>)>,
    backdrop_stale: bool,
    was_dragging: &mut bool,
    window_pick: Option<&(String, i32, i32, i32, i32)>,
    aspect_lock: &mut Option<f32>,
    window_pick_cycle: &mut usize,
) -> RegionHudResult {
    let mut result = RegionHudResult::Continue;
    let opaque = backdrop.is_some() || cfg!(target_os = "windows");

    let ppp = ctx.pixels_per_point().max(1.0);
    let mut builder = ViewportBuilder::default()
        .with_title("Vibecap Region")
        .with_decorations(false)
        .with_transparent(!opaque)
        .with_always_on_top();
    let mons = crate::platform::list_monitors();
    let mut origin = (0i32, 0i32);
    if !mons.is_empty() {
        let x = mons.iter().map(|m| m.x).min().unwrap_or(0);
        let y = mons.iter().map(|m| m.y).min().unwrap_or(0);
        let r = mons.iter().map(|m| m.x + m.w).max().unwrap_or(1920);
        let b = mons.iter().map(|m| m.y + m.h).max().unwrap_or(1080);
        origin = (x, y);
        builder = builder
            .with_fullscreen(false)
            .with_position(Pos2::new(x as f32 / ppp, y as f32 / ppp))
            .with_inner_size(Vec2::new((r - x) as f32 / ppp, (b - y) as f32 / ppp));
    } else {
        builder = builder.with_fullscreen(true);
    }

    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("region_selector"),
        builder,
        |ctx, class| {
            if class != ViewportClass::Immediate {
                return;
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            let panel_frame = Frame::none().fill(if backdrop.is_some() {
                Color32::BLACK
            } else {
                theme::OVERLAY_DIM()
            });
            egui::CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
                let screen = response.rect;
                if let Some(tex) = backdrop {
                    painter.image(
                        tex.id(),
                        screen,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                // Pre-warm stamp: the shown backdrop is the previous pick's
                // snap — the fresh one is in flight behind it.
                if backdrop_stale {
                    painter.text(
                        Pos2::new(screen.center().x, screen.min.y + 76.0),
                        Align2::CENTER_CENTER,
                        "⟳ refreshing…",
                        FontId::proportional(14.0),
                        theme::TEXT_MUTED(),
                    );
                }

                // Window-pick mode: the hover rect arrives in OS pixels — map it
                // into overlay points. Click confirms that rect; drag is off.
                let pick_mode = window_pick.is_some();
                let pick_rect = window_pick.map(|(_, wx, wy, ww, wh)| {
                    let vppp = ctx.pixels_per_point().max(1.0);
                    Rect::from_min_size(
                        Pos2::new(
                            (*wx - origin.0) as f32 / vppp,
                            (*wy - origin.1) as f32 / vppp,
                        ),
                        Vec2::new(*ww as f32 / vppp, *wh as f32 / vppp),
                    )
                });
                if pick_mode {
                    // Scroll wheel cycles through overlapping windows under
                    // the cursor (Z-order); a cursor move resets to topmost.
                    let scroll = ctx.input(|i| i.raw_scroll_delta.y);
                    if scroll < -1.0 {
                        *window_pick_cycle = window_pick_cycle.saturating_add(1);
                    } else if scroll > 1.0 {
                        *window_pick_cycle = window_pick_cycle.saturating_sub(1);
                    }
                    painter.text(
                        Pos2::new(screen.center().x, screen.min.y + 48.0),
                        Align2::CENTER_CENTER,
                        "Click a window to capture · scroll cycles overlaps · dead space = monitor · Esc cancel",
                        FontId::proportional(18.0),
                        theme::TEXT(),
                    );
                    if let (Some(rect), Some((title, _, _, ww, wh))) = (pick_rect, window_pick)
                    {
                        painter.rect_filled(rect, 0.0, theme::ACCENT().gamma_multiply(0.15));
                        painter.rect_stroke(rect, 0.0, Stroke::new(2.5_f32, theme::ACCENT()));
                        let label = format!("{title}  {ww}×{wh}");
                        let label_pos = rect.left_top() + Vec2::new(6.0, -28.0);
                        let galley = painter.layout_no_wrap(
                            label,
                            FontId::proportional(13.0),
                            theme::ON_SOLID(),
                        );
                        let pad = Vec2::new(8.0, 4.0);
                        let mut plate =
                            Rect::from_min_size(label_pos, galley.size() + pad * 2.0);
                        if plate.min.y < 4.0 {
                            plate = plate.translate(Vec2::new(0.0, rect.height() + 32.0));
                        }
                        painter.rect_filled(plate, 4.0, theme::ACCENT());
                        painter.galley(plate.min + pad, galley, theme::ON_SOLID());
                    }
                    if response.clicked() {
                        if let Some(rect) = pick_rect {
                            result =
                                RegionHudResult::Confirmed { selected: rect, overlay: screen };
                        }
                    }
                }

                // Ghost of last region in pixel space (falls back to overlay points).
                let ghost = if region_start.is_none() {
                    let (img_w, img_h) = backdrop_rgba
                        .map(|(w, h, _)| (*w, *h))
                        .unwrap_or((0, 0));
                    let g = if let Some(crop) = last_pixels {
                        if img_w > 0 && img_h > 0 {
                            Some(pixels_to_overlay_rect(crop, screen, img_w, img_h))
                        } else {
                            last_region
                        }
                    } else {
                        last_region
                    };
                    g.filter(|r| r.width() >= 8.0 && r.height() >= 8.0)
                } else {
                    None
                };
                if !pick_mode && region_start.is_none() {
                    if let Some(ghost) = ghost {
                        painter.rect_stroke(
                            ghost,
                            0.0,
                            Stroke::new(1.5_f32, theme::TEXT_MUTED()),
                        );
                        painter.text(
                            ghost.center(),
                            Align2::CENTER_CENTER,
                            "Last region · R / double-click captures · ←↑↓→ nudge · drag replaces",
                            FontId::proportional(13.0),
                            theme::TEXT_MUTED(),
                        );
                    }
                    painter.text(
                        Pos2::new(screen.center().x, screen.min.y + 48.0),
                        Align2::CENTER_CENTER,
                        "Drag to select · release captures · Esc / right-click cancel",
                        FontId::proportional(18.0),
                        theme::TEXT(),
                    );
                }
                // Repeat-last gestures: `R` or a double-click inside the ghost
                // confirms it without re-dragging.
                if !pick_mode {
                    if let Some(ghost) = ghost {
                        let repeat = ctx.input(|i| i.key_pressed(egui::Key::R));
                        let ghost_dbl = response.double_clicked()
                            && response
                                .interact_pointer_pos()
                                .map(|p| ghost.contains(p))
                                .unwrap_or(false);
                        if repeat || ghost_dbl {
                            result =
                                RegionHudResult::Confirmed { selected: ghost, overlay: screen };
                        }
                    }
                }

                if let (Some(start), Some(end)) = (*region_start, *region_end) {
                    let rect = Rect::from_two_pos(start, end);
                    paint_selection_hud(&painter, rect, ctx.pixels_per_point());
                }

                // Cursor loupe (samples backdrop pixels when frozen)
                if let Some(pos) = response.hover_pos().or_else(|| response.interact_pointer_pos())
                {
                    let sample = backdrop_rgba.and_then(|(w, h, px)| {
                        sample_backdrop_pixel(px, *w, *h, screen, pos)
                    });
                    paint_cursor_loupe(&painter, pos, screen, sample);
                }

                // Arrow-key nudge of the active selection
                if region_start.is_some() && region_end.is_some() {
                    let (dx, dy, step) = ctx.input(|i| {
                        let step = if i.modifiers.shift { 10.0 } else { 1.0 };
                        let mut dx = 0.0_f32;
                        let mut dy = 0.0_f32;
                        if i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::A) {
                            dx -= step;
                        }
                        if i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::D) {
                            dx += step;
                        }
                        if i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::W) {
                            dy -= step;
                        }
                        if i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::S) {
                            dy += step;
                        }
                        (dx, dy, step)
                    });
                    let _ = step;
                    if dx != 0.0 || dy != 0.0 {
                        let delta = Vec2::new(dx, dy);
                        if let Some(s) = region_start.as_mut() {
                            *s += delta;
                        }
                        if let Some(e) = region_end.as_mut() {
                            *e += delta;
                        }
                        ctx.request_repaint();
                    }
                    // Enter confirms without releasing drag again
                    if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let (Some(start), Some(end)) = (*region_start, *region_end) {
                            let selected = Rect::from_two_pos(start, end);
                            if selected.width() >= 8.0 && selected.height() >= 8.0 {
                                result = RegionHudResult::Confirmed { selected, overlay: screen };
                            }
                        }
                    }
                }

                if !pick_mode {
                    if response.drag_started() {
                    *was_dragging = true;
                    if let Some(pos) = response.interact_pointer_pos() {
                        *region_start = Some(pos);
                        *region_end = Some(pos);
                    }
                }
                if response.dragged() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        *region_end = Some(pos);
                    }
                    if let (Some(start), Some(end)) = (*region_start, *region_end) {
                        let mut rect = Rect::from_two_pos(start, end);
                        ctx.input(|i| {
                            if i.modifiers.shift {
                                let s = rect.width().abs().min(rect.height().abs());
                                rect = Rect::from_min_size(rect.min, Vec2::splat(s));
                                *region_end = Some(rect.max);
                            } else if i.modifiers.alt {
                                let w = rect.width().abs();
                                rect = Rect::from_min_size(rect.min, Vec2::new(w, w * 9.0 / 16.0));
                                *region_end = Some(rect.max);
                            } else if let Some(ratio) = *aspect_lock {
                                // Toolbar chip lock: keep the drag width,
                                // pin the height to the ratio.
                                rect = aspect_clamped(rect, ratio);
                                *region_end = Some(rect.max);
                            }
                        });
                    }
                }
                // Real drag (≥24px) captures on mouse-up. Tiny clicks keep the box for Enter.
                if response.drag_stopped() {
                    *was_dragging = false;
                    if let (Some(start), Some(end)) = (*region_start, *region_end) {
                        let selected = Rect::from_two_pos(start, end);
                        if selected.width() >= 24.0 && selected.height() >= 24.0 {
                            result = RegionHudResult::Confirmed { selected, overlay: screen };
                        }
                    }
                }
                // Immediate viewports occasionally miss drag_stopped (release at
                // the screen edge). Any primary release that ends a real drag
                // still confirms — this is the "release captures" contract.
                if *was_dragging && ctx.input(|i| i.pointer.primary_released()) {
                    *was_dragging = false;
                    if !matches!(result, RegionHudResult::Confirmed { .. }) {
                        if let (Some(start), Some(end)) = (*region_start, *region_end) {
                            let selected = Rect::from_two_pos(start, end);
                            if selected.width() >= 24.0 && selected.height() >= 24.0 {
                                result =
                                    RegionHudResult::Confirmed { selected, overlay: screen };
                            }
                        }
                    }
                }
                if response.double_clicked() {
                    if let (Some(start), Some(end)) = (*region_start, *region_end) {
                        let selected = Rect::from_two_pos(start, end);
                        if selected.width() >= 8.0 && selected.height() >= 8.0 {
                            result = RegionHudResult::Confirmed { selected, overlay: screen };
                        }
                    }
                }
                }
                if response.secondary_clicked() {
                    result = RegionHudResult::Cancelled;
                }

                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    result = RegionHudResult::Cancelled;
                }

                egui::Area::new(egui::Id::new("region_actions"))
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 12.0))
                    .show(ctx, |ui| {
                        Frame::none()
                            .fill(theme::SURFACE())
                            .rounding(theme::rounding_md())
                            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(if pick_mode { "Window" } else { "Region" })
                                            .size(13.0)
                                            .strong()
                                            .color(theme::TEXT()),
                                    );
                                    if !pick_mode {
                                        for (label, ratio) in [
                                            ("Free", None),
                                            ("1:1", Some(1.0_f32)),
                                            ("16:9", Some(16.0 / 9.0)),
                                            ("9:16", Some(9.0 / 16.0)),
                                        ] {
                                            if ui
                                                .selectable_label(
                                                    *aspect_lock == ratio,
                                                    RichText::new(label).size(11.0),
                                                )
                                                .clicked()
                                            {
                                                *aspect_lock = ratio;
                                            }
                                        }
                                    }
                                    if !pick_mode
                                        && ui.button(RichText::new("Capture").strong()).clicked()
                                    {
                                        if let (Some(start), Some(end)) =
                                            (*region_start, *region_end)
                                        {
                                            let selected = Rect::from_two_pos(start, end);
                                            if selected.width() >= 8.0 && selected.height() >= 8.0
                                            {
                                                result = RegionHudResult::Confirmed {
                                                    selected,
                                                    overlay: screen,
                                                };
                                            }
                                        }
                                    }
                                    if ui.button("Cancel").clicked() {
                                        result = RegionHudResult::Cancelled;
                                    }
                                });
                            });
                    });
            });
        },
    );

    result
}

/// Aspect-lock clamp: keep the drag's width, pin height to `width / ratio`.
/// Anchored at `rect.min` like the Shift/Alt modifier clamps.
fn aspect_clamped(rect: Rect, ratio: f32) -> Rect {
    let w = rect.width().abs();
    Rect::from_min_size(rect.min, Vec2::new(w, w / ratio.max(f32::EPSILON)))
}

fn paint_selection_hud(painter: &egui::Painter, rect: Rect, ppp: f32) {
    painter.rect_filled(rect, 0.0, Color32::TRANSPARENT);
    painter.rect_stroke(rect, 0.0, Stroke::new(2.0_f32, theme::ACCENT()));

    // Rule of thirds
    let third_stroke = Stroke::new(1.0_f32, theme::HUD_GUIDE());
    let w = rect.width();
    let h = rect.height();
    if w > 24.0 && h > 24.0 {
        for i in 1..3 {
            let x = rect.min.x + w * (i as f32) / 3.0;
            painter.line_segment(
                [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                third_stroke,
            );
            let y = rect.min.y + h * (i as f32) / 3.0;
            painter.line_segment(
                [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                third_stroke,
            );
        }
    }

    // Corner + edge handles
    let hs = 7.0_f32;
    let handle_fill = theme::ACCENT();
    let handle_stroke = Stroke::new(1.0_f32, theme::ON_SOLID());
    for corner in [
        rect.left_top(),
        rect.right_top(),
        rect.left_bottom(),
        rect.right_bottom(),
    ] {
        let hr = Rect::from_center_size(corner, Vec2::splat(hs * 2.0));
        painter.rect_filled(hr, 2.0, handle_fill);
        painter.rect_stroke(hr, 2.0, handle_stroke);
    }
    for mid in [
        Pos2::new(rect.center().x, rect.min.y),
        Pos2::new(rect.center().x, rect.max.y),
        Pos2::new(rect.min.x, rect.center().y),
        Pos2::new(rect.max.x, rect.center().y),
    ] {
        let hr = Rect::from_center_size(mid, Vec2::splat(hs * 1.6));
        painter.rect_filled(hr, 2.0, handle_fill);
    }

    // W×H plate — physical pixels alongside points when DPI ≠ 100 %.
    let wh = if ppp > 1.0 + f32::EPSILON {
        format!(
            "{}×{} pt · {}×{} px",
            rect.width() as i32,
            rect.height() as i32,
            (rect.width() * ppp) as i32,
            (rect.height() * ppp) as i32
        )
    } else {
        format!("{}×{}", rect.width() as i32, rect.height() as i32)
    };
    let label_pos = rect.left_top() + Vec2::new(6.0, -26.0);
    let galley = painter.layout_no_wrap(wh, FontId::proportional(13.0), theme::ON_SOLID());
    let pad = Vec2::new(8.0, 4.0);
    let plate = Rect::from_min_size(label_pos, galley.size() + pad * 2.0);
    let plate = if plate.min.y < 4.0 {
        plate.translate(Vec2::new(0.0, rect.height() + 30.0))
    } else {
        plate
    };
    painter.rect_filled(plate, 4.0, theme::ACCENT());
    painter.galley(plate.min + pad, galley, theme::ON_SOLID());
}

/// Pre-record countdown bubble (3 / 5 s). Returns true if Esc cancelled.
pub fn show_countdown_bubble(ctx: &egui::Context, seconds_left: u32) -> bool {
    let mut cancelled = false;
    let label = if seconds_left == 0 {
        "GO".to_string()
    } else {
        seconds_left.to_string()
    };

    let builder = ViewportBuilder::default()
        .with_title("Vibecap Countdown")
        .with_decorations(false)
        .with_always_on_top()
        .with_inner_size([280.0, 180.0])
        .with_transparent(false);
    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("countdown_bubble"),
        builder,
        |ctx, class| {
            if class != ViewportClass::Immediate {
                return;
            }
    egui::Area::new(egui::Id::new("vibecap_countdown"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            Frame::none()
                .fill(theme::SURFACE_GLASS())
                .stroke(Stroke::new(2.0_f32, theme::ACCENT()))
                .rounding(theme::rounding_lg())
                .inner_margin(egui::Margin::symmetric(36.0, 28.0))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Recording in")
                                .size(14.0)
                                .color(theme::TEXT_MUTED()),
                        );
                        ui.add_space(theme::SP_2);
                        ui.label(
                            RichText::new(label)
                                .size(64.0)
                                .strong()
                                .color(theme::ACCENT()),
                        );
                        ui.add_space(theme::SP_2);
                        ui.label(
                            RichText::new("Esc to cancel")
                                .size(12.0)
                                .color(theme::TEXT_DIM()),
                        );
                    });
                });
        });
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancelled = true;
            }
        },
    );
    cancelled
}

fn sample_backdrop_pixel(
    pixels: &[u8],
    img_w: u32,
    img_h: u32,
    overlay: Rect,
    cursor: Pos2,
) -> Option<[u8; 4]> {
    if img_w == 0 || img_h == 0 || overlay.width() < 1.0 || overlay.height() < 1.0 {
        return None;
    }
    let nx = ((cursor.x - overlay.min.x) / overlay.width()).clamp(0.0, 1.0);
    let ny = ((cursor.y - overlay.min.y) / overlay.height()).clamp(0.0, 1.0);
    let x = ((nx * img_w as f32).floor() as u32).min(img_w.saturating_sub(1));
    let y = ((ny * img_h as f32).floor() as u32).min(img_h.saturating_sub(1));
    let i = ((y as usize * img_w as usize) + x as usize) * 4;
    let slice = pixels.get(i..i + 4)?;
    Some([slice[0], slice[1], slice[2], slice[3]])
}

fn paint_cursor_loupe(
    painter: &egui::Painter,
    cursor: Pos2,
    screen: Rect,
    sample: Option<[u8; 4]>,
) {
    let radius = 54.0_f32;
    // Prefer upper-right of the cursor; flip if near edges.
    let mut offset = Vec2::new(78.0, -78.0);
    if cursor.x + offset.x + radius > screen.max.x {
        offset.x = -78.0;
    }
    if cursor.y + offset.y - radius < screen.min.y {
        offset.y = 78.0;
    }
    let center = cursor + offset;

    // Stem from cursor to loupe
    let dir = (center - cursor).normalized();
    let edge = center - dir * radius;
    painter.line_segment(
        [cursor, edge],
        Stroke::new(1.0_f32, theme::ACCENT()),
    );
    // Cursor hotspot
    painter.circle_filled(cursor, 3.0, theme::ACCENT());
    painter.circle_stroke(cursor, 6.0, Stroke::new(1.0_f32, theme::ON_SOLID()));

    // Loupe disc
    painter.circle_filled(center, radius, theme::SURFACE_GLASS());
    painter.circle_stroke(center, radius, Stroke::new(2.5_f32, theme::ACCENT()));
    painter.circle_stroke(center, radius - 6.0, Stroke::new(1.0_f32, theme::BORDER()));

    // Magnified-grid suggestion (2× visual language without sampling)
    let grid = Stroke::new(1.0_f32, theme::HUD_GUIDE());
    for i in -2..=2 {
        if i == 0 {
            continue;
        }
        let o = (i as f32) * 12.0;
        painter.line_segment(
            [
                Pos2::new(center.x + o, center.y - radius + 10.0),
                Pos2::new(center.x + o, center.y + radius - 10.0),
            ],
            grid,
        );
        painter.line_segment(
            [
                Pos2::new(center.x - radius + 10.0, center.y + o),
                Pos2::new(center.x + radius - 10.0, center.y + o),
            ],
            grid,
        );
    }

    // Crosshair
    let ch = Stroke::new(1.5_f32, theme::ACCENT());
    painter.line_segment(
        [
            Pos2::new(center.x - 18.0, center.y),
            Pos2::new(center.x + 18.0, center.y),
        ],
        ch,
    );
    painter.line_segment(
        [
            Pos2::new(center.x, center.y - 18.0),
            Pos2::new(center.x, center.y + 18.0),
        ],
        ch,
    );
    painter.circle_stroke(center, 4.0, Stroke::new(1.0_f32, theme::ON_SOLID()));

    // 2× badge
    let badge = "2×";
    let badge_pos = center + Vec2::new(0.0, radius - 18.0);
    painter.text(
        badge_pos,
        Align2::CENTER_CENTER,
        badge,
        FontId::proportional(11.0),
        theme::ACCENT(),
    );

    if let Some([r, g, b, _]) = sample {
        painter.rect_filled(
            Rect::from_center_size(center + Vec2::new(0.0, 22.0), Vec2::new(36.0, 14.0)),
            3.0,
            Color32::from_rgb(r, g, b),
        );
        painter.text(
            center + Vec2::new(0.0, 38.0),
            Align2::CENTER_CENTER,
            format!("#{r:02X}{g:02X}{b:02X}"),
            FontId::proportional(11.0),
            theme::ON_SOLID(),
        );
    }

    // Coordinate plate under loupe
    let coords = format!("{}, {}", cursor.x as i32, cursor.y as i32);
    let galley = painter.layout_no_wrap(coords, FontId::proportional(12.0), theme::ON_SOLID());
    let pad = Vec2::new(6.0, 3.0);
    let plate = Rect::from_center_size(
        center + Vec2::new(0.0, radius + 14.0),
        galley.size() + pad * 2.0,
    );
    painter.rect_filled(plate, 4.0, theme::ACCENT());
    painter.galley(plate.min + pad, galley, theme::ON_SOLID());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_mapping_full_selection_is_full_image() {
        let overlay = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(1000.0, 500.0));
        let (w, h, x, y) = overlay_rect_to_pixels(overlay, overlay, 1920, 1080);
        assert_eq!((x, y), (0, 0));
        assert_eq!(w, 1920);
        assert_eq!(h, 1080);
    }

    #[test]
    fn overlay_mapping_quarter_is_even() {
        let overlay = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
        let sel = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(50.0, 50.0));
        let (w, h, x, y) = overlay_rect_to_pixels(sel, overlay, 1920, 1080);
        assert_eq!((x, y), (0, 0));
        assert_eq!(w % 2, 0);
        assert_eq!(h % 2, 0);
        assert!((w - 960).abs() <= 2);
        assert!((h - 540).abs() <= 2);
    }

    #[test]
    fn pixels_roundtrip_through_overlay() {
        let overlay = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 500.0));
        let crop = overlay_rect_to_pixels(overlay, overlay, 1920, 1080);
        let back = pixels_to_overlay_rect(crop, overlay, 1920, 1080);
        assert!((back.width() - overlay.width()).abs() < 2.0);
        assert!((back.height() - overlay.height()).abs() < 2.0);
    }

    #[test]
    fn aspect_lock_pins_height_to_width_ratio() {
        let drag = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(160.0, 55.0));
        let locked = aspect_clamped(drag, 16.0 / 9.0);
        assert_eq!(locked.min, drag.min);
        assert!((locked.width() - 160.0).abs() < 0.01);
        assert!((locked.height() - 90.0).abs() < 0.01);
        // 9:16 portrait flips the same width into a tall box.
        let tall = aspect_clamped(drag, 9.0 / 16.0);
        assert!((tall.height() - 160.0 * 16.0 / 9.0).abs() < 0.01);
        // 1:1 and a zero-ratio guard.
        let square = aspect_clamped(drag, 1.0);
        assert!((square.height() - 160.0).abs() < 0.01);
        let safe = aspect_clamped(drag, 0.0);
        assert!(safe.height().is_finite());
    }
}
