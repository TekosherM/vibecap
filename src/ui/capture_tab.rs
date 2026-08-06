//! Capture tab UI (extracted from main for Phase 1a).

use eframe::egui;
use egui::{Color32, RichText, Stroke, Vec2};
use rfd::FileDialog;
use std::path::PathBuf;

use crate::ui::theme;
use crate::ui::icons::Icon;
use crate::ui::{empty_state, shutter_strip, ShutterAction};
use crate::app::{get_dir_size_bytes, live_usage_snapshot, load_budget, save_budget, MediaCategory, LIBRARY_PAGE_SIZE};
use crate::platform::{open_path, reveal_in_file_manager};
use crate::{AppTab, CaptureTarget, VibecapApp};
use crate::app::default_live_dir;

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {

                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Screenshot · record · agent-ready media")
                                .color(theme::TEXT_MUTED())
                                .size(13.0),
                        );
                        ui.add_space(theme::SP_5);

                        // ── Shutter bar (persistent capture dock) ─
                        let rec_label = if app.is_recording {
                            let elapsed = app.recording_elapsed_secs();
                            format!("Stop  [{:02}:{:02}]", elapsed / 60, elapsed % 60)
                        } else if app.recording_arming {
                            "Starting…".to_string()
                        } else {
                            "Record  (R)".to_string()
                        };
                        if let Some(act) = shutter_strip(
                            ui,
                            app.is_recording,
                            app.recording_arming,
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
                            }
                        }

                        ui.add_space(theme::SP_4);

                        // ── Subtle options (below shutter) ───────
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Target")
                                    .small()
                                    .color(theme::TEXT_DIM()),
                            );
                            ui.add_space(theme::SP_2);
                            ui.radio_value(&mut app.capture_target, CaptureTarget::Fullscreen, "Full");
                            ui.radio_value(&mut app.capture_target, CaptureTarget::Region, "Region");
                            ui.radio_value(&mut app.capture_target, CaptureTarget::Window, "Window");
                        });
                        if app.capture_target == CaptureTarget::Window {
                            if !app.window_list_scanned {
                                app.refresh_window_list();
                            }
                            ui.add_space(theme::SP_2);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("App")
                                        .small()
                                        .color(theme::TEXT_MUTED()),
                                );
                                egui::ComboBox::from_id_source("window_app_picker")
                                    .selected_text(if app.window_app.is_empty() {
                                        "Select app…".to_string()
                                    } else {
                                        app.window_app.clone()
                                    })
                                    .width(220.0)
                                    .show_ui(ui, |ui| {
                                        for name in app.window_app_list.clone() {
                                            ui.selectable_value(
                                                &mut app.window_app,
                                                name.clone(),
                                                name,
                                            );
                                        }
                                    });
                                if ui.small_button("↻").on_hover_text("Refresh app list").clicked()
                                {
                                    app.refresh_window_list();
                                }
                            });
                            ui.label(
                                RichText::new(
                                    "Focuses the app, then captures / records. Type a name if missing from the list.",
                                )
                                .small()
                                .color(theme::TEXT_DIM()),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut app.window_app)
                                    .hint_text("Or type app name (e.g. Google Chrome)")
                                    .desired_width(280.0),
                            );
                        }
                        ui.add_space(theme::SP_2);
                        ui.horizontal(|ui| {
                            ui.add_space(ui.available_width() / 2.0 - 80.0);
                            ui.checkbox(
                                &mut app.capture_audio,
                                RichText::new("Include audio")
                                    .small()
                                    .color(theme::TEXT_MUTED()),
                            );
                        });
                        ui.label(
                            RichText::new("S / R in app  ·  Ctrl+Shift+3 / 2 global  ·  FPS in Settings")
                                .small()
                                .color(theme::TEXT_DIM()),
                        );
                        if app.capture_target == CaptureTarget::Fullscreen {
                            if let Some(prev) = &app.last_front_app {
                                ui.label(
                                    RichText::new(format!(
                                        "Fullscreen will restore “{}” before capture (not bare desktop).",
                                        prev
                                    ))
                                    .small()
                                    .color(theme::TEXT_MUTED()),
                                );
                            } else {
                                ui.label(
                                    RichText::new(
                                        "Tip: click another app first, then Vibecap — Fullscreen restores that app before the shot.",
                                    )
                                    .small()
                                    .color(theme::TEXT_DIM()),
                                );
                            }
                        }

                        // Retro buffer status (only when enabled — stays quiet when off)
                        let retro = app.retro.status();
                        if retro.enabled || retro.frame_count > 0 {
                            ui.add_space(theme::SP_3);
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
                                if ui
                                    .small_button("Save GIF")
                                    .on_hover_text("Export last N seconds from retro buffer")
                                    .clicked()
                                {
                                    app.dump_retro_buffer();
                                }
                                if ui
                                    .small_button("Bug pack")
                                    .on_hover_text("Screenshot + retro GIF")
                                    .clicked()
                                {
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

                        ui.add_space(16.0);
                        egui::CollapsingHeader::new(
                            RichText::new("🤖 Agent session (live inspection & budget)")
                                .color(theme::TEXT_MUTED()),
                        )
                        .default_open(false)
                        .show(ui, |ui| {
                            let live_dir = default_live_dir().display().to_string();
                            let (bytes, count) = get_dir_size_bytes(&live_dir);
                            let mb = (bytes as f64) / (1024.0 * 1024.0);
                            let cfg = load_budget();
                            ui.label(format!("Live frames: {} · {:.2} MB in {}", count, mb, live_dir));
                            ui.label(format!(
                                "Budget: frames cap {} · MB cap {:.1} · minutes cap {} · tier {}",
                                if cfg.max_frames == 0 {
                                    "unlimited".to_string()
                                } else {
                                    cfg.max_frames.to_string()
                                },
                                cfg.max_mb,
                                if cfg.max_minutes == 0 {
                                    "unlimited".to_string()
                                } else {
                                    cfg.max_minutes.to_string()
                                },
                                cfg.analysis_tier
                            ));
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
