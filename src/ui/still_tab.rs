//! Still studio — image crop / adjust / annotate / save with keyboard shortcuts & instant software baking.

use eframe::egui;
use egui::{Align2, Color32, FontId, Frame, Margin, Pos2, Rect, RichText, Stroke, Vec2};
use rfd::FileDialog;

use crate::app::{AnnotationAction, AnnotationTool};
use crate::platform::reveal_in_file_manager;
use crate::ui::empty_state;
use crate::ui::icons::Icon;
use crate::ui::theme;
use crate::ui::{btn_primary, btn_secondary, btn_small, group, segmented, switch};
use crate::VibecapApp;

fn card(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    Frame::none()
        .fill(theme::SURFACE())
        .rounding(theme::rounding_md())
        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
        .inner_margin(Margin::same(theme::SP_3))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if !title.is_empty() {
                ui.label(
                    RichText::new(title)
                        .size(12.0)
                        .color(theme::TEXT_MUTED())
                        .strong(),
                );
                ui.add_space(theme::SP_2);
            }
            add(ui);
        });
}

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    // ── Keyboard Shortcuts (Cmd+C / Cmd+S) ──────────────────────────
    ctx.input_mut(|i| {
        if i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::C,
        )) || i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::CTRL,
            egui::Key::C,
        )) {
            app.copy_current_still_to_clipboard();
        }
        if i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::S,
        )) || i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::CTRL,
            egui::Key::S,
        )) {
            app.save_current_still();
        }
        if i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::C,
        )) || i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
            egui::Key::C,
        )) {
            if let Some(p) = &app.img_edit_file {
                if let Ok(mut board) = arboard::Clipboard::new() {
                    let _ = board.set_text(p.display().to_string());
                    app.show_toast("Path copied");
                }
            }
        }
        if i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::Z,
        )) || i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::CTRL,
            egui::Key::Z,
        )) {
            app.annotation_do_undo();
        }
        if i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        )) || i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
            egui::Key::Z,
        )) || i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::Y,
        )) || i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::CTRL,
            egui::Key::Y,
        )) {
            app.annotation_do_redo();
        }
        if i.key_pressed(egui::Key::Num0) {
            app.still_zoom = 1.0;
            app.still_pan = Vec2::ZERO;
        }
        if i.key_pressed(egui::Key::Num1) {
            app.still_zoom_to_100 = true;
        }
        if i.key_pressed(egui::Key::Escape) {
            app.text_edit_at = None;
            app.still_crop_mode = false;
            app.crop_drag = None;
        }
        // Scroll-zoom moved into the canvas block — it must not fire when
        // the pointer is over the inspector or toolbar.
    });

    // ── Header Toolbar ──────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            if let Some(p) = app.img_edit_file.clone() {
                let dims = if app.img_source_dims.is_empty() {
                    String::new()
                } else {
                    format!(" · {} px", app.img_source_dims)
                };
                ui.label(
                    RichText::new(format!(
                        "{}{}",
                        p.file_name().and_then(|n| n.to_str()).unwrap_or("image"),
                        dims
                    ))
                    .size(14.0)
                    .strong()
                    .color(theme::TEXT()),
                );
                ui.label(
                    RichText::new(p.display().to_string())
                        .size(10.0)
                        .color(theme::TEXT_DIM()),
                );
            } else {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Nothing to review yet")
                            .size(14.0)
                            .strong()
                            .color(theme::TEXT()),
                    );
                    ui.label(
                        RichText::new("Take a screenshot (S) — it lands here for annotate.")
                            .size(11.0)
                            .color(theme::TEXT_MUTED()),
                    );
                });
            }
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            // ⋯ the rest — secondary actions live in the menu, not the header.
            crate::ui::icon_menu_button(ui, "⋯", |ui| {
                ui.set_min_width(180.0);
                if ui.button("Select image…").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("Image", &["jpg", "jpeg", "png", "gif", "webp"])
                        .pick_file()
                    {
                        app.open_still_from_path(path);
                    }
                    ui.close_menu();
                }
                if let Some(p) = app.img_edit_file.clone() {
                    if ui.button("Save as copy").clicked() {
                        app.save_current_still_copy();
                        ui.close_menu();
                    }
                    if ui.button("Copy original (no markup)").clicked() {
                        app.copy_still_original_to_clipboard();
                        ui.close_menu();
                    }
                    if ui.button("Open in default app").clicked() {
                        let _ = crate::platform::open_path(&p);
                        ui.close_menu();
                    }
                    if ui.button("Reveal in Explorer").clicked() {
                        let _ = reveal_in_file_manager(&p);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .button(RichText::new("Reset all edits").color(theme::DANGER_SOFT()))
                        .clicked()
                    {
                        app.img_rotate = 0;
                        app.img_flip_h = false;
                        app.img_flip_v = false;
                        app.img_grayscale = false;
                        app.img_brightness = 0;
                        app.img_contrast = 0.0;
                        app.img_blur = 0.0;
                        app.img_resize_pct = 100;
                        app.img_crop_x.clear();
                        app.img_crop_y.clear();
                        app.img_crop_w.clear();
                        app.img_crop_h.clear();
                        app.img_preview_params.clear();
                        app.annotation_push_undo();
                        app.annotation_actions.clear();
                        app.step_counter = 1;
                        ui.close_menu();
                    }
                }
            });
            if let Some(_p) = app.img_edit_file.clone() {
                if btn_secondary(ui, "Save") {
                    app.save_current_still();
                }
                if btn_secondary(ui, "Copy") {
                    app.copy_current_still_to_clipboard();
                }
                if btn_primary(ui, "✓ Done") {
                    app.copy_current_still_to_clipboard();
                    app.show_toast("Copied — back to Capture for the next shot");
                    app.current_tab = crate::AppTab::Capture;
                }
            } else {
                if btn_secondary(ui, "Select image…") {
                    if let Some(path) = FileDialog::new()
                        .add_filter("Image", &["jpg", "jpeg", "png", "gif", "webp"])
                        .pick_file()
                    {
                        app.open_still_from_path(path);
                    }
                }
            }
        });
    });

    let img_path = app.img_edit_file.clone();
    let Some(path) = img_path else {
        empty_state(
            ui,
            Icon::Still,
            "No still loaded",
            "Take a screenshot (S) or pick one from Library — it lands here to mark up.",
        );
        return;
    };

    ui.add_space(theme::SP_3);

    // Canvas-dominant layout: image left, inspector right (like a photo app).
    const INSPECTOR_W: f32 = 244.0;
    let canvas_w = (ui.available_width() - INSPECTOR_W - theme::SP_3).max(300.0);
    let body_h = ui.available_height().max(380.0);

    ui.horizontal_top(|ui| {
    ui.allocate_ui_with_layout(
        Vec2::new(canvas_w, body_h),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {

    card(ui, "", |ui| {
        ui.horizontal(|ui| {
            switch(ui, "Live preview", &mut app.img_preview_on);
            ui.label(
                RichText::new(
                    "Drag to draw · Shift = snap · hold B = before · Space+drag pan · scroll zoom · 0/1 fit/100% · ⌘Z undo",
                )
                    .small()
                    .color(theme::TEXT_MUTED()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if btn_small(ui, "+") {
                    app.still_zoom = (app.still_zoom + 0.25).min(4.0);
                }
                ui.label(
                    RichText::new(format!("{:.0}%", app.still_zoom * 100.0))
                        .size(10.0)
                        .color(theme::TEXT_DIM()),
                );
                if btn_small(ui, "−") {
                    app.still_zoom = (app.still_zoom - 0.25).max(0.25);
                }
            });
        });
        ui.add_space(theme::SP_2);

        let max_w = ui.available_width();
        let max_h = (body_h - 80.0).clamp(240.0, 1200.0);

        if app.img_preview_on {
            app.refresh_img_preview(ctx);
        }

        let tex_opt = if app.img_preview_on {
            app.img_preview_tex.clone()
        } else {
            None
        };

        if let Some(tex) = tex_opt {
            let size = tex.size_vec2();
            let fit = (max_w / size.x).min(max_h / size.y).min(1.0);
            if app.still_zoom_to_100 {
                // `1` = true 100% — undo the fit scale, not just reset zoom.
                app.still_zoom = (1.0 / fit.max(0.01)).clamp(0.25, 4.0);
                app.still_zoom_to_100 = false;
            }
            let canvas_size = Vec2::new(max_w, max_h);
            let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::drag());
            // Wheel zooms only when the pointer is over the canvas.
            if response.hovered() {
                let scroll = ctx.input(|i| i.raw_scroll_delta.y);
                if scroll.abs() > 0.1 {
                    app.still_zoom =
                        (app.still_zoom * (1.0 + scroll * 0.001)).clamp(0.25, 4.0);
                }
            }
            let scale = fit * app.still_zoom;
            let img_rect = Rect::from_min_size(response.rect.min + app.still_pan, size * scale);
            app.annotation_canvas_rect = Some(img_rect);

            painter.image(
                tex.id(),
                img_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                theme::ON_SOLID(),
            );
            if let Some((a, b)) = app.crop_drag {
                painter.rect_stroke(
                    Rect::from_two_pos(a, b),
                    0.0,
                    Stroke::new(1.5_f32, theme::ACCENT()),
                );
            }

            // 2. Draw annotation actions helper
            let draw_action = |painter: &egui::Painter, action: &AnnotationAction| {
                if action.points.is_empty() {
                    return;
                }
                let mut color = action.color;
                if action.tool == AnnotationTool::Highlight {
                    color = color.linear_multiply(0.4);
                }
                let stroke = Stroke::new(action.stroke_width, color);

                match action.tool {
                    AnnotationTool::Pen | AnnotationTool::Highlight => {
                        for i in 1..action.points.len() {
                            painter.line_segment([action.points[i - 1], action.points[i]], stroke);
                        }
                    }
                    AnnotationTool::Arrow => {
                        if action.points.len() >= 2 {
                            let start = action.points[0];
                            let end = *action.points.last().unwrap();
                            painter.arrow(start, end - start, stroke);
                        }
                    }
                    AnnotationTool::Rectangle => {
                        if action.points.len() >= 2 {
                            let start = action.points[0];
                            let end = *action.points.last().unwrap();
                            let rect = Rect::from_two_pos(start, end);
                            painter.rect_stroke(rect, 0.0, stroke);
                        }
                    }
                    AnnotationTool::Ellipse => {
                        if action.points.len() >= 2 {
                            let start = action.points[0];
                            let end = *action.points.last().unwrap();
                            let rect = Rect::from_two_pos(start, end);
                            painter.add(egui::Shape::Ellipse(egui::epaint::EllipseShape::stroke(
                                rect.center(),
                                Vec2::new(rect.width() / 2.0, rect.height() / 2.0),
                                stroke,
                            )));
                        }
                    }
                    AnnotationTool::Blur => {
                        if action.points.len() >= 2 {
                            let start = action.points[0];
                            let end = *action.points.last().unwrap();
                            let rect = Rect::from_two_pos(start, end);
                            painter.rect_filled(rect, 0.0, theme::OVERLAY_BLUR());
                            painter.rect_stroke(
                                rect,
                                0.0,
                                Stroke::new(1.0_f32, theme::NEUTRAL_STROKE()),
                            );
                        }
                    }
                    AnnotationTool::Text => {
                        let pos = action.points[0];
                        painter.rect_filled(
                            Rect::from_min_size(
                                pos - Vec2::new(4.0, 2.0),
                                Vec2::new(action.text_content.len() as f32 * 10.0 + 8.0, 22.0),
                            ),
                            4.0,
                            theme::OVERLAY_LABEL(),
                        );
                        painter.text(
                            pos,
                            Align2::LEFT_TOP,
                            &action.text_content,
                            FontId::proportional(16.0),
                            action.color,
                        );
                    }
                    AnnotationTool::StepBadge => {
                        let pos = action.points[0];
                        painter.circle_filled(pos, 14.0, action.color);
                        painter.text(
                            pos,
                            Align2::CENTER_CENTER,
                            action.badge_number.to_string(),
                            FontId::proportional(14.0),
                            theme::ACCENT_INK(),
                        );
                    }
                }
            };

            // E120: hold B to peek the un-annotated original (before/after).
            let peek_original = !ctx.wants_keyboard_input()
                && ctx.input(|i| i.key_down(egui::Key::B));

            // Render existing annotations
            if !peek_original {
                for action in &app.annotation_actions {
                    draw_action(&painter, action);
                }

                // Render active shape being drawn
                if let Some(action) = &app.current_action {
                    draw_action(&painter, action);
                }
            }

            let space = ctx.input(|i| i.key_down(egui::Key::Space));
            if space && response.dragged() {
                app.still_pan += response.drag_delta();
            } else if app.still_crop_mode {
                if response.drag_started() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        app.crop_drag = Some((pos, pos));
                    }
                }
                if response.dragged() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        if let Some((a, _)) = app.crop_drag {
                            app.crop_drag = Some((a, pos));
                        }
                    }
                }
                if response.drag_stopped() {
                    if let Some((a, b)) = app.crop_drag.take() {
                        let r = Rect::from_two_pos(a, b);
                        let (iw, ih) = image::image_dimensions(&path).unwrap_or((1, 1));
                        let map = |p: Pos2| -> (u32, u32) {
                            let u = ((p.x - img_rect.min.x) / img_rect.width()).clamp(0.0, 1.0);
                            let v = ((p.y - img_rect.min.y) / img_rect.height()).clamp(0.0, 1.0);
                            ((u * iw as f32) as u32, (v * ih as f32) as u32)
                        };
                        let (x0, y0) = map(r.min);
                        let (x1, y1) = map(r.max);
                        app.img_crop_x = x0.min(x1).to_string();
                        app.img_crop_y = y0.min(y1).to_string();
                        app.img_crop_w = x0.abs_diff(x1).max(2).to_string();
                        app.img_crop_h = y0.abs_diff(y1).max(2).to_string();
                    }
                }
            } else if app.current_tool == AnnotationTool::Text {
                if response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        app.text_edit_at = Some(pos);
                    }
                }
            } else if response.drag_started() {
                if let Some(pos) = response.interact_pointer_pos() {
                    app.annotation_push_undo();
                    let action = AnnotationAction {
                        tool: app.current_tool,
                        color: app.current_color,
                        stroke_width: app.current_stroke_width,
                        points: vec![pos],
                        text_content: app.pending_text.clone(),
                        badge_number: app.step_counter,
                    };
                    if app.current_tool == AnnotationTool::StepBadge {
                        app.step_counter += 1;
                        app.annotation_actions.push(action);
                    } else {
                        app.current_action = Some(action);
                    }
                }
            }
            if !space && !app.still_crop_mode && response.dragged() {
                if let Some(pos) = response.interact_pointer_pos() {
                    if let Some(action) = &mut app.current_action {
                        // Shift snaps arrows to 15° and rects/blur to squares.
                        let pos = if ctx.input(|i| i.modifiers.shift) {
                            crate::app::snap_annotation_point(
                                action.tool,
                                action.points[0],
                                pos,
                            )
                        } else {
                            pos
                        };
                        action.points.push(pos);
                    }
                }
            }
            if !app.still_crop_mode && response.drag_stopped() {
                if let Some(action) = app.current_action.take() {
                    app.annotation_actions.push(action);
                }
            }
            if let Some(pos) = app.text_edit_at {
                egui::Area::new(egui::Id::new("still_inplace_text"))
                    .fixed_pos(pos)
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        ui.set_min_width(180.0);
                        let r = ui.text_edit_singleline(&mut app.pending_text);
                        r.request_focus();
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            app.annotation_push_undo();
                            app.annotation_actions.push(AnnotationAction {
                                tool: AnnotationTool::Text,
                                color: app.current_color,
                                stroke_width: app.current_stroke_width,
                                points: vec![pos],
                                text_content: app.pending_text.clone(),
                                badge_number: app.step_counter,
                            });
                            app.text_edit_at = None;
                        }
                    });
            }
        } else {
            ui.vertical_centered(|ui| {
                ui.add(
                    egui::Image::new(format!("file://{}", path.display()))
                        .max_width(max_w)
                        .max_height(max_h),
                );
            });
        }
    }); // canvas card

        },
    ); // left column

    ui.add_space(theme::SP_3);
    // ── Inspector (right rail — tools, brush, adjust) ────────────────
    ui.allocate_ui_with_layout(
        Vec2::new(INSPECTOR_W, body_h),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            egui::ScrollArea::vertical()
                .id_source("still_inspector")
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.set_max_width(INSPECTOR_W - 8.0);

                    group(ui, "TOOLS", |ui| {
                        for (tool, label) in [
                            (AnnotationTool::Pen, "✏ Pen"),
                            (AnnotationTool::Arrow, "➡ Arrow"),
                            (AnnotationTool::Rectangle, "🔲 Rect"),
                            (AnnotationTool::Ellipse, "⬭ Ellipse"),
                            (AnnotationTool::Highlight, "🖍 Highlight"),
                            (AnnotationTool::Text, "🔤 Text"),
                            (AnnotationTool::Blur, "💧 Blur"),
                            (AnnotationTool::StepBadge, "🔢 Steps"),
                        ] {
                            ui.selectable_value(&mut app.current_tool, tool, label);
                        }
                    });

                    group(ui, "BRUSH", |ui| {
                        ui.color_edit_button_srgba(&mut app.current_color);
                        // E113: one-tap swatches for the common markup colors —
                        // the picker stays for anything else.
                        ui.horizontal_wrapped(|ui| {
                            for c in [
                                Color32::from_rgb(0xFF, 0x4D, 0x4D), // red — callouts
                                Color32::from_rgb(0xFF, 0xB0, 0x2E), // amber
                                Color32::from_rgb(0x3E, 0xE6, 0x87), // green — OK
                                Color32::from_rgb(0x4D, 0x9E, 0xFF), // blue
                                Color32::WHITE,
                                Color32::BLACK,
                            ] {
                                let (r, resp) = ui.allocate_exact_size(
                                    Vec2::splat(16.0),
                                    egui::Sense::click(),
                                );
                                let cur = app.current_color;
                                let active = cur.r() == c.r()
                                    && cur.g() == c.g()
                                    && cur.b() == c.b();
                                let p = ui.painter();
                                p.circle_filled(r.center(), 6.5, c);
                                p.circle_stroke(
                                    r.center(),
                                    7.5,
                                    Stroke::new(
                                        if active { 2.0f32 } else { 1.0 },
                                        if active {
                                            theme::ACCENT()
                                        } else {
                                            theme::BORDER()
                                        },
                                    ),
                                );
                                if resp.clicked() {
                                    app.current_color = c;
                                }
                            }
                        });
                        // E114: tap a common width instead of nudging the slider.
                        ui.horizontal_wrapped(|ui| {
                            for w in [2.0f32, 4.0, 8.0] {
                                let sel =
                                    (app.current_stroke_width - w).abs() < 0.5;
                                if crate::ui::components::chip(
                                    ui,
                                    &format!("{w:.0}px"),
                                    sel,
                                ) {
                                    app.current_stroke_width = w;
                                }
                            }
                        });
                        ui.add(
                            egui::Slider::new(&mut app.current_stroke_width, 1.0f32..=12.0)
                                .text("px"),
                        );
                    });
                    if app.current_tool == AnnotationTool::Text {
                        ui.add(
                            egui::TextEdit::singleline(&mut app.pending_text)
                                .hint_text("Text to place…"),
                        );
                        ui.add_space(theme::SP_2);
                    }
                    ui.horizontal(|ui| {
                        if btn_small(ui, "↩ Undo") {
                            app.annotation_actions.pop();
                            app.step_counter =
                                crate::app::renumber_step_badges(&mut app.annotation_actions);
                        }
                        if btn_small(ui, "Clear marks") {
                            app.annotation_actions.clear();
                            app.step_counter = 1;
                        }
                    });

                    ui.add_space(theme::SP_2);
                    ui.separator();
                    ui.add_space(theme::SP_2);

                    group(ui, "ROTATE & FLIP", |ui| {
                        segmented(
                            ui,
                            &mut app.img_rotate,
                            &[(0u32, "0°"), (90, "90°"), (180, "180°"), (270, "270°")],
                        );
                        switch(ui, "Flip H", &mut app.img_flip_h);
                        switch(ui, "Flip V", &mut app.img_flip_v);
                    });

                    group(ui, "LOOK", |ui| {
                        switch(ui, "Gray", &mut app.img_grayscale);
                    });
                    ui.label(RichText::new("Brightness").size(11.0).color(theme::TEXT_DIM()));
                    ui.add(egui::Slider::new(&mut app.img_brightness, -100..=100).show_value(false));
                    ui.label(RichText::new("Contrast").size(11.0).color(theme::TEXT_DIM()));
                    ui.add(egui::Slider::new(&mut app.img_contrast, -100.0..=100.0).show_value(false));
                    ui.label(RichText::new("Soft blur").size(11.0).color(theme::TEXT_DIM()));
                    ui.add(egui::Slider::new(&mut app.img_blur, 0.0..=10.0).show_value(false));

                    ui.add_space(theme::SP_2);
                    ui.separator();
                    ui.add_space(theme::SP_2);

                    group(ui, "SIZE & CROP", |ui| {
                        switch(ui, "Drag on canvas to crop", &mut app.still_crop_mode);
                    });
                    ui.label(RichText::new("Resize %").size(11.0).color(theme::TEXT_DIM()));
                    ui.add(egui::Slider::new(&mut app.img_resize_pct, 10..=200).show_value(false));
                    ui.label(RichText::new("Crop px — x · y · w · h").size(11.0).color(theme::TEXT_DIM()));
                    ui.horizontal(|ui| {
                        for field in [
                            &mut app.img_crop_x,
                            &mut app.img_crop_y,
                            &mut app.img_crop_w,
                            &mut app.img_crop_h,
                        ] {
                            ui.add(egui::TextEdit::singleline(field).desired_width(44.0));
                        }
                    });

                    ui.add_space(theme::SP_3);
                    if btn_primary(ui, "Save overwrite") {
                        app.save_current_still();
                    }
                    ui.add_space(theme::SP_1);
                    if btn_secondary(ui, "Save as copy") {
                        app.save_current_still_copy();
                    }
                });
        },
    );
    }); // horizontal split
}
