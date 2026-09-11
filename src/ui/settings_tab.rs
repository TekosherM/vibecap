//! Settings tab UI — token cards + modern controls (rail-matching language).

use eframe::egui;
use egui::{RichText, ScrollArea};
use rfd::FileDialog;

use crate::app::{default_live_dir, live_usage_snapshot, load_budget, save_budget, BudgetConfig};
use crate::platform::{ffmpeg_available, ffmpeg_path};
use crate::ui::theme;
use crate::ui::{
    btn_danger, btn_primary, btn_secondary, btn_small, kbd, section_card, segmented, setting_row,
    switch,
};
use crate::VibecapApp;

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ScrollArea::vertical()
        .id_source("settings_scroll")
        .show(ui, |ui| {
            // ── Save location ─────────────────────────────────────
            section_card(ui, "SAVE LOCATION", |ui| {
                ui.label(
                    RichText::new(app.save_dir.display().to_string())
                        .size(12.0)
                        .color(theme::TEXT()),
                );
                ui.add_space(theme::SP_2);
                ui.horizontal_wrapped(|ui| {
                    if btn_secondary(ui, "Change directory") {
                        if let Some(path) = FileDialog::new().pick_folder() {
                            app.save_dir = path;
                            app.refresh_library();
                            app.show_toast("Directory updated");
                        }
                    }
                    let dir = app.save_dir.clone();
                    if btn_small(ui, "Reveal") {
                        let _ = crate::platform::reveal_in_file_manager(&dir);
                    }
                    if btn_small(ui, "Open") {
                        let _ = crate::platform::open_path(&dir);
                    }
                    if btn_small(ui, "Use for CLI/agents") {
                        std::env::set_var("VIBECAP_OUTPUT_DIR", dir.display().to_string());
                        app.show_toast("This folder is VIBECAP_OUTPUT_DIR for this process");
                    }
                });
                ui.add_space(theme::SP_2);
                ui.label(
                    RichText::new("Filename pattern ({app} {date} {time} {seq})")
                        .size(11.0)
                        .color(theme::TEXT_MUTED()),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut app.name_pattern)
                        .desired_width(280.0)
                        .hint_text("{app}-{date}-{seq}"),
                );
                let preview = crate::app::format_capture_stem(&app.name_pattern, Some("chrome"), 1);
                ui.label(
                    RichText::new(format!("Preview: {preview}.jpg"))
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
            });

            // ── Recording ────────────────────────────────────────
            section_card(ui, "RECORDING", |ui| {
                setting_row(ui, "Framerate", |ui| {
                    segmented(
                        ui,
                        &mut app.fps_target,
                        &[
                            (24, "24 FPS · light"),
                            (30, "30 FPS · balanced"),
                            (60, "60 FPS · pro"),
                        ],
                    );
                });
                switch(ui, "Include audio when recording", &mut app.capture_audio);
                ui.label(
                    RichText::new("Video-only by default; audio depends on the platform capture device.")
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
                ui.add_space(theme::SP_2);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("ffmpeg").size(12.0).color(theme::TEXT_MUTED()));
                    if let Some(p) = ffmpeg_path() {
                        ui.label(
                            RichText::new(p.display().to_string())
                                .size(11.0)
                                .color(theme::SUCCESS()),
                        );
                    } else if ffmpeg_available() {
                        ui.label(RichText::new("ok").size(11.0).color(theme::SUCCESS()));
                    } else {
                        ui.label(
                            RichText::new(if cfg!(target_os = "windows") {
                                "missing — winget install Gyan.FFmpeg"
                            } else {
                                "missing — brew install ffmpeg"
                            })
                            .size(11.0)
                            .color(theme::WARN()),
                        );
                    }
                });
                ui.label(
                    RichText::new(
                        "Finder launches ignore Homebrew PATH; Vibecap also searches /usr/local/bin and /opt/homebrew/bin. Override: VIBECAP_FFMPEG.",
                    )
                    .size(11.0)
                    .color(theme::TEXT_DIM()),
                );
                ui.add_space(theme::SP_2);
                setting_row(ui, "Countdown", |ui| {
                    if segmented(ui, &mut app.record_countdown_secs, &[(0u8, "Off"), (3, "3s"), (5, "5s")])
                    {
                        app.persist_session();
                    }
                });
                ui.label(
                    RichText::new("Big-number bubble before record starts (Esc cancels).")
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
                ui.add_space(theme::SP_2);
                if btn_secondary(ui, "Bug report pack") {
                    app.bug_report_pack(ctx);
                }
            });

            #[cfg(target_os = "windows")]
            section_card(ui, "WINDOWS CAPTURE", |ui| {
                ui.label(
                    RichText::new(
                        "Stills and recordings use ffmpeg gdigrab. GPU apps (Chrome) are brought to the front so the shot is not black. Pause is unavailable on Windows.",
                    )
                    .size(12.0)
                    .color(theme::TEXT_MUTED()),
                );
                ui.add_space(theme::SP_1);
                if btn_secondary(ui, "Test screenshot") {
                    app.trigger_capture(ctx, true);
                }
                ui.label(
                    RichText::new("winget install Gyan.FFmpeg  ·  tray icon is in the notification area")
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
            });

            // ── macOS permissions ─────────────────────────────────
            #[cfg(target_os = "macos")]
            section_card(ui, "MACOS PERMISSIONS", |ui| {
                ui.label(
                    RichText::new("Bare wallpaper / empty desktop = Screen Recording is off for this process.")
                        .size(12.0)
                        .color(theme::TEXT_MUTED()),
                );
                ui.add_space(theme::SP_1);
                ui.label(
                    RichText::new(
                        "Keep ONE “Vibecap” entry enabled; turn off or remove extras (old cargo / Terminal copies). Then tray Quit and reopen from Applications.",
                    )
                    .size(11.0)
                    .color(theme::TEXT_DIM()),
                );
                ui.add_space(theme::SP_2);
                if btn_secondary(ui, "Open Screen Recording settings…") {
                    match crate::platform::open_screen_recording_settings() {
                        Ok(()) => app.show_toast("Enable only Vibecap → tray Quit → reopen from Applications"),
                        Err(e) => app.show_toast(format!("❌ {e}")),
                    }
                }
            });

            // ── Retro buffer ──────────────────────────────────────
            section_card(ui, "RETRO BUFFER", |ui| {
                ui.label(
                    RichText::new("Rolling low-FPS capture so you can save the last N seconds after a bug. Off by default · ~2 fps · 200 MB cap.")
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
                ui.add_space(theme::SP_2);
                let mut cfg = app.retro.config();
                let mut dirty = false;
                if switch(ui, "Enable retro buffer", &mut cfg.enabled) {
                    dirty = true;
                }
                setting_row(ui, "Window", |ui| {
                    if segmented(ui, &mut cfg.seconds, &[(15u32, "15s"), (30, "30s"), (60, "60s")]) {
                        dirty = true;
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
                    .size(11.0)
                    .color(if st.enabled { theme::ACCENT() } else { theme::TEXT_DIM() }),
                );
                if let Some(err) = &st.last_error {
                    ui.label(RichText::new(format!("⚠ {err}")).size(11.0).color(theme::WARN()));
                }
                if st.enabled && !st.running {
                    ui.label(
                        RichText::new("Enabled but capturer not running — toggle off/on or restart the app.")
                            .size(11.0)
                            .color(theme::WARN()),
                    );
                }
                ui.add_space(theme::SP_2);
                ui.horizontal_wrapped(|ui| {
                    if btn_secondary(ui, "Save last as GIF") {
                        app.dump_retro_buffer();
                    }
                    if btn_danger(ui, "Clear buffer") {
                        app.retro.clear_frames();
                        app.show_toast("Retro buffer cleared");
                    }
                });
            });

            // ── Shortcuts & appearance ────────────────────────────
            section_card(ui, "SHORTCUTS & APPEARANCE", |ui| {
                ui.label(RichText::new("In app (window focused)").size(12.0).color(theme::TEXT_MUTED()));
                ui.add_space(theme::SP_1);
                for (key, what) in [
                    ("S", "Screenshot"),
                    ("R", "Start / stop recording"),
                    ("Z", "Undo last library delete"),
                    ("⌘K", "Command palette"),
                    ("Ctrl+1–5", "Jump to a stage"),
                    ("Alt+← / →", "Back / forward between stages"),
                ] {
                    ui.horizontal(|ui| {
                        kbd(ui, key);
                        ui.add_space(theme::SP_2);
                        ui.label(RichText::new(what).size(12.0).color(theme::TEXT_DIM()));
                    });
                    ui.add_space(2.0);
                }
                ui.add_space(theme::SP_2);
                ui.label(RichText::new("Global (tray / other apps)").size(12.0).color(theme::TEXT_MUTED()));
                ui.add_space(theme::SP_1);
                for (key, what) in [
                    ("Ctrl+Shift+3", "Screenshot"),
                    ("Ctrl+Shift+2", "Start / stop recording"),
                    ("Ctrl+Alt+V", "Summon / hide window"),
                ] {
                    ui.horizontal(|ui| {
                        kbd(ui, key);
                        ui.add_space(theme::SP_2);
                        ui.label(RichText::new(what).size(12.0).color(theme::TEXT_DIM()));
                    });
                    ui.add_space(2.0);
                }
                ui.label(
                    RichText::new(if cfg!(target_os = "windows") {
                        "Close window hides to the notification area / system tray; tray Quit exits."
                    } else {
                        "Close window hides to the menu bar; tray Quit exits."
                    })
                    .size(11.0)
                    .color(theme::TEXT_DIM()),
                );
                ui.add_space(theme::SP_2);
                ui.label(
                    RichText::new("Global hotkey digits (Ctrl+Shift+N) — applies next launch")
                        .size(12.0)
                        .color(theme::TEXT_MUTED()),
                );
                ui.horizontal(|ui| {
                    ui.label("Screenshot");
                    ui.add(egui::Slider::new(&mut app.hotkey_shot_digit, 0..=9).prefix("#"));
                    ui.label("Record");
                    ui.add(egui::Slider::new(&mut app.hotkey_rec_digit, 0..=9).prefix("#"));
                });
                if btn_small(ui, "Save hotkeys") {
                    app.persist_session();
                    app.show_toast("Hotkeys saved — restart the GUI to rebind");
                }
                ui.add_space(theme::SP_2);
                if cfg!(windows) {
                    if app.autostart_state.is_none() {
                        app.autostart_state = Some(crate::platform::run_at_login_enabled());
                    }
                    let mut on = app.autostart_state.unwrap_or(false);
                    if ui
                        .checkbox(&mut on, "Start Vibecap when I sign in (tray)")
                        .changed()
                    {
                        match crate::platform::set_run_at_login(on) {
                            Ok(()) => {
                                app.autostart_state = Some(on);
                                app.show_toast(if on {
                                    "Vibecap will start at sign-in"
                                } else {
                                    "Start at sign-in disabled"
                                });
                            }
                            Err(e) => app.show_toast(format!("Autostart failed: {e}")),
                        }
                    }
                }
                ui.add_space(theme::SP_2);
                ui.horizontal(|ui| {
                    if btn_secondary(ui, "Check for updates") {
                        match crate::app::update::check_latest_release() {
                            Ok(msg) => {
                                app.update_status = msg.clone();
                                app.show_toast(msg);
                            }
                            Err(e) => {
                                app.update_status = e.clone();
                                app.show_toast(e);
                            }
                        }
                    }
                    if !app.update_status.is_empty() {
                        ui.label(
                            RichText::new(&app.update_status)
                                .size(11.0)
                                .color(theme::TEXT_DIM()),
                        );
                    }
                });
                ui.add_space(theme::SP_3);
                setting_row(ui, "Density", |ui| {
                    if segmented(
                        ui,
                        &mut app.density,
                        &[(crate::ui::Density::Comfortable, "Comfortable"), (crate::ui::Density::Compact, "Compact")],
                    ) {
                        app.persist_session();
                    }
                });
                setting_row(ui, "Theme", |ui| {
                    let mut mode = theme::theme_mode();
                    if segmented(
                        ui,
                        &mut mode,
                        &[(theme::ThemeMode::Dark, "Dark"), (theme::ThemeMode::Light, "Light")],
                    ) {
                        app.set_theme(ctx, mode);
                    }
                });
                if btn_secondary(ui, "Replay first-run wizard") {
                    app.wizard_open = true;
                    app.wizard_step = 0;
                    app.wizard_budget_touched = false;
                    app.wizard_autostart = crate::platform::run_at_login_enabled();
                }
            });

            // ── Agent session & budget ────────────────────────────
            section_card(ui, "AGENT SESSION & BUDGET", |ui| {
                if !app.budget_loaded {
                    let cfg = load_budget();
                    app.budget_frames_input = cfg.max_frames.to_string();
                    app.budget_mb_input = format!("{:.1}", cfg.max_mb);
                    app.budget_minutes_input = cfg.max_minutes.to_string();
                    app.budget_tier = cfg.analysis_tier.clone();
                    app.budget_loaded = true;
                }
                ui.label(
                    RichText::new(
                        "Caps control agent spending on frame analysis. Agents adjust them via their budget tool; you can override here. 0 = no limit.",
                    )
                    .size(11.0)
                    .color(theme::TEXT_DIM()),
                );
                ui.add_space(theme::SP_2);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Max frames").size(12.0).color(theme::TEXT_MUTED()));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.budget_frames_input).desired_width(56.0),
                    );
                    ui.add_space(theme::SP_2);
                    ui.label(RichText::new("Max MB").size(12.0).color(theme::TEXT_MUTED()));
                    ui.add(egui::TextEdit::singleline(&mut app.budget_mb_input).desired_width(60.0));
                    ui.add_space(theme::SP_2);
                    ui.label(RichText::new("Max minutes").size(12.0).color(theme::TEXT_MUTED()));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.budget_minutes_input).desired_width(56.0),
                    );
                });
                ui.add_space(theme::SP_2);
                setting_row(ui, "Analysis tier", |ui| {
                    let mut tier: &str = match app.budget_tier.as_str() {
                        "eco" => "eco",
                        "intensive" => "intensive",
                        _ => "standard",
                    };
                    if segmented(
                        ui,
                        &mut tier,
                        &[("eco", "Eco"), ("standard", "Standard"), ("intensive", "Intensive")],
                    ) {
                        app.budget_tier = tier.to_string();
                    }
                });
                ui.add_space(theme::SP_2);
                ui.horizontal_wrapped(|ui| {
                    if btn_primary(ui, "Save budget") {
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
                                    Ok(_) => app.show_toast("Budget saved — agents follow these caps."),
                                    Err(e) => app.show_toast(format!("❌ Could not save budget: {}", e)),
                                }
                            }
                            _ => app.show_toast("❌ Budget values must be non-negative numbers (0 = no limit)."),
                        }
                    }
                    if btn_small(ui, "Reload") {
                        app.budget_loaded = false;
                    }
                });
                ui.add_space(theme::SP_2);
                let live_dir = default_live_dir().display().to_string();
                let (frames, mb, _) = live_usage_snapshot(&live_dir);
                let cfg_now = load_budget();
                let frames_cap = if cfg_now.max_frames == 0 {
                    "∞".to_string()
                } else {
                    cfg_now.max_frames.to_string()
                };
                let mb_cap = if cfg_now.max_mb <= 0.0 {
                    "∞".to_string()
                } else {
                    format!("{:.0}", cfg_now.max_mb)
                };
                ui.label(
                    RichText::new(format!(
                        "Live session now: {}/{} frames · {:.1}/{} MB · tier {}",
                        frames, frames_cap, mb, mb_cap, cfg_now.analysis_tier
                    ))
                    .size(11.0)
                    .color(theme::TEXT_MUTED()),
                );
            });
        });
}
