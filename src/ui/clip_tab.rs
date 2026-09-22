//! Clip studio — in-app preview player + trim / export (Safelight body).

use eframe::egui;
use egui::{Frame, Margin, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use rfd::FileDialog;

use crate::platform::{format_timecode, open_path, parse_timecode, reveal_in_file_manager};
use crate::ui::empty_state;
use crate::ui::icons::{self, Icon};
use crate::ui::theme;
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
    if n > 0 && app.clip_compare {
        // F146 — in/out split: see both cut points before committing a trim.
        let in_s = parse_timecode(&app.trim_start).unwrap_or(0.0);
        let out_s = parse_timecode(&app.trim_end)
            .filter(|v| *v > 0.05)
            .unwrap_or(duration);
        let gap = 6.0;
        let pane_w = ((rect.width() - gap) / 2.0).max(40.0);
        for (k, (t, tag)) in [(in_s, "IN"), (out_s, "OUT")].iter().enumerate() {
            let pane = Rect::from_min_size(
                Pos2::new(rect.left() + k as f32 * (pane_w + gap), rect.top()),
                Vec2::new(pane_w, rect.height()),
            );
            let idx = ((t * app.filmstrip_fps).floor() as usize).min(n - 1);
            let tex = &app.filmstrip[idx];
            let fw = pane.width().min(pane.height() * 16.0 / 9.0);
            let fh = fw * 9.0 / 16.0;
            let fit = Rect::from_center_size(pane.center(), Vec2::new(fw, fh));
            painter.image(
                tex.id(),
                fit,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            painter.rect_stroke(
                pane,
                theme::rounding_md(),
                Stroke::new(1.0_f32, theme::BORDER()),
            );
            // Label chip pinned to the pane's top-left.
            let label = format!("{tag} {}", format_timecode(*t));
            let chip = Rect::from_min_size(
                pane.min + Vec2::new(6.0, 6.0),
                Vec2::new(label.len() as f32 * 7.0 + 12.0, 18.0),
            );
            painter.rect_filled(chip, 4.0, theme::OVERLAY_LABEL());
            painter.text(
                chip.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(11.0),
                theme::ON_SOLID(),
            );
        }
    } else if n > 0 {
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
        let label = if app.filmstrip_progress.1 > 0 {
            format!(
                "Extracting preview… {}/{}",
                app.filmstrip_progress.0, app.filmstrip_progress.1
            )
        } else {
            "Extracting preview…".to_string()
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(13.0),
            theme::TEXT_MUTED(),
        );
    } else if let Some(err) = &app.filmstrip_error {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("Preview failed: {err}\nOpen plays the file in a real media player."),
            egui::FontId::proportional(13.0),
            theme::WARN(),
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

    // With no decoded frames Play is a dead button — fall back to the real
    // media player so the click never feels ignored.
    let open_external = |app: &mut VibecapApp| {
        if let Some(f) = &app.edit_file {
            let _ = open_path(f);
            app.show_toast("No in-app preview — opened in default player");
        }
    };

    if resp.clicked() {
        if app.player_playing {
            app.player_playing = false;
        } else if n > 0 {
            if app.player_pos >= duration {
                app.player_pos = 0.0;
            }
            app.player_playing = true;
        } else if !app.filmstrip_loading {
            open_external(app);
        }
    }
    if !ctx.wants_keyboard_input() {
        ctx.input(|i| {
            let frame = 1.0 / app.filmstrip_fps.max(1.0);
            if i.key_pressed(egui::Key::ArrowLeft) {
                app.player_pos = (app.player_pos - frame).max(0.0);
                app.player_playing = false;
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                app.player_pos = (app.player_pos + frame).min(duration);
                app.player_playing = false;
            }
            if i.key_pressed(egui::Key::J) {
                app.player_pos = (app.player_pos - 10.0 * frame).max(0.0);
                app.player_playing = false;
            }
            if i.key_pressed(egui::Key::K) {
                app.player_pos = (app.player_pos + 10.0 * frame).min(duration);
                app.player_playing = false;
            }
            if i.key_pressed(egui::Key::Home) {
                app.player_pos = 0.0;
                app.player_playing = false;
            }
            if i.key_pressed(egui::Key::End) {
                app.player_pos = duration;
                app.player_playing = false;
            }
            if i.key_pressed(egui::Key::Space) {
                if app.player_playing {
                    app.player_playing = false;
                } else if n > 0 {
                    if app.player_pos >= duration {
                        app.player_pos = 0.0;
                    }
                    app.player_playing = true;
                }
            }
            if i.key_pressed(egui::Key::L) {
                app.clip_loop = !app.clip_loop;
            }
            // NLE-style trim: I/O drop the in/out points at the playhead.
            if i.key_pressed(egui::Key::I) {
                let out = parse_timecode(&app.trim_end).unwrap_or(duration);
                app.trim_start = format_timecode(app.player_pos.min((out - 0.5).max(0.0)));
            }
            if i.key_pressed(egui::Key::O) {
                let start = parse_timecode(&app.trim_start).unwrap_or(0.0);
                app.trim_end = format_timecode(app.player_pos.max(start + 0.5).min(duration));
            }
        });
    }
    let in_s = parse_timecode(&app.trim_start).unwrap_or(0.0);
    let out_s = parse_timecode(&app.trim_end)
        .unwrap_or(duration)
        .min(duration);
    if app.clip_loop && app.player_playing {
        if app.player_pos < in_s {
            app.player_pos = in_s;
        }
        if app.player_pos >= out_s.max(in_s + 0.05) {
            app.player_pos = in_s;
        }
    } else if app.player_playing && app.player_pos >= duration {
        app.player_pos = duration;
        app.player_playing = false;
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
    resp.on_hover_text(
        "Click or Space = play/pause · ←/→ frame · J/K ×10 · I/O trim · L loop (preview flipbook, no audio)",
    );
    ui.label(
        RichText::new(if app.preview_audio_path.is_some() {
            "Preview · audio on — flipbook fidelity, Open for the real thing"
        } else {
            "Preview (no audio) — Open for full-fidelity playback"
        })
        .small()
        .color(theme::TEXT_DIM()),
    );

    // Transport bar: play/pause · scrubber · timecode · open externally.
    ui.add_space(theme::SP_2);
    ui.horizontal(|ui| {
        let (r, pr) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
        let p = ui.painter_at(r);
        p.circle_filled(r.center(), 15.0, theme::PRIMARY());
        icons::paint_icon(
            ui,
            Rect::from_center_size(r.center(), Vec2::splat(16.0)),
            if app.player_playing {
                Icon::Pause
            } else {
                Icon::Play
            },
            theme::PRIMARY_INK(),
        );
        if pr.clicked() {
            if app.player_playing {
                app.player_playing = false;
            } else if n > 0 {
                if app.player_pos >= duration {
                    app.player_pos = 0.0;
                }
                app.player_playing = true;
            } else if !app.filmstrip_loading {
                open_external(app);
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
        // Always-visible external Open — real-player fallback shouldn't only
        // exist on the error path.
        if btn_small(ui, "Open") {
            open_external(app);
        }
        // F146 — split the canvas into in-point / out-point frames.
        if n > 0 && btn_small(ui, if app.clip_compare { "I|O ●" } else { "I|O" }) {
            app.clip_compare = !app.clip_compare;
        }
        // Frame-grab — current preview frame → a new still next to the clip.
        if n > 0 && btn_small(ui, "Grab frame") {
            if let Some(f) = &app.edit_file {
                let src = f.clone();
                let out = src.with_file_name(format!(
                    "frame_{}.jpg",
                    format_timecode(app.player_pos).replace(':', "-")
                ));
                app.spawn_ffmpeg_job(
                    vec![
                        "-y".into(),
                        "-ss".into(),
                        format!("{:.3}", app.player_pos),
                        "-i".into(),
                        src.to_str().unwrap_or_default().into(),
                        "-frames:v".into(),
                        "1".into(),
                        "-q:v".into(),
                        "2".into(),
                        out.to_str().unwrap_or_default().into(),
                    ],
                    "Frame saved as JPG",
                );
            }
        }
    });
}

// ── Timeline (filmstrip + vertical in/out split lines) ─────────────

fn timeline(
    ui: &mut egui::Ui,
    filmstrip: &[egui::TextureHandle],
    duration: f64,
    start_s: f64,
    end_s: f64,
    markers: &[f64],
    cut: &mut std::collections::HashSet<usize>,
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
            // F136 — cut marks: red tint + ✕ so a marked frame reads at a glance.
            if cut.contains(&i) {
                painter.rect_filled(
                    r,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(255, 60, 60, 90),
                );
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    "✕",
                    egui::FontId::proportional(12.0),
                    egui::Color32::WHITE,
                );
            }
        }
        // Right-click a thumb to mark/unmark it for the cut-export.
        if resp.secondary_clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let i = (((pos.x - rect.left()) / tw).floor() as usize).min(n - 1);
                if !cut.remove(&i) {
                    cut.insert(i);
                }
            }
        }
    }

    let x = |t: f64| rect.left() + (t / duration).clamp(0.0, 1.0) as f32 * rect.width();
    let (xs, xe) = (x(start_s), x(end_s));

    if xs > rect.left() {
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.top()),
                Pos2::new(xs, rect.bottom()),
            ),
            egui::Rounding::ZERO,
            theme::OVERLAY_DIM(),
        );
    }
    if xe < rect.right() {
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(xe, rect.top()),
                Pos2::new(rect.right(), rect.bottom()),
            ),
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
    if duration > 0.05 {
        for t in markers {
            let px = x(*t);
            painter.line_segment(
                [Pos2::new(px, rect.top()), Pos2::new(px, rect.bottom())],
                Stroke::new(1.5_f32, theme::WARN()),
            );
        }
    }

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

// ── Export inspector groups ────────────────────────────────────────

/// E38 — file metadata block at the top of the inspector rail.
fn clip_meta(ui: &mut egui::Ui, app: &mut VibecapApp, file: &std::path::Path) {
    group(ui, "FILE", |ui| {
        ui.label(
            RichText::new(
                file.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file.display().to_string()),
            )
            .size(11.5)
            .strong(),
        );
        let meta = std::fs::metadata(file).ok();
        if let Some(m) = &meta {
            let mb = m.len() as f64 / 1_048_576.0;
            ui.label(
                RichText::new(format!("{mb:.1} MB"))
                    .size(11.0)
                    .color(theme::TEXT_MUTED()),
            );
            if let Ok(modified) = m.modified() {
                let dt: chrono::DateTime<Local> = modified.into();
                ui.label(
                    RichText::new(dt.format("%b %e · %H:%M").to_string())
                        .size(10.5)
                        .color(theme::TEXT_DIM()),
                );
            }
        }
        if app.clip_duration_secs > 0.0 {
            ui.label(
                RichText::new(format!("{} long", format_timecode(app.clip_duration_secs)))
                    .size(10.5)
                    .color(theme::TEXT_DIM()),
            );
        }
    });
}

fn export_groups(ui: &mut egui::Ui, app: &mut VibecapApp, file: &std::path::Path) {
    group(ui, "TRIM", |ui| {
        ui.label(RichText::new("Start").size(12.0).color(theme::TEXT_MUTED()));
        ui.add(
            egui::TextEdit::singleline(&mut app.trim_start)
                .desired_width(72.0)
                .hint_text("00:00:00"),
        );
        ui.label(RichText::new("End").size(12.0).color(theme::TEXT_MUTED()));
        ui.add(
            egui::TextEdit::singleline(&mut app.trim_end)
                .desired_width(72.0)
                .hint_text("00:00:10"),
        );
    });
    group(ui, "SPEED", |ui| {
        let mut speed: &str = match app.export_speed.as_str() {
            "0.5" => "0.5",
            "1.5" => "1.5",
            "2.0" => "2.0",
            _ => "1.0",
        };
        segmented(
            ui,
            &mut speed,
            &[
                ("0.5", "0.5×"),
                ("1.0", "1×"),
                ("1.5", "1.5×"),
                ("2.0", "2×"),
            ],
        );
        app.export_speed = speed.to_string();
    });
    group(ui, "GIF", |ui| {
        let dur = (parse_timecode(&app.trim_end).unwrap_or(5.0)
            - parse_timecode(&app.trim_start).unwrap_or(0.0))
        .max(0.2);
        let est = (dur
            * if app.gif_pingpong { 2.0 } else { 1.0 }
            * app.gif_fps as f64
            * (app.gif_width as f64 / 400.0)
            * 18.0) as u64;
        ui.label(
            RichText::new(format!("~{} KB", est.max(20)))
                .small()
                .color(theme::TEXT_DIM()),
        );
    });
    ui.label(RichText::new("fps").size(11.0).color(theme::TEXT_DIM()));
    ui.add(egui::Slider::new(&mut app.gif_fps, 8..=24).show_value(true));
    // E58 — frame delay is 1000/fps; show it in the units GIF editors use.
    ui.label(
        RichText::new(format!("{} ms/frame", 1000 / app.gif_fps.max(1) as u32))
            .size(10.5)
            .color(theme::TEXT_DIM()),
    );
    ui.label(RichText::new("width").size(11.0).color(theme::TEXT_DIM()));
    ui.add(egui::Slider::new(&mut app.gif_width, 320..=1280).show_value(true));
    // E58 — end-of-loop hold: clone the last frame for N ms so the loop
    // breathes instead of snapping back (tpad before the GIF muxer).
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("end hold")
                .size(11.0)
                .color(theme::TEXT_DIM()),
        );
        ui.add(
            egui::Slider::new(&mut app.gif_hold_ms, 0..=4000)
                .suffix(" ms")
                .fixed_decimals(0),
        )
        .on_hover_text("Hold the final frame this long before the loop restarts");
    });
    ui.checkbox(&mut app.gif_pingpong, "Ping-pong ↺")
        .on_hover_text("Boomerang — plays forward then in reverse (2× duration)");
    ui.add_space(theme::SP_2);
    group(ui, "PRESETS", |ui| {
        let file_clone = file.to_path_buf();
        if btn_small(ui, "Discord 8 MB") {
            let out = file_clone.with_file_name(format!(
                "discord_{}",
                file_clone.file_name().unwrap().to_str().unwrap()
            ));
            app.spawn_ffmpeg_job(
                vec![
                    "-y".into(),
                    "-i".into(),
                    file_clone.to_str().unwrap().into(),
                    "-fs".into(),
                    "8000000".into(),
                    "-c:v".into(),
                    "libx264".into(),
                    "-crf".into(),
                    "28".into(),
                    "-preset".into(),
                    "fast".into(),
                    "-vf".into(),
                    "scale=1280:-2".into(),
                    "-c:a".into(),
                    "aac".into(),
                    "-b:a".into(),
                    "96k".into(),
                    out.to_str().unwrap().into(),
                ],
                "Discord 8 MB export",
            );
        }
        if btn_small(ui, "README 480p 3s") {
            let out = file_clone.with_file_name("readme_480p.gif");
            app.spawn_ffmpeg_job(
                vec![
                    "-y".into(),
                    "-t".into(),
                    "3".into(),
                    "-i".into(),
                    file_clone.to_str().unwrap().into(),
                    "-vf".into(),
                    "fps=12,scale=854:-1:flags=lanczos".into(),
                    out.to_str().unwrap().into(),
                ],
                "README 480p 3s GIF",
            );
        }
        if btn_small(ui, "Full lossless") {
            let out = file_clone.with_file_name(format!(
                "lossless_{}",
                file_clone.file_name().unwrap().to_str().unwrap()
            ));
            app.spawn_ffmpeg_job(
                vec![
                    "-y".into(),
                    "-i".into(),
                    file_clone.to_str().unwrap().into(),
                    "-c:v".into(),
                    "libx264".into(),
                    "-crf".into(),
                    "0".into(),
                    "-c:a".into(),
                    "copy".into(),
                    out.to_str().unwrap().into(),
                ],
                "Lossless copy",
            );
        }
    });
    ui.add_space(theme::SP_3);
    ui.horizontal_wrapped(|ui| {
        let file_clone = file.to_path_buf();
        if btn_primary(ui, "Trim video") {
            let out = file_clone.with_file_name(format!(
                "trimmed_{}",
                file_clone.file_name().unwrap().to_str().unwrap()
            ));
            // Verify the cut actually produced the ruler span — `-c copy`
            // snaps to keyframes, so the output can drift by seconds.
            let expected = (parse_timecode(&app.trim_end).unwrap_or(0.0)
                - parse_timecode(&app.trim_start).unwrap_or(0.0))
            .max(0.0);
            let out_probe = out.clone();
            app.spawn_ffmpeg_job_ex(
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
                Some(Box::new(move || {
                    let got = crate::platform::probe_duration(&out_probe)?;
                    let drift = (got - expected).abs();
                    if drift > 1.5 {
                        Some(format!(
                            "output is {:.1}s, expected {:.1}s (keyframe snap — re-encode for exact cut)",
                            got, expected
                        ))
                    } else {
                        None
                    }
                })),
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
                    {
                        // E58 — `tpad` clones the last frame for the hold
                        // duration, so the GIF loop tail breathes.
                        let hold = if app.gif_hold_ms > 0 {
                            format!(
                                ",tpad=stop_mode=clone:stop_duration={:.2}",
                                app.gif_hold_ms as f32 / 1000.0
                            )
                        } else {
                            String::new()
                        };
                        if app.gif_pingpong {
                            format!(
                                "fps={},scale={}:-1:flags=lanczos,split[a][b];[b]reverse[r];[a][r]concat=n=2:v=1{hold}",
                                app.gif_fps.clamp(4, 30),
                                app.gif_width.clamp(160, 1920)
                            )
                        } else {
                            format!(
                                "fps={},scale={}:-1:flags=lanczos{hold}",
                                app.gif_fps.clamp(4, 30),
                                app.gif_width.clamp(160, 1920)
                            )
                        }
                    },
                    "-y".into(),
                    gif_out.to_str().unwrap().into(),
                ],
                "GIF exported",
            );
        }
    });
}

// ── Tools inspector groups ─────────────────────────────────────────

fn tools_groups(ui: &mut egui::Ui, app: &mut VibecapApp, file: &std::path::Path) {
    {
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
            if btn_small(ui, "WebM (VP9)") {
                let out = file_clone
                    .with_file_name(format!(
                        "webm_{}",
                        file_clone
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("clip")
                    ))
                    .with_extension("webm");
                app.spawn_ffmpeg_job(
                    vec![
                        "-i".into(),
                        file_clone.to_str().unwrap().into(),
                        "-c:v".into(),
                        "libvpx-vp9".into(),
                        "-crf".into(),
                        "32".into(),
                        "-b:v".into(),
                        "0".into(),
                        "-c:a".into(),
                        "libopus".into(),
                        "-y".into(),
                        out.to_str().unwrap().into(),
                    ],
                    "WebM exported",
                );
            }
            if btn_small(ui, "AV1 (SVT)") {
                let out = file_clone
                    .with_file_name(format!(
                        "av1_{}",
                        file_clone
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("clip")
                    ))
                    .with_extension("mp4");
                app.spawn_ffmpeg_job(
                    vec![
                        "-i".into(),
                        file_clone.to_str().unwrap().into(),
                        "-c:v".into(),
                        "libsvtav1".into(),
                        "-crf".into(),
                        "35".into(),
                        "-preset".into(),
                        "10".into(),
                        "-c:a".into(),
                        "aac".into(),
                        "-y".into(),
                        out.to_str().unwrap().into(),
                    ],
                    "AV1 exported",
                );
            }
            if btn_small(ui, "Frame @ playhead") {
                let out = file_clone
                    .with_file_name(format!("frame_{}.jpg", app.trim_start.replace(':', "-")));
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
        group(ui, "NOTES", |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut app.clip_notes)
                    .hint_text("Clip notes — saved to a .notes.txt sidecar")
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            if btn_small(ui, "Save note") {
                let side = file.with_extension("notes.txt");
                if app.clip_notes.trim().is_empty() {
                    let _ = std::fs::remove_file(&side);
                    app.show_toast("Note cleared");
                } else {
                    match std::fs::write(&side, app.clip_notes.trim()) {
                        Ok(_) => app.show_toast("Note saved"),
                        Err(e) => app.show_toast(format!("❌ Note save failed: {e}")),
                    }
                }
            }
        });
    }
}

// ── Tab body ───────────────────────────────────────────────────────

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    // Esc always backs out to Capture — a review screen should never feel
    // like a dead end.
    if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.current_tab = crate::AppTab::Capture;
        return;
    }
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
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Nothing to review yet")
                            .size(14.0)
                            .strong()
                            .color(theme::TEXT()),
                    );
                    ui.label(
                        RichText::new("Record a clip (R) — it lands here for trim & GIF.")
                            .size(11.0)
                            .color(theme::TEXT_MUTED()),
                    );
                });
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            crate::ui::icon_menu_button(ui, "⋯", |ui| {
                ui.set_min_width(180.0);
                if ui.button("Select video…").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("Video", &["mp4", "mov", "webm", "mkv"])
                        .pick_file()
                    {
                        app.edit_file = Some(path.clone());
                        app.load_filmstrip(ui.ctx(), path);
                    }
                    ui.close_menu();
                }
                if let Some(f) = app.edit_file.clone() {
                    if ui.button("Reload preview").clicked() {
                        app.load_filmstrip(ui.ctx(), f.clone());
                        ui.close_menu();
                    }
                    if ui.button("Open in default app").clicked() {
                        let _ = open_path(&f);
                        ui.close_menu();
                    }
                    if ui.button("Copy path").clicked() {
                        if let Ok(mut board) = arboard::Clipboard::new() {
                            let _ = board.set_text(f.display().to_string());
                        }
                        app.show_toast("Clip path copied");
                        ui.close_menu();
                    }
                    if ui.button("Reveal in Explorer").clicked() {
                        let _ = reveal_in_file_manager(&f);
                        ui.close_menu();
                    }
                }
            });
            if app.edit_file.is_some() {
                if btn_secondary(ui, "⧉ Path") {
                    if let Some(f) = &app.edit_file {
                        if let Ok(mut board) = arboard::Clipboard::new() {
                            let _ = board.set_text(f.display().to_string());
                        }
                        app.show_toast("Clip path copied");
                    }
                }
                if btn_secondary(ui, "Open") {
                    if let Some(f) = &app.edit_file {
                        let _ = open_path(f);
                    }
                }
                if btn_primary(ui, "✓ Done") {
                    let f = app.edit_file.clone().unwrap();
                    // arboard, not egui copy_text — the path must reach the OS
                    // clipboard, not just egui's internal paste buffer.
                    let _ = arboard::Clipboard::new()
                        .and_then(|mut b| b.set_text(f.display().to_string()));
                    app.show_toast("Clip path copied — back to Capture");
                    app.current_tab = crate::AppTab::Capture;
                }
            } else {
                if btn_secondary(ui, "Select video…") {
                    if let Some(path) = FileDialog::new()
                        .add_filter("Video", &["mp4", "mov", "webm", "mkv"])
                        .pick_file()
                    {
                        app.edit_file = Some(path.clone());
                        app.load_filmstrip(ctx, path);
                    }
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
            "Record from Capture, pick from Library, or select a video file.",
        );
        return;
    };

    ui.add_space(theme::SP_3);

    let duration = if app.clip_duration_secs > 0.0 {
        app.clip_duration_secs
    } else {
        (parse_timecode(&app.trim_end).unwrap_or(5.0) + 5.0).max(10.0)
    };

    // Player-dominant layout: screen + timeline left, export inspector right.
    // E38 — the rail is optional; closed, the player takes the full width.
    const INSPECTOR_W: f32 = 244.0;
    let inspector_w = if app.inspector_open {
        INSPECTOR_W + theme::SP_3
    } else {
        0.0
    };
    let player_w = (ui.available_width() - inspector_w).max(320.0);
    let body_h = ui.available_height().max(360.0);

    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(player_w, body_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                // ── Player card (big screen + transport) ─────────────────────
                card(ui, "", |ui| {
                    player(app, ui, ctx, duration);
                    ui.add_space(theme::SP_3);
                    let start_s = parse_timecode(&app.trim_start)
                        .unwrap_or(0.0)
                        .clamp(0.0, duration);
                    let end_s = parse_timecode(&app.trim_end)
                        .unwrap_or(duration.min(5.0))
                        .clamp(0.0, duration);
                    let markers = app.record_markers.clone();
                    let (ns, ne) = timeline(
                        ui,
                        &app.filmstrip,
                        duration,
                        start_s,
                        end_s,
                        &markers,
                        &mut app.filmstrip_cut,
                    );
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
                                RichText::new(
                                    "drag split lines to trim · right-click a thumb to mark a cut",
                                )
                                .size(10.0)
                                .color(theme::TEXT_DIM()),
                            );
                        });
                    });
                    // F136 — cut-marked thumbs: drop those sections on export.
                    if !app.filmstrip_cut.is_empty() {
                        ui.add_space(theme::SP_1);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "✂ {} section{} marked",
                                    app.filmstrip_cut.len(),
                                    if app.filmstrip_cut.len() == 1 {
                                        ""
                                    } else {
                                        "s"
                                    }
                                ))
                                .size(10.0)
                                .color(theme::WARN()),
                            );
                            if btn_small(ui, "Export without cuts") {
                                app.export_without_cuts(&file);
                            }
                            if btn_small(ui, "Clear") {
                                app.filmstrip_cut.clear();
                            }
                        });
                    }
                    // Marker list — click a chapter tick to jump the playhead.
                    if !markers.is_empty() {
                        ui.add_space(theme::SP_1);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("Markers").size(10.0).color(theme::TEXT_DIM()));
                            for m in &markers {
                                if btn_small(ui, &format_timecode(*m)) {
                                    app.player_pos = (*m).clamp(0.0, duration);
                                    app.player_playing = false;
                                }
                            }
                        });
                    }
                    // Dead-air offer — frozen head/tail detected at filmstrip
                    // load; one tap trims the ruler to the live span.
                    if let Some((cs, ce)) = app.dead_air_hint {
                        if !app.dead_air_dismissed {
                            ui.add_space(theme::SP_1);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "Dead air — content runs {}–{}",
                                        format_timecode(cs),
                                        format_timecode(ce)
                                    ))
                                    .size(10.0)
                                    .color(theme::WARN()),
                                );
                                if btn_small(ui, "Trim to content") {
                                    app.trim_start = format_timecode(cs);
                                    app.trim_end = format_timecode(ce);
                                    app.dead_air_dismissed = true;
                                }
                                if btn_small(ui, "Dismiss") {
                                    app.dead_air_dismissed = true;
                                }
                            });
                        }
                    }
                });
            },
        ); // left column

        ui.add_space(theme::SP_3);
        // ── Inspector (right rail — trim, export, tools) ─────────────────
        if app.inspector_open {
            ui.allocate_ui_with_layout(
                Vec2::new(INSPECTOR_W, body_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_source("clip_inspector")
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.set_max_width(INSPECTOR_W - 8.0);
                            clip_meta(ui, app, &file);
                            ui.add_space(theme::SP_2);
                            ui.separator();
                            ui.add_space(theme::SP_2);
                            export_groups(ui, app, &file);
                            ui.add_space(theme::SP_2);
                            ui.separator();
                            ui.add_space(theme::SP_2);
                            tools_groups(ui, app, &file);
                        });
                },
            );
        }
    }); // horizontal split
}
