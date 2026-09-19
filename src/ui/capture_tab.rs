//! Capture tab UI (extracted from main for Phase 1a).

use eframe::egui;
use egui::{RichText, Stroke};

use crate::ui::theme;
use crate::ui::{shutter_strip, ShutterAction};
use crate::{CaptureTarget, VibecapApp};
use crate::ui::{btn_small, switch};

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {

                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("What do you want to grab?")
                                .color(theme::TEXT_MUTED())
                                .size(13.0),
                        );
                        ui.add_space(theme::SP_3);

                        // ── Step 1: target cards — fixed thirds so none eats the row ──
                        ui.horizontal(|ui| {
                            let card_w =
                                ((ui.available_width() - 2.0 * theme::SP_2) / 3.0).min(240.0);
                            for (target, icon, label, hint) in [
                                (
                                    CaptureTarget::Fullscreen,
                                    crate::ui::icons::Icon::Monitor,
                                    "Full screen",
                                    "everything",
                                ),
                                (
                                    CaptureTarget::Region,
                                    crate::ui::icons::Icon::Region,
                                    "A region",
                                    "drag a box",
                                ),
                                (
                                    CaptureTarget::Window,
                                    crate::ui::icons::Icon::Window,
                                    "A window",
                                    "one app",
                                ),
                            ] {
                                let on = app.capture_target == target;
                                let (fill, stroke_c) = if on {
                                    (
                                        theme::ACCENT().gamma_multiply(0.14),
                                        theme::ACCENT(),
                                    )
                                } else {
                                    (theme::SURFACE(), theme::BORDER())
                                };
                                let resp = egui::Frame::none()
                                    .fill(fill)
                                    .stroke(Stroke::new(if on { 1.5_f32 } else { 1.0_f32 }, stroke_c))
                                    .rounding(theme::rounding_md())
                                    .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                                    .show(ui, |ui| {
                                        ui.set_width(card_w - 28.0);
                                        ui.set_min_height(56.0);
                                        ui.vertical_centered(|ui| {
                                            let (r, _) = ui.allocate_exact_size(
                                                egui::Vec2::splat(24.0),
                                                egui::Sense::hover(),
                                            );
                                            crate::ui::icons::paint_icon(
                                                ui,
                                                r,
                                                icon,
                                                if on { theme::ACCENT() } else { theme::TEXT_MUTED() },
                                            );
                                            ui.add_space(2.0);
                                            ui.label(
                                                RichText::new(label)
                                                    .size(13.0)
                                                    .strong()
                                                    .color(theme::TEXT()),
                                            );
                                            ui.label(
                                                RichText::new(hint)
                                                    .size(10.0)
                                                    .color(theme::TEXT_DIM()),
                                            );
                                        });
                                    })
                                    .response
                                    .interact(egui::Sense::click());
                                if resp.clicked() {
                                    app.capture_target = target;
                                }
                                ui.add_space(theme::SP_2);
                            }
                        });
                        if app.capture_target == CaptureTarget::Window {
                            ui.add_space(theme::SP_2);
                            if !app.window_list_scanned
                                || app
                                    .window_list_at
                                    .map(|t| t.elapsed().as_secs() >= 2)
                                    .unwrap_or(true)
                            {
                                app.refresh_window_list();
                                app.window_list_at = Some(std::time::Instant::now());
                            }
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_source("window_app_picker")
                                    .selected_text(if app.window_app.is_empty() {
                                        "Select app…".to_string()
                                    } else {
                                        app.window_app.clone()
                                    })
                                    .width(220.0)
                                    .show_ui(ui, |ui| {
                                        let wins =
                                            crate::platform::list_capture_windows_cached();
                                        if wins.is_empty() {
                                            for name in app.window_app_list.clone() {
                                                ui.selectable_value(
                                                    &mut app.window_app,
                                                    name.clone(),
                                                    name,
                                                );
                                            }
                                        } else {
                                            for w in wins {
                                                if w.is_self() {
                                                    continue;
                                                }
                                                let mut lab = w.label();
                                                if w.minimized {
                                                    lab.push_str(" (minimized)");
                                                }
                                                lab.push_str(&format!(
                                                    "  {}×{} @{},{}",
                                                    w.w, w.h, w.x, w.y
                                                ));
                                                ui.selectable_value(
                                                    &mut app.window_app,
                                                    w.label(),
                                                    lab,
                                                );
                                            }
                                        }
                                    });
                                if btn_small(ui, "↻") {
                                    app.refresh_window_list();
                                }
                                // Snagit-style: click the window itself.
                                #[cfg(windows)]
                                if btn_small(ui, "🎯 Pick") {
                                    app.start_region_pick(
                                        ctx,
                                        crate::RegionPickKind::WindowPick,
                                    );
                                }
                                // Same pick, but the rect becomes a recording
                                // crop — capture a window on video by click.
                                #[cfg(windows)]
                                if btn_small(ui, "🎯 Rec") {
                                    app.start_region_pick(
                                        ctx,
                                        crate::RegionPickKind::WindowRecord,
                                    );
                                }
                            });
                            ui.add(
                                egui::TextEdit::singleline(&mut app.window_app)
                                    .hint_text("Or type an app name (e.g. Chrome)")
                                    .desired_width(280.0),
                            );
                        }

                        ui.add_space(theme::SP_4);

                        // ── Step 2: the buttons ────────────────────
                        let rec_label = if app.is_recording {
                            let elapsed = app.recording_elapsed_secs();
                            format!("Stop  [{:02}:{:02}]", elapsed / 60, elapsed % 60)
                        } else if app.recording_arming {
                            "Starting…".to_string()
                        } else if app.recording_finalizing {
                            "Saving…".to_string()
                        } else {
                            "Record  (R)".to_string()
                        };
                        if let Some(act) = shutter_strip(
                            ui,
                            app.is_recording,
                            app.recording_arming || app.recording_finalizing,
                            &rec_label,
                        ) {
                            match act {
                                ShutterAction::Screenshot => app.trigger_capture(ctx, true),
                                ShutterAction::RecordToggle => {
                                    if app.is_recording {
                                        app.stop_recording(ctx);
                                    } else if app.recording_arming {
                                        app.cancel_recording(ctx);
                                    } else {
                                        app.trigger_capture(ctx, false);
                                    }
                                }
                                ShutterAction::Gif => app.trigger_gif_clip(ctx),
                            }
                        }

                        // ── Options row: Snipping-Tool delay + clipboard-only ──
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Delay")
                                    .size(11.0)
                                    .color(theme::TEXT_DIM()),
                            );
                            egui::ComboBox::from_id_source("capture_delay_secs")
                                .selected_text(if app.capture_delay_secs == 0 {
                                    "None".to_string()
                                } else {
                                    format!("{}s", app.capture_delay_secs)
                                })
                                .width(72.0)
                                .show_ui(ui, |ui| {
                                    for secs in [0u64, 3, 5, 10] {
                                        ui.selectable_value(
                                            &mut app.capture_delay_secs,
                                            secs,
                                            if secs == 0 {
                                                "None".to_string()
                                            } else {
                                                format!("{secs}s")
                                            },
                                        );
                                    }
                                });
                            ui.add_space(theme::SP_3);
                            let mut clip_only = app.clipboard_only;
                            if switch(ui, "Clipboard only", &mut clip_only) {
                                app.clipboard_only = clip_only;
                            }
                            ui.label(
                                RichText::new("copy, don't save")
                                    .size(10.0)
                                    .color(theme::TEXT_DIM()),
                            );
                        });

                        ui.add_space(theme::SP_4);

                        // ── Recent captures — "your stuff lands here" ──
                        {
                            let top: Vec<(std::path::PathBuf, bool)> = app
                                .library_items
                                .iter()
                                .take(3)
                                .map(|i| {
                                    (
                                        i.path.clone(),
                                        matches!(
                                            i.category,
                                            crate::app::MediaCategory::Video
                                                | crate::app::MediaCategory::Gif
                                        ),
                                    )
                                })
                                .collect();
                            let key = top
                                .iter()
                                .map(|(p, _)| p.display().to_string())
                                .collect::<Vec<_>>()
                                .join("|");
                            if key != app.recent_key {
                                app.recent_key = key;
                                app.recent_thumbs.clear();
                                if !top.is_empty() && app.recent_thumbs_rx.is_none() {
                                    let (tx, rx) = crossbeam_channel::bounded(1);
                                    app.recent_thumbs_rx = Some(rx);
                                    std::thread::spawn(move || {
                                        let mut out = Vec::new();
                                        for (p, is_vid) in top {
                                            let Some(tp) = crate::app::thumbs::ensure_thumb(&p)
                                            else {
                                                continue;
                                            };
                                            if let Ok(img) = image::open(&tp) {
                                                let rgba = img.to_rgba8();
                                                let (w, h) =
                                                    (rgba.width() as usize, rgba.height() as usize);
                                                out.push((
                                                    p,
                                                    is_vid,
                                                    egui::ColorImage::from_rgba_unmultiplied(
                                                        [w, h],
                                                        &rgba.into_raw(),
                                                    ),
                                                ));
                                            }
                                        }
                                        let _ = tx.send(out);
                                    });
                                }
                            }

                            if !app.recent_thumbs.is_empty() {
                                ui.label(
                                    RichText::new("RECENT — TAP TO REVIEW")
                                        .size(10.0)
                                        .strong()
                                        .color(theme::TEXT_DIM()),
                                );
                                ui.add_space(theme::SP_2);
                                ui.horizontal(|ui| {
                                    let mut open: Option<(std::path::PathBuf, bool)> = None;
                                    for (path, is_video, tex) in &app.recent_thumbs {
                                        let resp = egui::Frame::none()
                                            .stroke(Stroke::new(1.0_f32, theme::BORDER()))
                                            .rounding(theme::rounding_md())
                                            .show(ui, |ui| {
                                                let img = egui::Image::new(tex)
                                                    .fit_to_exact_size(egui::Vec2::new(120.0, 68.0))
                                                    .rounding(theme::rounding_md());
                                                let r = ui.add(img);
                                                if *is_video {
                                                    let p = ui.painter();
                                                    let c = r.rect.center();
                                                    p.circle_filled(
                                                        c,
                                                        10.0,
                                                        theme::CANVAS().gamma_multiply(0.75),
                                                    );
                                                    p.add(egui::Shape::convex_polygon(
                                                        vec![
                                                            c + egui::Vec2::new(-3.0, -5.0),
                                                            c + egui::Vec2::new(-3.0, 5.0),
                                                            c + egui::Vec2::new(6.0, 0.0),
                                                        ],
                                                        theme::TEXT(),
                                                        Stroke::NONE,
                                                    ));
                                                }
                                                r
                                            })
                                            .inner
                                            .interact(egui::Sense::click())
                                            .on_hover_text(format!(
                                                "{} — open in Review",
                                                path.file_name()
                                                    .map(|f| f.to_string_lossy().to_string())
                                                    .unwrap_or_default()
                                            ));
                                        if resp.clicked() {
                                            open = Some((path.clone(), *is_video));
                                        }
                                        ui.add_space(theme::SP_2);
                                    }
                                    if let Some((p, is_video)) = open {
                                        if is_video {
                                            app.edit_file = Some(p.clone());
                                            app.current_tab = crate::AppTab::Clip;
                                            app.load_filmstrip(ctx, p);
                                        } else {
                                            app.open_still_from_path(p);
                                        }
                                    }
                                });
                                ui.add_space(theme::SP_3);
                            } else if app.library_items.is_empty() {
                                ui.label(
                                    RichText::new(
                                        "Your captures land in Review → Library. Try Screenshot now.",
                                    )
                                    .size(11.0)
                                    .color(theme::TEXT_DIM()),
                                );
                                ui.add_space(theme::SP_3);
                            }
                        }

                        // ── Capture options (grouped card) ────────
                        egui::Frame::none()
                            .fill(theme::SURFACE())
                            .rounding(theme::rounding_md())
                            .stroke(Stroke::new(1.0_f32, theme::BORDER()))
                            .inner_margin(egui::Margin::same(theme::SP_3))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width().min(560.0));
                                // Secondary knobs collapse — the funnel is
                                // target → shutter, options on demand.
                                egui::CollapsingHeader::new(
                                    RichText::new("Options · cursor · audio · display")
                                        .size(11.0)
                                        .strong()
                                        .color(theme::TEXT_MUTED()),
                                )
                                .default_open(false)
                                .show(ui, |ui| {
                                    crate::ui::group(ui, "POINTER", |ui| {
                                        switch(ui, "Draw cursor on stills", &mut app.draw_mouse);
                                    });
                                    crate::ui::group(ui, "AUDIO", |ui| {
                                        switch(ui, "Include audio", &mut app.capture_audio);
                                        if cfg!(target_os = "windows") {
                                            // ffmpeg -list_devices takes ~1s — probe
                                            // on a worker so the UI thread never stalls.
                                            if app.audio_devices.is_empty()
                                                && app.audio_devices_rx.is_none()
                                            {
                                                let (tx, rx) = crossbeam_channel::bounded(1);
                                                app.audio_devices_rx = Some(rx);
                                                std::thread::spawn(move || {
                                                    let _ = tx.send(
                                                        crate::platform::list_audio_input_devices(),
                                                    );
                                                });
                                            }
                                            if let Some(rx) = app.audio_devices_rx.as_ref() {
                                                if let Ok(devs) = rx.try_recv() {
                                                    app.audio_devices_rx = None;
                                                    app.audio_devices = devs;
                                                }
                                            }
                                            if app.capture_audio && app.audio_devices.is_empty() {
                                                ui.label(
                                                    RichText::new(
                                                        "No DirectShow audio device — set VIBECAP_AUDIO_DEVICE or the recording will be silent.",
                                                    )
                                                    .size(10.0)
                                                    .color(theme::WARN()),
                                                );
                                            }
                                        }
                                    });
                                    let monitors = crate::platform::list_monitors();
                                    if monitors.len() > 1 {
                                        crate::ui::group(ui, "DISPLAY", |ui| {
                                            let mut idx = app.capture_monitor.unwrap_or(0);
                                            for m in &monitors {
                                                let lab = format!(
                                                    "{} {}×{}{}",
                                                    m.index + 1,
                                                    m.w,
                                                    m.h,
                                                    if m.primary { " · primary" } else { "" }
                                                );
                                                if ui
                                                    .selectable_label(idx == m.index, lab)
                                                    .clicked()
                                                {
                                                    idx = m.index;
                                                }
                                            }
                                            app.capture_monitor = Some(idx);
                                        });
                                    }
                                });
                                if app.capture_target == CaptureTarget::Region {
                                    if let Some((w, h, x, y)) = app.selected_screen_rect {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!(
                                                    "Last region {w}×{h} at {x},{y}"
                                                ))
                                                .size(10.0)
                                                .color(theme::TEXT_MUTED()),
                                            );
                                            if btn_small(ui, "Clear") {
                                                app.selected_screen_rect = None;
                                                app.selected_region = None;
                                            }
                                        });
                                    }
                                }
                                if app.capture_target == CaptureTarget::Window {
                                    ui.label(
                                        RichText::new(
                                            "GPU apps (Chrome, Electron) will be brought to the front so the shot is not black.",
                                        )
                                        .size(10.0)
                                        .color(theme::TEXT_DIM()),
                                    );
                                }
                                ui.label(
                                    RichText::new(
                                        "S / R in app · Ctrl+Shift+3 / 2 global · FPS & countdown in Settings",
                                    )
                                    .size(10.0)
                                    .color(theme::TEXT_DIM()),
                                );
                                if app.capture_target == CaptureTarget::Fullscreen {
                                    if let Some(prev) = &app.last_front_app {
                                        ui.label(
                                            RichText::new(format!(
                                                "Fullscreen restores “{}” before the shot (never bare desktop).",
                                                prev
                                            ))
                                            .size(10.0)
                                            .color(theme::TEXT_MUTED()),
                                        );
                                    } else {
                                        ui.label(
                                            RichText::new(
                                                "Tip: click another app first, then Vibecap — Fullscreen restores that app before the shot.",
                                            )
                                            .size(10.0)
                                            .color(theme::TEXT_DIM()),
                                        );
                                    }
                                }
                            });

                        ui.add_space(theme::SP_4);

                        // Advanced / power-user surface — collapsed for the
                        // snipping-tool flow, force-opens when it needs attention
                        // (budget blown or retro buffer running).
                        let live = app.live_stats_snapshot();
                        let retro = app.retro.status();
                        let needs_attention = live.over.is_some() || retro.enabled;
                        egui::CollapsingHeader::new(
                            RichText::new("Advanced · live session & agent budget")
                                .color(theme::TEXT_MUTED())
                                .size(12.0)
                                .strong(),
                        )
                        .default_open(false)
                        .open(if needs_attention { Some(true) } else { None })
                        .show(ui, |ui| {
                            // Compact live-stats row
                            let over = live.over.clone();
                            ui.horizontal(|ui| {
                                let dot_color = if over.is_some() {
                                    theme::DANGER()
                                } else if live.count > 0 {
                                    theme::ACCENT()
                                } else {
                                    theme::TEXT_DIM()
                                };
                                let (r, _) =
                                    ui.allocate_exact_size(egui::Vec2::splat(8.0), egui::Sense::hover());
                                ui.painter()
                                    .circle_filled(r.center(), 4.0, dot_color);
                                ui.label(
                                    RichText::new(format!(
                                        "Live {} frames · {:.2} MB · cap {}f/{:.0}MB",
                                        live.count,
                                        live.mb,
                                        if live.frames_cap == 0 { u64::MAX.to_string() } else { live.frames_cap.to_string() },
                                        live.mb_cap,
                                    ))
                                    .small()
                                    .color(theme::TEXT_MUTED()),
                                );
                                if let Some(reason) = &over {
                                    ui.label(
                                        RichText::new(format!("⚠ {}", reason))
                                            .small()
                                            .color(theme::DANGER()),
                                    );
                                }
                            });

                            // Retro buffer status (only when it has something to say)
                            if retro.enabled || retro.frame_count > 0 {
                                ui.add_space(theme::SP_2);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(if retro.enabled {
                                            format!(
                                                "Retro · {:.0}s / {}s · {:.1} MB",
                                                retro.span_secs, retro.max_secs, retro.mb
                                            )
                                        } else {
                                            "Retro · off".into()
                                        })
                                        .small()
                                        .color(if retro.enabled {
                                            theme::ACCENT()
                                        } else {
                                            theme::TEXT_DIM()
                                        }),
                                    );
                                    if btn_small(ui, "Save GIF") {
                                        app.dump_retro_buffer();
                                    }
                                    if btn_small(ui, "Bug pack") {
                                        app.bug_report_pack(ctx);
                                    }
                                });
                            }
                            if app.record_countdown_secs > 0 {
                                ui.label(
                                    RichText::new(format!(
                                        "Countdown · {}s before record (Settings)",
                                        app.record_countdown_secs
                                    ))
                                    .small()
                                    .color(theme::TEXT_DIM()),
                                );
                            }

                            ui.add_space(theme::SP_2);
                            ui.label(
                                RichText::new(format!(
                                    "Agent budget: frames cap {} · MB cap {:.1} · minutes cap {} · tier {}",
                                    if live.frames_cap == 0 {
                                        "unlimited".to_string()
                                    } else {
                                        live.frames_cap.to_string()
                                    },
                                    live.mb_cap,
                                    if live.minutes_cap == 0 {
                                        "unlimited".to_string()
                                    } else {
                                        live.minutes_cap.to_string()
                                    },
                                    live.tier
                                ))
                                .size(12.0)
                                .color(theme::TEXT_MUTED()),
                            );
                            ui.label(
                                RichText::new(
                                    "Agents use vibecap_set_budget; live inspection auto-stops at caps.",
                                )
                                .small()
                                .color(theme::TEXT_DIM()),
                            );
                        });
                    });
                
}
