//! Persist lightweight UI session (tab, paths, density) — no media.

use std::path::PathBuf;

use super::io::{vibecap_config_dir, write_json_atomic};
use crate::ui::theme::Density;

/// E257 — bump when a migration step is added to `migrate_session`.
/// Old files deserialize with `schema_version: 0` and run all migrations.
pub const SESSION_SCHEMA: u32 = 1;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionState {
    /// E257 — schema marker; absent on pre-versioned files → 0.
    #[serde(default)]
    pub schema_version: u32,
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
    /// Filename pattern with `{app}` `{date}` `{time}` `{seq}`.
    #[serde(default = "default_name_pattern")]
    pub name_pattern: String,
    /// Last pixel crop (w,h,x,y) for region re-record.
    #[serde(default)]
    pub last_screen_rect: Option<[i32; 4]>,
    #[serde(default)]
    pub draw_mouse: bool,
    #[serde(default = "default_fps")]
    pub fps: u32,
    /// C55 — recording quality (libx264 `-crf`): 18 sharp / 23 balanced / 28 small.
    #[serde(default = "default_crf")]
    pub record_crf: u8,
    /// E59 — picked DirectShow audio device for recordings; empty = auto.
    #[serde(default)]
    pub audio_device: String,
    /// E72 — time-lapse interval (0 = real-time). A frame every N seconds,
    /// retimed to the fps target on encode.
    #[serde(default)]
    pub timelapse_secs: u32,
    /// E69 — last Window-target pick so the card can show it across restarts.
    #[serde(default)]
    pub window_app: String,
    #[serde(default)]
    pub monitor: Option<u32>,
    #[serde(default)]
    pub inbox_snippets: Vec<String>,
    /// Global screenshot hotkey digit (Ctrl+Shift+N). Default 3.
    #[serde(default = "default_hotkey_shot")]
    pub hotkey_shot_digit: u8,
    /// Global record hotkey digit (Ctrl+Shift+N). Default 2.
    #[serde(default = "default_hotkey_rec")]
    pub hotkey_rec_digit: u8,
    /// E50 — pause/resume hotkey digit (Ctrl+Shift+N); None = unbound.
    #[serde(default)]
    pub hotkey_pause_digit: Option<u8>,
    /// E86 — region-pick hotkey digit (Ctrl+Shift+N); None = unbound.
    #[serde(default)]
    pub hotkey_region_digit: Option<u8>,
    /// E86 — window-still hotkey digit (Ctrl+Shift+N); None = unbound.
    #[serde(default)]
    pub hotkey_window_digit: Option<u8>,
    /// E86 — GIF-clip hotkey digit (Ctrl+Shift+N); None = unbound.
    #[serde(default)]
    pub hotkey_gif_digit: Option<u8>,
    /// E51 — per-target memory: last used target, plus hour-of-day → target.
    #[serde(default)]
    pub capture_target_name: String,
    #[serde(default)]
    pub target_hours: std::collections::BTreeMap<u8, String>,
    /// E204 — bare PrtScn takes a still (opt-in; steals the OS key).
    #[serde(default)]
    pub hotkey_prtscn: bool,
    /// E202 — subtle click when a still lands; off by default.
    #[serde(default)]
    pub shutter_sound: bool,
    /// E74 — watch the system clipboard; a fresh image opens Still review.
    #[serde(default)]
    pub clipboard_watcher: bool,
    /// E95 — lifetime completed region picks; first-run HUD hints hide at 3.
    #[serde(default)]
    pub region_pick_count: u32,
    /// E94 — region HUD toolbar docks to the bottom when set.
    #[serde(default)]
    pub hud_toolbar_bottom: bool,
    /// True after we have triggered the macOS Screen Recording permission probe once.
    #[serde(default)]
    pub screen_permission_prompted: bool,
    /// True only after a probe produced a capture that looks allowed.
    #[serde(default)]
    pub screen_permission_ok: bool,
    /// Left stage rail — hidden by default; the funnel column is the home UX.
    #[serde(default)]
    pub rail_open: bool,
    /// Inbox quiet mode — new agent questions update the badge/tray but skip
    /// the OS notify, toast, attention bounce, and auto-open.
    #[serde(default)]
    pub inbox_quiet: bool,
    /// Auto-play the clip preview when filmstrip frames land.
    #[serde(default = "default_clip_autoplay")]
    pub clip_autoplay: bool,
    /// B54 — auto-apply detected dead-air bounds to the trim on clip load.
    #[serde(default)]
    pub auto_dead_air: bool,
    /// E214 — opt-in release check on launch (off = fully offline by default).
    #[serde(default)]
    pub update_check_on_launch: bool,
    /// D70 — jump to Review after a capture lands. Off = toast only, for
    /// flows that never want the editor.
    #[serde(default = "default_clip_autoplay")]
    pub auto_open_review: bool,
    /// Half-width (240px) filmstrip preview — faster extraction, less GPU memory.
    #[serde(default)]
    pub filmstrip_low_res: bool,
    /// Region overlay dim alpha (0–200). The selection punches through at
    /// full brightness; the rest of the backdrop dims by this much.
    #[serde(default = "default_region_dim")]
    pub region_dim: u8,
    /// Favorited library file *names* (not paths — survives a moved media dir).
    #[serde(default)]
    pub library_favorites: Vec<String>,
    /// E83 — "needs attention" flagged library file names.
    #[serde(default)]
    pub library_flagged: Vec<String>,
    /// E189 — "new since last visit" watermark for the Inbox (same
    /// `%Y-%m-%d %H:%M:%S` format as `FeedbackRequest::created_at`).
    #[serde(default)]
    pub inbox_seen_at: String,
    /// B52 — remembered REC bar position `[x, y]` in screen px.
    #[serde(default)]
    pub rec_bar_pos: Option<[i32; 2]>,
    /// E76 — library tags: file name → tag list (survives a moved media dir).
    #[serde(default)]
    pub library_tags: std::collections::BTreeMap<String, Vec<String>>,
    /// E79 — retention rule: 0 off / 1 older-than-N-days / 2 keep-newest-N.
    #[serde(default)]
    pub retention_mode: u8,
    /// E79 — rule value (days or count).
    #[serde(default = "default_retention_value")]
    pub retention_value: u32,
    /// E79 — auto-sweep once per launch (off by default — destructive).
    #[serde(default)]
    pub retention_auto: bool,
    /// E157 — compact list view instead of the tile grid.
    #[serde(default)]
    pub library_list_view: bool,
    /// E206 — tray double-click action: "open" | "screenshot" | "record".
    #[serde(default = "default_tray_dblclick")]
    pub tray_dblclick: String,
    /// E211 — folder polled for media files to move into the library
    /// (empty = off).
    #[serde(default)]
    pub watch_folder: String,
    /// E225 — follow the OS light/dark preference (Windows reads
    /// AppsUseLightTheme). Off = the picked theme is fixed.
    #[serde(default)]
    pub theme_follow_os: bool,
    /// E6 — scheduled themes: Light by day (07:00–19:00), dark pick at
    /// night. Takes precedence over follow-OS when both are on.
    #[serde(default)]
    pub theme_schedule: bool,
    /// E225 — which dark theme "follow OS" falls back to (name string,
    /// e.g. "dark" | "carbon" | "celestial" | "celestial-pink").
    #[serde(default = "default_theme_dark")]
    pub theme_dark_pick: String,
    /// E292 — tag/notes of a just-applied update so the What's New card
    /// can show once after the relaunch; cleared on dismiss.
    #[serde(default)]
    pub whats_new_tag: String,
    #[serde(default)]
    pub whats_new_notes: String,
    /// E49 — restore the last stage on launch; off = always open Capture.
    #[serde(default = "default_true")]
    pub restore_tab: bool,
    /// E33 — remembered window size per stage (tab key → [w, h]).
    #[serde(default)]
    pub window_sizes: std::collections::HashMap<String, [f32; 2]>,
    /// E26 — icon-only rail (labels + divider text hidden).
    #[serde(default)]
    pub rail_collapsed: bool,
    /// E23 — flatten pulses/hover-grow for motion-sensitive users.
    #[serde(default)]
    pub reduce_motion: bool,
    /// E3 — celestial accent-hue offset (degrees, ±40). 0 = stock aurora.
    #[serde(default)]
    pub aurora_hue: f32,
    /// E22 — named region rects `(w, h, x, y)` in physical pixels —
    /// same layout as `selected_screen_rect`.
    #[serde(default)]
    pub saved_regions: Vec<(String, [i32; 4])>,
    /// E275 — opt-in local capture stats. Off by default; counters never
    /// leave the machine — they're shown in the Library stats line only.
    #[serde(default)]
    pub stats_opt_in: bool,
    #[serde(default)]
    pub stat_shots_ok: u32,
    #[serde(default)]
    pub stat_shots_fail: u32,
    #[serde(default)]
    pub stat_recs_ok: u32,
    #[serde(default)]
    pub stat_recs_fail: u32,
}

fn default_theme_dark() -> String {
    "dark".into()
}

fn default_true() -> bool {
    true
}

fn default_name_pattern() -> String {
    crate::app::naming::DEFAULT_PATTERN.to_string()
}

fn default_crf() -> u8 {
    23
}

fn default_fps() -> u32 {
    30
}

fn default_hotkey_shot() -> u8 {
    3
}

fn default_hotkey_rec() -> u8 {
    2
}

fn default_region_dim() -> u8 {
    110
}

fn default_retention_value() -> u32 {
    30
}

fn default_clip_autoplay() -> bool {
    true
}

fn default_tray_dblclick() -> String {
    "open".into()
}

/// Existing installs without the field skip the wizard.
fn default_wizard_done_migrate() -> bool {
    true
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            schema_version: SESSION_SCHEMA,
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
            name_pattern: default_name_pattern(),
            last_screen_rect: None,
            draw_mouse: false,
            fps: 30,
            record_crf: 23,
            timelapse_secs: 0,
            audio_device: String::new(),
            window_app: String::new(),
            monitor: None,
            inbox_snippets: vec![
                "Looks good".into(),
                "Blur the token".into(),
                "Re-record 16:9".into(),
            ],
            hotkey_shot_digit: 3,
            hotkey_rec_digit: 2,
            hotkey_pause_digit: None,
            hotkey_region_digit: None,
            hotkey_window_digit: None,
            hotkey_gif_digit: None,
            capture_target_name: String::new(),
            target_hours: std::collections::BTreeMap::new(),
            hotkey_prtscn: false,
            shutter_sound: false,
            clipboard_watcher: false,
            region_pick_count: 0,
            hud_toolbar_bottom: false,
            screen_permission_prompted: false,
            screen_permission_ok: false,
            rail_open: false,
            inbox_quiet: false,
            clip_autoplay: true,
            auto_dead_air: false,
            update_check_on_launch: false,
            auto_open_review: true,
            filmstrip_low_res: false,
            region_dim: default_region_dim(),
            library_favorites: Vec::new(),
            library_flagged: Vec::new(),
            inbox_seen_at: String::new(),
            rec_bar_pos: None,
            library_tags: std::collections::BTreeMap::new(),
            retention_mode: 0,
            retention_value: 30,
            retention_auto: false,
            library_list_view: false,
            tray_dblclick: default_tray_dblclick(),
            watch_folder: String::new(),
            theme_follow_os: false,
            theme_schedule: false,
            theme_dark_pick: default_theme_dark(),
            whats_new_tag: String::new(),
            whats_new_notes: String::new(),
            restore_tab: true,
            window_sizes: std::collections::HashMap::new(),
            rail_collapsed: false,
            reduce_motion: false,
            aurora_hue: 0.0,
            saved_regions: Vec::new(),
            stats_opt_in: false,
            stat_shots_ok: 0,
            stat_shots_fail: 0,
            stat_recs_ok: 0,
            stat_recs_fail: 0,
        }
    }
}

fn session_path() -> PathBuf {
    vibecap_config_dir().join("session.json")
}

/// E257 — schema migrations. v0 → v1 is a no-op (every field already
/// carries `#[serde(default)]`, so old files fill in safely); the hook
/// exists so a future breaking change has a place to transform data.
fn migrate_session(mut s: SessionState) -> SessionState {
    if s.schema_version < 1 {
        // v0 → v1: nothing to move — serde defaults covered the add.
        s.schema_version = 1;
    }
    s.schema_version = SESSION_SCHEMA;
    s
}

pub fn load_session() -> SessionState {
    match std::fs::read_to_string(session_path()) {
        Ok(s) => serde_json::from_str(&s)
            .map(migrate_session)
            .unwrap_or_default(),
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
