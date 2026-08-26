//! Persist lightweight UI session (tab, paths, density) — no media.

use std::path::PathBuf;

use super::io::{vibecap_config_dir, write_json_atomic};
use crate::ui::theme::Density;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionState {
    #[serde(default)]
    pub tab: String,
    #[serde(default)]
    pub edit_file: Option<String>,
    #[serde(default)]
    pub density: String,
    #[serde(default)]
    pub library_filter: String,
    #[serde(default)]
    pub window_w: f32,
    #[serde(default)]
    pub window_h: f32,
    /// False only on true first run (no prior session file).
    /// Missing field on old session.json → true so we don't re-onboard veterans.
    #[serde(default = "default_wizard_done_migrate")]
    pub wizard_done: bool,
    /// "dark" | "light"
    #[serde(default = "default_theme_dark")]
    pub theme: String,
    /// Last region-select rect as [min_x, min_y, max_x, max_y] in screen coords.
    #[serde(default)]
    pub last_region: Option<[f32; 4]>,
    /// Pre-record countdown: 0 | 3 | 5.
    #[serde(default)]
    pub record_countdown_secs: u8,
    /// True after we have triggered the macOS Screen Recording permission probe once.
    #[serde(default)]
    pub screen_permission_prompted: bool,
    /// True only after a probe produced a capture that looks allowed.
    #[serde(default)]
    pub screen_permission_ok: bool,
}

fn default_theme_dark() -> String {
    "dark".into()
}

/// Existing installs without the field skip the wizard.
fn default_wizard_done_migrate() -> bool {
    true
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            tab: "capture".into(),
            edit_file: None,
            density: "comfortable".into(),
            library_filter: "All".into(),
            window_w: 1160.0,
            window_h: 800.0,
            // Fresh install (no session.json) → show wizard.
            wizard_done: false,
            theme: "dark".into(),
            last_region: None,
            record_countdown_secs: 0,
            screen_permission_prompted: false,
            screen_permission_ok: false,
        }
    }
}

fn session_path() -> PathBuf {
    vibecap_config_dir().join("session.json")
}

pub fn load_session() -> SessionState {
    match std::fs::read_to_string(session_path()) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => SessionState::default(),
    }
}

pub fn save_session(state: &SessionState) {
    if let Ok(s) = serde_json::to_string_pretty(state) {
        let _ = write_json_atomic(&session_path(), &s);
    }
}

pub fn density_from_str(s: &str) -> Density {
    match s {
        "compact" => Density::Compact,
        _ => Density::Comfortable,
    }
}

pub fn density_to_str(d: Density) -> &'static str {
    match d {
        Density::Comfortable => "comfortable",
        Density::Compact => "compact",
    }
}

pub fn tab_from_str(s: &str) -> Option<&'static str> {
    match s {
        "capture" | "shutter" => Some("capture"),
        "library" | "media" => Some("library"),
        "edit" | "studio" | "clip" => Some("clip"),
        "still" | "image" => Some("still"),
        "feedback" | "inbox" => Some("feedback"),
        "settings" => Some("settings"),
        _ => None,
    }
}
