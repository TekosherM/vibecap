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
    // E284/E285 — filter box + section rail replace the flat scroll;
    // "All" keeps the old scroll-everything layout.
    let filter = app.settings_filter.trim().to_lowercase();
    let nav = app.settings_nav.clone();
    let want = |key: &str, hay: &str| -> bool {
        if !filter.is_empty() {
            hay.contains(filter.as_str())
        } else {
            nav == "all" || nav == key
        }
    };
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(128.0);
            ui.add(
                egui::TextEdit::singleline(&mut app.settings_filter)
                    .hint_text("Filter settings…")
                    .desired_width(120.0),
            );
            ui.add_space(theme::SP_2);
            for (key, label) in [
                ("all", "All"),
                ("save", "Save"),
                ("rec", "Recording"),
                ("lib", "Library"),
                ("adv", "Advanced"),
                ("keys", "Shortcuts & look"),
                ("agent", "Agent"),
                ("about", "About & help"),
            ] {
                if ui.selectable_label(app.settings_nav == key, label).clicked() {
                    app.settings_nav = key.to_string();
                }
            }
        });
        ui.separator();
        ScrollArea::vertical()
            .id_source("settings_scroll")
            .show(ui, |ui| {
            // ── Save location ─────────────────────────────────────
            if want("save", "save location directory folder filename pattern naming") {
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
                        // E34 — process-local for this session AND persisted
                        // to the user env so future agent shells inherit it.
                        std::env::set_var("VIBECAP_OUTPUT_DIR", dir.display().to_string());
                        match crate::platform::set_user_env(
                            "VIBECAP_OUTPUT_DIR",
                            Some(&dir.display().to_string()),
                        ) {
                            Ok(()) => app.show_toast(
                                "VIBECAP_OUTPUT_DIR saved — new shells/agents use this folder",
                            ),
                            Err(_) => app.show_toast(
                                "Set for this process only (persist not supported here)",
                            ),
                        }
                    }
                    #[cfg(target_os = "windows")]
                    if crate::platform::user_env("VIBECAP_OUTPUT_DIR").is_some()
                        && btn_small(ui, "Clear agent default")
                    {
                        let _ = crate::platform::set_user_env("VIBECAP_OUTPUT_DIR", None);
                        app.show_toast("Agent default cleared");
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
                // E296 — token chips compose the pattern: click inserts,
                // the preview below stays live.
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("insert:").size(11.0).color(theme::TEXT_DIM()),
                    );
                    for tok in ["{app}", "{date}", "{time}", "{seq}", "{orig}", "-", "_"] {
                        if ui
                            .button(
                                RichText::new(tok)
                                    .size(11.0)
                                    .monospace()
                                    .color(theme::ACCENT()),
                            )
                            .on_hover_text("Append to the pattern")
                            .clicked()
                        {
                            app.name_pattern.push_str(tok);
                            app.persist_session();
                        }
                    }
                });
                let preview = crate::app::format_capture_stem(&app.name_pattern, Some("chrome"), 1);
                ui.label(
                    RichText::new(format!("Preview: {preview}.jpg"))
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
                ui.add_space(theme::SP_1);
                // E287 — per-section reset back to shipped defaults.
                if ui
                    .small_button("↺ Reset section")
                    .on_hover_text("Restore this section's defaults")
                    .clicked()
                {
                    app.name_pattern = crate::app::naming::DEFAULT_PATTERN.to_string();
                    app.persist_session();
                    app.show_toast("Save-location settings reset");
                }
            });
            }

            // ── Recording ────────────────────────────────────────
            if want("rec", "recording framerate fps audio mic quality crf countdown region dim ffmpeg error bug report") {
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
                if switch(ui, "Open captures in Review", &mut app.auto_open_review) {
                    app.persist_session();
                }
                ui.label(
                    RichText::new("Off = captures land silently in Library + clipboard.")
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
                if switch(ui, "Auto-trim dead air", &mut app.auto_dead_air) {
                    app.persist_session();
                }
                ui.label(
                    RichText::new("Clip opens with the trim already set to the live span (no banner).")
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
                #[cfg(target_os = "windows")]
                if switch(ui, "Clipboard watcher", &mut app.clipboard_watcher) {
                    app.clipboard_seq_seen = crate::platform::clipboard_seq();
                    app.persist_session();
                }
                #[cfg(target_os = "windows")]
                ui.label(
                    RichText::new("A fresh image copied anywhere opens in Still — even while parked.")
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
                #[cfg(target_os = "windows")]
                {
                    ui.label(RichText::new("Clipboard copy format").size(12.0));
                    let label = match app.clipboard_encode.as_str() {
                        "jpeg" => "JPEG — smaller",
                        "off" => "Off — bitmap only",
                        _ => "PNG — sharp text",
                    };
                    let before = app.clipboard_encode.clone();
                    egui::ComboBox::from_id_source("clipboard_encode")
                        .selected_text(label)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut app.clipboard_encode,
                                "png".into(),
                                "PNG — sharp text",
                            );
                            ui.selectable_value(
                                &mut app.clipboard_encode,
                                "jpeg".into(),
                                "JPEG — smaller",
                            );
                            ui.selectable_value(
                                &mut app.clipboard_encode,
                                "off".into(),
                                "Off — bitmap only",
                            );
                        });
                    if app.clipboard_encode != before {
                        app.persist_session();
                    }
                    ui.label(
                        RichText::new(
                            "An encoded copy lands next to the bitmap — paste-ready in tools that skip DIB.",
                        )
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                    );
                }
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
                setting_row(ui, "Record quality", |ui| {
                    if segmented(
                        ui,
                        &mut app.record_crf,
                        &[(18u8, "Sharp"), (23, "Balanced"), (28, "Small file")],
                    ) {
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
                ui.add_space(theme::SP_1);
                if ui
                    .small_button("↺ Reset section")
                    .on_hover_text("Restore this section's defaults")
                    .clicked()
                {
                    app.fps_target = 30;
                    app.record_crf = 23;
                    app.capture_audio = false;
                    app.audio_device.clear();
                    app.clip_autoplay = true;
                    app.auto_open_review = true;
                    app.auto_dead_air = false;
                    app.filmstrip_low_res = false;
                    app.inbox_quiet = false;
                    app.clipboard_watcher = false;
                    app.clipboard_encode = "png".into();
                    app.record_countdown_secs = 0;
                    app.region_dim = 110;
                    app.persist_session();
                    app.show_toast("Recording settings reset");
                }
            });
            }

            // ── Library ──────────────────────────────────────────
            if want("lib", "library retention sweep cleanup keep newest older than") {
            section_card(ui, "LIBRARY", |ui| {
                setting_row(ui, "Retention", |ui| {
                    if segmented(
                        ui,
                        &mut app.retention_mode,
                        &[(0u8, "Off"), (1, "Older than"), (2, "Keep newest")],
                    ) {
                        app.persist_session();
                    }
                });
                if app.retention_mode != 0 {
                    ui.horizontal_wrapped(|ui| {
                        let lab = if app.retention_mode == 1 { "days" } else { "files" };
                        if ui
                            .add(
                                egui::DragValue::new(&mut app.retention_value)
                                    .speed(1)
                                    .range(1..=100000),
                            )
                            .changed()
                        {
                            app.persist_session();
                        }
                        ui.label(
                            RichText::new(lab)
                                .size(11.0)
                                .color(theme::TEXT_MUTED()),
                        );
                        if switch(ui, "Auto-sweep on launch", &mut app.retention_auto) {
                            app.persist_session();
                        }
                        if btn_secondary(ui, "Sweep now") {
                            app.apply_retention(false);
                        }
                    });
                    ui.label(
                        RichText::new(
                            "Swept files move to retention_trash — recoverable, never hard-deleted.",
                        )
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                    );
                }
                ui.add_space(theme::SP_1);
                if ui
                    .small_button("↺ Reset section")
                    .on_hover_text("Restore this section's defaults")
                    .clicked()
                {
                    app.retention_mode = 0;
                    app.retention_value = 30;
                    app.retention_auto = false;
                    app.persist_session();
                    app.show_toast("Library settings reset");
                }
            });
            }

            // Power-user internals collapse — the simple path stays above.
            if want("adv", "advanced windows status permissions tray retro buffer test screenshot capture internals") {
            egui::CollapsingHeader::new(
                RichText::new("Advanced · capture internals & retro buffer")
                    .size(12.0)
                    .strong()
                    .color(theme::TEXT_MUTED()),
            )
            .default_open(false)
            .open(if nav == "adv" || !filter.is_empty() { Some(true) } else { None })
            .show(ui, |ui| {
            #[cfg(target_os = "windows")]
            section_card(ui, "WINDOWS STATUS", |ui| {
                // E220 — one green/red line per dependency so "empty Settings
                // on Windows" reads as a health card, not a stub.
                let ffmpeg_ok = crate::platform::ffmpeg_path().is_some();
                let mic = if app.audio_device.is_empty() {
                    match app.audio_devices.first() {
                        Some(d) => format!("auto → {d}"),
                        None => "auto (first dshow device)".to_string(),
                    }
                } else {
                    app.audio_device.clone()
                };
                for (label, ok) in [
                    ("ffmpeg gdigrab (stills + recording)", ffmpeg_ok),
                    ("audio input device", !app.audio_devices.is_empty() || !app.audio_device.is_empty()),
                    ("tray icon", app.tray.is_some()),
                ] {
                    ui.label(
                        RichText::new(format!("{} {label}", if ok { "✓" } else { "✗" }))
                            .size(12.0)
                            .color(if ok { theme::SUCCESS() } else { theme::DANGER() }),
                    );
                }
                ui.label(
                    RichText::new(format!("mic: {mic}"))
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
                ui.add_space(theme::SP_1);
                ui.label(
                    RichText::new(
                        "GPU apps (Chrome) are brought to the front so the shot is not black. Pause suspends the ffmpeg child.",
                    )
                    .size(11.0)
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
                    if ui
                        .small_button("↺ Reset section")
                        .on_hover_text("Restore retro defaults (off · 60 s)")
                        .clicked()
                    {
                        app.retro.set_config(Default::default());
                        app.show_toast("Retro buffer reset");
                    }
                });
            });
            });
            }

            // ── Shortcuts & appearance ────────────────────────────
            if want("keys", "shortcuts appearance hotkey keys theme density tray autostart portable update wizard profile prtscn capture sounds explorer deep link watch folder") {
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
                // E50/E204/E86 — opt-in extras: pause/region/window digits + bare PrtScn.
                ui.horizontal(|ui| {
                    let mut pause_on = app.hotkey_pause_digit.is_some();
                    if ui.checkbox(&mut pause_on, "Pause hotkey").changed() {
                        app.hotkey_pause_digit = pause_on.then_some(4);
                    }
                    if let Some(d) = app.hotkey_pause_digit.as_mut() {
                        ui.add(egui::Slider::new(d, 0..=9).prefix("#"));
                    }
                });
                ui.horizontal(|ui| {
                    let mut region_on = app.hotkey_region_digit.is_some();
                    if ui
                        .checkbox(&mut region_on, "Region pick hotkey")
                        .on_hover_text("Ctrl+Shift+N jumps straight into the region picker")
                        .changed()
                    {
                        app.hotkey_region_digit = region_on.then_some(5);
                    }
                    if let Some(d) = app.hotkey_region_digit.as_mut() {
                        ui.add(egui::Slider::new(d, 0..=9).prefix("#"));
                    }
                    let mut window_on = app.hotkey_window_digit.is_some();
                    if ui
                        .checkbox(&mut window_on, "Window still hotkey")
                        .on_hover_text("Ctrl+Shift+N captures the remembered window")
                        .changed()
                    {
                        app.hotkey_window_digit = window_on.then_some(6);
                    }
                    if let Some(d) = app.hotkey_window_digit.as_mut() {
                        ui.add(egui::Slider::new(d, 0..=9).prefix("#"));
                    }
                });
                ui.horizontal(|ui| {
                    let mut gif_on = app.hotkey_gif_digit.is_some();
                    if ui
                        .checkbox(&mut gif_on, "GIF clip hotkey")
                        .on_hover_text("Ctrl+Shift+N records a 3 s clip and exports a GIF")
                        .changed()
                    {
                        app.hotkey_gif_digit = gif_on.then_some(7);
                    }
                    if let Some(d) = app.hotkey_gif_digit.as_mut() {
                        ui.add(egui::Slider::new(d, 0..=9).prefix("#"));
                    }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut app.hotkey_prtscn, "PrtScn still")
                        .on_hover_text("Bare PrtScn takes a screenshot while Vibecap runs");
                    if ui
                        .checkbox(
                            &mut app.shutter_sound,
                            "Capture sounds (shutter + record start/stop)",
                        )
                        .on_hover_text("Subtle click when a still lands")
                        .changed()
                    {
                        app.persist_session();
                    }
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
                    if app.explorer_verb_state.is_none() {
                        app.explorer_verb_state = Some(crate::platform::explorer_verb_enabled());
                    }
                    let mut verb = app.explorer_verb_state.unwrap_or(false);
                    if ui
                        .checkbox(&mut verb, "\"Annotate with Vibecap\" on image right-click")
                        .on_hover_text(
                            "Adds an Explorer context-menu verb for image files that \
                             opens the file straight into Review annotations (HKCU, no admin)",
                        )
                        .changed()
                    {
                        match crate::platform::set_explorer_verb(verb) {
                            Ok(()) => {
                                app.explorer_verb_state = Some(verb);
                                app.show_toast(if verb {
                                    "Right-click an image → Annotate with Vibecap"
                                } else {
                                    "Explorer verb removed"
                                });
                            }
                            Err(e) => app.show_toast(format!("Explorer verb failed: {e}")),
                        }
                    }
                    if app.url_scheme_state.is_none() {
                        app.url_scheme_state = Some(crate::platform::url_scheme_enabled());
                    }
                    let mut link = app.url_scheme_state.unwrap_or(false);
                    if ui
                        .checkbox(&mut link, "vibecap:// deep links")
                        .on_hover_text(
                            "Registers the vibecap:// URL scheme (HKCU, no admin) so \
                             vibecap://feedback/<id> opens that thread — usable from \
                             browsers, terminals, and agents",
                        )
                        .changed()
                    {
                        match crate::platform::set_url_scheme(link) {
                            Ok(()) => {
                                app.url_scheme_state = Some(link);
                                app.show_toast(if link {
                                    "vibecap:// links now open Vibecap"
                                } else {
                                    "URL scheme removed"
                                });
                            }
                            Err(e) => app.show_toast(format!("URL scheme failed: {e}")),
                        }
                    }
                    setting_row(ui, "Tray double-click", |ui| {
                        let mut sel: &'static str = match app.tray_dblclick.as_str() {
                            "screenshot" => "screenshot",
                            "record" => "record",
                            _ => "open",
                        };
                        if segmented(
                            ui,
                            &mut sel,
                            &[
                                ("open", "Open"),
                                ("screenshot", "Screenshot"),
                                ("record", "Record"),
                            ],
                        ) {
                            app.tray_dblclick = sel.to_string();
                            app.persist_session();
                        }
                    });
                    setting_row(ui, "Watch folder", |ui| {
                        ui.horizontal(|ui| {
                            let set = !app.watch_folder.trim().is_empty();
                            if btn_small(ui, "Choose…") {
                                if let Some(d) = rfd::FileDialog::new().pick_folder() {
                                    app.watch_folder = d.display().to_string();
                                    app.watch_last_scan = None;
                                    app.persist_session();
                                    app.show_toast("Watching — files dropped here move into Library");
                                }
                            }
                            if set && btn_small(ui, "Off") {
                                app.watch_folder.clear();
                                app.persist_session();
                            }
                            ui.label(
                                RichText::new(if set {
                                    app.watch_folder.as_str()
                                } else {
                                    "off"
                                })
                                .size(10.5)
                                .color(theme::TEXT_DIM()),
                            );
                        });
                    });
                }
                // E212 — portable mode: marker file beside the exe flips
                // config + media under `<exe>/portable/` on next launch.
                {
                    let active = crate::platform::is_portable();
                    let mut want = active;
                    if ui
                        .checkbox(&mut want, "Portable mode (restart to apply)")
                        .on_hover_text(
                            "Keeps settings + media in a `portable/` folder beside the \
                             executable instead of the OS profile. Writes/removes a \
                             `vibecap.portable` marker; takes effect on next launch. \
                             Existing files are not migrated.",
                        )
                        .changed()
                    {
                        match crate::platform::set_portable_marker(want) {
                            Ok(()) => app.show_toast(if want {
                                "Portable mode armed — restart Vibecap".to_string()
                            } else {
                                "Portable mode off — restart Vibecap".to_string()
                            }),
                            Err(e) => app.show_toast(format!("Portable toggle failed: {e}")),
                        }
                    }
                    if active {
                        ui.label(
                            RichText::new("running portable").size(10.5).color(theme::SUCCESS()),
                        );
                    }
                }
                ui.add_space(theme::SP_2);
                ui.horizontal(|ui| {
                    let checking = app.update_rx.is_some();
                    if btn_secondary(
                        ui,
                        if checking {
                            "Checking…"
                        } else {
                            "Check for updates"
                        },
                    ) && !checking
                    {
                        app.start_update_check();
                    }
                    if !app.update_status.is_empty() {
                        ui.label(
                            RichText::new(&app.update_status)
                                .size(11.0)
                                .color(theme::TEXT_DIM()),
                        );
                    }
                });
                setting_row(ui, "Check on launch", |ui| {
                    if ui
                        .checkbox(&mut app.update_check_on_launch, "")
                        .on_hover_text("Ask GitHub Releases once at startup; off = fully offline")
                        .changed()
                    {
                        app.persist_session();
                    }
                });
                // E214 — newer release: notes preview + download link.
                if let Some(info) = app.update_info.clone() {
                    if info.newer {
                        ui.horizontal(|ui| {
                            let downloading = app.update_dl_rx.is_some();
                            if let Some(url) = info.asset_url.clone() {
                                if btn_secondary(
                                    ui,
                                    if downloading {
                                        "Downloading…"
                                    } else {
                                        "Download update"
                                    },
                                ) && !downloading
                                {
                                    app.start_update_download(url);
                                }
                            }
                            if btn_secondary(ui, "Release page ↗") {
                                let _ = open::that(&info.url);
                            }
                            ui.label(
                                RichText::new(&info.tag).size(11.0).color(theme::ACCENT()),
                            );
                        });
                        if !info.notes.is_empty() {
                            ui.label(
                                RichText::new(&info.notes)
                                    .size(10.5)
                                    .color(theme::TEXT_DIM()),
                            );
                        }
                    }
                }
                // E215 — a staged binary waits beside the exe; applying it
                // swaps + relaunches after a short delay.
                if app.update_staged.is_some() {
                    if btn_secondary(ui, "Restart to apply update") {
                        // E292 — carry the notes over the restart so the
                        // What's New card can show them once.
                        if let Some(info) = &app.update_info {
                            app.whats_new_tag = info.tag.clone();
                            app.whats_new_notes = info.notes.clone();
                        }
                        app.persist_session();
                        match crate::app::update::apply_staged_and_restart() {
                            Ok(()) => app.quit_app(),
                            Err(e) => app.show_toast(format!("Update failed: {e}")),
                        }
                    }
                }
                // E264 — the previous build stays parked as <exe>.old; one
                // click swaps back and relaunches it.
                if crate::app::update::rollback_available() {
                    if btn_secondary(ui, "Roll back to previous version") {
                        match crate::app::update::rollback_and_restart() {
                            Ok(()) => app.quit_app(),
                            Err(e) => app.show_toast(format!("Rollback failed: {e}")),
                        }
                    }
                }
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
                // E49 — resume-last-stage vs always-open-on-Capture.
                if switch(ui, "Reopen where I left off", &mut app.restore_tab) {
                    app.persist_session();
                }
                ui.label(
                    RichText::new("Off = every launch starts on Capture.")
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
                // E26 — icon-only rail (the rail's « toggle writes the same flag).
                if switch(ui, "Icon-only rail", &mut app.rail_collapsed) {
                    app.persist_session();
                }
                // E23 — no pulses/tweens for motion-sensitive users.
                if switch(ui, "Reduce motion", &mut app.reduce_motion) {
                    app.persist_session();
                }
                ui.label(
                    RichText::new("Reduce motion flattens the REC pulse, hover-grow, and HUD flashes.")
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
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
                // E3 — celestial accent hue: rotate the aurora stops ±40°
                // while the sky structure stays put. Only meaningful on
                // the two cosmic themes.
                if theme::is_celestial() {
                    setting_row(ui, "Accent hue", |ui| {
                        if ui
                            .add(
                                egui::Slider::new(&mut app.aurora_hue, -40.0..=40.0)
                                    .suffix("°")
                                    .fixed_decimals(0),
                            )
                            .on_hover_text("Rotate the celestial aurora accent")
                            .changed()
                        {
                            app.persist_session();
                        }
                        if app.aurora_hue != 0.0
                            && ui
                                .button(RichText::new("reset").size(11.0).color(theme::TEXT_DIM()))
                                .clicked()
                        {
                            app.aurora_hue = 0.0;
                            app.persist_session();
                        }
                    });
                }
                setting_row(ui, "Follow OS", |ui| {
                    let mut on = app.theme_follow_os;
                    if ui
                        .checkbox(&mut on, "match Windows light/dark")
                        .on_hover_text("Poll the Windows app theme every few seconds and switch themes with it")
                        .changed()
                    {
                        app.theme_follow_os = on;
                        app.os_dark_seen = None; // force converge on next tick
                        app.persist_session();
                    }
                    if app.theme_follow_os || app.theme_schedule {
                        ui.label(
                            RichText::new("dark pick:").size(11.0).color(theme::TEXT_DIM()),
                        );
                        for mode in [
                            theme::ThemeMode::Dark,
                            theme::ThemeMode::Carbon,
                            theme::ThemeMode::Celestial,
                            theme::ThemeMode::CelestialPink,
                        ] {
                            let name = theme::theme_mode_to_str(mode);
                            let picked = app.theme_dark_pick == name;
                            if ui
                                .selectable_label(picked, theme::theme_mode_label(mode))
                                .clicked()
                            {
                                app.theme_dark_pick = name.to_string();
                                app.persist_session();
                            }
                        }
                    }
                });
                // E6 — scheduled themes: Light 07:00–19:00, dark pick at
                // night. Shares the follow-OS tick; wins when both are on.
                setting_row(ui, "Schedule", |ui| {
                    let mut on = app.theme_schedule;
                    if ui
                        .checkbox(&mut on, "Light by day, dark pick at night")
                        .on_hover_text("Switch themes on a clock: Light 7am–7pm, your dark pick otherwise")
                        .changed()
                    {
                        app.theme_schedule = on;
                        app.os_dark_seen = None; // force converge on next tick
                        app.persist_session();
                    }
                });
                ui.add_space(theme::SP_2);
                ui.horizontal(|ui| {
                    if btn_secondary(ui, "Export profile…") {
                        app.export_profile_dialog();
                    }
                    if btn_secondary(ui, "Import profile…") {
                        app.import_profile_dialog(ctx);
                    }
                    ui.label(
                        RichText::new("settings + hotkeys as a .vcap-profile zip")
                            .size(10.5)
                            .color(theme::TEXT_DIM()),
                    );
                });
                ui.add_space(theme::SP_2);
                if btn_secondary(ui, "Replay first-run wizard") {
                    app.wizard_open = true;
                    app.wizard_step = 0;
                    app.wizard_budget_touched = false;
                    app.wizard_autostart = crate::platform::run_at_login_enabled();
                    app.wizard_test_rx = None;
                    app.wizard_test_done = None;
                }
                ui.add_space(theme::SP_1);
                if ui
                    .small_button("↺ Reset section")
                    .on_hover_text("Restore hotkeys, theme and toggles to defaults")
                    .clicked()
                {
                    app.hotkey_shot_digit = 3;
                    app.hotkey_rec_digit = 2;
                    app.hotkey_pause_digit = None;
                    app.hotkey_region_digit = None;
                    app.hotkey_window_digit = None;
                    app.hotkey_gif_digit = None;
                    app.hotkey_prtscn = false;
                    app.shutter_sound = false;
                    app.tray_dblclick = "open".into();
                    app.watch_folder.clear();
                    app.theme_follow_os = false;
                    app.theme_schedule = false;
                    app.theme_dark_pick = "dark".into();
                    app.density = crate::ui::Density::Comfortable;
                    app.set_theme(ctx, theme::ThemeMode::Dark);
                    let _ = app.rebind_global_hotkeys();
                    app.persist_session();
                    app.show_toast("Shortcuts & appearance reset");
                }
            });
            }

            // ── Agent session & budget ────────────────────────────
            if want("agent", "agent session budget mcp frames minutes tier spend caps") {
            egui::CollapsingHeader::new(
                RichText::new("For agents · session & budget")
                    .size(12.0)
                    .strong()
                    .color(theme::TEXT_MUTED()),
            )
            .default_open(false)
            .open(if nav == "agent" || !filter.is_empty() { Some(true) } else { None })
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
                ui.add_space(theme::SP_1);
                if ui
                    .small_button("↺ Reset section")
                    .on_hover_text("Reset caps to unlimited / standard tier")
                    .clicked()
                {
                    app.budget_frames_input = "0".into();
                    app.budget_mb_input = "0.0".into();
                    app.budget_minutes_input = "0".into();
                    app.budget_tier = "standard".into();
                    let _ = save_budget(&BudgetConfig::default());
                    app.show_toast("Budget caps reset (0 = unlimited)");
                }
            });
            });
            }

            // ── About & help ──────────────────────────────────────
            if want("about", "about help docs documentation version changelog what's new quit exit stats telemetry privacy") {
            section_card(ui, "ABOUT & HELP", |ui| {
                ui.label(
                    RichText::new(format!("Vibecap v{}", env!("CARGO_PKG_VERSION")))
                        .size(12.0)
                        .color(theme::TEXT()),
                );
                // E292 — notes of the just-applied update, shown once.
                if !app.whats_new_tag.is_empty() {
                    ui.add_space(theme::SP_1);
                    ui.label(
                        RichText::new(format!("Updated to {}", app.whats_new_tag))
                            .size(11.5)
                            .color(theme::ACCENT())
                            .strong(),
                    );
                    if !app.whats_new_notes.is_empty() {
                        ui.label(
                            RichText::new(&app.whats_new_notes)
                                .size(10.5)
                                .color(theme::TEXT_DIM()),
                        );
                    }
                    if btn_small(ui, "Got it") {
                        app.whats_new_tag.clear();
                        app.whats_new_notes.clear();
                        app.persist_session();
                    }
                }
                ui.add_space(theme::SP_2);
                // E275 — opt-in local counters; off by default, nothing
                // ever leaves the machine (no network path exists for it).
                if switch(ui, "Local stats — count captures", &mut app.stats_opt_in) {
                    app.persist_session();
                }
                if app.stats_opt_in {
                    ui.label(
                        RichText::new(format!(
                            "stills: {} ok · {} failed    recordings: {} ok · {} failed",
                            app.stat_shots_ok,
                            app.stat_shots_fail,
                            app.stat_recs_ok,
                            app.stat_recs_fail,
                        ))
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                    );
                }
                ui.add_space(theme::SP_2);
                // E293 — in-app doc links per surface.
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Docs").size(12.0).color(theme::TEXT_MUTED()));
                    for (label, url) in [
                        ("Capture recipes ↗", "https://github.com/TekosherM/vibecap/blob/master/docs/AGENTS.md"),
                        ("MCP tools ↗", "https://github.com/TekosherM/vibecap/blob/master/docs/MCP.md"),
                        ("Roadmap ↗", "https://github.com/TekosherM/vibecap/blob/master/docs/IMPROVEMENTS.md"),
                        ("Releases ↗", "https://github.com/TekosherM/vibecap/releases"),
                    ] {
                        if btn_small(ui, label) {
                            let _ = open::that(url);
                        }
                    }
                });
            });
            }

            // ── Quit — the X hides to tray; this is the real exit ──
            if filter.is_empty() || "quit exit".contains(filter.as_str()) {
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
            }
        });
    });
}

/// Theme picker swatch — canvas preview + ink label, ink ring when active.
/// Celestial shows the aurora gradient as its preview.
pub(crate) fn theme_swatch(ui: &mut egui::Ui, mode: theme::ThemeMode) -> bool {
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
