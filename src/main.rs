mod app;
mod platform;
mod tray_ui;
mod ui;

use eframe::egui;
use std::process::{Command, Child};
use std::path::PathBuf;
use std::io::Write;
use chrono::Local;
use crossbeam_channel::Receiver;
use global_hotkey::{GlobalHotKeyManager, hotkey::{HotKey, Modifiers, Code}};
use global_hotkey::GlobalHotKeyEvent;
use egui::{
    Color32, Stroke, Pos2, Rect, Vec2, ViewportId, ViewportBuilder, ViewportCommand, RichText,
    Frame, Align2, FontId, UserAttentionType,
};
use rfd::FileDialog;
use std::time::{Duration, Instant};

use platform::{
    capture_screenshot, capture_screenshot_interactive, cont_process, focus_app,
    frontmost_app_name, list_running_apps, media_dir_display, notify_agent_question, open_path,
    reveal_in_file_manager, spawn_screen_recorder, spawn_voice_memo, stop_process,
};
use tray_ui::{TrayAction, TrayController, TrayLiveState};
use ui::{
    apply_current_theme, apply_graphite_theme, empty_state, loop_rail, show_capture_toast,
    show_countdown_bubble, show_palette, show_region_selector, show_toast_card, shutter_strip,
    status_strip, CaptureToastAction, Density, LoopStage, PaletteAction, RegionHudResult,
    ShutterAction, StatusSnapshot, ThemeMode, ToastLevel,
};
use ui::icons::Icon;
use ui::theme;

use app::{
    capture_screenshot_to_media_dir, default_live_dir, default_media_dir, even_crop,
    extract_filmstrip_thumbs, feedback_requests_dir, feedback_responses_dir, filter_items,
    finalize_recorder, format_feedback_answer, get_dir_size_bytes, kill_recorder,
    live_usage_snapshot, load_budget, mcp_live_dir, run_mcp_server, save_budget, scan_media_dir,
    write_json_atomic, BudgetConfig, FeedbackRequest, FeedbackResponse, MediaCategory, MediaItem,
    LIBRARY_PAGE_SIZE,
};
use app::io::vibecap_config_dir;
use app::session::{
    density_from_str, density_to_str, load_session, save_session, SessionState,
};

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum AppTab {
    Capture,
    Library,
    Clip,
    Still,
    Feedback,
    Settings,
}

impl AppTab {
    pub(crate) fn from_loop(stage: LoopStage) -> Self {
        match stage {
            LoopStage::Shutter => Self::Capture,
            LoopStage::Media => Self::Library,
            LoopStage::Clip => Self::Clip,
            LoopStage::Still => Self::Still,
            LoopStage::Inbox => Self::Feedback,
            LoopStage::Settings => Self::Settings,
        }
    }

    pub(crate) fn to_loop(self) -> LoopStage {
        match self {
            Self::Capture => LoopStage::Shutter,
            Self::Library => LoopStage::Media,
            Self::Clip => LoopStage::Clip,
            Self::Still => LoopStage::Still,
            Self::Feedback => LoopStage::Inbox,
            Self::Settings => LoopStage::Settings,
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Capture => "Shutter",
            Self::Library => "Media",
            Self::Clip => "Clip",
            Self::Still => "Still",
            Self::Feedback => "Inbox",
            Self::Settings => "Settings",
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum CaptureTarget {
    Fullscreen,
    Region,
    Window,
}

#[derive(PartialEq, Clone, Copy, Default)]
pub(crate) enum AnnotationTool {
    #[default]
    Pen,
    Arrow,
    Rectangle,
    Highlight,
    Text,
    Blur,
    StepBadge,
}

#[derive(Clone)]
pub(crate) struct AnnotationAction {
    pub tool: AnnotationTool,
    pub color: Color32,
    pub stroke_width: f32,
    pub points: Vec<Pos2>,
    pub text_content: String,
    pub badge_number: usize,
}

#[derive(Default)]
pub(crate) struct VibecapApp {
    current_tab: AppTab,
    capture_target: CaptureTarget,
    capture_audio: bool,
    fps_target: u32,
    is_recording: bool,
    /// Hide UI then spawn ffmpeg on a worker — true while countdown / spawn in flight.
    recording_arming: bool,
    /// User cancelled during arming (worker result is discarded / killed).
    recording_cancel_armed: bool,
    is_paused: bool,
    accumulated_duration: Duration,
    segment_start: Option<Instant>,
    child_process: Option<Child>,

    // Audio Voice Note Recording
    is_recording_voice_memo: bool,
    voice_memo_child: Option<Child>,
    voice_memo_start: Option<Instant>,
    
    // Channels for async capture
    /// Region-select → arm recording on next frame (avoids re-entrancy).
    pending_arm_record: bool,
    screenshot_tx: Option<crossbeam_channel::Sender<Result<PathBuf, String>>>,
    screenshot_rx: Option<crossbeam_channel::Receiver<Result<PathBuf, String>>>,
    /// Worker → main: ffmpeg child after hide delay (recording starts even if UI was minimized).
    record_spawn_rx: Option<crossbeam_channel::Receiver<Result<(Child, PathBuf), String>>>,
    
    // File paths & Media Library
    save_dir: PathBuf,
    current_mp4_file: Option<PathBuf>,
    latest_screenshot: Option<PathBuf>,
    library_items: Vec<MediaItem>,
    /// "All" | category labels from MediaCategory::label()
    library_filter: String,
    /// How many filtered items to show (starts at LIBRARY_PAGE_SIZE).
    library_show_limit: usize,
    /// Paths selected for bulk open/delete.
    library_selected: std::collections::HashSet<PathBuf>,
    /// Pending confirm for clear-all in current category.
    library_confirm_clear: bool,
    
    // Edit tab & Video Processing
    trim_start: String,
    trim_end: String,
    export_speed: String,
    edit_file: Option<PathBuf>,
    filmstrip: Vec<egui::TextureHandle>,
    filmstrip_loading: bool,
    filmstrip_error: Option<String>,

    // Annotation & Developer Feedback Note
    is_annotating: bool,
    annotation_texture: Option<egui::TextureHandle>,
    annotation_actions: Vec<AnnotationAction>,
    current_action: Option<AnnotationAction>,
    current_tool: AnnotationTool,
    current_color: Color32,
    current_stroke_width: f32,
    pending_text: String,
    feedback_description: String,
    step_counter: usize,
    
    hotkey_receiver: Option<Receiver<GlobalHotKeyEvent>>,
    /// Kept alive so the global hotkey stays registered (drop unregisters).
    #[allow(dead_code)]
    hotkey_manager: Option<GlobalHotKeyManager>,
    /// Hotkey id → start/stop recording (Ctrl+Shift+2).
    hotkey_id_record: u32,
    /// Hotkey id → screenshot (Ctrl+Shift+3).
    hotkey_id_screenshot: u32,

    // System tray (menu bar / notification area)
    tray: Option<TrayController>,
    /// When false, window close hides to tray instead of exiting.
    allow_exit: bool,
    /// Start window hidden (still in tray).
    start_hidden: bool,
    
    // Region Selection Overlay
    is_selecting_region: bool,
    region_start: Option<Pos2>,
    region_end: Option<Pos2>,
    selected_region: Option<Rect>,
    /// Ghost outline for next region select (session-persisted).
    last_region: Option<Rect>,

    // Notification toast (message, shown_at, severity)
    toast_message: Option<(String, Instant, ToastLevel)>,
    /// Post-capture action card (path + shown_at); mutually preferred over simple toast.
    capture_toast: Option<(PathBuf, Instant)>,

    // Phase 1d: palette, density, undo trash
    palette_open: bool,
    palette_query: String,
    palette_selected: usize,
    density: Density,
    /// Soft-deleted paths staged for undo (restore before expiry).
    undo_trash: Option<(Vec<PathBuf>, Instant, PathBuf)>,

    // Feedback Inbox (agent human-in-the-loop)
    feedback_requests: Vec<FeedbackRequest>,
    feedback_scanned: bool,
    feedback_selected: Option<String>,
    feedback_draft: String,
    /// Quick-choice chip selected for the open reply (maps to selected_option).
    feedback_choice: String,

    // Agent Budget panel (shared with MCP via ~/.config/vibecap/budget.json)
    budget_frames_input: String,
    budget_mb_input: String,
    budget_minutes_input: String,
    budget_tier: String,
    budget_loaded: bool,

    // Image Editor wardrobe
    img_edit_file: Option<PathBuf>,
    img_rotate: u32,
    img_flip_h: bool,
    img_flip_v: bool,
    img_grayscale: bool,
    img_brightness: i32,
    img_contrast: f32,
    img_blur: f32,
    img_resize_pct: u32,
    img_crop_x: String,
    img_crop_y: String,
    img_crop_w: String,
    img_crop_h: String,

    // Feedback arrival polling & richer replies
    feedback_last_poll: Option<Instant>,
    feedback_pending_count: usize,
    /// Request IDs we already notified about (OS toast + surface).
    feedback_notified_ids: std::collections::HashSet<String>,
    feedback_reply_cache: std::collections::HashMap<String, String>,
    annotating_feedback_id: Option<String>,
    feedback_voice_note: Option<PathBuf>,
    active_voice_memo_path: Option<PathBuf>,
    pending_annotated_save: Option<(PathBuf, Instant)>,
    annotation_canvas_rect: Option<Rect>,

    // ffmpeg job results (checked completions — no fabricated success toasts)
    ffmpeg_tx: Option<crossbeam_channel::Sender<(bool, String)>>,
    ffmpeg_rx: Option<crossbeam_channel::Receiver<(bool, String)>>,

    // Image editor live preview
    img_preview_on: bool,
    img_preview_tex: Option<egui::TextureHandle>,
    img_preview_params: String,
    img_source_dims: String,

    // First-run wizard (Phase 3)
    wizard_open: bool,
    wizard_step: u8,
    wizard_done: bool,
    wizard_budget_touched: bool,

    /// Retro buffer (off by default) — rolling low-FPS frames for “save last N s”.
    retro: app::RetroController,

    /// Pre-record countdown preference: 0 / 3 / 5 seconds.
    record_countdown_secs: u8,
    /// When set, big bubble counts down until this instant, then arm_recording.
    countdown_deadline: Option<Instant>,

    /// Window-target picker: selected app name + cached list.
    window_app: String,
    window_app_list: Vec<String>,
    window_list_scanned: bool,
    /// Last non-Vibecap frontmost app (so Fullscreen screenshots are not bare desktop).
    last_front_app: Option<String>,
    last_front_poll: Option<Instant>,
}

impl Default for AppTab {
    fn default() -> Self { AppTab::Capture }
}

impl Default for CaptureTarget {
    fn default() -> Self { CaptureTarget::Fullscreen }
}

impl VibecapApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        apply_graphite_theme(&cc.egui_ctx);

        // Hotkeys are best-effort: a second GUI may fail to claim them.
        // Ctrl+Shift+2 = record toggle · Ctrl+Shift+3 = screenshot
        let mut hotkey_id_record = 0u32;
        let mut hotkey_id_screenshot = 0u32;
        let (hotkey_manager, hotkey_receiver) = match GlobalHotKeyManager::new() {
            Ok(manager) => {
                let hk_rec = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Digit2);
                let hk_shot = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Digit3);
                hotkey_id_record = hk_rec.id();
                hotkey_id_screenshot = hk_shot.id();
                let _ = manager.register(hk_rec);
                let _ = manager.register(hk_shot);
                (Some(manager), Some(GlobalHotKeyEvent::receiver().clone()))
            }
            Err(_) => (None, None),
        };
        
        let default_dir = default_media_dir();

        let (screenshot_tx, screenshot_rx) = crossbeam_channel::unbounded();
        let (ffmpeg_tx, ffmpeg_rx) = crossbeam_channel::unbounded();

        let mut app = Self {
            current_tab: AppTab::Capture,
            capture_target: CaptureTarget::Fullscreen,
            capture_audio: false, // video-only by default; user can enable audio
            fps_target: 30,
            save_dir: default_dir,
            hotkey_receiver,
            hotkey_manager,
            hotkey_id_record,
            hotkey_id_screenshot,
            trim_start: "00:00:00".to_string(),
            trim_end: "00:00:05".to_string(),
            export_speed: "1.0".to_string(),
            current_tool: AnnotationTool::Pen,
            current_color: theme::ACCENT(),
            current_stroke_width: 3.0,
            pending_text: "Sample Text".to_string(),
            feedback_description: String::new(),
            feedback_choice: String::new(),
            step_counter: 1,
            screenshot_tx: Some(screenshot_tx),
            screenshot_rx: Some(screenshot_rx),
            ffmpeg_tx: Some(ffmpeg_tx),
            ffmpeg_rx: Some(ffmpeg_rx),
            library_filter: "All".to_string(),
            library_show_limit: LIBRARY_PAGE_SIZE,
            library_selected: std::collections::HashSet::new(),
            library_confirm_clear: false,
            budget_frames_input: "0".to_string(),
            budget_mb_input: "0.0".to_string(),
            budget_minutes_input: "0".to_string(),
            budget_tier: "standard".to_string(),
            img_resize_pct: 100,
            allow_exit: false,
            start_hidden: false,
            recording_arming: false,
            recording_cancel_armed: false,
            pending_arm_record: false,
            filmstrip_error: None,
            palette_open: false,
            palette_query: String::new(),
            palette_selected: 0,
            density: Density::Comfortable,
            undo_trash: None,
            capture_toast: None,
            ..Default::default() // wizard_* default closed / not done
        };

        let session = load_session();
        app.apply_session(session);
        // Re-apply visuals if session asked for light (graphite was applied above).
        apply_current_theme(&cc.egui_ctx);
        app.refresh_library();
        app
    }

    fn apply_session(&mut self, s: SessionState) {
        self.density = density_from_str(&s.density);
        if !s.library_filter.is_empty() {
            self.library_filter = s.library_filter;
        }
        self.current_tab = match s.tab.as_str() {
            "library" | "media" => AppTab::Library,
            "edit" | "studio" | "clip" => AppTab::Clip,
            "still" | "image" => AppTab::Still,
            "feedback" | "inbox" => AppTab::Feedback,
            "settings" => AppTab::Settings,
            _ => AppTab::Capture,
        };
        if let Some(p) = s.edit_file {
            let path = PathBuf::from(p);
            if path.exists() {
                self.edit_file = Some(path);
            }
        }
        self.wizard_done = s.wizard_done;
        self.wizard_open = !s.wizard_done;
        self.wizard_step = 0;
        theme::set_theme_mode(theme::theme_mode_from_str(&s.theme));
        self.last_region = s.last_region.map(|a| {
            Rect::from_min_max(Pos2::new(a[0], a[1]), Pos2::new(a[2], a[3]))
        });
        self.record_countdown_secs = match s.record_countdown_secs {
            3 | 5 => s.record_countdown_secs,
            _ => 0,
        };
    }

    pub(crate) fn persist_session(&self) {
        let tab = match self.current_tab {
            AppTab::Capture => "capture",
            AppTab::Library => "library",
            AppTab::Clip => "clip",
            AppTab::Still => "still",
            AppTab::Feedback => "feedback",
            AppTab::Settings => "settings",
        };
        let last_region = self.last_region.map(|r| {
            [r.min.x, r.min.y, r.max.x, r.max.y]
        });
        save_session(&SessionState {
            tab: tab.into(),
            edit_file: self.edit_file.as_ref().map(|p| p.display().to_string()),
            density: density_to_str(self.density).into(),
            library_filter: self.library_filter.clone(),
            window_w: 760.0,
            window_h: 640.0,
            wizard_done: self.wizard_done,
            theme: theme::theme_mode_to_str(theme::theme_mode()).into(),
            last_region,
            record_countdown_secs: self.record_countdown_secs,
        });
    }

    pub(crate) fn set_theme(&mut self, ctx: &egui::Context, mode: ThemeMode) {
        match mode {
            ThemeMode::Dark => apply_graphite_theme(ctx),
            ThemeMode::Light => theme::apply_light_theme(ctx),
        }
        self.persist_session();
    }

    pub(crate) fn dump_retro_buffer(&mut self) {
        match self.retro.dump_gif(&self.save_dir) {
            Ok(path) => {
                self.refresh_library();
                self.show_toast(format!(
                    "🎞 Retro GIF saved — {}",
                    path.file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string())
                ));
            }
            Err(e) => self.show_toast(format!("❌ Retro dump failed: {e}")),
        }
    }

    pub(crate) fn refresh_window_list(&mut self) {
        self.window_app_list = list_running_apps();
        self.window_list_scanned = true;
        if self.window_app.is_empty() {
            if let Some(first) = self.window_app_list.first() {
                self.window_app = first.clone();
            }
        } else if !self.window_app_list.iter().any(|a| a == &self.window_app)
            && !self.window_app_list.is_empty()
        {
            // Keep typed name even if not in list — user may have typed it.
        }
    }

    /// Track the app the user was in before focusing Vibecap (for Fullscreen capture).
    fn poll_frontmost_app(&mut self) {
        let due = self
            .last_front_poll
            .map(|t| t.elapsed() > Duration::from_millis(400))
            .unwrap_or(true);
        if !due {
            return;
        }
        self.last_front_poll = Some(Instant::now());
        if let Some(name) = frontmost_app_name() {
            let lower = name.to_ascii_lowercase();
            if lower != "vibecap" && !lower.contains("vibecap") {
                self.last_front_app = Some(name);
            }
        }
    }

    /// App to bring forward before Fullscreen / empty-Window capture.
    fn capture_focus_target(&self) -> Option<String> {
        match self.capture_target {
            CaptureTarget::Window if !self.window_app.trim().is_empty() => {
                Some(self.window_app.clone())
            }
            CaptureTarget::Window => self.last_front_app.clone(),
            CaptureTarget::Fullscreen => self.last_front_app.clone(),
            CaptureTarget::Region => None, // interactive selection
        }
    }

    /// Start recording, optionally after a countdown bubble.
    fn begin_recording(&mut self, ctx: &egui::Context) {
        if self.is_recording || self.recording_arming || self.countdown_deadline.is_some() {
            return;
        }
        if let Some(app) = self.capture_focus_target() {
            let _ = focus_app(&app);
            // Brief settle before hide+record arm
            std::thread::sleep(Duration::from_millis(200));
        }
        let secs = self.record_countdown_secs;
        if secs == 0 {
            self.arm_recording(ctx);
        } else {
            self.countdown_deadline =
                Some(Instant::now() + Duration::from_secs(secs as u64));
            self.show_window(ctx);
            ctx.request_repaint();
        }
    }

    /// One-shot bug pack: still + retro GIF (if buffer has frames).
    pub(crate) fn bug_report_pack(&mut self, ctx: &egui::Context) {
        let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
        let shot = self.save_dir.join(format!("bug_{}.jpg", stamp));
        let mut parts: Vec<String> = Vec::new();

        match capture_screenshot(&shot) {
            Ok(()) => {
                parts.push(
                    shot.file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| "screenshot".into()),
                );
                self.latest_screenshot = Some(shot.clone());
            }
            Err(e) => {
                self.show_toast(format!("❌ Bug report screenshot failed: {e}"));
                return;
            }
        }

        match self.retro.dump_gif(&self.save_dir) {
            Ok(gif) => {
                parts.push(
                    gif.file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| "retro.gif".into()),
                );
            }
            Err(_) => {
                // Retro empty / off — still is enough; hint to enable buffer next time.
                parts.push("(no retro — enable buffer for last-N GIF)".into());
            }
        }

        self.refresh_library();
        self.show_window(ctx);
        self.show_toast(format!("🐛 Bug pack saved — {}", parts.join(" · ")));
    }

    fn run_palette_action(&mut self, ctx: &egui::Context, action: PaletteAction) {
        match action {
            PaletteAction::GoShutter => self.current_tab = AppTab::Capture,
            PaletteAction::GoMedia => self.current_tab = AppTab::Library,
            PaletteAction::GoClip => self.current_tab = AppTab::Clip,
            PaletteAction::GoStill => self.current_tab = AppTab::Still,
            PaletteAction::GoInbox => self.current_tab = AppTab::Feedback,
            PaletteAction::GoSettings => self.current_tab = AppTab::Settings,
            PaletteAction::Screenshot => self.trigger_capture(ctx, true),
            PaletteAction::ToggleRecord => {
                if self.is_recording {
                    self.stop_recording(ctx);
                } else if self.recording_arming || self.countdown_deadline.is_some() {
                    self.cancel_recording(ctx);
                } else {
                    self.trigger_capture(ctx, false);
                }
            }
            PaletteAction::BugReport => {
                self.bug_report_pack(ctx);
            }
            PaletteAction::RefreshLibrary => {
                self.refresh_library();
                self.show_toast("Library refreshed");
            }
            PaletteAction::ToggleDensity => {
                self.density = match self.density {
                    Density::Comfortable => Density::Compact,
                    Density::Compact => Density::Comfortable,
                };
                self.persist_session();
                self.show_toast(format!(
                    "Density: {}",
                    density_to_str(self.density)
                ));
            }
            PaletteAction::ToggleTheme => {
                let next = match theme::theme_mode() {
                    ThemeMode::Dark => ThemeMode::Light,
                    ThemeMode::Light => ThemeMode::Dark,
                };
                self.set_theme(ctx, next);
                self.show_toast(format!(
                    "Theme: {}",
                    theme::theme_mode_to_str(next)
                ));
            }
            PaletteAction::ToggleRetro => {
                let on = !self.retro.config().enabled;
                self.retro.set_enabled(on);
                self.show_toast(if on {
                    "Retro buffer ON — rolling ~2 fps (off by default next launch if you disable)"
                } else {
                    "Retro buffer OFF — frames cleared"
                });
            }
            PaletteAction::SaveRetro => {
                self.dump_retro_buffer();
            }
            PaletteAction::OpenPaletteHelp => {
                self.show_toast("⌘K / Ctrl+K — type to filter, Enter to run");
            }
        }
        self.persist_session();
    }

    fn flush_expired_undo(&mut self) {
        if let Some((paths, at, trash_dir)) = self.undo_trash.take() {
            if at.elapsed() < Duration::from_secs(12) {
                self.undo_trash = Some((paths, at, trash_dir));
            } else {
                let _ = std::fs::remove_dir_all(trash_dir);
            }
        }
    }

    fn undo_last_delete(&mut self) {
        if let Some((paths, _, trash_dir)) = self.undo_trash.take() {
            let mut n = 0usize;
            for p in &paths {
                let name = p.file_name().map(|f| f.to_os_string());
                if let Some(name) = name {
                    let staged = trash_dir.join(&name);
                    if staged.exists() {
                        if std::fs::rename(&staged, p).is_ok() {
                            n += 1;
                        }
                    }
                }
            }
            let _ = std::fs::remove_dir_all(trash_dir);
            self.refresh_library();
            self.show_toast(format!("Undid delete ({n} file(s))"));
        } else {
            self.show_toast("Nothing to undo");
        }
    }

    fn show_window(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.request_repaint();
    }

    fn hide_to_tray(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        ctx.request_repaint();
    }

    fn handle_tray_actions(&mut self, ctx: &egui::Context) {
        let actions: Vec<TrayAction> = self
            .tray
            .as_ref()
            .map(|t| t.poll_actions())
            .unwrap_or_default();
        for action in actions {
            match action {
                TrayAction::Show => self.show_window(ctx),
                TrayAction::Hide => self.hide_to_tray(ctx),
                TrayAction::Screenshot => {
                    // Capture without forcing the main window up (tray-first workflow).
                    self.trigger_capture(ctx, true);
                }
                TrayAction::ToggleRecord => {
                    if self.is_recording {
                        self.stop_recording(ctx);
                    } else if self.recording_arming || self.countdown_deadline.is_some() {
                        self.cancel_recording(ctx);
                    } else {
                        self.trigger_capture(ctx, false);
                    }
                }
                TrayAction::GoShutter => {
                    self.current_tab = AppTab::Capture;
                    self.show_window(ctx);
                }
                TrayAction::GoMedia => {
                    self.current_tab = AppTab::Library;
                    self.refresh_library();
                    self.show_window(ctx);
                }
                TrayAction::GoClip => {
                    self.current_tab = AppTab::Clip;
                    self.show_window(ctx);
                }
                TrayAction::GoStill => {
                    self.current_tab = AppTab::Still;
                    self.show_window(ctx);
                }
                TrayAction::GoInbox => {
                    self.current_tab = AppTab::Feedback;
                    self.scan_feedback_requests();
                    self.show_window(ctx);
                }
                TrayAction::GoSettings => {
                    self.current_tab = AppTab::Settings;
                    self.show_window(ctx);
                }
                TrayAction::BugReport => {
                    self.show_window(ctx);
                    self.bug_report_pack(ctx);
                }
                TrayAction::Quit => {
                    self.allow_exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn sync_tray_recording_progress(&mut self) {
        let state = if self.is_recording {
            TrayLiveState::Recording {
                elapsed_secs: self.recording_elapsed_secs(),
            }
        } else if self.recording_arming || self.countdown_deadline.is_some() {
            TrayLiveState::Arming
        } else {
            TrayLiveState::Idle
        };
        let inbox = self.feedback_pending_count;
        if let Some(tray) = self.tray.as_mut() {
            tray.set_live_state(state, inbox);
        }
    }

    fn show_toast(&mut self, message: impl Into<String>) {
        let message = message.into();
        let level = ToastLevel::from_message(&message);
        self.toast_message = Some((message, Instant::now(), level));
    }

    fn toggle_voice_memo(&mut self) {
        if self.is_recording_voice_memo {
            if let Some(mut child) = self.voice_memo_child.take() {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(b"q\n");
                }
                let _ = child.wait();
            }
            self.is_recording_voice_memo = false;
            self.show_toast("🎙 Voice Note saved!");
            self.refresh_library();
        } else {
            let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
            let audio_file = self.save_dir.join(format!("voice_note_{}.m4a", timestamp));
            self.active_voice_memo_path = Some(audio_file.clone());

            match spawn_voice_memo(&audio_file) {
                Ok(c) => {
                    self.voice_memo_child = Some(c);
                    self.is_recording_voice_memo = true;
                    self.voice_memo_start = Some(Instant::now());
                    self.show_toast("🎙 Recording Voice Note... Speak now!");
                }
                Err(e) => self.show_toast(format!("🎙 Voice note failed: {}", e)),
            }
        }
    }

    fn refresh_library(&mut self) {
        self.library_selected.retain(|p| p.exists());
        self.library_items = scan_media_dir(&self.save_dir);
    }

    fn library_filtered(&self) -> Vec<&MediaItem> {
        filter_items(&self.library_items, &self.library_filter)
    }

    /// Chrome-only snapshot for the bottom status strip (no new backends).
    fn status_snapshot(&self) -> StatusSnapshot {
        let (bytes, count) = get_dir_size_bytes(&self.save_dir.display().to_string());
        let mb = bytes as f64 / (1024.0 * 1024.0);
        let storage_label = if mb >= 1024.0 {
            format!("{:.1} GB · {} files", mb / 1024.0, count)
        } else {
            format!("{:.0} MB · {} files", mb, count)
        };

        let cfg = load_budget();
        let live = default_live_dir().display().to_string();
        let (frames, live_mb, _) = live_usage_snapshot(&live);
        let frames_cap = if cfg.max_frames == 0 {
            "∞".into()
        } else {
            cfg.max_frames.to_string()
        };
        let budget_usage = format!("{frames}/{frames_cap} fr · {live_mb:.1} MB live");
        let budget_tier = format!("{} tier", cfg.analysis_tier);

        let ffmpeg_ok = platform::ffmpeg_available();

        let rec_live = self.is_recording || self.recording_arming;
        let rec_label = if self.is_recording {
            let e = self.recording_elapsed_secs();
            format!("REC {:02}:{:02}", e / 60, e % 60)
        } else if self.recording_arming {
            "Starting…".into()
        } else {
            String::new()
        };

        StatusSnapshot {
            storage_label,
            budget_tier,
            budget_usage,
            ffmpeg_ok,
            pending_inbox: self.feedback_pending_count,
            rec_live,
            rec_label,
        }
    }

    fn delete_library_paths(&mut self, paths: &[PathBuf]) {
        if paths.is_empty() {
            return;
        }
        // Stage into undo trash (12s window) instead of hard-delete only.
        let trash_root = vibecap_config_dir().join("undo_trash");
        let stamp = Local::now().format("%Y%m%d_%H%M%S%3f").to_string();
        let trash_dir = trash_root.join(&stamp);
        let _ = std::fs::create_dir_all(&trash_dir);
        // Drop any previous staging
        if let Some((_, _, old)) = self.undo_trash.take() {
            let _ = std::fs::remove_dir_all(old);
        }

        let mut staged = Vec::new();
        let mut n = 0usize;
        for p in paths {
            let name = match p.file_name() {
                Some(n) => n.to_os_string(),
                None => continue,
            };
            let dest = trash_dir.join(&name);
            if std::fs::rename(p, &dest).is_ok() || (std::fs::copy(p, &dest).is_ok() && std::fs::remove_file(p).is_ok()) {
                staged.push(p.clone());
                n += 1;
                self.library_selected.remove(p);
            }
        }
        if n > 0 {
            self.undo_trash = Some((staged, Instant::now(), trash_dir));
            self.refresh_library();
            self.show_toast(format!("Deleted {n} file(s) — press Z to undo"));
        } else {
            let _ = std::fs::remove_dir_all(trash_dir);
            self.show_toast("Could not delete file(s)");
        }
    }

    fn reveal_paths(&mut self, paths: &[PathBuf]) {
        if paths.is_empty() {
            self.show_toast("Nothing selected");
            return;
        }
        let mut ok = 0usize;
        let mut last_err = String::new();
        for p in paths {
            match reveal_in_file_manager(p) {
                Ok(()) => ok += 1,
                Err(e) => last_err = e,
            }
        }
        if ok > 0 {
            self.show_toast(format!("Opened {} in Finder", ok));
        } else {
            self.show_toast(format!("Finder reveal failed: {}", last_err));
        }
    }

    fn scan_feedback_requests(&mut self) {
        self.feedback_requests.clear();
        if let Ok(entries) = std::fs::read_dir(feedback_requests_dir()) {
            for entry in entries.flatten() {
                if let Ok(s) = std::fs::read_to_string(entry.path()) {
                    if let Ok(req) = serde_json::from_str::<FeedbackRequest>(&s) {
                        self.feedback_requests.push(req);
                    }
                }
            }
        }
        // High priority first, then newest.
        self.feedback_requests.sort_by(|a, b| {
            let rank = |p: &str| match p {
                "high" => 0,
                "low" => 2,
                _ => 1,
            };
            rank(a.priority.as_str())
                .cmp(&rank(b.priority.as_str()))
                .then_with(|| b.created_at.cmp(&a.created_at))
        });
    }

    /// Detect newly pending agent questions and make them unmissable:
    /// OS notification · Dock bounce · tray title · toast · open Inbox.
    fn surface_new_feedback(&mut self, ctx: &egui::Context) {
        let pending: Vec<FeedbackRequest> = self
            .feedback_requests
            .iter()
            .filter(|r| r.status == "pending")
            .cloned()
            .collect();
        let pending_ids: std::collections::HashSet<String> =
            pending.iter().map(|r| r.id.clone()).collect();

        // Drop ids that are no longer pending so a re-ask of the same id can fire again.
        self.feedback_notified_ids
            .retain(|id| pending_ids.contains(id));

        let mut new_ones: Vec<FeedbackRequest> = pending
            .into_iter()
            .filter(|r| !self.feedback_notified_ids.contains(&r.id))
            .collect();
        if new_ones.is_empty() {
            self.feedback_pending_count = pending_ids.len();
            return;
        }

        for r in &new_ones {
            self.feedback_notified_ids.insert(r.id.clone());
            notify_agent_question(&r.agent_label, &r.question, &r.priority);
        }

        // Prefer highest-priority (already sorted: high first).
        new_ones.sort_by(|a, b| {
            let rank = |p: &str| match p {
                "high" => 0,
                "low" => 2,
                _ => 1,
            };
            rank(a.priority.as_str())
                .cmp(&rank(b.priority.as_str()))
                .then_with(|| b.created_at.cmp(&a.created_at))
        });
        let first = &new_ones[0];
        let agent = if first.agent_label.trim().is_empty() {
            "Agent"
        } else {
            first.agent_label.trim()
        };
        let q: String = first
            .question
            .chars()
            .take(90)
            .collect();
        let more = if new_ones.len() > 1 {
            format!(" (+{} more)", new_ones.len() - 1)
        } else {
            String::new()
        };
        self.show_toast(format!("🤖 {agent} asks: {q}{more} — open Inbox"));

        // Bounce Dock / taskbar even when the window is hidden.
        ctx.send_viewport_cmd(ViewportCommand::RequestUserAttention(
            UserAttentionType::Critical,
        ));

        // Open the Inbox on the first new question so the loop feels connected.
        self.current_tab = AppTab::Feedback;
        self.feedback_selected = Some(first.id.clone());
        self.feedback_draft.clear();
        self.feedback_choice.clear();
        self.show_window(ctx);

        self.feedback_pending_count = pending_ids.len();
        // Force tray title refresh immediately (don't wait for next tick).
        let live = if self.is_recording {
            TrayLiveState::Recording {
                elapsed_secs: self.recording_elapsed_secs(),
            }
        } else if self.recording_arming || self.countdown_deadline.is_some() {
            TrayLiveState::Arming
        } else {
            TrayLiveState::Idle
        };
        let inbox_n = self.feedback_pending_count;
        if let Some(tray) = self.tray.as_mut() {
            // Reset debounce so Idle+Inbox title always applies.
            tray.force_live_state(live, inbox_n);
        }
    }

    fn mark_feedback_status(&self, request_id: &str, status: &str) {
        let req_path = feedback_requests_dir().join(format!("{}.json", request_id));
        if let Ok(s) = std::fs::read_to_string(&req_path) {
            if let Ok(mut req) = serde_json::from_str::<FeedbackRequest>(&s) {
                req.status = status.to_string();
                if let Ok(s2) = serde_json::to_string_pretty(&req) {
                    let _ = write_json_atomic(&req_path, &s2);
                }
            }
        }
    }

    fn submit_feedback_response(&mut self, request_id: &str) {
        let choice = self.feedback_choice.trim().to_string();
        let text = self.feedback_draft.trim().to_string();
        let voice = self
            .feedback_voice_note
            .take()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        if text.is_empty() && choice.is_empty() && voice.is_empty() {
            self.show_toast("Add a reply, pick a choice, or attach a voice note first.");
            return;
        }
        let response = FeedbackResponse {
            id: request_id.to_string(),
            feedback_text: text,
            voice_note_path: voice,
            annotated_media_path: String::new(),
            answered_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            selected_option: choice,
        };
        let resp_path = feedback_responses_dir().join(format!("{}.json", request_id));
        let saved = serde_json::to_string_pretty(&response)
            .ok()
            .and_then(|s| write_json_atomic(&resp_path, &s).ok());
        if saved.is_none() {
            self.show_toast("❌ Could not save feedback — check disk permissions.");
            return;
        }
        self.mark_feedback_status(request_id, "answered");
        self.feedback_draft.clear();
        self.feedback_choice.clear();
        self.feedback_selected = None;
        self.scan_feedback_requests();
        self.show_toast("✅ Feedback submitted — the agent can pick it up now!");
    }

    fn dismiss_feedback_request(&mut self, request_id: &str) {
        let response = FeedbackResponse {
            id: request_id.to_string(),
            feedback_text: String::new(),
            voice_note_path: String::new(),
            annotated_media_path: String::new(),
            answered_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            selected_option: "dismissed".to_string(),
        };
        let resp_path = feedback_responses_dir().join(format!("{}.json", request_id));
        if serde_json::to_string_pretty(&response)
            .ok()
            .and_then(|s| write_json_atomic(&resp_path, &s).ok())
            .is_some()
        {
            self.mark_feedback_status(request_id, "dismissed");
            self.feedback_selected = None;
            self.feedback_choice.clear();
            self.feedback_draft.clear();
            self.scan_feedback_requests();
            self.show_toast("Dismissed — agent will see choice=dismissed on poll.");
        } else {
            self.show_toast("❌ Could not dismiss request.");
        }
    }

    fn clear_answered_feedback(&mut self) {
        let answered: Vec<String> = self
            .feedback_requests
            .iter()
            .filter(|r| r.status != "pending")
            .map(|r| r.id.clone())
            .collect();
        for id in &answered {
            let _ = std::fs::remove_file(feedback_requests_dir().join(format!("{}.json", id)));
            let _ = std::fs::remove_file(feedback_responses_dir().join(format!("{}.json", id)));
        }
        if !answered.is_empty() {
            self.show_toast("🧹 Cleared closed requests");
        }
        self.scan_feedback_requests();
    }

    fn compute_edited_image(&self) -> Result<image::DynamicImage, String> {
        let path = self.img_edit_file.clone().ok_or("No image selected")?;
        let mut img = image::open(&path).map_err(|e| format!("Could not open image: {}", e))?;
        if img.width() as u64 * img.height() as u64 > 50_000_000 {
            return Err("Image too large (>50 MP) — refusing to edit.".to_string());
        }
        let any_crop = !self.img_crop_x.trim().is_empty() || !self.img_crop_y.trim().is_empty()
            || !self.img_crop_w.trim().is_empty() || !self.img_crop_h.trim().is_empty();
        if any_crop {
            let (cx, cy, cw, ch) = (
                self.img_crop_x.trim().parse::<u32>().unwrap_or(0),
                self.img_crop_y.trim().parse::<u32>().unwrap_or(0),
                self.img_crop_w.trim().parse::<u32>().unwrap_or(0),
                self.img_crop_h.trim().parse::<u32>().unwrap_or(0),
            );
            if cw == 0 || ch == 0
                || (cx as u64 + cw as u64) > img.width() as u64
                || (cy as u64 + ch as u64) > img.height() as u64
            {
                return Err("Crop exceeds image bounds — nothing was cropped.".to_string());
            }
            img = img.crop_imm(cx, cy, cw, ch);
        }
        img = match self.img_rotate {
            90 => img.rotate90(),
            180 => img.rotate180(),
            270 => img.rotate270(),
            _ => img,
        };
        if self.img_flip_h { img = img.fliph(); }
        if self.img_flip_v { img = img.flipv(); }
        if self.img_resize_pct != 100 && self.img_resize_pct > 0 {
            let w = (img.width() as f32 * self.img_resize_pct as f32 / 100.0).max(1.0) as u32;
            let h = (img.height() as f32 * self.img_resize_pct as f32 / 100.0).max(1.0) as u32;
            img = img.resize(w, h, image::imageops::FilterType::Triangle);
        }
        if self.img_grayscale { img = img.grayscale(); }
        if self.img_brightness != 0 { img = img.brighten(self.img_brightness); }
        if self.img_contrast != 0.0 { img = img.adjust_contrast(self.img_contrast); }
        if self.img_blur > 0.05 { img = img.blur(self.img_blur); }
        Ok(img)
    }

    fn apply_image_edits(&mut self) {
        let Some(path) = self.img_edit_file.clone() else { return; };
        match self.compute_edited_image() {
            Ok(img) => {
                let out = path.with_file_name(format!("edited_{}", path.file_name().unwrap().to_str().unwrap()));
                match img.save(&out) {
                    Ok(_) => {
                        self.show_toast("🖼 Edited image saved!");
                        self.refresh_library();
                    }
                    Err(e) => self.show_toast(&format!("❌ Save failed: {}", e)),
                }
            }
            Err(msg) => self.show_toast(&format!("❌ {}", msg)),
        }
    }

    fn refresh_img_preview(&mut self, ctx: &egui::Context) {
        let params = format!("{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.img_rotate, self.img_flip_h, self.img_flip_v, self.img_grayscale,
            self.img_brightness, self.img_contrast, self.img_blur, self.img_resize_pct,
            self.img_crop_x, self.img_crop_y, self.img_crop_w, self.img_crop_h);
        if params == self.img_preview_params || self.img_edit_file.is_none() {
            return;
        }
        self.img_preview_params = params;
        if let Ok(img) = self.compute_edited_image() {
            let preview = img.resize(640, 480, image::imageops::FilterType::Triangle);
            let size = [preview.width() as _, preview.height() as _];
            let buf = preview.to_rgba8();
            let pixels = buf.as_flat_samples();
            let ci = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            self.img_preview_tex = Some(ctx.load_texture("img_preview", ci, Default::default()));
        }
    }

    /// Runs ffmpeg on a background thread and reports the REAL outcome via channel —
    /// success toasts only fire after a verified exit status (no fabricated success).
    fn spawn_ffmpeg_job(&mut self, args: Vec<String>, ok_msg: &str) {
        let Some(tx) = self.ffmpeg_tx.clone() else { return; };
        let ok_msg = ok_msg.to_string();
        std::thread::spawn(move || {
            let (ok, msg) = match platform::ffmpeg_command() {
                Err(e) => (false, format!("❌ {e}")),
                Ok(mut cmd) => {
                    let result = cmd
                        .args(&args)
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::piped())
                        .output();
                    match result {
                        Ok(out) if out.status.success() => (true, ok_msg),
                        Ok(out) => {
                            let err = String::from_utf8_lossy(&out.stderr);
                            let tail = err
                                .lines()
                                .last()
                                .unwrap_or("unknown ffmpeg error")
                                .trim()
                                .to_string();
                            (false, format!("❌ ffmpeg failed: {}", tail))
                        }
                        Err(e) => (false, format!("❌ could not start ffmpeg: {}", e)),
                    }
                }
            };
            let _ = tx.send((ok, msg));
        });
    }

    fn drain_ffmpeg_results(&mut self) {
        let mut msgs = Vec::new();
        if let Some(rx) = &self.ffmpeg_rx {
            while let Ok(m) = rx.try_recv() {
                msgs.push(m);
            }
        }
        for (ok, msg) in msgs {
            self.show_toast(&msg);
            if ok {
                self.refresh_library();
            }
        }
    }

    /// Load any image file into the Annotation Studio (used by 📸 screenshots and ✏ Annotate & Reply).
    fn annotate_media(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.latest_screenshot = Some(path.clone());
        self.is_annotating = true;
        if let Ok(img) = image::open(&path) {
            let size = [img.width() as _, img.height() as _];
            let image_buffer = img.to_rgba8();
            let pixels = image_buffer.as_flat_samples();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            self.annotation_texture = Some(ctx.load_texture("screenshot", color_image, Default::default()));
        }
        self.annotation_actions.clear();
        self.step_counter = 1;
    }

    /// Handles the ViewportCommand::Screenshot reply: crops to the annotation canvas and saves it,
    /// producing a flattened image with annotations baked in.
    fn check_annotated_save(&mut self, ctx: &egui::Context) {
        let Some((target, requested_at)) = self.pending_annotated_save.clone() else { return; };
        let mut found: Option<std::sync::Arc<egui::ColorImage>> = None;
        ctx.input(|i| {
            for ev in &i.events {
                if let egui::Event::Screenshot { image, .. } = ev {
                    found = Some(image.clone());
                }
            }
        });
        if found.is_none() {
            if requested_at.elapsed() > Duration::from_millis(1500) {
                if requested_at.elapsed() > Duration::from_secs(5) {
                    self.pending_annotated_save = None;
                    self.show_toast("❌ Timed out capturing annotated image");
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
                }
            }
            return;
        }
        self.pending_annotated_save = None;
        let Some(img) = found else { return; };

        let (w, h) = (img.width(), img.height());
        let raw: Vec<u8> = img.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b(), c.a()]).collect();
        let mut dynimg = match image::RgbaImage::from_raw(w as u32, h as u32, raw) {
            Some(r) => image::DynamicImage::ImageRgba8(r),
            None => {
                self.show_toast("❌ Could not decode annotated capture");
                return;
            }
        };
        // Crop the full window capture down to the annotation canvas rect.
        if let Some(rect) = self.annotation_canvas_rect {
            let screen = ctx.screen_rect();
            let sx = w as f32 / screen.width().max(1.0);
            let sy = h as f32 / screen.height().max(1.0);
            let cx = (rect.min.x * sx).max(0.0) as u32;
            let cy = (rect.min.y * sy).max(0.0) as u32;
            let cw = ((rect.width() * sx) as u32).min(dynimg.width().saturating_sub(cx));
            let ch = ((rect.height() * sy) as u32).min(dynimg.height().saturating_sub(cy));
            if cw > 0 && ch > 0 {
                dynimg = dynimg.crop_imm(cx, cy, cw, ch);
            }
        }
        match dynimg.save(&target) {
            Ok(_) => self.show_toast("🎨 Annotated image saved!"),
            Err(e) => self.show_toast(&format!("❌ Could not save annotated image: {}", e)),
        }
    }

    fn copy_image_to_clipboard(&mut self, path: &PathBuf) {
        if let Ok(img) = image::open(path) {
            let rgba = img.to_rgba8();
            let (w, h) = (img.width() as usize, img.height() as usize);
            if let Ok(mut board) = arboard::Clipboard::new() {
                let img_data = arboard::ImageData {
                    width: w,
                    height: h,
                    bytes: std::borrow::Cow::Borrowed(rgba.as_raw()),
                };
                if board.set_image(img_data).is_ok() {
                    self.show_toast("📋 Image copied to system clipboard!");
                }
            }
        }
    }

    fn recording_elapsed_secs(&self) -> u64 {
        let current = self.segment_start.map(|s| s.elapsed()).unwrap_or(Duration::ZERO);
        (self.accumulated_duration + current).as_secs()
    }

    fn toggle_pause(&mut self) {
        if !self.is_recording { return; }
        if let Some(child) = &self.child_process {
            let pid = child.id();
            if self.is_paused {
                cont_process(pid);
                self.segment_start = Some(Instant::now());
                self.is_paused = false;
            } else {
                if let Some(start) = self.segment_start.take() {
                    self.accumulated_duration += start.elapsed();
                }
                stop_process(pid);
                self.is_paused = true;
            }
        }
    }

    fn cancel_recording(&mut self, ctx: &egui::Context) {
        if self.countdown_deadline.take().is_some() {
            self.show_window(ctx);
            self.show_toast("❌ Countdown cancelled");
            return;
        }

        if self.recording_arming {
            self.recording_cancel_armed = true;
            self.recording_arming = false;
            // Drop receiver; worker may still finish — drain_record_spawn kills it.
            self.record_spawn_rx = None;
            self.show_window(ctx);
            self.show_toast("❌ Recording cancelled");
            return;
        }

        if let Some(child) = self.child_process.take() {
            kill_recorder(child, self.is_paused);
        }

        if let Some(file) = self.current_mp4_file.take() {
            let _ = std::fs::remove_file(file);
        }

        self.is_recording = false;
        self.is_paused = false;
        self.accumulated_duration = Duration::ZERO;
        self.segment_start = None;
        self.recording_arming = false;
        self.recording_cancel_armed = false;

        self.show_window(ctx);
        self.show_toast("❌ Recording cancelled");
    }

    fn stop_recording(&mut self, ctx: &egui::Context) {
        if self.recording_arming {
            // Nothing to save yet — treat as cancel.
            self.cancel_recording(ctx);
            return;
        }

        if let Some(mut child) = self.child_process.take() {
            if self.is_paused {
                cont_process(child.id());
            }
            let _ = finalize_recorder(child);
        }
        self.is_recording = false;
        self.is_paused = false;
        self.accumulated_duration = Duration::ZERO;
        self.segment_start = None;
        self.recording_arming = false;

        // Always surface the main window (Visible + unminimize) so Editor is usable after tray/hidden rec.
        self.show_window(ctx);

        if let Some(mp4) = self.current_mp4_file.clone() {
            // Brief settle so the filesystem sees a complete file.
            if !mp4.exists() || std::fs::metadata(&mp4).map(|m| m.len()).unwrap_or(0) < 512 {
                std::thread::sleep(Duration::from_millis(150));
            }
            let bytes = std::fs::metadata(&mp4).map(|m| m.len()).unwrap_or(0);
            self.edit_file = Some(mp4.clone());
            self.current_tab = AppTab::Clip;
            self.load_filmstrip(ctx, mp4.clone());
            self.refresh_library();
            if bytes < 512 {
                self.show_toast(format!(
                    "⚠️ Saved {} but file looks empty ({bytes} bytes) — check Screen Recording permission.",
                    mp4.file_name().and_then(|n| n.to_str()).unwrap_or("video")
                ));
            } else {
                self.show_toast(format!(
                    "💾 Video saved — open in Clip · {}",
                    mp4.file_name().and_then(|n| n.to_str()).unwrap_or("video.mp4")
                ));
            }
        } else {
            self.refresh_library();
            self.show_toast("⚠️ Stopped but no video path was set.");
        }
    }

    fn load_filmstrip(&mut self, ctx: &egui::Context, file: PathBuf) {
        self.filmstrip.clear();
        self.filmstrip_error = None;
        self.filmstrip_loading = true;

        match extract_filmstrip_thumbs(&file) {
            Ok((_out_dir, thumbs)) => {
                let file_s = file.display().to_string();
                for (i, thumb_path) in thumbs.iter().enumerate() {
                    if let Ok(img) = image::open(thumb_path) {
                        let size = [img.width() as _, img.height() as _];
                        let image_buffer = img.to_rgba8();
                        let pixels = image_buffer.as_flat_samples();
                        let color_image =
                            egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                        let tex = ctx.load_texture(
                            format!("thumb_{}_{}", i + 1, file_s),
                            color_image,
                            Default::default(),
                        );
                        self.filmstrip.push(tex);
                    }
                    let _ = std::fs::remove_file(thumb_path);
                }
                if self.filmstrip.is_empty() {
                    self.filmstrip_error = Some(
                        "No frames extracted — video may be corrupt or too short.".into(),
                    );
                }
            }
            Err(e) => {
                self.filmstrip_error = Some(e);
            }
        }
        self.filmstrip_loading = false;
    }
    fn arm_recording(&mut self, ctx: &egui::Context) {
        if self.is_recording || self.recording_arming {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let mp4_file = self.save_dir.join(format!("video_{}.mp4", timestamp));
        let fps = self.fps_target.max(1);
        let with_audio = self.capture_audio;
        let crop = if self.capture_target == CaptureTarget::Region {
            self.selected_region.map(|rect| {
                even_crop(
                    rect.width() as i32,
                    rect.height() as i32,
                    rect.min.x as i32,
                    rect.min.y as i32,
                )
            })
        } else {
            None
        };

        let (tx, rx) = crossbeam_channel::bounded(1);
        self.record_spawn_rx = Some(rx);
        self.recording_arming = true;
        self.recording_cancel_armed = false;
        self.current_mp4_file = Some(mp4_file.clone());

        // Hide main window so it is not painted into the capture. Prefer Visible(false)
        // over Minimized — minimized windows often stop receiving update ticks on macOS.
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        ctx.request_repaint();

        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            // Let the compositor hide our UI before avfoundation/gdigrab starts.
            std::thread::sleep(Duration::from_millis(350));
            let result = spawn_screen_recorder(&mp4_file, fps, with_audio, crop)
                .map(|child| (child, mp4_file));
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }

    fn drain_record_spawn(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.record_spawn_rx.as_ref() else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        self.record_spawn_rx = None;

        if self.recording_cancel_armed {
            self.recording_cancel_armed = false;
            self.recording_arming = false;
            if let Ok((mut child, path)) = result {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(path);
            }
            self.current_mp4_file = None;
            return;
        }

        self.recording_arming = false;
        match result {
            Ok((child, path)) => {
                self.child_process = Some(child);
                self.current_mp4_file = Some(path);
                self.is_recording = true;
                self.is_paused = false;
                self.accumulated_duration = Duration::ZERO;
                self.segment_start = Some(Instant::now());
                // Keep main hidden; floating REC bar is the control surface.
                ctx.request_repaint();
            }
            Err(e) => {
                self.current_mp4_file = None;
                self.show_window(ctx);
                self.show_toast(format!("❌ Record failed: {e}"));
            }
        }
    }

    fn trigger_capture(&mut self, ctx: &egui::Context, is_screenshot: bool) {
        if !is_screenshot {
            if self.capture_target == CaptureTarget::Region {
                self.selected_region = None;
                self.is_selecting_region = true;
                self.show_window(ctx);
                return;
            }
            self.begin_recording(ctx);
            return;
        }

        // Screenshot: re-focus the user's previous app (or Window picker), hide Vibecap,
        // capture, then UI reopens when the worker returns a path.
        let focus_target = self.capture_focus_target();
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));

        let ctx_clone = ctx.clone();
        let capture_target = self.capture_target;
        let save_dir = self.save_dir.clone();
        let screenshot_tx = self.screenshot_tx.clone().unwrap();

        std::thread::spawn(move || {
            if let Some(app) = focus_target {
                let _ = focus_app(&app);
                std::thread::sleep(Duration::from_millis(500));
            } else {
                // No prior app known — wait for compositor hide only.
                std::thread::sleep(Duration::from_millis(400));
            }
            let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
            let shot_file = save_dir.join(format!("screenshot_{}.jpg", timestamp));
            // Region uses interactive selection; Fullscreen/Window capture full display
            // after focusing the target app (avoids empty desktop).
            let interactive = matches!(capture_target, CaptureTarget::Region);
            let result = capture_screenshot_interactive(&shot_file, interactive)
                .map(|_| shot_file);
            let _ = screenshot_tx.send(result);
            ctx_clone.request_repaint();
        });
        ctx.request_repaint();
    }

    fn show_annotation(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Annotation Studio").color(theme::ACCENT()).strong());
            ui.separator();
            ui.radio_value(&mut self.current_tool, AnnotationTool::Pen, "✏ Pen");
            ui.radio_value(&mut self.current_tool, AnnotationTool::Arrow, "➡ Arrow");
            ui.radio_value(&mut self.current_tool, AnnotationTool::Rectangle, "🔲 Rect");
            ui.radio_value(&mut self.current_tool, AnnotationTool::Highlight, "🖍 Highlight");
            ui.radio_value(&mut self.current_tool, AnnotationTool::Text, "🔤 Text");
            ui.radio_value(&mut self.current_tool, AnnotationTool::Blur, "💧 Blur");
            ui.radio_value(&mut self.current_tool, AnnotationTool::StepBadge, "🔢 Badge");
            
            ui.separator();
            ui.color_edit_button_srgba(&mut self.current_color);
            ui.add(egui::Slider::new(&mut self.current_stroke_width, 1.0..=10.0).text("Size"));
            
            if self.current_tool == AnnotationTool::Text {
                ui.separator();
                ui.label("Text:");
                ui.text_edit_singleline(&mut self.pending_text);
            }
            
            ui.separator();
            if ui.button("↩ Undo").clicked() {
                self.annotation_actions.pop();
            }
            if ui.button("🗑 Clear").clicked() {
                self.annotation_actions.clear();
                self.step_counter = 1;
            }
            
            ui.separator();
            let voice_btn_text = if self.is_recording_voice_memo {
                RichText::new("🔴 Stop Voice Note").color(theme::ON_SOLID()).strong()
            } else {
                RichText::new("🎙 Voice Note").color(theme::SUCCESS()).strong()
            };
            if ui.button(voice_btn_text).clicked() {
                self.toggle_voice_memo();
            }

            if let Some(shot) = &self.latest_screenshot {
                let shot_clone = shot.clone();
                if ui.button("📋 Copy").clicked() {
                    self.copy_image_to_clipboard(&shot_clone);
                }
            }
            
            if ui.button(RichText::new("💾 Save & Close").color(theme::ACCENT_INK()).strong()).clicked() {
                if !self.feedback_description.trim().is_empty() {
                    if let Some(shot) = &self.latest_screenshot {
                        let txt_path = shot.with_extension("txt");
                        let _ = std::fs::write(&txt_path, &self.feedback_description);
                    }
                }
                // Bake annotations into a flattened *_annotated.png next to the source image.
                let mut annotated_path = String::new();
                if !self.annotation_actions.is_empty() {
                    if let Some(shot) = self.latest_screenshot.clone() {
                        let stem = shot.file_stem().unwrap_or_default().to_string_lossy().to_string();
                        let target = shot.with_file_name(format!("{}_annotated.png", stem));
                        annotated_path = target.display().to_string();
                        self.pending_annotated_save = Some((target, Instant::now()));
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Screenshot);
                    }
                }
                // If this annotation answered an agent's feedback request, submit it as the response.
                if let Some(fid) = self.annotating_feedback_id.take() {
                    let resp = FeedbackResponse {
                        id: fid.clone(),
                        feedback_text: self.feedback_description.trim().to_string(),
                        voice_note_path: self
                            .feedback_voice_note
                            .take()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default(),
                        annotated_media_path: annotated_path,
                        answered_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        selected_option: self.feedback_choice.trim().to_string(),
                    };
                    let resp_path = feedback_responses_dir().join(format!("{}.json", fid));
                    let saved = serde_json::to_string_pretty(&resp)
                        .ok()
                        .and_then(|s| write_json_atomic(&resp_path, &s).ok());
                    if saved.is_some() {
                        self.mark_feedback_status(&fid, "answered");
                        self.show_toast("✅ Annotated feedback submitted to the agent!");
                    } else {
                        self.show_toast("❌ Could not save feedback — check disk permissions.");
                    }
                    self.feedback_draft.clear();
                    self.feedback_choice.clear();
                    self.scan_feedback_requests();
                } else {
                    self.show_toast("Saved feedback note & annotations!");
                }
                self.is_annotating = false;
                self.refresh_library();
            }
        });

        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Optional note to attach with this capture:").small().color(theme::TEXT_MUTED()));
            ui.text_edit_singleline(&mut self.feedback_description);
        });
        ui.separator();
        
        if let Some(tex) = &self.annotation_texture {
            let max_size = ui.available_size();
            let mut tex_size = tex.size_vec2();
            if tex_size.x > max_size.x {
                tex_size = tex_size * (max_size.x / tex_size.x);
            }
            if tex_size.y > max_size.y {
                tex_size = tex_size * (max_size.y / tex_size.y);
            }
            
            let (response, painter) = ui.allocate_painter(tex_size, egui::Sense::drag());
            self.annotation_canvas_rect = Some(response.rect);
            painter.image(
                tex.id(),
                response.rect,
                egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                theme::ON_SOLID()
            );

            let draw_action = |painter: &egui::Painter, action: &AnnotationAction| {
                if action.points.is_empty() { return; }
                let mut color = action.color;
                if action.tool == AnnotationTool::Highlight {
                    color = color.linear_multiply(0.4);
                }
                let stroke = Stroke::new(action.stroke_width, color);
                
                match action.tool {
                    AnnotationTool::Pen | AnnotationTool::Highlight => {
                        for i in 1..action.points.len() {
                            painter.line_segment([action.points[i-1], action.points[i]], stroke);
                        }
                    }
                    AnnotationTool::Arrow => {
                        if action.points.len() >= 2 {
                            let start = action.points[0];
                            let end = *action.points.last().unwrap();
                            painter.arrow(start, end - start, stroke);
                        }
                    }
                    AnnotationTool::Rectangle => {
                        if action.points.len() >= 2 {
                            let start = action.points[0];
                            let end = *action.points.last().unwrap();
                            let rect = Rect::from_two_pos(start, end);
                            painter.rect_stroke(rect, 0.0, stroke);
                        }
                    }
                    AnnotationTool::Blur => {
                        if action.points.len() >= 2 {
                            let start = action.points[0];
                            let end = *action.points.last().unwrap();
                            let rect = Rect::from_two_pos(start, end);
                            painter.rect_filled(rect, 0.0, theme::OVERLAY_BLUR());
                            painter.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, theme::NEUTRAL_STROKE()));
                        }
                    }
                    AnnotationTool::Text => {
                        let pos = action.points[0];
                        painter.rect_filled(Rect::from_min_size(pos - Vec2::new(4.0, 2.0), Vec2::new(action.text_content.len() as f32 * 10.0 + 8.0, 22.0)), 4.0, theme::OVERLAY_LABEL());
                        painter.text(pos, Align2::LEFT_TOP, &action.text_content, FontId::proportional(16.0), action.color);
                    }
                    AnnotationTool::StepBadge => {
                        let pos = action.points[0];
                        painter.circle_filled(pos, 14.0, action.color);
                        painter.text(pos, Align2::CENTER_CENTER, action.badge_number.to_string(), FontId::proportional(14.0), theme::ACCENT_INK());
                    }
                }
            };

            for action in &self.annotation_actions {
                draw_action(&painter, action);
            }
            
            if let Some(action) = &self.current_action {
                draw_action(&painter, action);
            }

            if response.drag_started() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let action = AnnotationAction {
                        tool: self.current_tool,
                        color: self.current_color,
                        stroke_width: self.current_stroke_width,
                        points: vec![pos],
                        text_content: self.pending_text.clone(),
                        badge_number: self.step_counter,
                    };
                    
                    if self.current_tool == AnnotationTool::Text || self.current_tool == AnnotationTool::StepBadge {
                        if self.current_tool == AnnotationTool::StepBadge {
                            self.step_counter += 1;
                        }
                        self.annotation_actions.push(action);
                    } else {
                        self.current_action = Some(action);
                    }
                }
            }
            if response.dragged() {
                if let Some(pos) = response.interact_pointer_pos() {
                    if let Some(action) = &mut self.current_action {
                        action.points.push(pos);
                    }
                }
            }
            if response.drag_stopped() {
                if let Some(action) = self.current_action.take() {
                    self.annotation_actions.push(action);
                }
            }
        }
    }
}

impl eframe::App for VibecapApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // First frame: honor --hidden (window already created; hide after paint setup).
        if self.start_hidden {
            self.start_hidden = false;
            self.hide_to_tray(ctx);
        }

        // Close button → hide to tray (multi-agent + human: keep app alive in menu bar).
        // Tray "Quit" sets allow_exit so the process can terminate.
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.tray.is_some() && !self.allow_exit {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.hide_to_tray(ctx);
                self.show_toast("Hidden to tray — click the menu bar icon to show again.");
            }
        }

        self.handle_tray_actions(ctx);
        self.poll_frontmost_app();
        self.drain_record_spawn(ctx);
        if self.pending_arm_record {
            self.pending_arm_record = false;
            self.begin_recording(ctx);
        }

        // Pre-record countdown bubble
        if let Some(deadline) = self.countdown_deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.countdown_deadline = None;
                self.arm_recording(ctx);
            } else {
                let secs_left = remaining.as_secs().saturating_add(1) as u32;
                if show_countdown_bubble(ctx, secs_left) {
                    self.countdown_deadline = None;
                    self.show_toast("❌ Countdown cancelled");
                }
                ctx.request_repaint();
            }
        }

        self.sync_tray_recording_progress();
        // Keep pumping tray events, feedback poll, and recording timer even when hidden.
        // Always repaint on a short cadence when tray is present so agent questions surface.
        if self.tray.is_some()
            || self.is_recording
            || self.recording_arming
            || self.countdown_deadline.is_some()
            || self.feedback_pending_count > 0
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if self.retro.config().enabled {
            // Keep status strip / Capture tab live while retro is rolling.
            ctx.request_repaint_after(Duration::from_millis(500));
        }

        
        // --- Capture HUD: region selection (thirds + handles + W×H) ---
        if self.is_selecting_region {
            match show_region_selector(
                ctx,
                &mut self.region_start,
                &mut self.region_end,
                self.last_region,
            ) {
                RegionHudResult::Continue => {}
                RegionHudResult::Confirmed { selected } => {
                    self.selected_region = Some(selected);
                    self.last_region = Some(selected);
                    self.persist_session();
                    self.is_selecting_region = false;
                    self.region_start = None;
                    self.region_end = None;
                    self.pending_arm_record = true;
                    ctx.request_repaint();
                }
                RegionHudResult::Cancelled => {
                    self.is_selecting_region = false;
                    self.region_start = None;
                    self.region_end = None;
                }
            }
            return;
        }

        // --- Floating controller: arming countdown + active recording ---
        // Immediate viewport keeps the event loop awake while the main window is hidden.
        if self.is_recording || self.recording_arming {
            let builder = ViewportBuilder::default()
                .with_title("Vibecap Recorder")
                .with_decorations(false)
                .with_always_on_top()
                .with_inner_size([320.0, 52.0])
                .with_resizable(false)
                .with_transparent(true)
                .with_visible(true);

            ctx.show_viewport_immediate(
                ViewportId::from_hash_of("recording_bar"),
                builder,
                |ctx, class| {
                    if class == egui::ViewportClass::Immediate {
                        let bar_frame = Frame::none()
                            .fill(theme::SURFACE())
                            .rounding(theme::rounding_lg())
                            .stroke(Stroke::new(1.5_f32, theme::ACCENT()));

                        egui::CentralPanel::default().frame(bar_frame).show(ctx, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_space(6.0);

                                let pulse = (ctx.input(|i| i.time) * 4.0).sin().abs() as f32;
                                let dot_color = if self.recording_arming {
                                    theme::ACCENT()
                                } else if self.is_paused {
                                    theme::WARN()
                                } else {
                                    theme::danger_pulse(pulse)
                                };
                                ui.colored_label(dot_color, "●");

                                if self.recording_arming {
                                    ui.label(
                                        RichText::new("Starting…")
                                            .strong()
                                            .color(theme::TEXT()),
                                    );
                                } else {
                                    let elapsed = self.recording_elapsed_secs();
                                    let mins = elapsed / 60;
                                    let secs = elapsed % 60;
                                    let status_text = if self.is_paused { "PAUSED" } else { "REC" };
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {:02}:{:02}",
                                            status_text, mins, secs
                                        ))
                                        .strong()
                                        .color(theme::TEXT()),
                                    );
                                }

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.add_space(4.0);

                                        if ui
                                            .button(
                                                RichText::new("✖")
                                                    .color(theme::DANGER())
                                                    .strong(),
                                            )
                                            .on_hover_text("Cancel Recording")
                                            .clicked()
                                        {
                                            self.cancel_recording(ctx);
                                        }

                                        if !self.recording_arming {
                                            if ui
                                                .button(
                                                    RichText::new("⏹")
                                                        .color(theme::ON_SOLID())
                                                        .strong(),
                                                )
                                                .on_hover_text("Stop & Save")
                                                .clicked()
                                            {
                                                self.stop_recording(ctx);
                                            }

                                            let pause_icon = if self.is_paused { "▶" } else { "⏸" };
                                            let pause_color = if self.is_paused {
                                                theme::SUCCESS()
                                            } else {
                                                theme::WARN()
                                            };
                                            if ui
                                                .button(
                                                    RichText::new(pause_icon)
                                                        .color(pause_color)
                                                        .strong(),
                                                )
                                                .on_hover_text(if self.is_paused {
                                                    "Resume"
                                                } else {
                                                    "Pause"
                                                })
                                                .clicked()
                                            {
                                                self.toggle_pause();
                                            }
                                        }
                                    },
                                );
                            });
                        });
                    }
                },
            );
            ctx.request_repaint();
        }

        // Agent feedback polling: OS notify + Dock bounce + tray title + open Inbox.
        let poll_due = self
            .feedback_last_poll
            .map(|t| t.elapsed() > Duration::from_secs(2))
            .unwrap_or(true);
        if poll_due {
            self.scan_feedback_requests();
            self.surface_new_feedback(ctx);
            // Keep count in sync even when nothing new (answers cleared ids).
            self.feedback_pending_count = self
                .feedback_requests
                .iter()
                .filter(|r| r.status == "pending")
                .count();
            self.feedback_last_poll = Some(Instant::now());
            self.feedback_scanned = true;
        }
        // Poll often enough for HITL feel while hidden in tray.
        ctx.request_repaint_after(Duration::from_secs(2));

        self.drain_ffmpeg_results();
        self.check_annotated_save(ctx);

        if let Some(rx) = &self.screenshot_rx {
            if let Ok(result) = rx.try_recv() {
                self.show_window(ctx);
                match result {
                    Ok(shot_file) => {
                        self.latest_screenshot = Some(shot_file.clone());
                        // Do not auto-open annotation — offer post-capture actions instead.
                        self.refresh_library();
                        self.toast_message = None;
                        self.capture_toast = Some((shot_file, Instant::now()));
                    }
                    Err(e) => {
                        self.show_toast(format!("❌ {e}"));
                    }
                }
            }
        }

        if self.is_recording || self.is_recording_voice_memo || self.recording_arming {
            ctx.request_repaint();
        }

        // Global hotkeys (work even when window is hidden in tray).
        let mut hotkey_shots = 0u32;
        let mut hotkey_recs = 0u32;
        if let Some(rx) = &self.hotkey_receiver {
            while let Ok(event) = rx.try_recv() {
                if event.state != global_hotkey::HotKeyState::Pressed {
                    continue;
                }
                if event.id == self.hotkey_id_screenshot {
                    hotkey_shots += 1;
                } else if event.id == self.hotkey_id_record {
                    hotkey_recs += 1;
                }
            }
        }
        for _ in 0..hotkey_shots {
            self.trigger_capture(ctx, true);
        }
        for _ in 0..hotkey_recs {
            if self.is_recording {
                self.stop_recording(ctx);
            } else if self.recording_arming || self.countdown_deadline.is_some() {
                self.cancel_recording(ctx);
            } else {
                self.trigger_capture(ctx, false);
            }
        }

        // In-window short commands when the app is focused.
        // S = screenshot · R = record · Z = undo delete · ⌘K/Ctrl+K = palette
        if !self.is_annotating && !self.palette_open {
            let (press_s, press_r, press_z, press_palette) = ctx.input(|i| {
                let mod_cmd = i.modifiers.command || i.modifiers.ctrl;
                (
                    i.key_pressed(egui::Key::S) && !i.modifiers.any(),
                    i.key_pressed(egui::Key::R) && !i.modifiers.any(),
                    i.key_pressed(egui::Key::Z) && !i.modifiers.any(),
                    mod_cmd && i.key_pressed(egui::Key::K),
                )
            });
            if press_palette {
                self.palette_open = true;
                self.palette_query.clear();
                self.palette_selected = 0;
            } else if press_s {
                self.trigger_capture(ctx, true);
            } else if press_r {
                if self.is_recording {
                    self.stop_recording(ctx);
                } else if self.recording_arming || self.countdown_deadline.is_some() {
                    self.cancel_recording(ctx);
                } else {
                    self.trigger_capture(ctx, false);
                }
            } else if press_z {
                self.undo_last_delete();
            }
        } else if self.palette_open {
            // Allow re-toggle close with ⌘K
            let press_palette = ctx.input(|i| {
                (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(egui::Key::K)
            });
            if press_palette {
                self.palette_open = false;
            }
        }

        self.flush_expired_undo();

        // First-run wizard (blocks main chrome while open)
        if ui::wizard::show(self, ctx) {
            return;
        }

        // Command palette
        if let Some(action) = show_palette(
            ctx,
            &mut self.palette_query,
            &mut self.palette_selected,
            &mut self.palette_open,
        ) {
            self.run_palette_action(ctx, action);
        }

        // ── Loop rail (left) ─────────────────────────────────────
        if !self.is_annotating {
            egui::SidePanel::left("loop_rail")
                .exact_width(76.0)
                .resizable(false)
                .frame(
                    Frame::none()
                        .fill(theme::SURFACE())
                        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
                        .inner_margin(0.0),
                )
                .show(ctx, |ui| {
                    let rec_live = self.is_recording || self.recording_arming;
                    if let Some(stage) = loop_rail(
                        ui,
                        self.current_tab.to_loop(),
                        self.feedback_pending_count,
                        rec_live,
                    ) {
                        self.current_tab = AppTab::from_loop(stage);
                    }
                });
        }

        egui::CentralPanel::default()
            .frame(Frame::none().fill(theme::CANVAS()).inner_margin(theme::SP_4))
            .show(ctx, |ui| {
            if self.is_annotating {
                self.show_annotation(ui);
                return;
            }

            // ── Stage header ─────────────────────────────────────
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new(self.current_tab.title())
                        .size(22.0)
                        .color(theme::TEXT())
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(RichText::new("⌘K").color(theme::TEXT_DIM()))
                        .on_hover_text("Command palette (Ctrl+K / ⌘K)")
                        .clicked()
                    {
                        self.palette_open = true;
                        self.palette_query.clear();
                        self.palette_selected = 0;
                    }
                    if self.is_recording {
                        let e = self.recording_elapsed_secs();
                        ui.label(
                            RichText::new(format!("● REC {:02}:{:02}", e / 60, e % 60))
                                .color(theme::DANGER())
                                .strong()
                                .small(),
                        );
                    } else if self.recording_arming {
                        ui.label(
                            RichText::new("● Starting…")
                                .color(theme::ACCENT())
                                .strong()
                                .small(),
                        );
                    } else if self.tray.is_some() {
                        ui.label(
                            RichText::new("tray on")
                                .color(theme::TEXT_DIM())
                                .small(),
                        );
                    }
                });
            });
            ui.add_space(self.density.sp(theme::SP_3));

            match self.current_tab {
                AppTab::Capture => ui::capture_tab::show(self, ui, ctx),
                AppTab::Library => ui::library_tab::show(self, ui, ctx),
                AppTab::Clip => ui::clip_tab::show(self, ui, ctx),
                AppTab::Still => ui::still_tab::show(self, ui, ctx),
                AppTab::Feedback => ui::inbox_tab::show(self, ui, ctx),
                AppTab::Settings => ui::settings_tab::show(self, ui, ctx),
            }

            // ── Status strip (storage · budget · ffmpeg · inbox) ─
            ui.add_space(theme::SP_2);
            let snap = self.status_snapshot();
            status_strip(ui, &snap);
        });

        // Capture action toast takes priority over plain toasts.
        if let Some((path, at)) = self.capture_toast.clone() {
            if at.elapsed() > Duration::from_secs(12) {
                self.capture_toast = None;
            } else if let Some(act) = show_capture_toast(ctx, &path) {
                match act {
                    CaptureToastAction::Annotate => {
                        self.capture_toast = None;
                        self.annotate_media(ctx, path);
                    }
                    CaptureToastAction::Copy => {
                        self.copy_image_to_clipboard(&path);
                        self.capture_toast = None;
                    }
                    CaptureToastAction::Reveal => {
                        let _ = reveal_in_file_manager(&path);
                        self.capture_toast = None;
                    }
                    CaptureToastAction::Dismiss => {
                        self.capture_toast = None;
                    }
                }
            }
        } else if let Some((msg, time, level)) = &self.toast_message {
            if time.elapsed() < Duration::from_secs(4) {
                show_toast_card(ctx, msg, *level);
            } else {
                self.toast_message = None;
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.iter().any(|a| a == "--version" || a == "-v" || a == "version") {
        println!("vibecap 0.1.0");
        return Ok(());
    }

    if args.iter().any(|a| a == "--help" || a == "-h" || a == "help") {
        println!("Vibecap Studio 0.1.0");
        println!("Native screen capture, annotation studio, and MCP sidecar for AI agents.");
        println!("\nUsage: vibecap [FLAGS]");
        println!("\nFlags:");
        println!("  (none)         Launch the desktop UI (system tray enabled)");
        println!("  --mcp          Run as Model Context Protocol (MCP) stdio server");
        println!("                 Multiple --mcp processes are supported (one per agent/client).");
        println!("  --screenshot   Headless full-screen capture → {}", media_dir_display());
        println!("  --no-tray      Disable system tray (window close quits the app)");
        println!("  --hidden       Start hidden in the tray (implies tray)");
        println!("  --version, -v  Print version");
        println!("  --help, -h     Print this help");
        println!("\nMulti-instance:");
        println!("  · GUI + one or more `vibecap --mcp` processes can run together.");
        println!("  · Each MCP process has its own live-inspection session dir.");
        println!("  · Budget + feedback inbox are shared via config files.");
        println!("\nCapture backend: {}", platform::capture_backend_label());
        println!("Docs: README.md  ·  docs/USAGE.md  ·  docs/MCP.md");
        println!("MCP tools: vibecap_capture | record_video | export_gif |");
        println!("           start/get/stop_live_inspection | set_budget | get_spending |");
        println!("           request_feedback | get_feedback | list_feedback | cancel_feedback");
        return Ok(());
    }

    if args.iter().any(|a| a == "--screenshot" || a == "screenshot") {
        match capture_screenshot_to_media_dir() {
            Ok(path) => {
                println!("{}", path.display());
                return Ok(());
            }
            Err(e) => {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        }
    }

    if args.iter().any(|a| a == "--mcp" || a == "mcp") {
        // Intentionally no process-wide lock: many agents may each spawn --mcp.
        eprintln!(
            "vibecap mcp ready (pid {}, live session {})",
            std::process::id(),
            mcp_live_dir().display()
        );
        run_mcp_server();
        return Ok(());
    }

    let no_tray = args.iter().any(|a| a == "--no-tray");
    let start_hidden = args.iter().any(|a| a == "--hidden");
    let enable_tray = !no_tray || start_hidden;

    // Brand dock / taskbar icon. Without this, eframe uses the default white "e".
    let app_icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/app_icon.png"))
        .unwrap_or_default();

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_decorations(true)
            .with_transparent(false)
            .with_inner_size([760.0, 640.0])
            .with_min_inner_size([640.0, 560.0])
            .with_icon(app_icon)
            // Multiple GUI instances are allowed (human + optional second window).
            .with_title(format!("Vibecap Studio · {}", std::process::id())),
        ..Default::default()
    };
    
    eframe::run_native(
        "Vibecap Studio",
        options,
        Box::new(move |cc| {
            let mut app = VibecapApp::new(cc);
            app.start_hidden = start_hidden;
            if enable_tray {
                match TrayController::try_new("Vibecap — click to show") {
                    Ok(tray) => {
                        app.tray = Some(tray);
                        app.allow_exit = false;
                    }
                    Err(e) => {
                        eprintln!("warning: system tray unavailable ({e}); close will quit");
                        app.allow_exit = true;
                    }
                }
            } else {
                app.allow_exit = true;
            }
            Ok(Box::new(app))
        }),
    )
}
