//! Settings tab UI — token cards + modern controls (rail-matching language).

use eframe::egui;
use egui::{RichText, ScrollArea};
use rfd::FileDialog;

use crate::app::{load_budget, save_budget, BudgetConfig};
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
                if switch(ui, "Auto-play clip preview", &mut app.clip_autoplay) {
                    app.persist_session();
                }
                ui.label(
                    RichText::new("Review · Clip starts playing when the preview frames land.")
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
                if switch(ui, "Low-res clip preview", &mut app.filmstrip_low_res) {
                    app.persist_session();
                }
                ui.label(
                    RichText::new("Half-width filmstrip — extracts ~4× faster, softer preview.")
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
                if switch(ui, "Inbox quiet mode", &mut app.inbox_quiet) {
                    app.persist_session();
                }
                ui.label(
                    RichText::new("New agent questions still badge + tray — no notify, toast, or auto-open.")
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
                // K253: persistent last-error surface — toasts scroll away,
                // this stays until cleared.
                if let Some(err) = app.last_error.clone() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new("Last error").size(12.0).color(theme::TEXT_MUTED()),
                        );
                        ui.label(
                            RichText::new(&err).size(11.0).color(theme::WARN()),
                        );
                        if ui.small_button("Clear").clicked() {
                            app.last_error = None;
                        }
                    });
                }
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
                setting_row(ui, "Region dim", |ui| {
                    if ui
                        .add(egui::Slider::new(&mut app.region_dim, 0..=200))
                        .changed()
                    {
                        app.persist_session();
                    }
                });
                ui.label(
                    RichText::new("How dark the frozen desktop goes while picking a region.")
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
                ui.add_space(theme::SP_2);
                if btn_secondary(ui, "Bug report pack") {
                    app.bug_report_pack(ctx);
                }
            });

            // Power-user internals collapse — the simple path stays above.
            egui::CollapsingHeader::new(
                RichText::new("Advanced · capture internals & retro buffer")
                    .size(12.0)
                    .strong()
                    .color(theme::TEXT_MUTED()),
            )
            .default_open(false)
            .show(ui, |ui| {
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
                    ("Ctrl+B", "Toggle the stage rail"),
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
                    RichText::new("Global hotkey digits (Ctrl+Shift+N) — applies immediately")
                        .size(12.0)
                        .color(theme::TEXT_MUTED()),
                );
                ui.horizontal(|ui| {
                    ui.label("Screenshot");
                    ui.add(egui::Slider::new(&mut app.hotkey_shot_digit, 0..=9).prefix("#"));
                    ui.label("Record");
                    ui.add(egui::Slider::new(&mut app.hotkey_rec_digit, 0..=9).prefix("#"));
                });
                if btn_small(ui, "Apply hotkeys") {
                    match app.rebind_global_hotkeys() {
                        Ok(()) => {
                            app.persist_session();
                            app.show_toast("Hotkeys live — try them now");
                        }
                        Err(e) => app.show_toast(format!("⚠ {e}")),
                    }
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
                    ui.horizontal(|ui| {
                        for mode in theme::THEME_ORDER {
                            if theme_swatch(ui, mode) {
                                app.set_theme(ctx, mode);
                            }
                            ui.add_space(theme::SP_2);
                        }
                    });
                });
                if btn_secondary(ui, "Replay first-run wizard") {
                    app.wizard_open = true;
                    app.wizard_step = 0;
                    app.wizard_budget_touched = false;
                    app.wizard_autostart = crate::platform::run_at_login_enabled();
                }
            });

            // ── Agent session & budget ────────────────────────────
            egui::CollapsingHeader::new(
                RichText::new("For agents · session & budget")
                    .size(12.0)
                    .strong()
                    .color(theme::TEXT_MUTED()),
            )
            .default_open(false)
            .show(ui, |ui| {
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
                let live = app.live_stats_snapshot();
                let frames_cap = if live.frames_cap == 0 {
                    "∞".to_string()
                } else {
                    live.frames_cap.to_string()
                };
                let mb_cap = if live.mb_cap <= 0.0 {
                    "∞".to_string()
                } else {
                    format!("{:.0}", live.mb_cap)
                };
                ui.label(
                    RichText::new(format!(
                        "Live session now: {}/{} frames · {:.1}/{} MB · tier {}",
                        live.count, frames_cap, live.mb, mb_cap, live.tier
                    ))
                    .size(11.0)
                    .color(theme::TEXT_MUTED()),
                );
            });
            });

            // ── Quit — the X hides to tray; this is the real exit ──
            ui.add_space(theme::SP_3);
            ui.separator();
            ui.add_space(theme::SP_2);
            ui.horizontal(|ui| {
                if btn_danger(ui, "Quit Vibecap") {
                    app.quit_app();
                }
                ui.label(
                    RichText::new(
                        "The X button only hides to the tray — this fully exits.",
                    )
                    .size(11.0)
                    .color(theme::TEXT_DIM()),
                );
            });
        });
}

/// Theme picker swatch — canvas preview + ink label, ink ring when active.
/// Celestial shows the aurora gradient as its preview.
fn theme_swatch(ui: &mut egui::Ui, mode: theme::ThemeMode) -> bool {
    let active = theme::theme_mode() == mode;
    let (canvas, surface, ink) = theme::preview_colors(mode);
    let (rect, resp) = ui.allocate_exact_size(egui::Vec2::new(84.0, 58.0), egui::Sense::click());
    let p = ui.painter_at(rect);
    let ring = if active {
        theme::PRIMARY()
    } else if resp.hovered() {
        theme::BORDER_STRONG()
    } else {
        theme::BORDER()
    };
    p.rect_filled(rect, theme::rounding_sm(), theme::SURFACE());
    let pv = egui::Rect::from_min_max(
        rect.min + egui::Vec2::new(6.0, 6.0),
        egui::pos2(rect.max.x - 6.0, rect.min.y + 32.0),
    );
    match mode {
        theme::ThemeMode::Celestial | theme::ThemeMode::CelestialPink => {
            theme::paint_aurora_strip_for(&p, pv, mode);
        }
        _ => {
            p.rect_filled(pv, 3.0, canvas);
            let chip = egui::Rect::from_min_size(
                pv.min + egui::Vec2::new(4.0, 4.0),
                egui::Vec2::new(pv.width() * 0.55, 10.0),
            );
            p.rect_filled(chip, 2.0, surface);
            p.circle_filled(egui::pos2(pv.max.x - 6.0, pv.min.y + 6.0), 3.0, ink);
        }
    }
    p.rect_stroke(pv, 3.0, egui::Stroke::new(1.0_f32, ring));
    p.text(
        egui::pos2(rect.center().x, rect.max.y - 11.0),
        egui::Align2::CENTER_CENTER,
        theme::theme_mode_label(mode),
        egui::FontId::new(10.5, egui::FontFamily::Proportional),
        if active {
            theme::TEXT()
        } else {
            theme::TEXT_MUTED()
        },
    );
    let clicked = resp.clicked();
    resp.on_hover_text(format!("Switch to {}", theme::theme_mode_label(mode)));
    clicked
}
