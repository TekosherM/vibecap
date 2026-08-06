//! Clip studio — video trim / export (preview-first Safelight body).

use eframe::egui;
use egui::{Frame, Margin, RichText, ScrollArea, Vec2};
use rfd::FileDialog;

use crate::platform::{open_path, reveal_in_file_manager};
use crate::ui::icons::Icon;
use crate::ui::theme;
use crate::ui::empty_state;
use crate::VibecapApp;
use chrono::Local;

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.label(
        RichText::new("Trim · GIF · export")
            .color(theme::TEXT_MUTED())
            .size(13.0),
    );
    ui.add_space(theme::SP_3);

    // Toolbar row
    ui.horizontal(|ui| {
        if ui
            .button(RichText::new("Select video").color(theme::TEXT()))
            .clicked()
        {
            if let Some(path) = FileDialog::new()
                .add_filter("Video", &["mp4", "mov", "webm", "mkv"])
                .pick_file()
            {
                app.edit_file = Some(path.clone());
                app.load_filmstrip(ctx, path);
            }
        }
        if let Some(file) = app.edit_file.clone() {
            if ui.small_button("Reveal").on_hover_text("Show in Finder").clicked() {
                let _ = reveal_in_file_manager(&file);
            }
            if ui.small_button("Open").on_hover_text("System player").clicked() {
                let _ = open_path(&file);
            }
            if ui.small_button("Reload").on_hover_text("Regenerate filmstrip").clicked() {
                app.load_filmstrip(ctx, file);
            }
        }
    });

    let edit_file = app.edit_file.clone();
    let Some(file) = edit_file else {
        empty_state(
            ui,
            Icon::Clip,
            "No clip loaded",
            "Record from Shutter, pick from Media, or select a video file.",
        );
        return;
    };

    ui.add_space(theme::SP_2);
    ui.label(
        RichText::new(
            file.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("video"),
        )
        .color(theme::TEXT())
        .size(14.0)
        .strong(),
    );
    ui.label(
        RichText::new(file.display().to_string())
            .small()
            .color(theme::TEXT_DIM()),
    );
    ui.add_space(theme::SP_3);

    // ── Preview canvas (filmstrip) ────────────────────────────────
    Frame::none()
        .fill(theme::SURFACE())
        .rounding(theme::rounding_md())
        .stroke(egui::Stroke::new(1.0_f32, theme::BORDER()))
        .inner_margin(Margin::same(theme::SP_3))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Preview")
                    .size(12.0)
                    .color(theme::TEXT_MUTED())
                    .strong(),
            );
            ui.add_space(theme::SP_2);

            let canvas_h = 110.0_f32;
            ScrollArea::horizontal()
                .id_source("clip_filmstrip")
                .show(ui, |ui| {
                    ui.set_min_height(canvas_h);
                    ui.horizontal(|ui| {
                        if app.filmstrip_loading {
                            ui.centered_and_justified(|ui| {
                                ui.label(
                                    RichText::new("Generating filmstrip…")
                                        .color(theme::TEXT_MUTED()),
                                );
                            });
                        } else if let Some(err) = &app.filmstrip_error {
                            ui.label(RichText::new(err).color(theme::WARN()));
                        } else if app.filmstrip.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(24.0);
                                ui.label(
                                    RichText::new("No filmstrip thumbs — path is loaded; tools still work.")
                                        .color(theme::TEXT_DIM())
                                        .size(12.0),
                                );
                            });
                        } else {
                            for tex in &app.filmstrip {
                                Frame::none()
                                    .fill(theme::SURFACE_2())
                                    .rounding(theme::rounding_sm())
                                    .inner_margin(Margin::same(2.0))
                                    .show(ui, |ui| {
                                        ui.image((tex.id(), Vec2::new(160.0, 90.0)));
                                    });
                                ui.add_space(4.0);
                            }
                        }
                    });
                });
        });

    ui.add_space(theme::SP_3);

    // ── Range + primary actions ───────────────────────────────────
    Frame::none()
        .fill(theme::SURFACE())
        .rounding(theme::rounding_md())
        .stroke(egui::Stroke::new(1.0_f32, theme::BORDER()))
        .inner_margin(Margin::same(theme::SP_3))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Range")
                    .size(12.0)
                    .color(theme::TEXT_MUTED())
                    .strong(),
            );
            ui.add_space(theme::SP_2);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Start").small().color(theme::TEXT_DIM()));
                ui.add(
                    egui::TextEdit::singleline(&mut app.trim_start)
                        .desired_width(88.0)
                        .hint_text("00:00:00"),
                );
                ui.add_space(theme::SP_3);
                ui.label(RichText::new("End").small().color(theme::TEXT_DIM()));
                ui.add(
                    egui::TextEdit::singleline(&mut app.trim_end)
                        .desired_width(88.0)
                        .hint_text("00:00:10"),
                );
                ui.add_space(theme::SP_3);
                ui.label(RichText::new("Speed").small().color(theme::TEXT_DIM()));
                for (v, lab) in [("0.5", "0.5×"), ("1.0", "1×"), ("1.5", "1.5×"), ("2.0", "2×")] {
                    ui.selectable_value(&mut app.export_speed, v.to_string(), lab);
                }
            });

            ui.add_space(theme::SP_3);
            ui.horizontal(|ui| {
                let file_clone = file.clone();
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Trim video").color(theme::ON_SOLID()).strong(),
                        )
                        .fill(theme::ACCENT())
                        .rounding(theme::rounding_sm()),
                    )
                    .clicked()
                {
                    let out = file_clone.with_file_name(format!(
                        "trimmed_{}",
                        file_clone.file_name().unwrap().to_str().unwrap()
                    ));
                    app.spawn_ffmpeg_job(
                        vec![
                            "-y".into(),
                            "-i".into(),
                            file_clone.to_str().unwrap().into(),
                            "-ss".into(),
                            app.trim_start.clone(),
                            "-to".into(),
                            app.trim_end.clone(),
                            "-c".into(),
                            "copy".into(),
                            out.to_str().unwrap().into(),
                        ],
                        "Video trimmed",
                    );
                }
                if ui.button("Export GIF").clicked() {
                    let timestamp = Local::now().format("%H-%M-%S").to_string();
                    let gif_out = file_clone.with_file_name(format!(
                        "clip_{}_{}.gif",
                        app.trim_start.replace(':', "-"),
                        timestamp
                    ));
                    app.spawn_ffmpeg_job(
                        vec![
                            "-ss".into(),
                            app.trim_start.clone(),
                            "-to".into(),
                            app.trim_end.clone(),
                            "-i".into(),
                            file_clone.to_str().unwrap().into(),
                            "-vf".into(),
                            "fps=15,scale=800:-1:flags=lanczos".into(),
                            "-y".into(),
                            gif_out.to_str().unwrap().into(),
                        ],
                        "GIF exported",
                    );
                }
                if ui.button("Extract audio").clicked() {
                    let audio_out = file_clone.with_extension("m4a");
                    app.spawn_ffmpeg_job(
                        vec![
                            "-y".into(),
                            "-i".into(),
                            file_clone.to_str().unwrap().into(),
                            "-vn".into(),
                            "-acodec".into(),
                            "copy".into(),
                            audio_out.to_str().unwrap().into(),
                        ],
                        "Audio extracted",
                    );
                }
            });
        });

    ui.add_space(theme::SP_3);

    // ── More tools (collapsed) ────────────────────────────────────
    egui::CollapsingHeader::new(
        RichText::new("More tools")
            .color(theme::TEXT_MUTED())
            .size(13.0),
    )
    .default_open(false)
    .show(ui, |ui| {
        ui.label(
            RichText::new("Frame extract · mute · compress · rotate · speed")
                .small()
                .color(theme::TEXT_DIM()),
        );
        ui.add_space(theme::SP_2);
        ui.horizontal_wrapped(|ui| {
            let file_clone = file.clone();
            if ui.button("Frame @ start").clicked() {
                let out = file_clone.with_file_name(format!(
                    "frame_{}.jpg",
                    app.trim_start.replace(':', "-")
                ));
                app.spawn_ffmpeg_job(
                    vec![
                        "-ss".into(),
                        app.trim_start.clone(),
                        "-i".into(),
                        file_clone.to_str().unwrap().into(),
                        "-vframes".into(),
                        "1".into(),
                        "-q:v".into(),
                        "2".into(),
                        "-y".into(),
                        out.to_str().unwrap().into(),
                    ],
                    "Frame extracted",
                );
            }
            if ui.button("Remove audio").clicked() {
                let out = file_clone.with_file_name(format!(
                    "muted_{}",
                    file_clone.file_name().unwrap().to_str().unwrap()
                ));
                app.spawn_ffmpeg_job(
                    vec![
                        "-i".into(),
                        file_clone.to_str().unwrap().into(),
                        "-an".into(),
                        "-c:v".into(),
                        "copy".into(),
                        "-y".into(),
                        out.to_str().unwrap().into(),
                    ],
                    "Audio removed",
                );
            }
            if ui.button("Compress").clicked() {
                let out = file_clone.with_file_name(format!(
                    "compressed_{}",
                    file_clone.file_name().unwrap().to_str().unwrap()
                ));
                app.spawn_ffmpeg_job(
                    vec![
                        "-i".into(),
                        file_clone.to_str().unwrap().into(),
                        "-c:v".into(),
                        "libx264".into(),
                        "-crf".into(),
                        "28".into(),
                        "-preset".into(),
                        "medium".into(),
                        "-c:a".into(),
                        "aac".into(),
                        "-b:a".into(),
                        "96k".into(),
                        "-y".into(),
                        out.to_str().unwrap().into(),
                    ],
                    "Video compressed",
                );
            }
            if ui.button("Rotate 90° CW").clicked() {
                let out = file_clone.with_file_name(format!(
                    "rot90_{}",
                    file_clone.file_name().unwrap().to_str().unwrap()
                ));
                app.spawn_ffmpeg_job(
                    vec![
                        "-i".into(),
                        file_clone.to_str().unwrap().into(),
                        "-vf".into(),
                        "transpose=1".into(),
                        "-y".into(),
                        out.to_str().unwrap().into(),
                    ],
                    "Rotated 90° CW",
                );
            }
            if ui.button("Rotate 90° CCW").clicked() {
                let out = file_clone.with_file_name(format!(
                    "rot270_{}",
                    file_clone.file_name().unwrap().to_str().unwrap()
                ));
                app.spawn_ffmpeg_job(
                    vec![
                        "-i".into(),
                        file_clone.to_str().unwrap().into(),
                        "-vf".into(),
                        "transpose=2".into(),
                        "-y".into(),
                        out.to_str().unwrap().into(),
                    ],
                    "Rotated 90° CCW",
                );
            }
            if ui.button("Rotate 180°").clicked() {
                let out = file_clone.with_file_name(format!(
                    "rot180_{}",
                    file_clone.file_name().unwrap().to_str().unwrap()
                ));
                app.spawn_ffmpeg_job(
                    vec![
                        "-i".into(),
                        file_clone.to_str().unwrap().into(),
                        "-vf".into(),
                        "hflip,vflip".into(),
                        "-y".into(),
                        out.to_str().unwrap().into(),
                    ],
                    "Rotated 180°",
                );
            }
            if ui
                .button(format!("Apply {}× speed", app.export_speed))
                .clicked()
            {
                let out = file_clone.with_file_name(format!(
                    "speed{}_{}",
                    app.export_speed,
                    file_clone.file_name().unwrap().to_str().unwrap()
                ));
                app.spawn_ffmpeg_job(
                    vec![
                        "-i".into(),
                        file_clone.to_str().unwrap().into(),
                        "-filter:v".into(),
                        format!("setpts=PTS/{}", app.export_speed),
                        "-filter:a".into(),
                        format!("atempo={}", app.export_speed),
                        "-y".into(),
                        out.to_str().unwrap().into(),
                    ],
                    "Speed change applied",
                );
            }
        });
    });
}
