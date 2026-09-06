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
pub fn show_region_selector(
    ctx: &egui::Context,
    region_start: &mut Option<Pos2>,
    region_end: &mut Option<Pos2>,
    last_pixels: Option<(i32, i32, i32, i32)>,
    last_region: Option<Rect>,
    backdrop: Option<&egui::TextureHandle>,
    backdrop_rgba: Option<&(u32, u32, Vec<u8>)>,
) -> RegionHudResult {
    let mut result = RegionHudResult::Continue;
    let opaque = backdrop.is_some() || cfg!(target_os = "windows");

    let builder = ViewportBuilder::default()
        .with_title("Vibecap Region")
        .with_decorations(false)
        .with_transparent(!opaque)
        .with_fullscreen(true)
        .with_always_on_top();

    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("region_selector"),
        builder,
        |ctx, class| {
            if class != ViewportClass::Immediate {
                return;
            }
            let panel_frame = Frame::none().fill(if backdrop.is_some() {
                Color32::BLACK
            } else {
                theme::OVERLAY_DIM()
            });
            egui::CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), Sense::drag());
                let screen = response.rect;
                if let Some(tex) = backdrop {
                    painter.image(
                        tex.id(),
                        screen,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }

                // Ghost of last region in pixel space (falls back to overlay points).
                if region_start.is_none() {
                    let (img_w, img_h) = backdrop_rgba
                        .map(|(w, h, _)| (*w, *h))
                        .unwrap_or((0, 0));
                    let ghost = if let Some(crop) = last_pixels {
                        if img_w > 0 && img_h > 0 {
                            Some(pixels_to_overlay_rect(crop, screen, img_w, img_h))
                        } else {
                            last_region
                        }
                    } else {
                        last_region
                    };
                    if let Some(ghost) = ghost {
                        if ghost.width() >= 8.0 && ghost.height() >= 8.0 {
                            painter.rect_stroke(
                                ghost,
                                0.0,
                                Stroke::new(1.5_f32, theme::TEXT_MUTED()),
                            );
                            painter.text(
                                ghost.center(),
                                Align2::CENTER_CENTER,
                                "Last region · drag to replace · Enter captures · ←↑↓→ nudge",
                                FontId::proportional(13.0),
                                theme::TEXT_MUTED(),
                            );
                        }
                    }
                    painter.text(
                        Pos2::new(screen.center().x, screen.min.y + 48.0),
                        Align2::CENTER_CENTER,
                        "Drag to select a region · Esc to cancel · arrows nudge (⇧ = 10px)",
                        FontId::proportional(18.0),
                        theme::TEXT(),
                    );
                }

                if let (Some(start), Some(end)) = (*region_start, *region_end) {
                    let rect = Rect::from_two_pos(start, end);
                    paint_selection_hud(&painter, rect);
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
                        if i.key_pressed(egui::Key::ArrowLeft) {
                            dx -= step;
                        }
                        if i.key_pressed(egui::Key::ArrowRight) {
                            dx += step;
                        }
                        if i.key_pressed(egui::Key::ArrowUp) {
                            dy -= step;
                        }
                        if i.key_pressed(egui::Key::ArrowDown) {
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

                if response.drag_started() {
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
                            }
                        });
                    }
                }
                // Drag-stop keeps the box; Enter (below) captures. Right-click cancels.
                if response.secondary_clicked() {
                    result = RegionHudResult::Cancelled;
                }

                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    result = RegionHudResult::Cancelled;
                }
            });
        },
    );

    result
}

fn paint_selection_hud(painter: &egui::Painter, rect: Rect) {
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

    // W×H plate
    let wh = format!("{}×{}", rect.width() as i32, rect.height() as i32);
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
}
