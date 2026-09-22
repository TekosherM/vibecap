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
/// `picks_done` — lifetime completed picks; first-run hints hide after 3 (E95).
/// `region_history` — confirmed rects this session; Ctrl+Z pops (E96).
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
    dim_alpha: u8,
    picks_done: u32,
    region_history: &mut Vec<Rect>,
    // E71 — stills already saved this overlay session (Shift-release batch).
    batch_count: u32,
    // E94 — toolbar docks top or bottom; persisted by the caller.
    toolbar_bottom: &mut bool,
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
            // Dim is painted per-monitor below (E84), so the panel itself
            // stays transparent — or black under the frozen backdrop.
            let panel_frame = Frame::none().fill(if backdrop.is_some() {
                Color32::BLACK
            } else {
                Color32::TRANSPARENT
            });
            egui::CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
                let screen = response.rect;
                let ctrl_held = ctx.input(|i| i.modifiers.ctrl);
                if let Some(tex) = backdrop {
                    painter.image(
                        tex.id(),
                        screen,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                // E24 dim + E84: with several monitors only the one under the
                // cursor dims; others stay lit so adjacent displays read
                // untouched. No pointer yet → dim everything.
                let vppp = ctx.pixels_per_point().max(1.0);
                let pointer =
                    response.hover_pos().or_else(|| response.interact_pointer_pos());
                let dim_rects: Vec<Rect> = if mons.len() > 1 {
                    let all: Vec<Rect> = mons
                        .iter()
                        .map(|m| {
                            Rect::from_min_size(
                                Pos2::new(
                                    (m.x - origin.0) as f32 / vppp,
                                    (m.y - origin.1) as f32 / vppp,
                                ),
                                Vec2::new(m.w as f32 / vppp, m.h as f32 / vppp),
                            )
                        })
                        .collect();
                    match pointer.and_then(|p| all.iter().find(|r| r.contains(p))) {
                        Some(r) => vec![*r],
                        None => all,
                    }
                } else {
                    vec![screen]
                };
                for r in &dim_rects {
                    painter.rect_filled(*r, 0.0, Color32::from_black_alpha(dim_alpha));
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

                // E78/E79 — snap targets: other visible windows' edges. The
                // cache read is non-blocking; a cold cache just means no
                // window snapping this frame (screen edges still snap).
                let pick_mode_early = window_pick.is_some();
                let snap_rects: Vec<Rect> = if pick_mode_early {
                    Vec::new()
                } else {
                    let vppp = ctx.pixels_per_point().max(1.0);
                    crate::platform::list_capture_windows_cached()
                        .into_iter()
                        .filter(|w| !w.minimized && !w.is_self())
                        .map(|w| {
                            Rect::from_min_size(
                                Pos2::new(
                                    (w.x - origin.0) as f32 / vppp,
                                    (w.y - origin.1) as f32 / vppp,
                                ),
                                Vec2::new(w.w as f32 / vppp, w.h as f32 / vppp),
                            )
                        })
                        .collect()
                };
                let mut snap_guides: Vec<(Pos2, Pos2)> = Vec::new();

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
                    // E95 — first-run hints hide once picking is learned.
                    if picks_done < 3 {
                        paint_hint_pill(
                            &painter,
                            Pos2::new(screen.center().x, screen.min.y + 48.0),
                            "Click a window to capture · scroll cycles overlaps · dead space = monitor · Esc cancel",
                            18.0,
                        );
                    }
                    if let (Some(rect), Some((title, _, _, ww, wh))) = (pick_rect, window_pick)
                    {
                        painter.rect_filled(rect, 0.0, theme::ACCENT().gamma_multiply(0.15));
                        painter.rect_stroke(rect, 0.0, Stroke::new(2.5_f32, theme::ACCENT()));
                        // E88 — confidence flash: one decaying pulse when the
                        // hovered window changes.
                        let key = (
                            rect.min.x as i32,
                            rect.min.y as i32,
                            rect.width() as i32,
                            rect.height() as i32,
                        );
                        let flash_id = egui::Id::new("pick_flash");
                        let now = ctx.input(|i| i.time);
                        let prev: Option<((i32, i32, i32, i32), f64)> =
                            ctx.data_mut(|d| d.get_temp(flash_id));
                        let t0 = match prev {
                            Some((k, t)) if k == key => t,
                            _ => {
                                ctx.data_mut(|d| d.insert_temp(flash_id, (key, now)));
                                now
                            }
                        };
                        let age = (now - t0) as f32;
                        if age < 0.3 {
                            let k = 1.0 - age / 0.3;
                            painter.rect_stroke(
                                rect.expand(3.0 + 9.0 * (1.0 - k)),
                                0.0,
                                Stroke::new(
                                    1.5_f32 + 2.0 * k,
                                    theme::ACCENT().gamma_multiply(0.15 + 0.75 * k),
                                ),
                            );
                            ctx.request_repaint();
                        }
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
                    // Ctrl+click samples a hex color instead of picking (E92).
                    if response.clicked() && !ctrl_held {
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
                    if picks_done < 3 {
                        paint_hint_pill(
                            &painter,
                            Pos2::new(screen.center().x, screen.min.y + 48.0),
                            "Drag to select · wheel resizes · arrows nudge · Shift+drag = batch · Esc cancel",
                            18.0,
                        );
                    }
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
                    // Punch the hole: re-draw the backdrop crop inside the
                    // selection so it reads full-bright against the dim.
                    if let Some(tex) = backdrop {
                        let uv = Rect::from_min_max(
                            Pos2::new(
                                ((rect.min.x - screen.min.x) / screen.width()).clamp(0.0, 1.0),
                                ((rect.min.y - screen.min.y) / screen.height()).clamp(0.0, 1.0),
                            ),
                            Pos2::new(
                                ((rect.max.x - screen.min.x) / screen.width()).clamp(0.0, 1.0),
                                ((rect.max.y - screen.min.y) / screen.height()).clamp(0.0, 1.0),
                            ),
                        );
                        painter.image(tex.id(), rect, uv, Color32::WHITE);
                    }
                    // E83 — a locked aspect tints the surround accent so the
                    // mode is legible at a glance.
                    if aspect_lock.is_some() {
                        let tint = theme::ACCENT().gamma_multiply(0.10);
                        for r in [
                            Rect::from_min_max(screen.min, Pos2::new(screen.max.x, rect.min.y)),
                            Rect::from_min_max(
                                Pos2::new(screen.min.x, rect.max.y),
                                screen.max,
                            ),
                            Rect::from_min_max(
                                Pos2::new(screen.min.x, rect.min.y),
                                Pos2::new(rect.min.x, rect.max.y),
                            ),
                            Rect::from_min_max(
                                Pos2::new(rect.max.x, rect.min.y),
                                Pos2::new(screen.max.x, rect.max.y),
                            ),
                        ] {
                            painter.rect_filled(r, 0.0, tint);
                        }
                    }
                    let cursor =
                        response.hover_pos().or_else(|| response.interact_pointer_pos());
                    let grid: u8 = ctx
                        .data_mut(|d| d.get_temp(egui::Id::new("region_grid_mode")))
                        .unwrap_or(0);
                    let plate = paint_selection_hud(
                        &painter,
                        rect,
                        ctx.pixels_per_point(),
                        screen,
                        cursor,
                        grid,
                    );
                    // E91 — click the W×H plate to copy `x,y,w,h` (pixels).
                    let pr = ui.interact(
                        plate,
                        egui::Id::new("region_wh_plate"),
                        Sense::click(),
                    );
                    if pr.hovered() {
                        painter.rect_stroke(
                            plate,
                            4.0,
                            Stroke::new(1.0_f32, theme::ON_SOLID()),
                        );
                    }
                    if pr.clicked() {
                        let ppp = ctx.pixels_per_point();
                        ctx.copy_text(format!(
                            "{},{},{},{}",
                            (rect.min.x * ppp) as i32,
                            (rect.min.y * ppp) as i32,
                            (rect.width() * ppp) as i32,
                            (rect.height() * ppp) as i32
                        ));
                    }
                    pr.on_hover_text("Click to copy x,y,w,h");
                }

                // Cursor loupe — hold Ctrl to magnify (E77); samples the
                // frozen backdrop pixels when present.
                if ctrl_held {
                    if let Some(pos) =
                        response.hover_pos().or_else(|| response.interact_pointer_pos())
                    {
                        let sample = backdrop_rgba.and_then(|(w, h, px)| {
                            sample_backdrop_pixel(px, *w, *h, screen, pos)
                        });
                        paint_cursor_loupe(&painter, pos, screen, sample);
                        // E92 — Ctrl+click copies the sampled hex color.
                        if response.clicked() {
                            if let Some(c) = sample {
                                let hex =
                                    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
                                ctx.copy_text(hex.clone());
                                ctx.data_mut(|d| {
                                    d.insert_temp(
                                        egui::Id::new("hex_copied"),
                                        (hex, ctx.input(|i| i.time)),
                                    )
                                });
                            }
                        }
                        if let Some((hex, t0)) = ctx.data_mut(|d| {
                            d.get_temp::<(String, f64)>(egui::Id::new("hex_copied"))
                        }) {
                            if ctx.input(|i| i.time) - t0 < 0.9 {
                                paint_hint_pill(
                                    &painter,
                                    pos + Vec2::new(0.0, -34.0),
                                    &format!("{hex} copied"),
                                    12.0,
                                );
                                ctx.request_repaint();
                            }
                        }
                    }
                }

                // E82 — keyboard-only: with no box, an arrow key grows one
                // from the center; the nudge/Enter path below takes over.
                if !pick_mode && region_start.is_none() {
                    let grew = ctx.input(|i| {
                        i.key_pressed(egui::Key::ArrowLeft)
                            || i.key_pressed(egui::Key::ArrowRight)
                            || i.key_pressed(egui::Key::ArrowUp)
                            || i.key_pressed(egui::Key::ArrowDown)
                    });
                    if grew {
                        let c = screen.center();
                        *region_start = Some(c - Vec2::new(100.0, 75.0));
                        *region_end = Some(c + Vec2::new(100.0, 75.0));
                        ctx.request_repaint();
                    }
                }
                // E97 — wheel resizes the box: plain scroll adjusts width,
                // Shift+scroll adjusts height.
                if !pick_mode && !response.dragged() {
                    let dy = ctx.input(|i| i.raw_scroll_delta.y);
                    if dy.abs() > 0.5 {
                        if let (Some(_start), Some(end)) = (*region_start, *region_end) {
                            let delta = if dy > 0.0 { 8.0 } else { -8.0 };
                            let shift = ctx.input(|i| i.modifiers.shift);
                            let e = if shift {
                                Pos2::new(end.x, (end.y + delta).clamp(screen.min.y, screen.max.y))
                            } else {
                                Pos2::new((end.x + delta).clamp(screen.min.x, screen.max.x), end.y)
                            };
                            *region_end = Some(e);
                            ctx.request_repaint();
                        }
                    }
                }

                // E96 — Ctrl+Z steps back through confirmed rects this
                // session (history pushed by the caller on each confirm).
                if !pick_mode
                    && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z))
                {
                    if let Some(r) = region_history.pop() {
                        *region_start = Some(r.min);
                        *region_end = Some(r.max);
                        ctx.request_repaint();
                    }
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
                    if let Some(pos) = response.interact_pointer_pos() {
                        // E6/E7 — a press on an existing box's corner resizes
                        // from the opposite corner; inside the box moves it;
                        // anywhere else starts a fresh drag.
                        let cur = match (*region_start, *region_end) {
                            (Some(s), Some(e)) => {
                                let r = Rect::from_two_pos(s, e);
                                (r.width() >= 8.0 && r.height() >= 8.0).then_some(r)
                            }
                            _ => None,
                        };
                        let mut mode = 0u8;
                        let mut anchor = pos;
                        if let Some(r) = cur {
                            for c in [
                                r.left_top(),
                                r.right_top(),
                                r.left_bottom(),
                                r.right_bottom(),
                            ] {
                                if pos.distance(c) <= 14.0 {
                                    mode = 2;
                                    anchor = Pos2::new(
                                        r.min.x + r.max.x - c.x,
                                        r.min.y + r.max.y - c.y,
                                    );
                                    break;
                                }
                            }
                            if mode == 0 && r.contains(pos) {
                                mode = 1;
                                anchor = pos - r.min.to_vec2();
                            }
                        }
                        ctx.data_mut(|d| {
                            d.insert_temp(egui::Id::new("rd_mode"), mode);
                            d.insert_temp(egui::Id::new("rd_grab"), anchor);
                        });
                        // Only a fresh drag confirms on release — a move or
                        // resize leaves the box up for nudge/Enter (E7).
                        *was_dragging = mode == 0;
                        if mode == 0 {
                            let (p, _) = snap_with_guides(pos, screen, &snap_rects);
                            *region_start = Some(p);
                            *region_end = Some(p);
                        }
                    }
                }
                if response.dragged() {
                    let mode: u8 = ctx
                        .data_mut(|d| d.get_temp(egui::Id::new("rd_mode")))
                        .unwrap_or(0);
                    let grab: Pos2 = ctx
                        .data_mut(|d| d.get_temp(egui::Id::new("rd_grab")))
                        .unwrap_or(Pos2::ZERO);
                    if let Some(pos) = response.interact_pointer_pos() {
                        match mode {
                            // E6 — move: keep the grab offset, clamped on screen.
                            1 => {
                                if let (Some(s), Some(e)) = (*region_start, *region_end) {
                                    let size = Rect::from_two_pos(s, e).size();
                                    let min = Pos2::new(
                                        (pos.x - grab.x).clamp(
                                            screen.min.x,
                                            (screen.max.x - size.x).max(screen.min.x),
                                        ),
                                        (pos.y - grab.y).clamp(
                                            screen.min.y,
                                            (screen.max.y - size.y).max(screen.min.y),
                                        ),
                                    );
                                    *region_start = Some(min);
                                    *region_end = Some(min + size);
                                }
                            }
                            // E7 — corner resize: the opposite corner stays put.
                            2 => {
                                let (p, g) = snap_with_guides(pos, screen, &snap_rects);
                                *region_start = Some(grab);
                                *region_end = Some(p);
                                snap_guides = g;
                            }
                            _ => {
                                // E78/E79 — snap the dragged corner to screen and
                                // window edges within 8 px; matched edges paint
                                // as alignment guides.
                                let (p, g) = snap_with_guides(pos, screen, &snap_rects);
                                *region_end = Some(p);
                                snap_guides = g;
                            }
                        }
                    }
                    if mode != 1 {
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
                }
                // Real drag (≥24px) captures on mouse-up. Tiny clicks keep the box for Enter.
                if response.drag_stopped() {
                    let mode: u8 = ctx
                        .data_mut(|d| d.get_temp(egui::Id::new("rd_mode")))
                        .unwrap_or(0);
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("rd_mode"), 0u8));
                    *was_dragging = false;
                    // Move/resize drags never confirm — they edit the box.
                    if mode == 0 {
                        if let (Some(start), Some(end)) = (*region_start, *region_end) {
                            let selected = Rect::from_two_pos(start, end);
                            if selected.width() >= 24.0 && selected.height() >= 24.0 {
                                result =
                                    RegionHudResult::Confirmed { selected, overlay: screen };
                            }
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
                // E79 — alignment guides for edges this frame snapped to.
                for (a, b) in &snap_guides {
                    painter.line_segment(
                        [*a, *b],
                        Stroke::new(1.0_f32, theme::HUD_GUIDE()),
                    );
                }
                if response.secondary_clicked() {
                    result = RegionHudResult::Cancelled;
                }

                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    result = RegionHudResult::Cancelled;
                }

                // E81 — a tiny selection gets a compact toolbar.
                let compact = matches!((*region_start, *region_end), (Some(s), Some(e)) if {
                    let r = Rect::from_two_pos(s, e);
                    r.width() < 260.0 || r.height() < 140.0
                });
                egui::Area::new(egui::Id::new("region_actions"))
                    .order(egui::Order::Foreground)
                    .anchor(
                        if *toolbar_bottom {
                            Align2::CENTER_BOTTOM
                        } else {
                            Align2::CENTER_TOP
                        },
                        Vec2::new(0.0, if *toolbar_bottom { -12.0 } else { 12.0 }),
                    )
                    .show(ctx, |ui| {
                        // E100 — the HUD chrome stays neutral dark regardless
                        // of the app theme so it reads over any backdrop.
                        Frame::none()
                            .fill(Color32::from_black_alpha(230))
                            .rounding(theme::rounding_md())
                            .inner_margin(egui::Margin::symmetric(
                                if compact { 8.0 } else { 12.0 },
                                if compact { 4.0 } else { 8.0 },
                            ))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let ink = Color32::from_gray(235);
                                    if !compact {
                                        ui.label(
                                            RichText::new(if pick_mode { "Window" } else { "Region" })
                                                .size(13.0)
                                                .strong()
                                                .color(ink),
                                        );
                                    }
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
                                                    RichText::new(label).size(11.0).color(ink),
                                                )
                                                .clicked()
                                            {
                                                *aspect_lock = ratio;
                                            }
                                        }
                                        // E87 — grid overlay cycles thirds →
                                        // quarters → off.
                                        let grid_id = egui::Id::new("region_grid_mode");
                                        let grid: u8 = ctx
                                            .data_mut(|d| d.get_temp(grid_id))
                                            .unwrap_or(0);
                                        let glabel = ["▦", "▩", "▢"][grid.min(2) as usize];
                                        if ui
                                            .button(RichText::new(glabel).size(12.0).color(ink))
                                            .on_hover_text("Grid: thirds → quarters → off")
                                            .clicked()
                                        {
                                            ctx.data_mut(|d| {
                                                d.insert_temp(grid_id, (grid + 1) % 3)
                                            });
                                        }
                                        // E25 — suppress the shutter flash for
                                        // this capture only (consumed on land).
                                        let nf_id = egui::Id::new("hud_no_flash");
                                        let nf: bool = ctx
                                            .data_mut(|d| d.get_temp(nf_id))
                                            .unwrap_or(false);
                                        if ui
                                            .selectable_label(
                                                nf,
                                                RichText::new("⚡")
                                                    .size(12.0)
                                                    .color(if nf {
                                                        theme::ACCENT()
                                                    } else {
                                                        ink
                                                    }),
                                            )
                                            .on_hover_text("No shutter flash on this capture")
                                            .clicked()
                                        {
                                            ctx.data_mut(|d| d.insert_temp(nf_id, !nf));
                                        }
                                        // E21 — centered-box presets in *pixels*
                                        // for README/demo captures.
                                        for (label, (pw, ph)) in
                                            [("1080p", (1920.0_f32, 1080.0)), ("720p", (1280.0, 720.0))]
                                        {
                                            if ui
                                                .button(RichText::new(label).size(11.0))
                                                .on_hover_text("Centered pixel-size box — Enter captures")
                                                .clicked()
                                            {
                                                let ppp = ctx.pixels_per_point().max(1.0);
                                                let size = Vec2::new(
                                                    (pw / ppp).min(screen.width()),
                                                    (ph / ppp).min(screen.height()),
                                                );
                                                let min = screen.center() - size / 2.0;
                                                *region_start = Some(min);
                                                *region_end = Some(min + size);
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
                                    // E71 — batch counter: how many stills
                                    // this overlay session has saved.
                                    if batch_count > 0 {
                                        ui.label(
                                            RichText::new(format!("📷 {batch_count}"))
                                                .size(11.0)
                                                .color(theme::ACCENT())
                                                .strong(),
                                        )
                                        .on_hover_text(
                                            "Batch stills saved — Shift+drag keeps grabbing",
                                        );
                                    }
                                    if ui.button("Cancel").clicked() {
                                        result = RegionHudResult::Cancelled;
                                    }
                                    // E94 — dock the toolbar top or bottom.
                                    let dock = if *toolbar_bottom { "⬆" } else { "⬇" };
                                    if ui
                                        .button(RichText::new(dock).size(11.0).color(ink))
                                        .on_hover_text("Dock toolbar top/bottom")
                                        .clicked()
                                    {
                                        *toolbar_bottom = !*toolbar_bottom;
                                    }
                                });
                            });
                    });
            });
        },
    );

    result
}

/// Nearest edge (coord, span_lo, span_hi) within `tol` of `v`, if any.
fn nearest_edge(v: f32, edges: &[(f32, f32, f32)], tol: f32) -> Option<(f32, f32, f32)> {
    edges
        .iter()
        .copied()
        .filter(|(c, _, _)| (v - c).abs() <= tol)
        .min_by(|a, b| {
            (v - a.0)
                .abs()
                .partial_cmp(&(v - b.0).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Snap `p` to the nearest screen or window edge within 8 px (E78) and
/// return guide segments for the matched edges (E79).
fn snap_with_guides(p: Pos2, screen: Rect, win_rects: &[Rect]) -> (Pos2, Vec<(Pos2, Pos2)>) {
    const TOL: f32 = 8.0;
    // (coord, span_lo, span_hi): x-edges carry their y-extent and vice versa.
    let mut xs: Vec<(f32, f32, f32)> = vec![
        (screen.min.x, screen.min.y, screen.max.y),
        (screen.max.x, screen.min.y, screen.max.y),
    ];
    let mut ys: Vec<(f32, f32, f32)> = vec![
        (screen.min.y, screen.min.x, screen.max.x),
        (screen.max.y, screen.min.x, screen.max.x),
    ];
    for r in win_rects {
        if r.width() < 4.0 || r.height() < 4.0 {
            continue;
        }
        xs.push((r.min.x, r.min.y, r.max.y));
        xs.push((r.max.x, r.min.y, r.max.y));
        ys.push((r.min.y, r.min.x, r.max.x));
        ys.push((r.max.y, r.min.x, r.max.x));
    }
    let mut out = p;
    let mut guides = Vec::new();
    if let Some((c, lo, hi)) = nearest_edge(p.x, &xs, TOL) {
        out.x = c;
        guides.push((Pos2::new(c, lo), Pos2::new(c, hi)));
    }
    if let Some((c, lo, hi)) = nearest_edge(p.y, &ys, TOL) {
        out.y = c;
        guides.push((Pos2::new(lo, c), Pos2::new(hi, c)));
    }
    (out, guides)
}

/// Aspect-lock clamp: keep the drag's width, pin height to `width / ratio`.
/// Anchored at `rect.min` like the Shift/Alt modifier clamps.
fn aspect_clamped(rect: Rect, ratio: f32) -> Rect {
    let w = rect.width().abs();
    Rect::from_min_size(rect.min, Vec2::new(w, w / ratio.max(f32::EPSILON)))
}

/// Hint text on a translucent dark pill — legible over any backdrop
/// brightness (E80) regardless of app theme (E100).
fn paint_hint_pill(painter: &egui::Painter, center: Pos2, text: &str, size: f32) {
    let ink = Color32::from_gray(235);
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(size), ink);
    let pad = Vec2::new(10.0, 5.0);
    let plate = Rect::from_center_size(center, galley.size() + pad * 2.0);
    painter.rect_filled(plate, plate.height() / 2.0, Color32::from_black_alpha(170));
    painter.galley(plate.min + pad, galley, ink);
}

/// Returns the W×H plate rect so the caller can attach click-to-copy (E91).
/// `cursor` lets the plate hop to a corner that isn't under the pointer (E76).
/// `grid`: 0 = thirds, 1 = quarters, 2 = off (E87).
fn paint_selection_hud(
    painter: &egui::Painter,
    rect: Rect,
    ppp: f32,
    screen: Rect,
    cursor: Option<Pos2>,
    grid: u8,
) -> Rect {
    painter.rect_filled(rect, 0.0, Color32::TRANSPARENT);
    painter.rect_stroke(rect, 0.0, Stroke::new(2.0_f32, theme::ACCENT()));

    // Composition grid — thirds or quarters, toggled by the ▦ chip (E87).
    let third_stroke = Stroke::new(1.0_f32, theme::HUD_GUIDE());
    let w = rect.width();
    let h = rect.height();
    let cells = if grid == 1 { 4 } else { 3 };
    if grid < 2 && w > 24.0 && h > 24.0 {
        for i in 1..cells {
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
    let galley = painter.layout_no_wrap(wh, FontId::proportional(13.0), theme::ON_SOLID());
    let pad = Vec2::new(8.0, 4.0);
    let size = galley.size() + pad * 2.0;
    // E76 — plate follows the selection but never sits under the cursor:
    // above-left, then below-left, then inside top-right, then inside
    // bottom-right; first fit on screen + off-cursor wins.
    let candidates = [
        Rect::from_min_size(rect.left_top() + Vec2::new(6.0, -size.y - 4.0), size),
        Rect::from_min_size(rect.left_bottom() + Vec2::new(6.0, 4.0), size),
        Rect::from_min_size(rect.right_top() + Vec2::new(-size.x - 6.0, 4.0), size),
        Rect::from_min_size(
            rect.right_bottom() + Vec2::new(-size.x - 6.0, -size.y - 4.0),
            size,
        ),
    ];
    let plate = candidates
        .iter()
        .copied()
        .find(|p| {
            screen.contains(p.min)
                && screen.contains(p.max)
                && cursor.map(|c| !p.expand(4.0).contains(c)).unwrap_or(true)
        })
        .unwrap_or_else(|| {
            // Tiny selection / no fit — inside top-left, clamped on screen.
            let p = Pos2::new(
                (rect.min.x + 4.0).clamp(
                    screen.min.x + 4.0,
                    (screen.max.x - size.x - 4.0).max(screen.min.x + 4.0),
                ),
                (rect.min.y + 4.0).clamp(
                    screen.min.y + 4.0,
                    (screen.max.y - size.y - 4.0).max(screen.min.y + 4.0),
                ),
            );
            Rect::from_min_size(p, size)
        });
    painter.rect_filled(plate, 4.0, theme::ACCENT());
    painter.galley(plate.min + pad, galley, theme::ON_SOLID());
    plate
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
                                    RichText::new("Esc or click to cancel")
                                        .size(12.0)
                                        .color(theme::TEXT_DIM()),
                                );
                            });
                        });
                });
            // E65 — Esc or a click inside the bubble aborts the countdown.
            if ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.pointer.any_pressed()) {
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
    painter.line_segment([cursor, edge], Stroke::new(1.0_f32, theme::ACCENT()));
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

    #[test]
    fn snap_prefers_nearest_window_edge_over_screen() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1080.0));
        let win = Rect::from_min_size(Pos2::new(300.0, 200.0), Vec2::new(800.0, 600.0));
        // 5 px inside the window's right edge (1100) — snaps to it, not the
        // screen edge 820 px away.
        let (p, guides) = snap_with_guides(Pos2::new(1095.0, 500.0), screen, &[win]);
        assert_eq!(p.x, 1100.0);
        assert_eq!(p.y, 500.0);
        assert_eq!(guides.len(), 1);
        // The guide traces the snapped vertical edge.
        assert_eq!(guides[0].0, Pos2::new(1100.0, 200.0));
        assert_eq!(guides[0].1, Pos2::new(1100.0, 800.0));
        // Beyond the 8 px tolerance — no snap, no guide.
        let (p2, g2) = snap_with_guides(Pos2::new(1090.0, 500.0), screen, &[win]);
        assert_eq!(p2.x, 1090.0);
        assert!(g2.is_empty());
    }

    #[test]
    fn snap_hits_screen_and_window_on_both_axes() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1080.0));
        let win = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(400.0, 300.0));
        // x near screen edge, y near window top — both snap.
        let (p, guides) = snap_with_guides(Pos2::new(1916.0, 104.0), screen, &[win]);
        assert_eq!(p, Pos2::new(1920.0, 100.0));
        assert_eq!(guides.len(), 2);
    }
}
