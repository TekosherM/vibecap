//! System tray (menu bar / notification area) for the desktop GUI.
//!
//! Menu is grouped: Window · Capture · Agents · Quit.
//! While recording, the menu-bar title shows live elapsed time.

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
    /// Toggle: start if idle, stop if recording.
    ToggleRecord,
    Feedback,
    Quit,
}

pub struct TrayController {
    tray: TrayIcon,
    record_item: MenuItem,
    show_id: tray_icon::menu::MenuId,
    hide_id: tray_icon::menu::MenuId,
    screenshot_id: tray_icon::menu::MenuId,
    record_id: tray_icon::menu::MenuId,
    feedback_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
    last_progress_key: String,
}

impl TrayController {
    pub fn try_new(tooltip: &str) -> Result<Self, String> {
        let icon = make_tray_icon(false).map_err(|e| format!("tray icon image: {e}"))?;

        // ── Window ──────────────────────────────────────────────
        let show_item = MenuItem::new("Show Window", true, None);
        let hide_item = MenuItem::new("Hide to Tray", true, None);

        // ── Capture ─────────────────────────────────────────────
        let screenshot_item = MenuItem::new("Screenshot\t⌃⇧3", true, None);
        let record_item = MenuItem::new("Start Recording\t⌃⇧2", true, None);

        // ── Agents ──────────────────────────────────────────────
        let feedback_item = MenuItem::new("Feedback Inbox", true, None);

        // ── App ─────────────────────────────────────────────────
        let quit_item = MenuItem::new("Quit Vibecap", true, None);

        let show_id = show_item.id().clone();
        let hide_id = hide_item.id().clone();
        let screenshot_id = screenshot_item.id().clone();
        let record_id = record_item.id().clone();
        let feedback_id = feedback_item.id().clone();
        let quit_id = quit_item.id().clone();

        let menu = Menu::new();
        menu.append_items(&[
            &show_item,
            &hide_item,
            &PredefinedMenuItem::separator(),
            &screenshot_item,
            &record_item,
            &PredefinedMenuItem::separator(),
            &feedback_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ])
        .map_err(|e| format!("tray menu: {e}"))?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(tooltip)
            .with_icon(icon)
            .with_title("") // idle: icon only
            .build()
            .map_err(|e| format!("tray build: {e}"))?;

        Ok(Self {
            tray,
            record_item,
            show_id,
            hide_id,
            screenshot_id,
            record_id,
            feedback_id,
            quit_id,
            last_progress_key: String::new(),
        })
    }

    /// Update tray title, tooltip, menu label, and icon for recording state.
    /// Call every frame (or ~1s) while recording; pass `None` when idle.
    pub fn set_recording_progress(&mut self, elapsed_secs: Option<u64>) {
        let key = match elapsed_secs {
            Some(s) => format!("rec:{s}"),
            None => "idle".to_string(),
        };
        if key == self.last_progress_key {
            return;
        }
        self.last_progress_key = key;

        match elapsed_secs {
            Some(secs) => {
                let mins = secs / 60;
                let s = secs % 60;
                let clock = format!("{mins:02}:{s:02}");
                // Compact menu-bar title so progress is visible without opening the menu.
                self.tray.set_title(Some(format!("● {clock}")));
                let _ = self
                    .tray
                    .set_tooltip(Some(format!("Recording {clock} — open menu to stop")));
                self.record_item
                    .set_text(format!("Stop Recording  [{clock}]\t⌃⇧2"));
                if let Ok(icon) = make_tray_icon(true) {
                    let _ = self.tray.set_icon(Some(icon));
                }
            }
            None => {
                self.tray.set_title(Some(""));
                let _ = self
                    .tray
                    .set_tooltip(Some("Vibecap Studio — click to show"));
                self.record_item
                    .set_text("Start Recording\t⌃⇧2");
                if let Ok(icon) = make_tray_icon(false) {
                    let _ = self.tray.set_icon(Some(icon));
                }
            }
        }
    }

    /// Drain pending tray / menu events (non-blocking).
    pub fn poll_actions(&self) -> Vec<TrayAction> {
        let mut actions = Vec::new();

        // Left-click on tray icon → show window
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
            } else if id == self.feedback_id {
                actions.push(TrayAction::Feedback);
            } else if id == self.quit_id {
                actions.push(TrayAction::Quit);
            }
        }

        actions
    }
}

/// 32×32 tray icon. `recording = true` draws a red rec badge.
fn make_tray_icon(recording: bool) -> Result<Icon, String> {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            let cx = x as f32 - 15.5;
            let cy = y as f32 - 15.5;
            let r2 = cx * cx + cy * cy;
            if r2 > 15.5 * 15.5 {
                rgba[i] = 0;
                rgba[i + 1] = 0;
                rgba[i + 2] = 0;
                rgba[i + 3] = 0;
                continue;
            }
            if recording {
                // Deep red base while recording
                rgba[i] = 0x3a;
                rgba[i + 1] = 0x12;
                rgba[i + 2] = 0x12;
            } else {
                rgba[i] = 0x2a;
                rgba[i + 1] = 0x1f;
                rgba[i + 2] = 0x12;
            }
            rgba[i + 3] = 0xff;
        }
    }

    let paint = |buf: &mut [u8], x: i32, y: i32, r: u8, g: u8, b: u8| {
        if x < 0 || y < 0 || x >= size as i32 || y >= size as i32 {
            return;
        }
        let i = ((y as u32 * size + x as u32) * 4) as usize;
        buf[i] = r;
        buf[i + 1] = g;
        buf[i + 2] = b;
        buf[i + 3] = 0xff;
    };

    if recording {
        // Solid red record disc in the center
        for y in 0..size {
            for x in 0..size {
                let cx = x as f32 - 15.5;
                let cy = y as f32 - 15.5;
                if cx * cx + cy * cy <= 7.0 * 7.0 {
                    paint(&mut rgba, x as i32, y as i32, 0xe8, 0x3b, 0x3b);
                }
            }
        }
    } else {
        // Orange “V”
        for t in 0..14 {
            let x = 8 + t / 2;
            let y = 8 + t;
            for dx in 0..3 {
                paint(&mut rgba, x + dx, y, 0xf5, 0x9e, 0x4b);
            }
        }
        for t in 0..14 {
            let x = 22 - t / 2;
            let y = 8 + t;
            for dx in 0..3 {
                paint(&mut rgba, x + dx, y, 0xf5, 0x9e, 0x4b);
            }
        }
    }

    Icon::from_rgba(rgba, size, size).map_err(|e| e.to_string())
}
