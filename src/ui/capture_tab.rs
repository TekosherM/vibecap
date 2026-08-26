//! Capture tab UI (extracted from main for Phase 1a).

use eframe::egui;
use egui::{RichText, Stroke};

use crate::ui::theme;
use crate::ui::{shutter_strip, ShutterAction};
use crate::app::{budget_exceeded_reason, get_dir_size_bytes, load_budget};
use crate::{CaptureTarget, VibecapApp};
use crate::app::default_live_dir;
use crate::ui::{btn_small, segmented, switch};

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

                        // ── Capture options (grouped card) ────────
                        egui::Frame::none()
                            .fill(theme::SURFACE())
                            .rounding(theme::rounding_md())
                            .stroke(Stroke::new(1.0_f32, theme::BORDER()))
                            .inner_margin(egui::Margin::same(theme::SP_3))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width().min(560.0));
                                ui.label(
                                    RichText::new("CAPTURE OPTIONS")
                                        .size(11.0)
                                        .strong()
                                        .color(theme::TEXT_MUTED()),
                                );
                                ui.add_space(theme::SP_2);
                                crate::ui::group(ui, "TARGET", |ui| {
                                    segmented(
                                        ui,
                                        &mut app.capture_target,
                                        &[
                                            (CaptureTarget::Fullscreen, "Full"),
                                            (CaptureTarget::Region, "Region"),
                                            (CaptureTarget::Window, "Window"),
                                        ],
                                    );
                                });
                                if app.capture_target == CaptureTarget::Window {
                                    if !app.window_list_scanned {
                                        app.refresh_window_list();
                                    }
                                    crate::ui::group(ui, "WINDOW", |ui| {
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
                                        if btn_small(ui, "↻") {
                                            app.refresh_window_list();
                                        }
                                        ui.add(
                                            egui::TextEdit::singleline(&mut app.window_app)
                                                .hint_text("Or type app name (e.g. Google Chrome)")
                                                .desired_width(220.0),
                                        );
                                    });
                                }
                                crate::ui::group(ui, "AUDIO", |ui| {
                                    switch(ui, "Include audio", &mut app.capture_audio);
                                });
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

                        // ── Compact live-stats row (always visible proof of life) ──
                        {
                            let live_dir = default_live_dir().display().to_string();
                            let (bytes, count) = get_dir_size_bytes(&live_dir);
                            let mb = bytes as f64 / (1024.0 * 1024.0);
                            let cfg = load_budget();
                            let over = budget_exceeded_reason(&live_dir);
                            ui.horizontal(|ui| {
                                let dot_color = if over.is_some() {
                                    theme::DANGER()
                                } else if count > 0 {
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
                                        count,
                                        mb,
                                        if cfg.max_frames == 0 { u64::MAX.to_string() } else { cfg.max_frames.to_string() },
                                        cfg.max_mb,
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

                        ui.add_space(16.0);
                        egui::CollapsingHeader::new(
                            RichText::new("Agent session · live inspection & budget")
                                .color(theme::TEXT_MUTED())
                                .size(12.0)
                                .strong(),
                        )
                        .default_open(true)
                        .show(ui, |ui| {
                            let live_dir = default_live_dir().display().to_string();
                            let (bytes, count) = get_dir_size_bytes(&live_dir);
                            let mb = bytes as f64 / (1024.0 * 1024.0);
                            let cfg = load_budget();
                            ui.label(
                                RichText::new(format!("Live frames: {} · {:.2} MB", count, mb))
                                    .size(12.0)
                                    .color(theme::TEXT_MUTED()),
                            );
                            ui.label(
                                RichText::new(format!(
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
