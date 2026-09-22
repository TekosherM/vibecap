//! Capture tab UI (extracted from main for Phase 1a).

use eframe::egui;
use egui::{RichText, Stroke};

use crate::ui::theme;
use crate::ui::{btn_small, switch};
use crate::ui::{shutter_strip, ShutterAction};
use crate::{CaptureTarget, VibecapApp};

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    // Centered content column — the column centers, rows inside
    // it left-align (Chromie popup column). vertical_centered
    // centered every row independently, orphaning short labels.
    // E35 — the column widens on big windows instead of capping at 720.
    let avail = ui.available_width();
    let col_w = avail.min(if avail > 1100.0 { 900.0 } else { 720.0 });
    ui.horizontal(|ui| {
                        ui.add_space(((ui.available_width() - col_w) / 2.0).max(0.0));
                        ui.allocate_ui_with_layout(
                            egui::Vec2::new(col_w, ui.available_height()),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                        // ── ffmpeg fix-it card — capture can't work without it;
                        //    don't let the user discover this via a dead click ──
                        if !crate::platform::ffmpeg_available() {
                            egui::Frame::none()
                                .fill(theme::SURFACE())
                                .rounding(theme::rounding_md())
                                .stroke(Stroke::new(1.0_f32, theme::DANGER_SOFT()))
                                // E2 — inline alert card sits at the flat
                                // "rest" tier, below raised cards.
                                .shadow(theme::elevation_rest())
                                .inner_margin(egui::Margin::same(theme::SP_3))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("ffmpeg not found — capture can't run")
                                            .size(12.0)
                                            .strong()
                                            .color(theme::DANGER_SOFT()),
                                    );
                                    ui.label(
                                        RichText::new(if cfg!(target_os = "windows") {
                                            "winget install Gyan.FFmpeg"
                                        } else if cfg!(target_os = "macos") {
                                            "brew install ffmpeg"
                                        } else {
                                            "sudo apt install ffmpeg"
                                        })
                                        .size(11.0)
                                        .monospace()
                                        .color(theme::TEXT_MUTED()),
                                    );
                                    ui.horizontal(|ui| {
                                        if btn_small(ui, "Copy command") {
                                            let cmd = if cfg!(target_os = "windows") {
                                                "winget install Gyan.FFmpeg"
                                            } else if cfg!(target_os = "macos") {
                                                "brew install ffmpeg"
                                            } else {
                                                "sudo apt install ffmpeg"
                                            };
                                            if arboard::Clipboard::new()
                                                .and_then(|mut b| b.set_text(cmd))
                                                .is_ok()
                                            {
                                                app.show_toast("Install command copied");
                                            }
                                        }
                                        if btn_small(ui, "Re-check") {
                                            if crate::platform::ffmpeg_recheck() {
                                                app.show_toast("ffmpeg found ✓");
                                            } else {
                                                app.show_toast("Still not found — check PATH or set VIBECAP_FFMPEG");
                                            }
                                        }
                                    });
                                });
                            ui.add_space(theme::SP_3);
                        }

                        // ── Primary actions first: Screenshot / Record / GIF ──
                        // The user decides WHAT to do before WHERE — the target
                        // selector below is a secondary setting, not a step.
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
                        // E67 — pressing Record opens the Options header once
                        // so the audio/display knobs are visible before arming.
                        let mut open_options = false;
                        if let Some(act) = shutter_strip(
                            ui,
                            app.is_recording,
                            app.recording_arming || app.recording_finalizing,
                            &rec_label,
                        ) {
                            match act {
                                ShutterAction::Screenshot => app.trigger_capture(ctx, true),
                                // E52 — one-shot variant: borrow the target
                                // for this capture, then restore the From pick.
                                ShutterAction::ShotFull
                                | ShutterAction::ShotRegion
                                | ShutterAction::ShotWindow => {
                                    let t = match act {
                                        ShutterAction::ShotFull => CaptureTarget::Fullscreen,
                                        ShutterAction::ShotRegion => CaptureTarget::Region,
                                        _ => CaptureTarget::Window,
                                    };
                                    let prev = app.capture_target;
                                    app.capture_target = t;
                                    app.trigger_capture(ctx, true);
                                    app.capture_target = prev;
                                }
                                ShutterAction::RecordToggle => {
                                    if app.is_recording {
                                        app.stop_recording(ctx);
                                    } else if app.recording_arming {
                                        app.cancel_recording(ctx);
                                    } else {
                                        open_options = true;
                                        app.trigger_capture(ctx, false);
                                    }
                                }
                                ShutterAction::Gif => app.trigger_gif_clip(ctx),
                            }
                        }
                        // E57 — if a capture owns the park but the studio is
                        // visible anyway (tray open, CLI poke), say so instead
                        // of looking idle.
                        if app.screenshot_in_flight
                            || app
                                .wake_shared
                                .still_busy
                                .load(std::sync::atomic::Ordering::Relaxed)
                        {
                            ui.add_space(theme::SP_1);
                            ui.label(
                                RichText::new("⏳ Waiting for capture — grab in progress…")
                                    .size(11.0)
                                    .color(theme::WARN()),
                            );
                        }
                        if app.record_excluded {
                            ui.add_space(theme::SP_1);
                            ui.label(
                                RichText::new("● recording — this window is hidden from the capture")
                                    .size(11.0)
                                    .color(theme::DANGER()),
                            );
                        }
                        // Low-disk guard — warn before a long record fills the drive.
                        if let Some(free) =
                            crate::platform::disk_free_bytes_for(&app.save_dir)
                        {
                            const WARN_AT: u64 = 500 * 1024 * 1024;
                            if free < WARN_AT {
                                ui.add_space(theme::SP_1);
                                ui.label(
                                    RichText::new(format!(
                                        "⚠ only {} free on the capture drive",
                                        crate::app::library::format_size(free)
                                    ))
                                    .size(11.0)
                                    .color(theme::WARN()),
                                );
                            }
                        }
                        // E62 — battery-aware hint: suggest the lighter fps
                        // when the machine is unplugged.
                        if crate::platform::on_battery() == Some(true) && app.fps_target > 24 {
                            ui.add_space(theme::SP_1);
                            ui.label(
                                RichText::new(
                                    "🔋 On battery — 24 fps or shorter clips extend recording time",
                                )
                                .size(11.0)
                                .color(theme::WARN()),
                            );
                        }

                        ui.add_space(theme::SP_4);

                        // ── Capture source — quiet segmented row, clearly a
                        //    setting rather than an action ──
                        ui.horizontal(|ui| {
                            theme::caps_label(ui, "From");
                            ui.add_space(theme::SP_1);
                            egui::Frame::none()
                                .fill(theme::SURFACE_2())
                                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER()))
                                .rounding(egui::Rounding::same(8.0))
                                .inner_margin(egui::Margin::same(3.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        for (target, icon, label) in [
                                            (
                                                CaptureTarget::Fullscreen,
                                                crate::ui::icons::Icon::Monitor,
                                                "Full screen",
                                            ),
                                            (
                                                CaptureTarget::Region,
                                                crate::ui::icons::Icon::Region,
                                                "Region",
                                            ),
                                            (
                                                CaptureTarget::Window,
                                                crate::ui::icons::Icon::Window,
                                                "Window",
                                            ),
                                        ] {
                                            let on = app.capture_target == target;
                                            let resp = egui::Frame::none()
                                                .fill(if on {
                                                    theme::SURFACE()
                                                } else {
                                                    egui::Color32::TRANSPARENT
                                                })
                                                .rounding(theme::rounding_sm())
                                                .inner_margin(egui::Margin::symmetric(12.0, 6.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        let (r, _) = ui.allocate_exact_size(
                                                            egui::Vec2::splat(14.0),
                                                            egui::Sense::hover(),
                                                        );
                                                        crate::ui::icons::paint_icon(
                                                            ui,
                                                            r,
                                                            icon,
                                                            if on {
                                                                theme::ACCENT()
                                                            } else {
                                                                theme::TEXT_MUTED()
                                                            },
                                                        );
                                                        ui.add_space(2.0);
                                                        let mut rt = RichText::new(label)
                                                            .size(12.0)
                                                            .color(if on {
                                                                theme::TEXT()
                                                            } else {
                                                                theme::TEXT_MUTED()
                                                            });
                                                        if on {
                                                            rt = rt.strong();
                                                        }
                                                        ui.label(rt);
                                                    });
                                                })
                                                .response
                                                .interact(egui::Sense::click());
                                            if resp.clicked() {
                                                app.capture_target = target;
                                            }
                                        }
                                    });
                                });
                        });
                        ui.label(
                            RichText::new(match app.capture_target {
                                CaptureTarget::Fullscreen => "Everything on screen".to_string(),
                                CaptureTarget::Region => {
                                    "You'll drag a box on the screen".to_string()
                                }
                                // E69 — surface the persisted pick on the card.
                                CaptureTarget::Window if !app.window_app.is_empty() => {
                                    format!("Window: {}", app.window_app)
                                }
                                CaptureTarget::Window => {
                                    "Pick or choose an app window".to_string()
                                }
                            })
                            .size(10.0)
                            .color(theme::TEXT_DIM()),
                        );

                        // E22 — saved regions: persist named pixel rects and
                        // fire them without re-dragging (palette lists them too).
                        if app.capture_target == CaptureTarget::Region {
                            ui.horizontal(|ui| {
                                if !app.saved_regions.is_empty() {
                                    let regions = app.saved_regions.clone();
                                    let mut fire: Option<usize> = None;
                                    let mut del: Option<usize> = None;
                                    egui::ComboBox::from_id_source("saved_regions")
                                        .selected_text("Saved regions…")
                                        .width(180.0)
                                        .show_ui(ui, |ui| {
                                            for (i, (name, r)) in regions.iter().enumerate() {
                                                ui.horizontal(|ui| {
                                                    if ui
                                                        .selectable_label(
                                                            false,
                                                            format!(
                                                                "{name}  {}×{}",
                                                                r[0], r[1]
                                                            ),
                                                        )
                                                        .clicked()
                                                    {
                                                        fire = Some(i);
                                                        ui.close_menu();
                                                    }
                                                    if ui.small_button("✕").clicked() {
                                                        del = Some(i);
                                                    }
                                                });
                                            }
                                        });
                                    if let Some(i) = fire {
                                        let (_, r) = app.saved_regions[i].clone();
                                        app.capture_rect_still(ctx, (r[0], r[1], r[2], r[3]));
                                    }
                                    if let Some(i) = del {
                                        app.saved_regions.remove(i);
                                        app.persist_session();
                                    }
                                }
                                if app.selected_screen_rect.is_some()
                                    && ui
                                        .small_button("＋ Save region")
                                        .on_hover_text(
                                            "Remember this box — palette and this menu can re-fire it",
                                        )
                                        .clicked()
                                {
                                    let (w, h, x, y) =
                                        app.selected_screen_rect.unwrap();
                                    let n = app.saved_regions.len() + 1;
                                    app.saved_regions
                                        .push((format!("Region {n} · {w}×{h}"), [w, h, x, y]));
                                    app.persist_session();
                                    app.show_toast(format!("Saved region {n}"));
                                }
                            });
                            ui.add_space(theme::SP_1);
                        }

                        ui.add_space(theme::SP_2);
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
                            let before_pick = app.window_app.clone();
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
                            // E69 — persist the last window pick so the card
                            // can show it across restarts.
                            if app.window_app != before_pick {
                                app.persist_session();
                            }
                            ui.add(
                                egui::TextEdit::singleline(&mut app.window_app)
                                    .hint_text("Or type an app name (e.g. Chrome)")
                                    .desired_width(280.0),
                            );
                        }

                        ui.add_space(theme::SP_3);

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
                                } else if app.capture_delay_secs >= 60 {
                                    format!("{}m", app.capture_delay_secs / 60)
                                } else {
                                    format!("{}s", app.capture_delay_secs)
                                })
                                .width(72.0)
                                .show_ui(ui, |ui| {
                                    // E73 — the same delay doubles as a
                                    // scheduler: "in 10 min, grab this".
                                    for secs in [0u64, 3, 5, 10, 60, 300, 600] {
                                        ui.selectable_value(
                                            &mut app.capture_delay_secs,
                                            secs,
                                            if secs == 0 {
                                                "None".to_string()
                                            } else if secs >= 60 {
                                                format!("{}m", secs / 60)
                                            } else {
                                                format!("{secs}s")
                                            },
                                        );
                                    }
                                });
                            // E73 — scheduled still pending: live countdown
                            // + Cancel so a parked shot is never invisible.
                            if let Some(t) = app.scheduled_shot_at {
                                let left = t
                                    .saturating_duration_since(std::time::Instant::now())
                                    .as_secs();
                                ui.add_space(theme::SP_3);
                                ui.label(
                                    RichText::new(format!("⏰ fires in {}:{:02}", left / 60, left % 60))
                                        .size(11.0)
                                        .color(theme::WARN()),
                                );
                                if ui.small_button("Cancel").clicked() {
                                    app.scheduled_shot_at = None;
                                    app.show_toast("Scheduled capture cancelled");
                                }
                            }
                            ui.add_space(theme::SP_3);
                            let mut clip_only = app.clipboard_only;
                            if switch(ui, "Clipboard only — copy, don't save", &mut clip_only) {
                                app.clipboard_only = clip_only;
                            }
                            ui.add_space(theme::SP_3);
                            let mut silent = app.silent_mode;
                            if switch(ui, "Silent — no toasts/flash", &mut silent) {
                                app.silent_mode = silent;
                            }
                            // E58 — icon quick-toggles so cursor/mic/display
                            // don't hide inside the collapsed Options card.
                            ui.add_space(theme::SP_3);
                            if ui
                                // E9 — off-state ink comes from the
                                // disabled token pair, not egui's default.
                                .selectable_label(
                                    app.draw_mouse,
                                    RichText::new("🖱").color(if app.draw_mouse {
                                        theme::ACCENT()
                                    } else {
                                        theme::DISABLED_TEXT()
                                    }),
                                )
                                .on_hover_text("Draw cursor on stills")
                                .clicked()
                            {
                                app.draw_mouse = !app.draw_mouse;
                                app.persist_session();
                            }
                            if ui
                                .selectable_label(
                                    app.capture_audio,
                                    RichText::new("🎙").color(if app.capture_audio {
                                        theme::ACCENT()
                                    } else {
                                        theme::DISABLED_TEXT()
                                    }),
                                )
                                .on_hover_text("Include audio in recordings")
                                .clicked()
                            {
                                app.capture_audio = !app.capture_audio;
                                app.persist_session();
                            }
                            let monitors = crate::platform::list_monitors();
                            if monitors.len() > 1 {
                                let cur = app.capture_monitor.unwrap_or(0);
                                if ui
                                    .selectable_label(false, format!("🖥{}", cur + 1))
                                    .on_hover_text("Capture display — click to cycle")
                                    .clicked()
                                {
                                    app.capture_monitor =
                                        Some((cur + 1) % monitors.len() as u32);
                                    app.persist_session();
                                }
                            }
                        });

                        ui.add_space(theme::SP_4);

                        // ── Recent captures — "your stuff lands here" ──
                        {
                            let top: Vec<(std::path::PathBuf, bool)> = app
                                .library_items
                                .iter()
                                .take(8)
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
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("RECENT — CLICK REVIEWS · DRAG OUT")
                                            .size(10.0)
                                            .strong()
                                            .color(theme::TEXT_DIM()),
                                    );
                                    // E63 — 7-day capture-activity sparkline.
                                    let now_day = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .map(|d| d.as_secs() / 86_400)
                                        .unwrap_or(0);
                                    let mut days = [0u32; 7];
                                    for item in &app.library_items {
                                        let d = item.modified_secs / 86_400;
                                        if d <= now_day && now_day - d < 7 {
                                            days[(now_day - d) as usize] += 1;
                                        }
                                    }
                                    let peak = days.iter().copied().max().unwrap_or(1).max(1);
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::Vec2::new(7.0 * 8.0, 12.0),
                                        egui::Sense::hover(),
                                    );
                                    for (i, n) in days.iter().enumerate() {
                                        // i=0 is today — draw oldest→newest left→right.
                                        let x = rect.min.x + (6 - i) as f32 * 8.0;
                                        let h = ((*n as f32 / peak as f32) * 10.0).max(1.5);
                                        ui.painter().rect_filled(
                                            egui::Rect::from_min_size(
                                                egui::pos2(x, rect.max.y - h),
                                                egui::vec2(5.0, h),
                                            ),
                                            1.5,
                                            if i == 0 {
                                                theme::ACCENT()
                                            } else {
                                                theme::TEXT_DIM().gamma_multiply(0.6)
                                            },
                                        );
                                    }
                                    ui.label(
                                        RichText::new("7d")
                                            .size(9.0)
                                            .color(theme::TEXT_DIM()),
                                    );
                                });
                                ui.add_space(theme::SP_2);
                                // E56 — carousel: scroll instead of hiding
                                // captures past the third tile.
                                egui::ScrollArea::horizontal()
                                    .auto_shrink([false, true])
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let mut open: Option<(std::path::PathBuf, bool)> = None;
                                            let mut copy_path: Option<std::path::PathBuf> = None;
                                            let mut delete_path: Option<std::path::PathBuf> = None;
                                            let mut hovered_now: Option<std::path::PathBuf> = None;
                                            let mut scrub_want: Option<std::path::PathBuf> = None;
                                            // E53 — hover state from the
                                            // previous frame drives the grow
                                            // animation (size feeds the rect,
                                            // so same-frame hover would lag a
                                            // frame anyway).
                                            let hover_prev = app.recent_hover.clone();
                                            for (path, is_video, tex) in &app.recent_thumbs {
                                                let grow_id = ui
                                                    .make_persistent_id(("recent_grow", path));
                                                // E23 — reduced motion:
                                                // size snaps, no tween.
                                                let grow = if theme::reduce_motion() {
                                                    if hover_prev.as_ref() == Some(path) {
                                                        1.0
                                                    } else {
                                                        0.0
                                                    }
                                                } else {
                                                    ui.ctx().animate_bool(
                                                        grow_id,
                                                        hover_prev.as_ref() == Some(path),
                                                    )
                                                };
                                                let size = egui::Vec2::new(
                                                    120.0 + 60.0 * grow,
                                                    68.0 + 34.0 * grow,
                                                );
                                                let resp = egui::Frame::none()
                                                    .stroke(Stroke::new(1.0_f32, theme::BORDER()))
                                                    .rounding(theme::rounding_md())
                                                    .show(ui, |ui| {
                                                        let img = egui::Image::new(tex)
                                                            .fit_to_exact_size(size)
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
                                                    .interact(egui::Sense::click_and_drag())
                                                    .on_hover_text(format!(
                                                        "{} — click: Review · drag: out to Explorer/Slack",
                                                        path.file_name()
                                                            .map(|f| f.to_string_lossy().to_string())
                                                            .unwrap_or_default()
                                                    ));
                                                if resp.hovered() {
                                                    hovered_now = Some(path.clone());
                                                    // E53 — play-on-hover:
                                                    // reuse the Library scrub
                                                    // frames, advanced by time.
                                                    if *is_video {
                                                        if let Some(frames) =
                                                            app.scrub_cache.get(path)
                                                        {
                                                            if !frames.is_empty() {
                                                                let fi = (ui.ctx().input(|i| {
                                                                    i.time
                                                                }) * 6.0)
                                                                    as usize
                                                                    % frames.len();
                                                                ui.painter().image(
                                                                    frames[fi].id(),
                                                                    resp.rect,
                                                                    egui::Rect::from_min_max(
                                                                        egui::pos2(0.0, 0.0),
                                                                        egui::pos2(1.0, 1.0),
                                                                    ),
                                                                    egui::Color32::WHITE,
                                                                );
                                                                ui.ctx().request_repaint();
                                                            }
                                                        } else {
                                                            scrub_want = Some(path.clone());
                                                        }
                                                    }
                                                }
                                                // E54 — OS-level drag-out straight
                                                // from the tile (Windows shell drag).
                                                if resp.drag_started() {
                                                    let _ = crate::platform::start_file_drag(&[
                                                        path.clone(),
                                                    ]);
                                                }
                                                // E55 — hover quick-actions:
                                                // copy path / delete (undo-trash).
                                                // Pointer-in-rect (not resp.hovered):
                                                // the chips sit on top of the
                                                // tile, so hovered() flips false
                                                // when the pointer enters them —
                                                // containment keeps them steady.
                                                let pointer_in = ui
                                                    .ctx()
                                                    .pointer_latest_pos()
                                                    .map(|p| resp.rect.contains(p))
                                                    .unwrap_or(false);
                                                if pointer_in {
                                                    let chip = egui::Rect::from_min_size(
                                                        resp.rect.right_top()
                                                            + egui::vec2(-50.0, 3.0),
                                                        egui::vec2(47.0, 17.0),
                                                    );
                                                    ui.allocate_ui_at_rect(chip, |ui| {
                                                        ui.horizontal(|ui| {
                                                            if ui
                                                                .small_button("📋")
                                                                .on_hover_text("Copy path")
                                                                .clicked()
                                                            {
                                                                copy_path = Some(path.clone());
                                                            }
                                                            if ui
                                                                .small_button("🗑")
                                                                .on_hover_text("Delete (undo-able)")
                                                                .clicked()
                                                            {
                                                                delete_path = Some(path.clone());
                                                            }
                                                        });
                                                    });
                                                }
                                                if resp.clicked() {
                                                    open = Some((path.clone(), *is_video));
                                                }
                                                ui.add_space(theme::SP_2);
                                            }
                                            app.recent_hover = hovered_now;
                                            if let Some(p) = scrub_want {
                                                app.request_scrub(p, ctx);
                                            }
                                            if let Some(p) = copy_path {
                                                if let Ok(mut b) = arboard::Clipboard::new() {
                                                    let _ = b.set_text(p.display().to_string());
                                                    app.show_toast("📋 Path copied");
                                                }
                                            }
                                            if let Some(p) = delete_path {
                                                app.delete_library_paths(&[p]);
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
                                .open(open_options.then_some(true))
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
                                                        "No DirectShow audio device — set VIBECAP_AUDIO_DEVICE or recording will fail.",
                                                    )
                                                    .size(10.0)
                                                    .color(theme::WARN()),
                                                );
                                            }
                                            // E59 — pick the capture device
                                            // instead of trusting the mic-name
                                            // heuristic.
                                            if app.capture_audio && !app.audio_devices.is_empty() {
                                                let sel = if app.audio_device.is_empty() {
                                                    "Auto (first mic)".to_string()
                                                } else {
                                                    app.audio_device.clone()
                                                };
                                                egui::ComboBox::from_id_source("audio_dev")
                                                    .selected_text(sel)
                                                    .width(220.0)
                                                    .show_ui(ui, |ui| {
                                                        if ui
                                                            .selectable_label(
                                                                app.audio_device.is_empty(),
                                                                "Auto (first mic)",
                                                            )
                                                            .clicked()
                                                        {
                                                            app.audio_device.clear();
                                                            app.persist_session();
                                                        }
                                                        for d in app.audio_devices.clone() {
                                                            if ui
                                                                .selectable_label(
                                                                    app.audio_device == d,
                                                                    &d,
                                                                )
                                                                .clicked()
                                                            {
                                                                app.audio_device = d;
                                                                app.persist_session();
                                                            }
                                                        }
                                                    });
                                                // A picked device that vanished
                                                // from the list falls back to Auto.
                                                if !app.audio_device.is_empty()
                                                    && !app
                                                        .audio_devices
                                                        .contains(&app.audio_device)
                                                {
                                                    app.audio_device.clear();
                                                    app.persist_session();
                                                }
                                                // E49 — optional second source
                                                // mixed via amix (system loopback
                                                // needs virtual-audio-capturer).
                                                let mix_sel = if app.audio_mix_device.is_empty() {
                                                    "Mic only".to_string()
                                                } else {
                                                    app.audio_mix_device.clone()
                                                };
                                                egui::ComboBox::from_id_source("audio_mix_dev")
                                                    .selected_text(mix_sel)
                                                    .width(220.0)
                                                    .show_ui(ui, |ui| {
                                                        if ui
                                                            .selectable_label(
                                                                app.audio_mix_device.is_empty(),
                                                                "Mic only",
                                                            )
                                                            .clicked()
                                                        {
                                                            app.audio_mix_device.clear();
                                                            app.persist_session();
                                                        }
                                                        for d in app.audio_devices.clone() {
                                                            if d == app.audio_device {
                                                                continue;
                                                            }
                                                            if ui
                                                                .selectable_label(
                                                                    app.audio_mix_device == d,
                                                                    format!("+ {d}"),
                                                                )
                                                                .clicked()
                                                            {
                                                                app.audio_mix_device = d;
                                                                app.persist_session();
                                                            }
                                                        }
                                                    });
                                                if !app.audio_mix_device.is_empty()
                                                    && (!app
                                                        .audio_devices
                                                        .contains(&app.audio_mix_device)
                                                        || app.audio_mix_device
                                                            == app.audio_device)
                                                {
                                                    app.audio_mix_device.clear();
                                                    app.persist_session();
                                                }
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
                                    crate::ui::group(ui, "STORAGE", |ui| {
                                        // ≈4 MB/min @1080p30 h264, scaled by fps and
                                        // the selected monitor's pixel count.
                                        let mpx = monitors
                                            .get(app.capture_monitor.unwrap_or(0) as usize)
                                            .or_else(|| monitors.first())
                                            .map(|m| (m.w as f64 * m.h as f64) / 2_073_600.0)
                                            .unwrap_or(1.0);
                                        let mb_min =
                                            4.0 * (app.fps_target as f64 / 30.0) * mpx.max(0.2);
                                        ui.label(
                                            RichText::new(format!(
                                                "Recording ≈{mb_min:.0} MB/min @ {}fps",
                                                app.fps_target
                                            ))
                                            .size(11.0)
                                            .color(theme::TEXT_MUTED()),
                                        );
                                        if let Some(free) =
                                            crate::platform::disk_free_bytes_for(&app.save_dir)
                                        {
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} free on the capture drive",
                                                    crate::app::library::format_size(free)
                                                ))
                                                .size(11.0)
                                                .color(theme::TEXT_DIM()),
                                            );
                                        }
                                    });
                                    // E72 — a frame every N s retimed to the
                                    // fps target: 5 s ≈ 150× at 30 fps.
                                    crate::ui::group(ui, "TIME-LAPSE", |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new("Frame every")
                                                    .size(11.0)
                                                    .color(theme::TEXT_DIM()),
                                            );
                                            let sel = match app.timelapse_secs {
                                                0 => "Off".to_string(),
                                                s => format!("{s}s"),
                                            };
                                            let before = app.timelapse_secs;
                                            egui::ComboBox::from_id_source("timelapse_secs")
                                                .selected_text(sel)
                                                .width(80.0)
                                                .show_ui(ui, |ui| {
                                                    for s in [0u32, 2, 5, 15, 60, 300] {
                                                        ui.selectable_value(
                                                            &mut app.timelapse_secs,
                                                            s,
                                                            if s == 0 {
                                                                "Off".to_string()
                                                            } else {
                                                                format!("{s}s")
                                                            },
                                                        );
                                                    }
                                                });
                                            if app.timelapse_secs != before {
                                                app.persist_session();
                                            }
                                        });
                                        if app.timelapse_secs > 0 {
                                            let speed =
                                                app.timelapse_secs * app.fps_target.max(1);
                                            ui.label(
                                                RichText::new(format!(
                                                    "≈{speed}× playback · audio off · REC bar shows real time"
                                                ))
                                                .size(10.0)
                                                .color(theme::TEXT_DIM()),
                                            );
                                        }
                                    });
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
                            },
                        );
                    });
}
