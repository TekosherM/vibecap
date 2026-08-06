//! Still studio — image crop / adjust / save (preview-first Safelight body).

use eframe::egui;
use egui::{Frame, Margin, RichText};
use rfd::FileDialog;

use crate::platform::{open_path, reveal_in_file_manager};
use crate::ui::icons::Icon;
use crate::ui::theme;
use crate::ui::empty_state;
use crate::VibecapApp;

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.label(
        RichText::new("Crop · rotate · adjust · save")
            .color(theme::TEXT_MUTED())
            .size(13.0),
    );
    ui.add_space(theme::SP_3);

    ui.horizontal(|ui| {
        if ui
            .button(RichText::new("Select image").color(theme::TEXT()))
            .clicked()
        {
            if let Some(path) = FileDialog::new()
                .add_filter("Image", &["jpg", "jpeg", "png", "gif", "webp"])
                .pick_file()
            {
                app.img_source_dims = image::image_dimensions(&path)
                    .map(|(w, h)| format!("{}×{}", w, h))
                    .unwrap_or_default();
                app.img_preview_params.clear();
                app.img_preview_on = true;
                app.img_edit_file = Some(path);
            }
        }
        if let Some(p) = app.img_edit_file.clone() {
            if ui.small_button("Reveal").clicked() {
                let _ = reveal_in_file_manager(&p);
            }
            if ui.small_button("Open").clicked() {
                let _ = open_path(&p);
            }
        }
    });

    let img_path = app.img_edit_file.clone();
    let Some(path) = img_path else {
        empty_state(
            ui,
            Icon::Still,
            "No still loaded",
            "Screenshot from Shutter, pick from Media, or select an image file.",
        );
        return;
    };

    ui.add_space(theme::SP_2);
    let dims = if app.img_source_dims.is_empty() {
        String::new()
    } else {
        format!(" · {} px", app.img_source_dims)
    };
    ui.label(
        RichText::new(format!(
            "{}{}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("image"),
            dims
        ))
        .color(theme::TEXT())
        .size(14.0)
        .strong(),
    );
    ui.label(
        RichText::new(path.display().to_string())
            .small()
            .color(theme::TEXT_DIM()),
    );
    ui.add_space(theme::SP_3);

    // ── Preview canvas first ──────────────────────────────────────
    Frame::none()
        .fill(theme::SURFACE())
        .rounding(theme::rounding_md())
        .stroke(egui::Stroke::new(1.0_f32, theme::BORDER()))
        .inner_margin(Margin::same(theme::SP_3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Preview")
                        .size(12.0)
                        .color(theme::TEXT_MUTED())
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut app.img_preview_on, "Live");
                    if ui.small_button("Reset").clicked() {
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
                    }
                });
            });
            ui.add_space(theme::SP_2);

            let max_w = ui.available_width().min(560.0);
            let max_h = 280.0_f32;

            if app.img_preview_on {
                app.refresh_img_preview(ctx);
                if let Some(tex) = &app.img_preview_tex {
                    let size = tex.size_vec2();
                    let scale = (max_w / size.x).min(max_h / size.y).min(1.0);
                    ui.vertical_centered(|ui| {
                        Frame::none()
                            .fill(theme::SURFACE_2())
                            .rounding(theme::rounding_sm())
                            .inner_margin(Margin::same(4.0))
                            .show(ui, |ui| {
                                ui.image((tex.id(), size * scale));
                            });
                    });
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new("Building preview…")
                                .color(theme::TEXT_MUTED())
                                .size(12.0),
                        );
                        ui.add_space(40.0);
                    });
                }
            } else {
                // Static file preview via egui image loader
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

    // ── Adjustments ───────────────────────────────────────────────
    Frame::none()
        .fill(theme::SURFACE())
        .rounding(theme::rounding_md())
        .stroke(egui::Stroke::new(1.0_f32, theme::BORDER()))
        .inner_margin(Margin::same(theme::SP_3))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Adjust")
                    .size(12.0)
                    .color(theme::TEXT_MUTED())
                    .strong(),
            );
            ui.add_space(theme::SP_2);

            ui.horizontal(|ui| {
                ui.label(RichText::new("Rotate").small().color(theme::TEXT_DIM()));
                for (deg, lab) in [(0, "0°"), (90, "90°"), (180, "180°"), (270, "270°")] {
                    ui.selectable_value(&mut app.img_rotate, deg, lab);
                }
                ui.add_space(theme::SP_2);
                ui.checkbox(&mut app.img_flip_h, "Flip H");
                ui.checkbox(&mut app.img_flip_v, "Flip V");
                ui.checkbox(&mut app.img_grayscale, "Gray");
            });

            ui.add_space(theme::SP_2);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Bright").small().color(theme::TEXT_DIM()));
                ui.add(egui::Slider::new(&mut app.img_brightness, -100..=100).show_value(true));
                ui.label(RichText::new("Contrast").small().color(theme::TEXT_DIM()));
                ui.add(egui::Slider::new(&mut app.img_contrast, -100.0..=100.0).show_value(true));
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Blur").small().color(theme::TEXT_DIM()));
                ui.add(egui::Slider::new(&mut app.img_blur, 0.0..=10.0).show_value(true));
                ui.label(RichText::new("Resize %").small().color(theme::TEXT_DIM()));
                ui.add(egui::Slider::new(&mut app.img_resize_pct, 10..=200).show_value(true));
            });

            ui.add_space(theme::SP_2);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Crop px").small().color(theme::TEXT_DIM()));
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

            ui.add_space(theme::SP_3);
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Save edited image")
                            .color(theme::ON_SOLID())
                            .strong(),
                    )
                    .fill(theme::ACCENT())
                    .rounding(theme::rounding_sm()),
                )
                .clicked()
            {
                app.apply_image_edits();
            }
        });
}
