//! Settings tab UI (extracted from main for Phase 1a).

use eframe::egui;
use egui::{Color32, RichText, Stroke, Vec2};
use rfd::FileDialog;
use std::path::PathBuf;

use crate::ui::theme;
use crate::ui::icons::Icon;
use crate::ui::{empty_state, shutter_strip, ShutterAction};
use crate::app::{get_dir_size_bytes, live_usage_snapshot, load_budget, save_budget, MediaCategory, LIBRARY_PAGE_SIZE};
use crate::platform::{ffmpeg_available, ffmpeg_path, open_path, reveal_in_file_manager};
use crate::{AppTab, CaptureTarget, VibecapApp};
use crate::app::{default_live_dir, BudgetConfig};

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {

                    ui.heading(RichText::new("Settings & Preferences").color(theme::ACCENT()).strong());
                    ui.add_space(15.0);
                    
                    ui.group(|ui| {
                        ui.label(RichText::new("SAVE LOCATION").small().color(theme::TEXT_DIM()));
                        ui.add_space(4.0);
                        ui.label(format!("{}", app.save_dir.display()));
                        ui.add_space(8.0);
                        if ui.button("📂 Change Save Directory").clicked() {
                            if let Some(path) = FileDialog::new().pick_folder() {
                                app.save_dir = path;
                                app.refresh_library();
                                app.show_toast("Directory updated!");
                            }
                        }
                    });
                    
                    ui.add_space(15.0);
                    ui.group(|ui| {
                        ui.label(RichText::new("RECORDING").small().color(theme::TEXT_DIM()));
                        ui.add_space(4.0);
                        ui.label(RichText::new("Framerate").small().color(theme::TEXT_MUTED()));
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut app.fps_target, 30, "30 FPS (Balanced)");
                            ui.radio_value(&mut app.fps_target, 60, "60 FPS (Pro High-FPS)");
                        });
                        ui.add_space(6.0);
                        ui.checkbox(&mut app.capture_audio, "Include audio when recording");
                        ui.label(
                            RichText::new("Video-only by default. Enable audio for mic/system sound (platform-dependent).")
                                .small()
                                .color(theme::TEXT_DIM()),
                        );
                        ui.add_space(theme::SP_2);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("ffmpeg")
                                    .small()
                                    .color(theme::TEXT_MUTED()),
                            );
                            if let Some(p) = ffmpeg_path() {
                                ui.label(
                                    RichText::new(p.display().to_string())
                                        .small()
                                        .color(theme::SUCCESS()),
                                );
                            } else if ffmpeg_available() {
                                ui.label(
                                    RichText::new("ok")
                                        .small()
                                        .color(theme::SUCCESS()),
                                );
                            } else {
                                ui.label(
                                    RichText::new("missing — brew install ffmpeg")
                                        .small()
                                        .color(theme::WARN()),
                                );
                            }
                        });
                        ui.label(
                            RichText::new(
                                "Finder launches ignore Homebrew PATH; Vibecap searches /usr/local/bin and /opt/homebrew/bin. Override: VIBECAP_FFMPEG.",
                            )
                            .small()
                            .color(theme::TEXT_DIM()),
                        );
                        ui.add_space(theme::SP_2);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Countdown")
                                    .small()
                                    .color(theme::TEXT_MUTED()),
                            );
                            for secs in [0u8, 3, 5] {
                                let label = if secs == 0 {
                                    "Off".to_string()
                                } else {
                                    format!("{secs}s")
                                };
                                if ui
                                    .selectable_label(app.record_countdown_secs == secs, label)
                                    .clicked()
                                {
                                    app.record_countdown_secs = secs;
                                    app.persist_session();
                                }
                            }
                        });
                        ui.label(
                            RichText::new("Big-number bubble before record starts (Esc cancels).")
                                .small()
                                .color(theme::TEXT_DIM()),
                        );
                        ui.add_space(theme::SP_2);
                        if ui
                            .button("🐛 Bug report pack")
                            .on_hover_text("Screenshot + retro GIF into Media")
                            .clicked()
                        {
                            app.bug_report_pack(ctx);
                        }
                        #[cfg(target_os = "macos")]
                        {
                            ui.add_space(theme::SP_2);
                            ui.label(
                                RichText::new("macOS PERMISSIONS")
                                    .small()
                                    .color(theme::TEXT_DIM()),
                            );
                            ui.label(
                                RichText::new(
                                    "If screenshots show only wallpaper / empty desktop, Screen Recording is not granted to Vibecap. Enable it, then quit and reopen the app.",
                                )
                                .small()
                                .color(theme::TEXT_MUTED()),
                            );
                            if ui
                                .button("Open Screen Recording settings…")
                                .clicked()
                            {
                                match crate::platform::open_screen_recording_settings() {
                                    Ok(()) => app.show_toast(
                                        "Enable Vibecap → quit app completely → reopen → try Screenshot",
                                    ),
                                    Err(e) => app.show_toast(format!("❌ {e}")),
                                }
                            }
                        }
                        ui.add_space(theme::SP_3);
                        ui.separator();
                        ui.add_space(theme::SP_2);
                        ui.label(
                            RichText::new("RETRO BUFFER")
                                .small()
                                .color(theme::TEXT_DIM()),
                        );
                        ui.label(
                            RichText::new(
                                "Rolling low-FPS capture so you can save the last N seconds after a bug. Off by default · ~2 fps · hard cap 200 MB.",
                            )
                            .small()
                            .color(theme::TEXT_MUTED()),
                        );
                        ui.add_space(4.0);
                        let mut cfg = app.retro.config();
                        let mut dirty = false;
                        if ui
                            .checkbox(&mut cfg.enabled, "Enable retro buffer")
                            .changed()
                        {
                            dirty = true;
                        }
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Window").small().color(theme::TEXT_MUTED()));
                            for secs in [15u32, 30, 60] {
                                if ui
                                    .selectable_label(cfg.seconds == secs, format!("{secs}s"))
                                    .clicked()
                                {
                                    cfg.seconds = secs;
                                    dirty = true;
                                }
                            }
                        });
                        if dirty {
                            app.retro.set_config(cfg);
                        }
                        let st = app.retro.status();
                        ui.label(
                            RichText::new(format!(
                                "Now: {} frames · {:.0}s / {}s · {:.1} / {:.0} MB{}",
                                st.frame_count,
                                st.span_secs,
                                st.max_secs,
                                st.mb,
                                st.max_mb,
                                if st.enabled { " · capturing" } else { " · idle" }
                            ))
                            .small()
                            .color(if st.enabled {
                                theme::ACCENT()
                            } else {
                                theme::TEXT_DIM()
                            }),
                        );
                        if let Some(err) = &st.last_error {
                            ui.label(
                                RichText::new(format!("⚠ {err}"))
                                    .small()
                                    .color(theme::WARN()),
                            );
                        }
                        if st.enabled && !st.running {
                            ui.label(
                                RichText::new(
                                    "Enabled but capturer not running — toggle off/on or restart the app.",
                                )
                                .small()
                                .color(theme::WARN()),
                            );
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .button("Save last as GIF")
                                .on_hover_text("Export ring buffer to Media library")
                                .clicked()
                            {
                                app.dump_retro_buffer();
                            }
                            if ui.button("Clear buffer").clicked() {
                                app.retro.clear_frames();
                                app.show_toast("Retro buffer cleared");
                            }
                        });
                    });
                    
                    ui.add_space(15.0);
                    ui.group(|ui| {
                        ui.label(RichText::new("SHORTCUTS & TRAY").small().color(theme::TEXT_DIM()));
                        ui.add_space(4.0);
                        ui.label("In app (window focused)");
                        ui.label("  S  — Screenshot");
                        ui.label("  R  — Start / stop recording");
                        ui.label("  Z  — Undo last library delete");
                        ui.label("  ⌘K / Ctrl+K  — Command palette");
                        ui.add_space(4.0);
                        ui.label("Global (works from tray / other apps)");
                        ui.label("  Ctrl + Shift + 3  — Screenshot");
                        ui.label("  Ctrl + Shift + 2  — Start / stop recording");
                        ui.add_space(4.0);
                        ui.label("Tray menu: status · Show/Hide · Screenshot/Record · Loop stages · Bug pack · Quit");
                        ui.label("Menu bar shows REC mm:ss while recording; Inbox badge when agents wait");
                        ui.label("Close window → hide to menu bar (not quit)");
                        ui.add_space(theme::SP_2);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Density").small().color(theme::TEXT_MUTED()));
                            if ui
                                .selectable_label(
                                    app.density == crate::ui::Density::Comfortable,
                                    "Comfortable",
                                )
                                .clicked()
                            {
                                app.density = crate::ui::Density::Comfortable;
                                app.persist_session();
                            }
                            if ui
                                .selectable_label(
                                    app.density == crate::ui::Density::Compact,
                                    "Compact",
                                )
                                .clicked()
                            {
                                app.density = crate::ui::Density::Compact;
                                app.persist_session();
                            }
                        });
                        ui.add_space(theme::SP_2);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Theme").small().color(theme::TEXT_MUTED()));
                            let mode = theme::theme_mode();
                            if ui
                                .selectable_label(mode == theme::ThemeMode::Dark, "Dark")
                                .clicked()
                            {
                                app.set_theme(ctx, theme::ThemeMode::Dark);
                            }
                            if ui
                                .selectable_label(mode == theme::ThemeMode::Light, "Light")
                                .clicked()
                            {
                                app.set_theme(ctx, theme::ThemeMode::Light);
                            }
                        });
                        ui.add_space(theme::SP_2);
                        if ui.button("Replay first-run wizard").clicked() {
                            app.wizard_open = true;
                            app.wizard_step = 0;
                            app.wizard_budget_touched = false;
                        }
                    });

                    ui.add_space(15.0);
                    ui.group(|ui| {
                        ui.label(RichText::new("🤖 AGENT SESSION & BUDGET").small().color(theme::TEXT_DIM()));
                        ui.add_space(4.0);
                        if !app.budget_loaded {
                            let cfg = load_budget();
                            app.budget_frames_input = cfg.max_frames.to_string();
                            app.budget_mb_input = format!("{:.1}", cfg.max_mb);
                            app.budget_minutes_input = cfg.max_minutes.to_string();
                            app.budget_tier = cfg.analysis_tier.clone();
                            app.budget_loaded = true;
                        }
                        ui.label(RichText::new("Intensive frame analysis can be expensive — these caps control agent spending. Agents adjust them via their budget tool; you can override here any time. 0 = no limit. When a cap is hit, the agent's stream stops and it's told the budget is spent.").small().color(theme::TEXT_MUTED()));
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label("Max frames:");
                            ui.add(egui::TextEdit::singleline(&mut app.budget_frames_input).desired_width(55.0));
                            ui.label("Max MB:");
                            ui.add(egui::TextEdit::singleline(&mut app.budget_mb_input).desired_width(60.0));
                            ui.label("Max minutes:");
                            ui.add(egui::TextEdit::singleline(&mut app.budget_minutes_input).desired_width(55.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Analysis tier:");
                            ui.selectable_value(&mut app.budget_tier, "eco".to_string(), "🌱 Eco — a still every few seconds");
                            ui.selectable_value(&mut app.budget_tier, "standard".to_string(), "⚖ Standard — short GIF ~3s");
                            ui.selectable_value(&mut app.budget_tier, "intensive".to_string(), "🔥 Intensive — 1s, expensive");
                        });
                        ui.horizontal(|ui| {
                            if ui.button("💾 Save Budget").clicked() {
                                let frames_p = app.budget_frames_input.trim().parse::<u32>();
                                let mb_p = app.budget_mb_input.trim().parse::<f64>();
                                let mins_p = app.budget_minutes_input.trim().parse::<u32>();
                                match (frames_p, mb_p, mins_p) {
                                    (Ok(f), Ok(mb), Ok(m)) if mb.is_finite() && mb >= 0.0 => {
                                        let cfg = BudgetConfig {
                                            max_frames: f,
                                            max_mb: mb,
                                            max_minutes: m,
                                            analysis_tier: app.budget_tier.clone(),
                                        };
                                        match save_budget(&cfg) {
                                            Ok(_) => app.show_toast("💾 Budget saved — agents follow these caps."),
                                            Err(e) => app.show_toast(&format!("❌ Could not save budget: {}", e)),
                                        }
                                    }
                                    _ => app.show_toast("❌ Budget values must be non-negative whole numbers (0 = no limit) — not saved."),
                                }
                            }
                            if ui.button("🔄 Reload").clicked() {
                                app.budget_loaded = false;
                            }
                        });
                        ui.add_space(6.0);
                        let live_dir = default_live_dir().display().to_string();
                        let (frames, mb, _) = live_usage_snapshot(&live_dir);
                        let cfg_now = load_budget();
                        let frames_cap = if cfg_now.max_frames == 0 { "∞".to_string() } else { cfg_now.max_frames.to_string() };
                        let mb_cap = if cfg_now.max_mb <= 0.0 { "∞".to_string() } else { format!("{:.0}", cfg_now.max_mb) };
                        ui.label(RichText::new(format!("Live session now: {}/{} frames · {:.1}/{} MB · tier {}", frames, frames_cap, mb, mb_cap, cfg_now.analysis_tier)).small().color(theme::TEXT_MUTED()));
                    });
                
}
