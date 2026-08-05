//! System tray (menu bar / notification area) for the desktop GUI.
//!
//! Closing the window hides to tray; Quit exits. Multiple GUI/MCP instances
//! are supported — the tray only belongs to the GUI process that created it.

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
    Feedback,
    Quit,
}

pub struct TrayController {
    _tray: TrayIcon,
    show_id: tray_icon::menu::MenuId,
    hide_id: tray_icon::menu::MenuId,
    screenshot_id: tray_icon::menu::MenuId,
    feedback_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
}

impl TrayController {
    pub fn try_new(tooltip: &str) -> Result<Self, String> {
        let icon = make_tray_icon().map_err(|e| format!("tray icon image: {e}"))?;

        let show_item = MenuItem::new("Show Vibecap", true, None);
        let hide_item = MenuItem::new("Hide to Tray", true, None);
        let screenshot_item = MenuItem::new("Screenshot…", true, None);
        let feedback_item = MenuItem::new("Open Feedback Inbox", true, None);
        let quit_item = MenuItem::new("Quit Vibecap", true, None);

        let show_id = show_item.id().clone();
        let hide_id = hide_item.id().clone();
        let screenshot_id = screenshot_item.id().clone();
        let feedback_id = feedback_item.id().clone();
        let quit_id = quit_item.id().clone();

        let menu = Menu::new();
        menu.append_items(&[
            &show_item,
            &hide_item,
            &PredefinedMenuItem::separator(),
            &screenshot_item,
            &feedback_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ])
        .map_err(|e| format!("tray menu: {e}"))?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(tooltip)
            .with_icon(icon)
            .with_title("") // macOS: icon-only in menu bar
            .build()
            .map_err(|e| format!("tray build: {e}"))?;

        Ok(Self {
            _tray: tray,
            show_id,
            hide_id,
            screenshot_id,
            feedback_id,
            quit_id,
        })
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
            } else if id == self.feedback_id {
                actions.push(TrayAction::Feedback);
            } else if id == self.quit_id {
                actions.push(TrayAction::Quit);
            }
        }

        actions
    }
}

/// Simple branded 32×32 orange “V” icon (no external assets).
fn make_tray_icon() -> Result<Icon, String> {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    // Background: rounded dark square with orange accent
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            // soft circle mask
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
            // dark brown base
            rgba[i] = 0x2a;
            rgba[i + 1] = 0x1f;
            rgba[i + 2] = 0x12;
            rgba[i + 3] = 0xff;
        }
    }

    // Draw a simple “V” in orange (#f59e4b)
    let paint = |buf: &mut [u8], x: i32, y: i32| {
        if x < 0 || y < 0 || x >= size as i32 || y >= size as i32 {
            return;
        }
        let i = ((y as u32 * size + x as u32) * 4) as usize;
        buf[i] = 0xf5;
        buf[i + 1] = 0x9e;
        buf[i + 2] = 0x4b;
        buf[i + 3] = 0xff;
    };

    // Left stroke of V
    for t in 0..14 {
        let x = 8 + t / 2;
        let y = 8 + t;
        for dx in 0..3 {
            paint(&mut rgba, x + dx, y);
        }
    }
    // Right stroke of V
    for t in 0..14 {
        let x = 22 - t / 2;
        let y = 8 + t;
        for dx in 0..3 {
            paint(&mut rgba, x + dx, y);
        }
    }

    Icon::from_rgba(rgba, size, size).map_err(|e| e.to_string())
}
