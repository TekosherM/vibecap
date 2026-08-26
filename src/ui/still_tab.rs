//! Still studio — image crop / adjust / annotate / save with keyboard shortcuts & instant software baking.

use eframe::egui;
use egui::{Align2, FontId, Frame, Margin, Pos2, Rect, RichText, Stroke, Vec2};
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
            ui.label(
                RichText::new(title)
                    .size(12.0)
                    .color(theme::TEXT_MUTED())
                    .strong(),
            );
            ui.add_space(theme::SP_2);
            add(ui);
        });
}

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    // ── Keyboard Shortcuts (Cmd+C / Cmd+S) ──────────────────────────
    ctx.input_mut(|i| {
        if i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::C))
            || i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::C))
        {
            app.copy_current_still_to_clipboard();
        }
        if i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S))
            || i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S))
        {
            app.save_current_still();
        }
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
                ui.label(
                    RichText::new("No still loaded")
                        .size(14.0)
                        .strong()
                        .color(theme::TEXT()),
                );
            }
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if btn_secondary(ui, "📁 Select image") {
                if let Some(path) = FileDialog::new()
                    .add_filter("Image", &["jpg", "jpeg", "png", "gif", "webp"])
                    .pick_file()
                {
                    app.open_still_from_path(path);
                }
            }
            if let Some(p) = app.img_edit_file.clone() {
                if btn_primary(ui, "💾 Save (⌘S)") {
                    app.save_current_still();
                }
                if btn_secondary(ui, "📋 Copy Image (⌘C)") {
                    app.copy_current_still_to_clipboard();
                }
                if btn_small(ui, "🔄 Reset") {
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
                    app.annotation_actions.clear();
                    app.step_counter = 1;
                }
                if btn_small(ui, "📂 Finder") {
                    let _ = reveal_in_file_manager(&p);
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
            "Screenshot from Shutter, pick from Media, or select an image file to annotate & edit.",
        );
        return;
    };

    ui.add_space(theme::SP_3);

    // ── Annotation Studio Toolbar ────────────────────────────────────
    card(ui, "ANNOTATION TOOLS", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.radio_value(&mut app.current_tool, AnnotationTool::Pen, "✏ Pen");
            ui.radio_value(&mut app.current_tool, AnnotationTool::Arrow, "➡ Arrow");
            ui.radio_value(&mut app.current_tool, AnnotationTool::Rectangle, "🔲 Rect");
            ui.radio_value(&mut app.current_tool, AnnotationTool::Highlight, "🖍 Highlight");
            ui.radio_value(&mut app.current_tool, AnnotationTool::Text, "🔤 Text");
            ui.radio_value(&mut app.current_tool, AnnotationTool::Blur, "💧 Blur");
            ui.radio_value(&mut app.current_tool, AnnotationTool::StepBadge, "🔢 Badge");

            ui.separator();
            ui.color_edit_button_srgba(&mut app.current_color);
            ui.add(egui::Slider::new(&mut app.current_stroke_width, 1.0..=12.0).text("Size"));

            if app.current_tool == AnnotationTool::Text {
                ui.separator();
                ui.label("Text:");
                ui.text_edit_singleline(&mut app.pending_text);
            }

            ui.separator();
            if btn_small(ui, "↩ Undo") {
                app.annotation_actions.pop();
            }
            if btn_small(ui, "🗑 Clear Annotations") {
                app.annotation_actions.clear();
                app.step_counter = 1;
            }
        });
    });

    ui.add_space(theme::SP_3);

    // ── Preview Canvas & Interactive Annotation Painter ───────────────
    card(ui, "CANVAS (DRAW TO ANNOTATE)", |ui| {
        ui.horizontal(|ui| {
            switch(ui, "Live preview", &mut app.img_preview_on);
            ui.label(
                RichText::new("Drag on image to draw shapes · ⌘C to copy · ⌘S to save")
                    .small()
                    .color(theme::TEXT_MUTED()),
            );
        });
        ui.add_space(theme::SP_2);

        let max_w = ui.available_width();
        let max_h = (ui.available_height() * 0.50).clamp(240.0, 480.0);

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
            let scale = (max_w / size.x).min(max_h / size.y).min(1.0);
            let canvas_size = size * scale;

            let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::drag());
            app.annotation_canvas_rect = Some(response.rect);

            // 1. Draw base image
            painter.image(
                tex.id(),
                response.rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                theme::ON_SOLID(),
            );

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

            // Render existing annotations
            for action in &app.annotation_actions {
                draw_action(&painter, action);
            }

            // Render active shape being drawn
            if let Some(action) = &app.current_action {
                draw_action(&painter, action);
            }

            // Interactive mouse input handling
            if response.drag_started() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let action = AnnotationAction {
                        tool: app.current_tool,
                        color: app.current_color,
                        stroke_width: app.current_stroke_width,
                        points: vec![pos],
                        text_content: app.pending_text.clone(),
                        badge_number: app.step_counter,
                    };

                    if app.current_tool == AnnotationTool::Text
                        || app.current_tool == AnnotationTool::StepBadge
                    {
                        if app.current_tool == AnnotationTool::StepBadge {
                            app.step_counter += 1;
                        }
                        app.annotation_actions.push(action);
                    } else {
                        app.current_action = Some(action);
                    }
                }
            }
            if response.dragged() {
                if let Some(pos) = response.interact_pointer_pos() {
                    if let Some(action) = &mut app.current_action {
                        action.points.push(pos);
                    }
                }
            }
            if response.drag_stopped() {
                if let Some(action) = app.current_action.take() {
                    app.annotation_actions.push(action);
                }
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
    });

    ui.add_space(theme::SP_3);

    // ── Adjustments (grouped) ─────────────────────────────────────
    card(ui, "ADJUSTMENTS", |ui| {
        group(ui, "TRANSFORM", |ui| {
            segmented(
                ui,
                &mut app.img_rotate,
                &[(0u32, "0°"), (90, "90°"), (180, "180°"), (270, "270°")],
            );
            ui.add_space(theme::SP_2);
            switch(ui, "Flip H", &mut app.img_flip_h);
            ui.add_space(theme::SP_2);
            switch(ui, "Flip V", &mut app.img_flip_v);
        });
        group(ui, "COLOR", |ui| {
            switch(ui, "Gray", &mut app.img_grayscale);
            ui.add_space(theme::SP_3);
            ui.label(RichText::new("Bright").size(11.0).color(theme::TEXT_DIM()));
            ui.add(egui::Slider::new(&mut app.img_brightness, -100..=100).show_value(false));
            ui.add_space(theme::SP_2);
            ui.label(RichText::new("Contrast").size(11.0).color(theme::TEXT_DIM()));
            ui.add(egui::Slider::new(&mut app.img_contrast, -100.0..=100.0).show_value(false));
            ui.add_space(theme::SP_2);
            ui.label(RichText::new("Blur").size(11.0).color(theme::TEXT_DIM()));
            ui.add(egui::Slider::new(&mut app.img_blur, 0.0..=10.0).show_value(false));
        });
        group(ui, "SIZE & CROP", |ui| {
            ui.label(RichText::new("Resize %").size(11.0).color(theme::TEXT_DIM()));
            ui.add(egui::Slider::new(&mut app.img_resize_pct, 10..=200).show_value(false));
            ui.add_space(theme::SP_3);
            ui.label(RichText::new("Crop px").size(11.0).color(theme::TEXT_DIM()));
            ui.add(
                egui::TextEdit::singleline(&mut app.img_crop_x)
                    .hint_text("x")
                    .desired_width(48.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.img_crop_y)
                    .hint_text("y")
                    .desired_width(48.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.img_crop_w)
                    .hint_text("w")
                    .desired_width(48.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.img_crop_h)
                    .hint_text("h")
                    .desired_width(48.0),
            );
        });
        ui.add_space(theme::SP_1);
        ui.horizontal(|ui| {
            if btn_primary(ui, "Save edited & annotated image") {
                app.save_current_still();
            }
        });
    });
}
