//! Clip studio — in-app preview player + trim / export (Safelight body).

use eframe::egui;
use egui::{Frame, Margin, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use rfd::FileDialog;

use crate::platform::{format_timecode, open_path, parse_timecode, reveal_in_file_manager};
use crate::ui::icons::{self, Icon};
use crate::ui::theme;
use crate::ui::empty_state;
use crate::ui::{btn_primary, btn_secondary, btn_small, group, segmented};
use crate::VibecapApp;
use chrono::Local;

// ── Player (big-screen flipbook preview) ───────────────────────────

fn player(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context, duration: f64) {
    // Advance the playhead from real time while playing.
    let now = ctx.input(|i| i.time);
    if app.player_playing {
        if let Some(last) = app.player_last_time {
            app.player_pos = (app.player_pos + (now - last)).min(duration);
        }
        if app.player_pos >= duration {
            app.player_pos = duration;
            app.player_playing = false;
        }
        ctx.request_repaint();
    }
    app.player_last_time = Some(now);

    // Big screen: 16:9 canvas, letterboxed frame at the playhead.
    let canvas_w = ui.available_width();
    let canvas_h = (canvas_w * 9.0 / 16.0).clamp(180.0, 460.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(canvas_w, canvas_h), Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, theme::rounding_md(), egui::Color32::BLACK);

    let n = app.filmstrip.len();
    if n > 0 {
        let idx = ((app.player_pos * app.filmstrip_fps).floor() as usize).min(n - 1);
        let tex = &app.filmstrip[idx];
        // Fit 16:9 frame inside the canvas.
        let fw = rect.width().min(rect.height() * 16.0 / 9.0);
        let fh = fw * 9.0 / 16.0;
        let fit = Rect::from_center_size(rect.center(), Vec2::new(fw, fh));
        painter.image(
            tex.id(),
            fit,
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else if app.filmstrip_loading {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Preparing preview frames…",
            egui::FontId::proportional(13.0),
            theme::TEXT_MUTED(),
        );
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No preview frames",
            egui::FontId::proportional(13.0),
            theme::TEXT_DIM(),
        );
    }

    if resp.clicked() {
        if app.player_playing {
            app.player_playing = false;
        } else {
            if app.player_pos >= duration {
                app.player_pos = 0.0;
            }
            app.player_playing = n > 0;
        }
    }
    // Paused → centered play badge so the canvas reads as a player.
    if !app.player_playing && n > 0 {
        let c = rect.center();
        painter.circle_filled(c, 26.0, egui::Color32::from_black_alpha(140));
        icons::paint_icon(
            ui,
            Rect::from_center_size(c, Vec2::splat(24.0)),
            Icon::Play,
            theme::PRIMARY(),
        );
    }
    resp.on_hover_text("Click to play / pause (preview flipbook, no audio)");

    // Transport bar: play/pause · scrubber · timecode · open externally.
    ui.add_space(theme::SP_2);
    ui.horizontal(|ui| {
        let (r, pr) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
        let p = ui.painter_at(r);
        p.circle_filled(r.center(), 15.0, theme::PRIMARY());
        icons::paint_icon(
            ui,
            Rect::from_center_size(r.center(), Vec2::splat(16.0)),
            if app.player_playing { Icon::Pause } else { Icon::Play },
            theme::PRIMARY_INK(),
        );
        if pr.clicked() {
            if app.player_playing {
                app.player_playing = false;
            } else {
                if app.player_pos >= duration {
                    app.player_pos = 0.0;
                }
                app.player_playing = n > 0;
            }
        }
        pr.on_hover_text(if app.player_playing { "Pause" } else { "Play" });
        ui.add_space(theme::SP_2);
        ui.label(
            RichText::new(format_timecode(app.player_pos))
                .size(11.0)
                .color(theme::TEXT()),
        );
        ui.add(
            egui::Slider::new(&mut app.player_pos, 0.0..=duration.max(0.1))
                .show_value(false)
                .trailing_fill(true),
        );
        ui.label(
            RichText::new(format_timecode(duration))
                .size(11.0)
                .color(theme::TEXT_DIM()),
        );
    });
}

// ── Timeline (filmstrip + vertical in/out split lines) ─────────────

fn timeline(
    ui: &mut egui::Ui,
    filmstrip: &[egui::TextureHandle],
    duration: f64,
    start_s: f64,
    end_s: f64,
) -> (f64, f64) {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 56.0),
        Sense::click_and_drag(),
    );
    let painter = ui.painter_at(rect);
    let rounding = theme::rounding_sm();

    painter.rect_filled(rect, rounding, theme::SURFACE_2());

    let n = filmstrip.len();
    if n > 0 {
        let tw = rect.width() / n as f32;
        for (i, tex) in filmstrip.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(rect.left() + i as f32 * tw, rect.top()),
                Vec2::new(tw, rect.height()),
            );
            painter.image(
                tex.id(),
                r,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    }

    let x = |t: f64| rect.left() + (t / duration).clamp(0.0, 1.0) as f32 * rect.width();
    let (xs, xe) = (x(start_s), x(end_s));

    if xs > rect.left() {
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(rect.left(), rect.top()), Pos2::new(xs, rect.bottom())),
            egui::Rounding::ZERO,
            theme::OVERLAY_DIM(),
        );
    }
    if xe < rect.right() {
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(xe, rect.top()), Pos2::new(rect.right(), rect.bottom())),
            egui::Rounding::ZERO,
            theme::OVERLAY_DIM(),
        );
    }

    for px in [xs, xe] {
        painter.line_segment(
            [Pos2::new(px, rect.top()), Pos2::new(px, rect.bottom())],
            Stroke::new(2.0_f32, theme::TEXT()),
        );
        let grip = Rect::from_center_size(Pos2::new(px, rect.center().y), Vec2::new(8.0, 20.0));
        painter.rect_filled(grip, theme::rounding_sm(), theme::PRIMARY());
    }
    painter.rect_stroke(rect, rounding, Stroke::new(1.0_f32, theme::BORDER()));

    let mut ns = start_s;
    let mut ne = end_s;
    if resp.dragged() || resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64 * duration;
            if (t - start_s).abs() < (t - end_s).abs() {
                ns = t.min(end_s - 0.5).max(0.0);
            } else {
                ne = t.max(start_s + 0.5).min(duration);
            }
        }
    }
    resp.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    (ns, ne)
}

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

// ── Export zone ────────────────────────────────────────────────────

fn export_card(ui: &mut egui::Ui, app: &mut VibecapApp, file: &std::path::Path) {
    card(ui, "EXPORT", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Start").size(12.0).color(theme::TEXT_MUTED()));
            ui.add(
                egui::TextEdit::singleline(&mut app.trim_start)
                    .desired_width(88.0)
                    .hint_text("00:00:00"),
            );
            ui.add_space(theme::SP_3);
            ui.label(RichText::new("End").size(12.0).color(theme::TEXT_MUTED()));
            ui.add(
                egui::TextEdit::singleline(&mut app.trim_end)
                    .desired_width(88.0)
                    .hint_text("00:00:10"),
            );
        });
        ui.add_space(theme::SP_2);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Speed").size(12.0).color(theme::TEXT_MUTED()));
            let mut speed: &str = match app.export_speed.as_str() {
                "0.5" => "0.5",
                "1.5" => "1.5",
                "2.0" => "2.0",
                _ => "1.0",
            };
            segmented(
                ui,
                &mut speed,
                &[("0.5", "0.5×"), ("1.0", "1×"), ("1.5", "1.5×"), ("2.0", "2×")],
            );
            app.export_speed = speed.to_string();
        });
        ui.add_space(theme::SP_3);
        ui.horizontal_wrapped(|ui| {
            let file_clone = file.to_path_buf();
            if btn_primary(ui, "Trim video") {
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
            if btn_secondary(ui, "Export GIF") {
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
        });
    });
}

// ── Tools zone (grouped) ───────────────────────────────────────────

fn tools_card(ui: &mut egui::Ui, app: &mut VibecapApp, file: &std::path::Path) {
    card(ui, "TOOLS", |ui| {
        group(ui, "AUDIO", |ui| {
            let file_clone = file.to_path_buf();
            if btn_small(ui, "Extract audio") {
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
            if btn_small(ui, "Remove audio") {
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
        });
        group(ui, "TRANSFORM", |ui| {
            let file_clone = file.to_path_buf();
            if btn_small(ui, "Rotate 90° CW") {
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
            if btn_small(ui, "Rotate 90° CCW") {
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
            if btn_small(ui, "Rotate 180°") {
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
        });
        group(ui, "ENCODE", |ui| {
            let file_clone = file.to_path_buf();
            if btn_small(ui, "Compress") {
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
            if btn_small(ui, &format!("Apply {}× speed", app.export_speed)) {
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
            if btn_small(ui, "Frame @ playhead") {
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
        });
    });
}

// ── Tab body ───────────────────────────────────────────────────────

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    // Toolbar: file info left · actions right.
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            if let Some(f) = app.edit_file.clone() {
                ui.label(
                    RichText::new(f.file_name().and_then(|n| n.to_str()).unwrap_or("video"))
                        .size(14.0)
                        .strong()
                        .color(theme::TEXT()),
                );
                ui.label(
                    RichText::new(f.display().to_string())
                        .size(10.0)
                        .color(theme::TEXT_DIM()),
                );
            } else {
                ui.label(
                    RichText::new("No clip loaded")
                        .size(14.0)
                        .strong()
                        .color(theme::TEXT()),
                );
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if btn_secondary(ui, "Select video") {
                if let Some(path) = FileDialog::new()
                    .add_filter("Video", &["mp4", "mov", "webm", "mkv"])
                    .pick_file()
                {
                    app.edit_file = Some(path.clone());
                    app.load_filmstrip(ctx, path);
                }
            }
            if let Some(f) = app.edit_file.clone() {
                if btn_small(ui, "Reload") {
                    app.load_filmstrip(ctx, f.clone());
                }
                if btn_small(ui, "Reveal") {
                    let _ = reveal_in_file_manager(&f);
                }
                if btn_small(ui, "Open") {
                    let _ = open_path(&f);
                }
            }
        });
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

    ui.add_space(theme::SP_3);

    let duration = if app.clip_duration_secs > 0.0 {
        app.clip_duration_secs
    } else {
        (parse_timecode(&app.trim_end).unwrap_or(5.0) + 5.0).max(10.0)
    };

    // ── Player card (big screen + transport) ─────────────────────
    card(ui, "PLAYER", |ui| {
        player(app, ui, ctx, duration);
        ui.add_space(theme::SP_3);
        let start_s = parse_timecode(&app.trim_start).unwrap_or(0.0).clamp(0.0, duration);
        let end_s = parse_timecode(&app.trim_end)
            .unwrap_or(duration.min(5.0))
            .clamp(0.0, duration);
        let (ns, ne) = timeline(ui, &app.filmstrip, duration, start_s, end_s);
        if (ns - start_s).abs() > 0.4 {
            app.trim_start = format_timecode(ns);
        }
        if (ne - end_s).abs() > 0.4 {
            app.trim_end = format_timecode(ne);
        }
        ui.add_space(theme::SP_1);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "In {} · Out {} · span {}",
                    format_timecode(ns),
                    format_timecode(ne),
                    format_timecode(ne - ns)
                ))
                .size(11.0)
                .color(theme::TEXT_DIM()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("drag split lines to trim · preview is silent")
                        .size(10.0)
                        .color(theme::TEXT_DIM()),
                );
            });
        });
    });

    ui.add_space(theme::SP_3);

    // ── Export + Tools: two columns when wide, stacked when narrow ──
    let wide = ui.available_width() > 720.0;
    if wide {
        let half = (ui.available_width() - theme::SP_3) / 2.0;
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_min_width(half);
                ui.set_max_width(half);
                export_card(ui, app, &file);
            });
            ui.add_space(theme::SP_3);
            ui.vertical(|ui| {
                ui.set_min_width(half);
                ui.set_max_width(half);
                tools_card(ui, app, &file);
            });
        });
    } else {
        export_card(ui, app, &file);
        ui.add_space(theme::SP_3);
        tools_card(ui, app, &file);
    }
}
