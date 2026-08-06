//! System tray (menu bar / notification area) for the desktop GUI.
//!
//! Menu mirrors the Loop rail: Window · Capture · Stages · Quit.
//! Live progress: menu-bar title + disabled status row + dynamic Record label.
//!
//! Idle icon: monochrome aperture as a macOS **template** image.
//! Recording: non-template red REC iris.

use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

/// User action requested via the tray menu or icon click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    Hide,
    Screenshot,
    /// Idle → start; arming → cancel; recording → stop.
    ToggleRecord,
    GoShutter,
    GoMedia,
    GoClip,
    GoStill,
    GoInbox,
    GoSettings,
    BugReport,
    Quit,
}

/// Live capture state for menu-bar progress + menu labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayLiveState {
    Idle,
    /// Countdown / spawn in flight.
    Arming,
    Recording { elapsed_secs: u64 },
}

pub struct TrayController {
    tray: TrayIcon,
    status_item: MenuItem,
    record_item: MenuItem,
    inbox_item: MenuItem,
    show_id: tray_icon::menu::MenuId,
    hide_id: tray_icon::menu::MenuId,
    screenshot_id: tray_icon::menu::MenuId,
    record_id: tray_icon::menu::MenuId,
    shutter_id: tray_icon::menu::MenuId,
    media_id: tray_icon::menu::MenuId,
    clip_id: tray_icon::menu::MenuId,
    still_id: tray_icon::menu::MenuId,
    inbox_id: tray_icon::menu::MenuId,
    settings_id: tray_icon::menu::MenuId,
    bug_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
    last_progress_key: String,
}

impl TrayController {
    pub fn try_new(tooltip: &str) -> Result<Self, String> {
        let icon = make_tray_icon(false).map_err(|e| format!("tray icon image: {e}"))?;

        // ── Live status (disabled; updated every second) ────────
        let status_item = MenuItem::new("Vibecap · Ready", false, None);

        // ── Window ──────────────────────────────────────────────
        let show_item = MenuItem::new("Show Window", true, None);
        let hide_item = MenuItem::new("Hide to Menu Bar", true, None);

        // ── Capture ─────────────────────────────────────────────
        let screenshot_item = MenuItem::new("Screenshot\t⌃⇧3", true, None);
        let record_item = MenuItem::new("Record\t⌃⇧2", true, None);

        // ── Loop stages ─────────────────────────────────────────
        let shutter_item = MenuItem::new("Shutter", true, None);
        let media_item = MenuItem::new("Media", true, None);
        let clip_item = MenuItem::new("Clip", true, None);
        let still_item = MenuItem::new("Still", true, None);
        let inbox_item = MenuItem::new("Inbox", true, None);
        let settings_item = MenuItem::new("Settings", true, None);

        // ── Tools ───────────────────────────────────────────────
        let bug_item = MenuItem::new("Bug Report Pack", true, None);

        // ── App ─────────────────────────────────────────────────
        let quit_item = MenuItem::new("Quit Vibecap", true, None);

        let show_id = show_item.id().clone();
        let hide_id = hide_item.id().clone();
        let screenshot_id = screenshot_item.id().clone();
        let record_id = record_item.id().clone();
        let shutter_id = shutter_item.id().clone();
        let media_id = media_item.id().clone();
        let clip_id = clip_item.id().clone();
        let still_id = still_item.id().clone();
        let inbox_id = inbox_item.id().clone();
        let settings_id = settings_item.id().clone();
        let bug_id = bug_item.id().clone();
        let quit_id = quit_item.id().clone();

        let menu = Menu::new();
        menu.append_items(&[
            &status_item,
            &PredefinedMenuItem::separator(),
            &show_item,
            &hide_item,
            &PredefinedMenuItem::separator(),
            &screenshot_item,
            &record_item,
            &PredefinedMenuItem::separator(),
            &shutter_item,
            &media_item,
            &clip_item,
            &still_item,
            &inbox_item,
            &settings_item,
            &PredefinedMenuItem::separator(),
            &bug_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ])
        .map_err(|e| format!("tray menu: {e}"))?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(tooltip)
            .with_icon(icon)
            .with_icon_as_template(true)
            .with_title("") // idle: icon only
            .build()
            .map_err(|e| format!("tray build: {e}"))?;

        Ok(Self {
            tray,
            status_item,
            record_item,
            inbox_item,
            show_id,
            hide_id,
            screenshot_id,
            record_id,
            shutter_id,
            media_id,
            clip_id,
            still_id,
            inbox_id,
            settings_id,
            bug_id,
            quit_id,
            last_progress_key: String::new(),
        })
    }

    /// Same as [`set_live_state`] but always refreshes (e.g. new Inbox item).
    pub fn force_live_state(&mut self, state: TrayLiveState, inbox_pending: usize) {
        self.last_progress_key.clear();
        self.set_live_state(state, inbox_pending);
    }

    /// Update menu-bar title, tooltip, status row, Record label, and icon.
    /// Call ~1s while active; pass `TrayLiveState::Idle` when idle.
    pub fn set_live_state(&mut self, state: TrayLiveState, inbox_pending: usize) {
        let key = match state {
            TrayLiveState::Idle => format!("idle:{inbox_pending}"),
            TrayLiveState::Arming => format!("arm:{inbox_pending}"),
            TrayLiveState::Recording { elapsed_secs } => {
                format!("rec:{elapsed_secs}:{inbox_pending}")
            }
        };
        if key == self.last_progress_key {
            return;
        }
        self.last_progress_key = key;

        // Inbox badge in menu (always refresh with progress).
        if inbox_pending > 0 {
            self.inbox_item.set_text(format!(
                "Inbox ({})",
                if inbox_pending > 99 {
                    "99+".into()
                } else {
                    inbox_pending.to_string()
                }
            ));
        } else {
            self.inbox_item.set_text("Inbox");
        }

        match state {
            TrayLiveState::Recording { elapsed_secs } => {
                let clock = format_clock(elapsed_secs);
                // Menu bar: compact live tracker next to aperture icon.
                self.tray.set_title(Some(format!("REC {clock}")));
                let _ = self.tray.set_tooltip(Some(format!(
                    "Recording {clock} — menu: Stop · ⌃⇧2"
                )));
                self.status_item
                    .set_text(format!("Recording · {clock}"));
                self.record_item
                    .set_text(format!("Stop Recording  [{clock}]\t⌃⇧2"));
                if let Ok(icon) = make_tray_icon(true) {
                    let _ = self.tray.set_icon_with_as_template(Some(icon), false);
                }
            }
            TrayLiveState::Arming => {
                self.tray.set_title(Some("…"));
                let _ = self
                    .tray
                    .set_tooltip(Some("Starting recording… — menu: Cancel · Esc"));
                self.status_item.set_text("Starting…");
                self.record_item.set_text("Cancel Start\t⌃⇧2");
                if let Ok(icon) = make_tray_icon(true) {
                    let _ = self.tray.set_icon_with_as_template(Some(icon), false);
                }
            }
            TrayLiveState::Idle => {
                self.record_item.set_text("Record\t⌃⇧2");
                if inbox_pending > 0 {
                    // Visible menu-bar signal while agents wait (template icon + title).
                    let label = if inbox_pending == 1 {
                        "Inbox".to_string()
                    } else {
                        format!("Inbox {inbox_pending}")
                    };
                    self.tray.set_title(Some(label.as_str()));
                    let _ = self.tray.set_tooltip(Some(format!(
                        "Agent waiting — {inbox_pending} question(s). Open Inbox to answer."
                    )));
                    self.status_item
                        .set_text(format!("Agent waiting · {inbox_pending}"));
                } else {
                    self.tray.set_title(Some(""));
                    let _ = self.tray.set_tooltip(Some("Vibecap — click to show"));
                    self.status_item.set_text("Vibecap · Ready");
                }
                if let Ok(icon) = make_tray_icon(false) {
                    let _ = self.tray.set_icon_with_as_template(Some(icon), true);
                }
            }
        }
    }

    /// Drain pending tray / menu events (non-blocking).
    pub fn poll_actions(&self) -> Vec<TrayAction> {
        let mut actions = Vec::new();

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                actions.push(TrayAction::Show);
            }
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = event.id;
            if id == self.show_id {
                actions.push(TrayAction::Show);
            } else if id == self.hide_id {
                actions.push(TrayAction::Hide);
            } else if id == self.screenshot_id {
                actions.push(TrayAction::Screenshot);
            } else if id == self.record_id {
                actions.push(TrayAction::ToggleRecord);
            } else if id == self.shutter_id {
                actions.push(TrayAction::GoShutter);
            } else if id == self.media_id {
                actions.push(TrayAction::GoMedia);
            } else if id == self.clip_id {
                actions.push(TrayAction::GoClip);
            } else if id == self.still_id {
                actions.push(TrayAction::GoStill);
            } else if id == self.inbox_id {
                actions.push(TrayAction::GoInbox);
            } else if id == self.settings_id {
                actions.push(TrayAction::GoSettings);
            } else if id == self.bug_id {
                actions.push(TrayAction::BugReport);
            } else if id == self.quit_id {
                actions.push(TrayAction::Quit);
            }
        }

        actions
    }
}

fn format_clock(secs: u64) -> String {
    let mins = secs / 60;
    let s = secs % 60;
    format!("{mins:02}:{s:02}")
}

/// 32×32 tray icon — brand aperture shutter (Safelight mark).
///
/// * Idle (`recording = false`): black ink + alpha for macOS **template** images.
/// * Recording: light aperture ring + solid red REC disc (non-template color).
fn make_tray_icon(recording: bool) -> Result<Icon, String> {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let cx = (size as f32 - 1.0) * 0.5;
    let cy = cx;

    let r_outer = 13.0_f32;
    let stroke = 1.55_f32;
    let r_hex = 4.0_f32;
    let n_blades = 6_i32;
    let twist = std::f32::consts::TAU / n_blades as f32;

    let (ink_r, ink_g, ink_b) = if recording {
        (0xf2_u8, 0xf2, 0xf2)
    } else {
        (0x00, 0x00, 0x00)
    };
    let (rec_r, rec_g, rec_b) = (0xe8_u8, 0x3b, 0x3b);

    let put = |buf: &mut [u8], x: u32, y: u32, r: u8, g: u8, b: u8, a: u8| {
        if x >= size || y >= size || a == 0 {
            return;
        }
        let i = ((y * size + x) * 4) as usize;
        if a >= buf[i + 3] {
            buf[i] = r;
            buf[i + 1] = g;
            buf[i + 2] = b;
            buf[i + 3] = a;
        }
    };

    let aa = |d: f32, half: f32| -> u8 {
        let t = (half - d).clamp(0.0, 1.0);
        (t * 255.0).round() as u8
    };

    let dist_seg = |px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32| -> f32 {
        let abx = bx - ax;
        let aby = by - ay;
        let apx = px - ax;
        let apy = py - ay;
        let ab2 = abx * abx + aby * aby;
        if ab2 < 1e-6 {
            return (apx * apx + apy * apy).sqrt();
        }
        let t = ((apx * abx + apy * aby) / ab2).clamp(0.0, 1.0);
        let qx = ax + t * abx - px;
        let qy = ay + t * aby - py;
        (qx * qx + qy * qy).sqrt()
    };

    let hex_edge_dist = |px: f32, py: f32, r: f32| -> f32 {
        let mut best = f32::MAX;
        for i in 0..n_blades {
            let a0 = (i as f32) * twist + std::f32::consts::FRAC_PI_6;
            let a1 = ((i + 1) as f32) * twist + std::f32::consts::FRAC_PI_6;
            let ax = a0.cos() * r;
            let ay = a0.sin() * r;
            let bx = a1.cos() * r;
            let by = a1.sin() * r;
            best = best.min(dist_seg(px, py, ax, ay, bx, by));
        }
        best
    };

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 - cx;
            let py = y as f32 - cy;
            let dist = (px * px + py * py).sqrt();

            let ring_d = (dist - r_outer).abs();
            let ring_half = stroke * 0.5 + 0.4;
            if ring_d < ring_half && dist < r_outer + 1.2 {
                put(&mut rgba, x, y, ink_r, ink_g, ink_b, aa(ring_d, ring_half));
            }

            if dist >= r_hex * 0.55 && dist <= r_outer + 0.5 {
                let mut best = f32::MAX;
                for i in 0..n_blades {
                    let a_out0 = (i as f32) * twist + std::f32::consts::FRAC_PI_6;
                    let a_out1 = ((i + 1) as f32) * twist + std::f32::consts::FRAC_PI_6;
                    let a_hex = a_out0 + twist * 0.5;
                    let ox0 = a_out0.cos() * r_outer;
                    let oy0 = a_out0.sin() * r_outer;
                    let ox1 = a_out1.cos() * r_outer;
                    let oy1 = a_out1.sin() * r_outer;
                    let hx = a_hex.cos() * r_hex;
                    let hy = a_hex.sin() * r_hex;
                    best = best.min(dist_seg(px, py, ox0, oy0, hx, hy));
                    best = best.min(dist_seg(px, py, ox1, oy1, hx, hy));
                }
                let half = stroke * 0.5 + 0.35;
                if best < half {
                    put(&mut rgba, x, y, ink_r, ink_g, ink_b, aa(best, half));
                }
            }

            let he = hex_edge_dist(px, py, r_hex);
            if he < stroke * 0.55 + 0.35 && dist < r_hex + 1.5 {
                put(
                    &mut rgba,
                    x,
                    y,
                    ink_r,
                    ink_g,
                    ink_b,
                    aa(he, stroke * 0.55 + 0.35),
                );
            }
        }
    }

    if recording {
        let rec_rad = 4.6_f32;
        for y in 0..size {
            for x in 0..size {
                let px = x as f32 - cx;
                let py = y as f32 - cy;
                let dist = (px * px + py * py).sqrt();
                if dist <= rec_rad {
                    let edge = (rec_rad - dist).clamp(0.0, 1.2);
                    let a = if dist <= rec_rad - 0.5 {
                        255
                    } else {
                        ((edge / 1.2) * 255.0).round() as u8
                    };
                    if a > 40 {
                        put(&mut rgba, x, y, rec_r, rec_g, rec_b, a.max(180));
                    }
                }
            }
        }
    }

    Icon::from_rgba(rgba, size, size).map_err(|e| e.to_string())
}
