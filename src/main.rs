mod platform;
mod tray_ui;

use eframe::egui;
use std::process::{Command, Child};
use std::path::PathBuf;
use std::io::Write;
use chrono::Local;
use crossbeam_channel::Receiver;
use global_hotkey::{GlobalHotKeyManager, hotkey::{HotKey, Modifiers, Code}};
use global_hotkey::GlobalHotKeyEvent;
use egui::{Color32, Stroke, Pos2, Rect, Vec2, ViewportId, ViewportBuilder, RichText, Frame, Rounding, Visuals, Align2, FontId};
use rfd::FileDialog;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use platform::{
    capture_live_frame, capture_screenshot_interactive, capture_to_media_dir, config_dir as platform_config_dir,
    cont_process, export_gif_clip, focus_app, live_dir as platform_live_dir, live_session_dir,
    media_dir, media_dir_display, open_path, record_screen_clip, reveal_in_file_manager,
    spawn_screen_recorder, spawn_voice_memo, stop_process, LiveFormat,
};
use tray_ui::{TrayAction, TrayController};

static LIVE_INSPECTION_RUNNING: AtomicBool = AtomicBool::new(false);
static LATEST_LIVE_GIF: OnceLock<Mutex<String>> = OnceLock::new();

fn get_latest_live_gif_mutex() -> &'static Mutex<String> {
    LATEST_LIVE_GIF.get_or_init(|| Mutex::new(String::new()))
}


#[derive(PartialEq, Clone, Copy)]
enum AppTab {
    Capture,
    Library,
    Edit,
    Feedback,
    Settings,
}

#[derive(PartialEq, Clone, Copy)]
enum CaptureTarget {
    Fullscreen,
    Region,
    Window,
}

#[derive(PartialEq, Clone, Copy, Default)]
enum AnnotationTool {
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
struct AnnotationAction {
    tool: AnnotationTool,
    color: Color32,
    stroke_width: f32,
    points: Vec<Pos2>,
    text_content: String,
    badge_number: usize,
}

#[derive(Clone)]
struct MediaItem {
    path: PathBuf,
    name: String,
    is_video: bool,
    size_str: String,
}

// ---------- Paths / capture (see `platform` module) ----------

fn default_media_dir() -> PathBuf {
    media_dir()
}

fn default_live_dir() -> PathBuf {
    platform_live_dir()
}

/// MCP / agent live stream dir — unique per process so multiple agents can run at once.
fn mcp_live_dir() -> PathBuf {
    live_session_dir()
}

fn capture_screenshot_to_media_dir() -> Result<PathBuf, String> {
    capture_to_media_dir()
}

// ---------- Agent Budget & Feedback (shared between app & MCP server via platform config dir) ----------

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct BudgetConfig {
    /// 0 = unlimited
    max_frames: u32,
    /// 0.0 = unlimited
    max_mb: f64,
    /// 0 = unlimited
    max_minutes: u32,
    /// "eco" | "standard" | "intensive"
    analysis_tier: String,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self { max_frames: 0, max_mb: 0.0, max_minutes: 0, analysis_tier: "standard".to_string() }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct FeedbackRequest {
    id: String,
    media_path: String,
    question: String,
    created_at: String,
    status: String, // "pending" | "answered"
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct FeedbackResponse {
    id: String,
    feedback_text: String,
    #[serde(default)]
    voice_note_path: String,
    #[serde(default)]
    annotated_media_path: String,
    answered_at: String,
}

fn vibecap_config_dir() -> PathBuf {
    platform_config_dir()
}

fn budget_file_path() -> PathBuf { vibecap_config_dir().join("budget.json") }

/// Ok(None) = no budget set; Ok(Some) = budget; Err = corrupt/unreadable (enforcement fails closed).
fn budget_file_state() -> Result<Option<BudgetConfig>, String> {
    match std::fs::read_to_string(budget_file_path()) {
        Ok(s) => serde_json::from_str(&s).map(Some).map_err(|e| format!("corrupt budget.json: {}", e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("unreadable budget.json: {}", e)),
    }
}

fn load_budget() -> BudgetConfig {
    budget_file_state().ok().flatten().unwrap_or_default()
}

fn save_budget(cfg: &BudgetConfig) -> Result<(), String> {
    let s = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    write_json_atomic(&budget_file_path(), &s)
}

/// Write-then-rename so a concurrent reader never sees a partial file.
fn write_json_atomic(path: &PathBuf, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, contents).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn feedback_requests_dir() -> PathBuf {
    let dir = vibecap_config_dir().join("feedback").join("requests");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn feedback_responses_dir() -> PathBuf {
    let dir = vibecap_config_dir().join("feedback").join("responses");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// When the current live-inspection session started (MCP process-local).
static LIVE_STARTED_AT: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
fn get_live_started_mutex() -> &'static Mutex<Option<Instant>> {
    LIVE_STARTED_AT.get_or_init(|| Mutex::new(None))
}

/// Set by the live loop when it auto-stops on a budget cap.
static BUDGET_NOTE: OnceLock<Mutex<String>> = OnceLock::new();
fn get_budget_note_mutex() -> &'static Mutex<String> {
    BUDGET_NOTE.get_or_init(|| Mutex::new(String::new()))
}

/// Budget usage snapshot for the live dir: (frames, mb, elapsed_minutes).
/// Frames = timestamped capture files only (frame_*/live_*/video_*); MB = all bytes on disk.
/// Elapsed is 0 when no stream is running (prevents stale timers from haunting later sessions).
fn live_usage_snapshot(live_dir: &str) -> (usize, f64, f64) {
    let mut frames = 0usize;
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(live_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                    if name.starts_with("frame_") || name.starts_with("live_") || name.starts_with("video_") {
                        frames += 1;
                    }
                }
            }
        }
    }
    let mb = (total as f64) / (1024.0 * 1024.0);
    let elapsed = if LIVE_INSPECTION_RUNNING.load(Ordering::SeqCst) {
        match get_live_started_mutex().lock() {
            Ok(l) => (*l).map(|t| t.elapsed().as_secs_f64() / 60.0).unwrap_or(0.0),
            Err(_) => 0.0,
        }
    } else {
        0.0
    };
    (frames, mb, elapsed)
}

/// Returns Some(reason) if any budget cap is exceeded.
fn budget_exceeded_reason(live_dir: &str) -> Option<String> {
    if let Err(e) = budget_file_state() {
        return Some(format!("⚠️ {} — enforcement is fail-closed; fix or delete {}", e, budget_file_path().display()));
    }
    let cfg = load_budget();
    let (frames, mb, minutes) = live_usage_snapshot(live_dir);
    if cfg.max_frames > 0 && frames >= cfg.max_frames as usize {
        return Some(format!("frame cap reached ({}/{} frames)", frames, cfg.max_frames));
    }
    if cfg.max_mb > 0.0 && mb >= cfg.max_mb {
        return Some(format!("storage cap reached ({:.1}/{:.1} MB)", mb, cfg.max_mb));
    }
    if cfg.max_minutes > 0 && minutes >= cfg.max_minutes as f64 {
        return Some(format!("time cap reached ({:.1}/{} min)", minutes, cfg.max_minutes));
    }
    None
}

/// One-line budget status for MCP tool responses.
fn budget_status_line(live_dir: &str) -> String {
    let cfg = load_budget();
    let (frames, mb, minutes) = live_usage_snapshot(live_dir);
    let note = get_budget_note_mutex().lock().map(|n| n.clone()).unwrap_or_default();
    let frames_cap = if cfg.max_frames == 0 { "unlimited".to_string() } else { cfg.max_frames.to_string() };
    let mb_cap = if cfg.max_mb <= 0.0 { "unlimited".to_string() } else { format!("{:.1}", cfg.max_mb) };
    let min_cap = if cfg.max_minutes == 0 { "unlimited".to_string() } else { cfg.max_minutes.to_string() };
    let mut s = format!("💰 BUDGET: frames {}/{}, {:.1}/{} MB, {:.1}/{} min (tier: {})", frames, frames_cap, mb, mb_cap, minutes, min_cap, cfg.analysis_tier);
    if !note.is_empty() {
        s.push_str(&format!(" — ⚠️ {}", note));
    }
    s
}

#[derive(Default)]
struct VibecapApp {
    current_tab: AppTab,
    capture_target: CaptureTarget,
    capture_audio: bool,
    fps_target: u32,
    is_recording: bool,
    is_paused: bool,
    accumulated_duration: Duration,
    segment_start: Option<Instant>,
    child_process: Option<Child>,

    // Audio Voice Note Recording
    is_recording_voice_memo: bool,
    voice_memo_child: Option<Child>,
    voice_memo_start: Option<Instant>,
    
    // Channels for async capture
    capture_trigger_tx: Option<crossbeam_channel::Sender<bool>>,
    capture_trigger_rx: Option<crossbeam_channel::Receiver<bool>>,
    screenshot_tx: Option<crossbeam_channel::Sender<PathBuf>>,
    screenshot_rx: Option<crossbeam_channel::Receiver<PathBuf>>,
    
    // File paths & Media Library
    save_dir: PathBuf,
    current_mp4_file: Option<PathBuf>,
    latest_screenshot: Option<PathBuf>,
    library_items: Vec<MediaItem>,
    library_filter: String,
    
    // Edit tab & Video Processing
    trim_start: String,
    trim_end: String,
    export_speed: String,
    edit_file: Option<PathBuf>,
    filmstrip: Vec<egui::TextureHandle>,
    filmstrip_loading: bool,

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

    // Notification toast
    toast_message: Option<(String, Instant)>,

    // Feedback Inbox (agent human-in-the-loop)
    feedback_requests: Vec<FeedbackRequest>,
    feedback_scanned: bool,
    feedback_selected: Option<String>,
    feedback_draft: String,

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
        
        let mut visuals = Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(0x14, 0x10, 0x08);
        visuals.window_fill = Color32::from_rgb(0x14, 0x10, 0x08);
        visuals.extreme_bg_color = Color32::from_rgb(0x21, 0x1a, 0x11);
        visuals.faint_bg_color = Color32::from_rgb(0x2a, 0x21, 0x14);
        
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(0x21, 0x1a, 0x11);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(235, 210, 170, 25));
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xec, 0xe5, 0xd6));
        visuals.widgets.noninteractive.rounding = Rounding::same(10.0);
        
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(0x2a, 0x21, 0x14);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(235, 210, 170, 30));
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xec, 0xe5, 0xd6));
        visuals.widgets.inactive.rounding = Rounding::same(10.0);

        visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x35, 0x2a, 0x1a);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xf5, 0x9e, 0x4b));
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.hovered.rounding = Rounding::same(10.0);

        visuals.widgets.active.bg_fill = Color32::from_rgb(0xf5, 0x9e, 0x4b);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x1c, 0x14, 0x08));
        visuals.widgets.active.rounding = Rounding::same(10.0);

        visuals.selection.bg_fill = Color32::from_rgb(0xf5, 0x9e, 0x4b);
        visuals.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x1c, 0x14, 0x08));
        
        cc.egui_ctx.set_visuals(visuals);

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

        let (capture_tx, capture_rx) = crossbeam_channel::unbounded();
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
            current_color: Color32::from_rgb(0xf5, 0x9e, 0x4b),
            current_stroke_width: 3.0,
            pending_text: "Sample Text".to_string(),
            feedback_description: String::new(),
            step_counter: 1,
            capture_trigger_tx: Some(capture_tx),
            capture_trigger_rx: Some(capture_rx),
            screenshot_tx: Some(screenshot_tx),
            screenshot_rx: Some(screenshot_rx),
            ffmpeg_tx: Some(ffmpeg_tx),
            ffmpeg_rx: Some(ffmpeg_rx),
            library_filter: "All".to_string(),
            budget_frames_input: "0".to_string(),
            budget_mb_input: "0.0".to_string(),
            budget_minutes_input: "0".to_string(),
            budget_tier: "standard".to_string(),
            img_resize_pct: 100,
            allow_exit: false,
            start_hidden: false,
            ..Default::default()
        };

        app.refresh_library();
        app
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
                    } else {
                        self.trigger_capture(ctx, false);
                    }
                }
                TrayAction::Feedback => {
                    self.current_tab = AppTab::Feedback;
                    self.scan_feedback_requests();
                    self.show_window(ctx);
                }
                TrayAction::Quit => {
                    self.allow_exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn sync_tray_recording_progress(&mut self) {
        let elapsed = if self.is_recording {
            Some(self.recording_elapsed_secs())
        } else {
            None
        };
        if let Some(tray) = self.tray.as_mut() {
            tray.set_recording_progress(elapsed);
        }
    }

    fn show_toast(&mut self, message: impl Into<String>) {
        self.toast_message = Some((message.into(), Instant::now()));
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
        self.library_items.clear();
        if let Ok(entries) = std::fs::read_dir(&self.save_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                    if ext == "png" || ext == "jpg" || ext == "jpeg" || ext == "mp4" || ext == "gif" || ext == "mov" || ext == "m4a" || ext == "txt" {
                        let name = path.file_name().unwrap().to_str().unwrap().to_string();
                        let meta = entry.metadata().ok();
                        let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                        let size_str = if size_bytes > 1_048_576 {
                            format!("{:.1} MB", size_bytes as f64 / 1_048_576.0)
                        } else {
                            format!("{} KB", size_bytes / 1024)
                        };

                        self.library_items.push(MediaItem {
                            path,
                            name,
                            is_video: ext == "mp4" || ext == "mov" || ext == "gif",
                            size_str,
                        });
                    }
                }
            }
        }
        self.library_items.sort_by(|a, b| b.name.cmp(&a.name));
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
        self.feedback_requests.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    }

    fn submit_feedback_response(&mut self, request_id: &str) {
        let response = FeedbackResponse {
            id: request_id.to_string(),
            feedback_text: self.feedback_draft.trim().to_string(),
            voice_note_path: self.feedback_voice_note.take().map(|p| p.display().to_string()).unwrap_or_default(),
            annotated_media_path: String::new(),
            answered_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        };
        let resp_path = feedback_responses_dir().join(format!("{}.json", request_id));
        let saved = serde_json::to_string_pretty(&response).ok()
            .and_then(|s| write_json_atomic(&resp_path, &s).ok());
        if saved.is_none() {
            self.show_toast("❌ Could not save feedback — check disk permissions.");
            return;
        }
        // Only mark the request answered after the response is durably written.
        let req_path = feedback_requests_dir().join(format!("{}.json", request_id));
        if let Ok(s) = std::fs::read_to_string(&req_path) {
            if let Ok(mut req) = serde_json::from_str::<FeedbackRequest>(&s) {
                req.status = "answered".to_string();
                if let Ok(s2) = serde_json::to_string_pretty(&req) {
                    let _ = write_json_atomic(&req_path, &s2);
                }
            }
        }
        self.feedback_draft.clear();
        self.feedback_selected = None;
        self.scan_feedback_requests();
        self.show_toast("✅ Feedback submitted — the agent can pick it up now!");
    }

    fn clear_answered_feedback(&mut self) {
        let answered: Vec<String> = self.feedback_requests.iter()
            .filter(|r| r.status == "answered")
            .map(|r| r.id.clone())
            .collect();
        for id in &answered {
            let _ = std::fs::remove_file(feedback_requests_dir().join(format!("{}.json", id)));
            let _ = std::fs::remove_file(feedback_responses_dir().join(format!("{}.json", id)));
        }
        if !answered.is_empty() {
            self.show_toast("🧹 Cleared answered requests");
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
            let result = Command::new("ffmpeg")
                .args(&args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .output();
            let (ok, msg) = match result {
                Ok(out) if out.status.success() => (true, ok_msg),
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr);
                    let tail = err.lines().last().unwrap_or("unknown ffmpeg error").trim().to_string();
                    (false, format!("❌ ffmpeg failed: {}", tail))
                }
                Err(e) => (false, format!("❌ could not start ffmpeg: {}", e)),
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
        if let Some(mut child) = self.child_process.take() {
            if self.is_paused {
                cont_process(child.id());
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        
        if let Some(file) = self.current_mp4_file.take() {
            let _ = std::fs::remove_file(file);
        }
        
        self.is_recording = false;
        self.is_paused = false;
        self.accumulated_duration = Duration::ZERO;
        self.segment_start = None;
        
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.show_toast("❌ Recording cancelled");
    }

    fn stop_recording(&mut self, ctx: &egui::Context) {
        if let Some(mut child) = self.child_process.take() {
            if self.is_paused {
                cont_process(child.id());
            }
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(b"q\n");
            }
            let _ = child.wait();
        }
        self.is_recording = false;
        self.is_paused = false;
        self.accumulated_duration = Duration::ZERO;
        self.segment_start = None;
        
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);

        if let Some(mp4) = &self.current_mp4_file {
            self.edit_file = Some(mp4.clone());
            self.current_tab = AppTab::Edit;
            self.load_filmstrip(ctx, mp4.clone());
        }
        self.refresh_library();
        self.show_toast("💾 Video saved successfully!");
    }

    fn load_filmstrip(&mut self, _ctx: &egui::Context, file: PathBuf) {
        self.filmstrip.clear();
        let out_dir = file.parent().unwrap().join("frames_temp");
        let _ = std::fs::remove_dir_all(&out_dir);
        let _ = std::fs::create_dir_all(&out_dir);
        let out = out_dir.join("thumb_%03d.jpg");
        
        let _ = Command::new("ffmpeg")
            .args(&["-i", file.to_str().unwrap(), "-vf", "fps=1", "-vframes", "10", "-s", "320x180", out.to_str().unwrap()])
            .spawn()
            .and_then(|mut c| c.wait());
            
        self.filmstrip_loading = true;
    }

    fn trigger_capture(&mut self, ctx: &egui::Context, is_screenshot: bool) {
        if !is_screenshot && self.capture_target == CaptureTarget::Region {
            self.is_selecting_region = true;
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        
        let ctx_clone = ctx.clone();
        let is_screenshot = is_screenshot;
        let capture_target = self.capture_target;
        let save_dir = self.save_dir.clone();
        
        let screenshot_tx = self.screenshot_tx.clone().unwrap();
        let capture_trigger_tx = self.capture_trigger_tx.clone().unwrap();
        
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            
            if is_screenshot {
                let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                let shot_file = save_dir.join(format!("screenshot_{}.jpg", timestamp));
                let interactive = matches!(
                    capture_target,
                    CaptureTarget::Region | CaptureTarget::Window
                );
                let _ = capture_screenshot_interactive(&shot_file, interactive);
                let _ = screenshot_tx.send(shot_file);
                ctx_clone.request_repaint();
            } else {
                let _ = capture_trigger_tx.send(false);
                ctx_clone.request_repaint();
            }
        });
    }

    fn execute_capture(&mut self, ctx: &egui::Context) {
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let mp4_file = self.save_dir.join(format!("video_{}.mp4", timestamp));

        let crop = if self.capture_target == CaptureTarget::Region {
            self.selected_region.map(|rect| {
                (
                    rect.width() as i32,
                    rect.height() as i32,
                    rect.min.x as i32,
                    rect.min.y as i32,
                )
            })
        } else {
            None
        };

        match spawn_screen_recorder(&mp4_file, self.fps_target, self.capture_audio, crop) {
            Ok(c) => {
                self.child_process = Some(c);
                self.current_mp4_file = Some(mp4_file);
                self.is_recording = true;
                self.is_paused = false;
                self.accumulated_duration = Duration::ZERO;
                self.segment_start = Some(Instant::now());
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            Err(e) => {
                self.show_toast(format!("❌ Record failed: {}", e));
            }
        }
    }

    fn show_annotation(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Annotation & Feedback Studio").color(Color32::from_rgb(0xf5, 0x9e, 0x4b)).strong());
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
                RichText::new("🔴 Stop Voice Note").color(Color32::WHITE).strong()
            } else {
                RichText::new("🎙 Voice Note").color(Color32::from_rgb(0x5e, 0xc2, 0x6a)).strong()
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
            
            if ui.button(RichText::new("💾 Save & Close").color(Color32::from_rgb(0x1c, 0x14, 0x08)).strong()).clicked() {
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
                        voice_note_path: self.feedback_voice_note.take().map(|p| p.display().to_string()).unwrap_or_default(),
                        annotated_media_path: annotated_path,
                        answered_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    };
                    let resp_path = feedback_responses_dir().join(format!("{}.json", fid));
                    let saved = serde_json::to_string_pretty(&resp).ok()
                        .and_then(|s| write_json_atomic(&resp_path, &s).ok());
                    if saved.is_some() {
                        let req_path = feedback_requests_dir().join(format!("{}.json", fid));
                        if let Ok(s) = std::fs::read_to_string(&req_path) {
                            if let Ok(mut req) = serde_json::from_str::<FeedbackRequest>(&s) {
                                req.status = "answered".to_string();
                                if let Ok(s2) = serde_json::to_string_pretty(&req) {
                                    let _ = write_json_atomic(&req_path, &s2);
                                }
                            }
                        }
                        self.show_toast("✅ Annotated feedback submitted to the agent!");
                    } else {
                        self.show_toast("❌ Could not save feedback — check disk permissions.");
                    }
                    self.feedback_draft.clear();
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
            ui.label(RichText::new("Developer Note / Agent Feedback Context:").small().color(Color32::from_rgb(0xa2, 0x95, 0x7f)));
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
                Color32::WHITE
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
                            painter.rect_filled(rect, 0.0, Color32::from_black_alpha(220));
                            painter.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, Color32::GRAY));
                        }
                    }
                    AnnotationTool::Text => {
                        let pos = action.points[0];
                        painter.rect_filled(Rect::from_min_size(pos - Vec2::new(4.0, 2.0), Vec2::new(action.text_content.len() as f32 * 10.0 + 8.0, 22.0)), 4.0, Color32::from_black_alpha(180));
                        painter.text(pos, Align2::LEFT_TOP, &action.text_content, FontId::proportional(16.0), action.color);
                    }
                    AnnotationTool::StepBadge => {
                        let pos = action.points[0];
                        painter.circle_filled(pos, 14.0, action.color);
                        painter.text(pos, Align2::CENTER_CENTER, action.badge_number.to_string(), FontId::proportional(14.0), Color32::from_rgb(0x1c, 0x14, 0x08));
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
        self.sync_tray_recording_progress();
        // Keep pumping tray events + recording timer even when the window is hidden.
        if self.tray.is_some() || self.is_recording {
            ctx.request_repaint_after(Duration::from_millis(250));
        }

        
        // --- Region Selection Overlay Window ---
        if self.is_selecting_region {
            let builder = ViewportBuilder::default()
                .with_decorations(false)
                .with_transparent(true)
                .with_fullscreen(true)
                .with_always_on_top();
                
            ctx.show_viewport_immediate(
                ViewportId::from_hash_of("region_selector"),
                builder,
                |ctx, class| {
                    if class == egui::ViewportClass::Immediate {
                        let panel_frame = Frame::none().fill(Color32::from_black_alpha(100));
                        egui::CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
                            let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::drag());

                            if self.region_start.is_none() {
                                painter.text(
                                    Pos2::new(response.rect.center().x, response.rect.min.y + 48.0),
                                    Align2::CENTER_CENTER,
                                    "Drag to select a region · Esc to cancel",
                                    FontId::proportional(20.0),
                                    Color32::from_rgb(0xec, 0xe5, 0xd6),
                                );
                            }
                            
                            if let (Some(start), Some(end)) = (self.region_start, self.region_end) {
                                let rect = Rect::from_two_pos(start, end);
                                painter.rect_filled(rect, 0.0, Color32::TRANSPARENT);
                                painter.rect_stroke(rect, 0.0, Stroke::new(2.0_f32, Color32::from_rgb(0xf5, 0x9e, 0x4b)));
                            }
                            
                            if response.drag_started() {
                                if let Some(pos) = response.interact_pointer_pos() {
                                    self.region_start = Some(pos);
                                    self.region_end = Some(pos);
                                }
                            }
                            if response.dragged() {
                                if let Some(pos) = response.interact_pointer_pos() {
                                    self.region_end = Some(pos);
                                }
                            }
                            if response.drag_stopped() {
                                if let (Some(start), Some(end)) = (self.region_start, self.region_end) {
                                    self.selected_region = Some(Rect::from_two_pos(start, end));
                                }
                                self.is_selecting_region = false;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                                
                                let capture_trigger_tx = self.capture_trigger_tx.clone().unwrap();
                                let ctx_clone = ctx.clone();
                                std::thread::spawn(move || {
                                    std::thread::sleep(Duration::from_millis(400));
                                    let _ = capture_trigger_tx.send(false);
                                    ctx_clone.request_repaint();
                                });
                            }
                            
                            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                                self.is_selecting_region = false;
                            }
                        });
                    }
                }
            );
            return;
        }

        // --- Floating Minimized Recording Controller Bar ---
        if self.is_recording {
            let builder = ViewportBuilder::default()
                .with_title("Vibecap Recorder")
                .with_decorations(false)
                .with_always_on_top()
                .with_inner_size([310.0, 52.0])
                .with_resizable(false)
                .with_transparent(true);

            ctx.show_viewport_immediate(
                ViewportId::from_hash_of("recording_bar"),
                builder,
                |ctx, class| {
                    if class == egui::ViewportClass::Immediate {
                        let bar_frame = Frame::none()
                            .fill(Color32::from_rgb(0x1a, 0x14, 0x0b))
                            .rounding(Rounding::same(12.0))
                            .stroke(Stroke::new(1.5_f32, Color32::from_rgb(0xf5, 0x9e, 0x4b)));
                            
                        egui::CentralPanel::default().frame(bar_frame).show(ctx, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_space(6.0);
                                
                                let pulse = (ctx.input(|i| i.time) * 4.0).sin().abs() as f32;
                                let dot_color = if self.is_paused {
                                    Color32::from_rgb(216, 164, 65)
                                } else {
                                    Color32::from_rgb((180.0 + pulse * 75.0) as u8, 50, 50)
                                };
                                ui.colored_label(dot_color, "●");

                                let elapsed = self.recording_elapsed_secs();
                                let mins = elapsed / 60;
                                let secs = elapsed % 60;
                                let status_text = if self.is_paused { "PAUSED" } else { "REC" };
                                
                                ui.label(RichText::new(format!("{} {:02}:{:02}", status_text, mins, secs))
                                    .strong()
                                    .color(Color32::from_rgb(0xec, 0xe5, 0xd6)));

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(4.0);
                                    
                                    if ui.button(RichText::new("✖").color(Color32::from_rgb(228, 106, 94)).strong()).on_hover_text("Cancel Recording").clicked() {
                                        self.cancel_recording(ctx);
                                    }
                                    
                                    if ui.button(RichText::new("⏹").color(Color32::WHITE).strong()).on_hover_text("Stop & Save").clicked() {
                                        self.stop_recording(ctx);
                                    }
                                    
                                    let pause_icon = if self.is_paused { "▶" } else { "⏸" };
                                    let pause_color = if self.is_paused { Color32::from_rgb(0x5e, 0xc2, 0x6a) } else { Color32::from_rgb(0xd8, 0xa4, 0x41) };
                                    if ui.button(RichText::new(pause_icon).color(pause_color).strong()).on_hover_text(if self.is_paused { "Resume" } else { "Pause" }).clicked() {
                                        self.toggle_pause();
                                    }
                                });
                            });
                        });
                    }
                }
            );
        }

        if self.filmstrip_loading {
            if let Some(file) = &self.edit_file {
                let out_dir = file.parent().unwrap().join("frames_temp");
                for i in 1..=10 {
                    let thumb_path = out_dir.join(format!("thumb_{:03}.jpg", i));
                    if thumb_path.exists() {
                        if let Ok(img) = image::open(&thumb_path) {
                            let size = [img.width() as _, img.height() as _];
                            let image_buffer = img.to_rgba8();
                            let pixels = image_buffer.as_flat_samples();
                            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                            let tex = ctx.load_texture(format!("thumb_{}", i), color_image, Default::default());
                            self.filmstrip.push(tex);
                        }
                        let _ = std::fs::remove_file(thumb_path);
                    }
                }
                self.filmstrip_loading = false;
            }
        }

        // Agent feedback polling: surface new arrivals on any tab + badge the inbox tab.
        let poll_due = self.feedback_last_poll.map(|t| t.elapsed() > Duration::from_secs(2)).unwrap_or(true);
        if poll_due {
            self.scan_feedback_requests();
            let pending = self.feedback_requests.iter().filter(|r| r.status != "answered").count();
            if pending > self.feedback_pending_count {
                self.show_toast("🤖 Your agent asked for feedback — open 💬 Feedback to answer.");
            }
            self.feedback_pending_count = pending;
            self.feedback_last_poll = Some(Instant::now());
            self.feedback_scanned = true;
        }
        ctx.request_repaint_after(Duration::from_secs(2));

        self.drain_ffmpeg_results();
        self.check_annotated_save(ctx);

        if let Some(rx) = &self.capture_trigger_rx {
            if let Ok(_) = rx.try_recv() {
                self.execute_capture(ctx);
            }
        }
        
        if let Some(rx) = &self.screenshot_rx {
            if let Ok(shot_file) = rx.try_recv() {
                self.latest_screenshot = Some(shot_file.clone());
                self.is_annotating = true;
                
                if let Ok(img) = image::open(&shot_file) {
                    let size = [img.width() as _, img.height() as _];
                    let image_buffer = img.to_rgba8();
                    let pixels = image_buffer.as_flat_samples();
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                    self.annotation_texture = Some(ctx.load_texture("screenshot", color_image, Default::default()));
                }
                self.annotation_actions.clear();
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.refresh_library();
                self.show_toast("📸 Screenshot captured!");
            }
        }
        
        if self.is_recording || self.is_recording_voice_memo {
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
            } else {
                self.trigger_capture(ctx, false);
            }
        }

        // In-window short commands when the app is focused (no modifiers needed).
        // S = screenshot · R = toggle record · Esc = cancel close-to-tray hide is separate.
        if !self.is_annotating {
            let (press_s, press_r) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::S) && !i.modifiers.any(),
                    i.key_pressed(egui::Key::R) && !i.modifiers.any(),
                )
            });
            if press_s {
                self.trigger_capture(ctx, true);
            } else if press_r {
                if self.is_recording {
                    self.stop_recording(ctx);
                } else {
                    self.trigger_capture(ctx, false);
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.is_annotating {
                self.show_annotation(ui);
                return;
            }

            // ── Top tab bar ──────────────────────────────────────
            Frame::none()
                .fill(Color32::from_rgb(0x1c, 0x16, 0x0e))
                .rounding(Rounding::same(8.0))
                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let tab_btn = |ui: &mut egui::Ui, active: bool, text: &str| -> bool {
                            let text_style = if active {
                                RichText::new(text).color(Color32::from_rgb(0xf5, 0x9e, 0x4b)).strong()
                            } else {
                                RichText::new(text).color(Color32::from_rgb(0xa2, 0x95, 0x7f))
                            };
                            let fill = if active { Color32::from_rgb(0x2d, 0x22, 0x14) } else { Color32::TRANSPARENT };
                            ui.add(egui::Button::new(text_style).fill(fill).rounding(Rounding::same(6.0))).clicked()
                        };

                        if tab_btn(ui, self.current_tab == AppTab::Capture, "🎥 Capture") {
                            self.current_tab = AppTab::Capture;
                        }
                        if tab_btn(ui, self.current_tab == AppTab::Library, "📂 Library") {
                            self.current_tab = AppTab::Library;
                        }
                        if tab_btn(ui, self.current_tab == AppTab::Edit, "✂ Editor") {
                            self.current_tab = AppTab::Edit;
                        }
                        let fb_label = if self.feedback_pending_count > 0 {
                            format!("💬 Feedback ({})", self.feedback_pending_count)
                        } else {
                            "💬 Feedback".to_string()
                        };
                        if tab_btn(ui, self.current_tab == AppTab::Feedback, &fb_label) {
                            self.current_tab = AppTab::Feedback;
                        }
                        if tab_btn(ui, self.current_tab == AppTab::Settings, "⚙ Settings") {
                            self.current_tab = AppTab::Settings;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if self.is_recording {
                                let e = self.recording_elapsed_secs();
                                ui.label(
                                    RichText::new(format!("● REC {:02}:{:02}", e / 60, e % 60))
                                        .color(Color32::from_rgb(0xe8, 0x3b, 0x3b))
                                        .strong()
                                        .small(),
                                );
                            } else if self.tray.is_some() {
                                ui.label(
                                    RichText::new("tray on")
                                        .color(Color32::from_rgb(0x6d, 0x63, 0x50))
                                        .small(),
                                );
                            }
                        });
                    });
                });

            ui.add_space(12.0);

            match self.current_tab {
                AppTab::Capture => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(12.0);
                        ui.heading(
                            RichText::new("Capture")
                                .size(26.0)
                                .color(Color32::from_rgb(0xf5, 0x9e, 0x4b))
                                .strong(),
                        );
                        ui.label(
                            RichText::new("Screenshot · screen record · agent-ready media")
                                .color(Color32::from_rgb(0xa2, 0x95, 0x7f)),
                        );
                        ui.add_space(22.0);

                        // ── Primary actions (top) ────────────────
                        ui.horizontal(|ui| {
                            ui.add_space(ui.available_width() / 2.0 - 180.0);

                            let screenshot_btn = egui::Button::new(
                                RichText::new("📸  Screenshot  (S)")
                                    .color(Color32::from_rgb(0xf5, 0x9e, 0x4b))
                                    .strong(),
                            )
                            .fill(Color32::from_rgb(0x2d, 0x22, 0x14))
                            .stroke(Stroke::new(1.5_f32, Color32::from_rgb(0xf5, 0x9e, 0x4b)))
                            .rounding(Rounding::same(10.0));

                            if ui
                                .add_sized([170.0, 52.0], screenshot_btn)
                                .on_hover_text("Shortcut: S (in app) · Ctrl+Shift+3 (global)")
                                .clicked()
                            {
                                self.trigger_capture(ctx, true);
                            }

                            ui.add_space(12.0);

                            if self.is_recording {
                                let elapsed = self.recording_elapsed_secs();
                                let mins = elapsed / 60;
                                let secs = elapsed % 60;
                                let text = RichText::new(format!("⏹  Stop  [{:02}:{:02}]", mins, secs))
                                    .color(Color32::WHITE)
                                    .strong();
                                let pulse = (ctx.input(|i| i.time) * 3.0).sin().abs() as f32;
                                let bg_color = Color32::from_rgb((200.0 + pulse * 55.0) as u8, 40, 40);
                                if ui
                                    .add_sized(
                                        [170.0, 52.0],
                                        egui::Button::new(text).fill(bg_color).rounding(Rounding::same(10.0)),
                                    )
                                    .on_hover_text("Shortcut: R · Ctrl+Shift+2 · also in tray")
                                    .clicked()
                                {
                                    self.stop_recording(ctx);
                                }
                            } else {
                                let record_btn = egui::Button::new(
                                    RichText::new("🎥  Record  (R)")
                                        .color(Color32::from_rgb(0x1c, 0x14, 0x08))
                                        .strong(),
                                )
                                .fill(Color32::from_rgb(0xf5, 0x9e, 0x4b))
                                .rounding(Rounding::same(10.0));
                                if ui
                                    .add_sized([170.0, 52.0], record_btn)
                                    .on_hover_text("Shortcut: R (in app) · Ctrl+Shift+2 (global) · tray menu")
                                    .clicked()
                                {
                                    self.trigger_capture(ctx, false);
                                }
                            }
                        });

                        ui.add_space(18.0);

                        // ── Subtle options (below buttons) ───────
                        ui.horizontal(|ui| {
                            ui.add_space(ui.available_width() / 2.0 - 200.0);
                            ui.label(
                                RichText::new("Target")
                                    .small()
                                    .color(Color32::from_rgb(0x6d, 0x63, 0x50)),
                            );
                            ui.add_space(6.0);
                            ui.radio_value(&mut self.capture_target, CaptureTarget::Fullscreen, "Full");
                            ui.radio_value(&mut self.capture_target, CaptureTarget::Region, "Region");
                            ui.radio_value(&mut self.capture_target, CaptureTarget::Window, "Window");
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add_space(ui.available_width() / 2.0 - 80.0);
                            ui.checkbox(
                                &mut self.capture_audio,
                                RichText::new("Include audio")
                                    .small()
                                    .color(Color32::from_rgb(0xa2, 0x95, 0x7f)),
                            );
                        });
                        ui.label(
                            RichText::new("S / R in app  ·  Ctrl+Shift+3 / 2 global  ·  FPS in Settings")
                                .small()
                                .color(Color32::from_rgb(0x6d, 0x63, 0x50)),
                        );

                        ui.add_space(16.0);
                        egui::CollapsingHeader::new(
                            RichText::new("🤖 Agent session (live inspection & budget)")
                                .color(Color32::from_rgb(0xa2, 0x95, 0x7f)),
                        )
                        .default_open(false)
                        .show(ui, |ui| {
                            let live_dir = default_live_dir().display().to_string();
                            let (bytes, count) = get_dir_size_bytes(&live_dir);
                            let mb = (bytes as f64) / (1024.0 * 1024.0);
                            let cfg = load_budget();
                            ui.label(format!("Live frames: {} · {:.2} MB in {}", count, mb, live_dir));
                            ui.label(format!(
                                "Budget: frames cap {} · MB cap {:.1} · minutes cap {} · tier {}",
                                if cfg.max_frames == 0 {
                                    "unlimited".to_string()
                                } else {
                                    cfg.max_frames.to_string()
                                },
                                cfg.max_mb,
                                if cfg.max_minutes == 0 {
                                    "unlimited".to_string()
                                } else {
                                    cfg.max_minutes.to_string()
                                },
                                cfg.analysis_tier
                            ));
                            ui.label(
                                RichText::new(
                                    "Agents use vibecap_set_budget; live inspection auto-stops at caps.",
                                )
                                .small()
                                .color(Color32::from_rgb(0x6d, 0x63, 0x50)),
                            );
                        });
                    });
                }

                AppTab::Library => {
                    ui.horizontal(|ui| {
                        ui.heading(RichText::new("Media Library").color(Color32::from_rgb(0xf5, 0x9e, 0x4b)).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("🔄 Refresh").clicked() {
                                self.refresh_library();
                            }
                            ui.selectable_value(&mut self.library_filter, "Videos".to_string(), "Videos");
                            ui.selectable_value(&mut self.library_filter, "Screenshots".to_string(), "Screenshots");
                            ui.selectable_value(&mut self.library_filter, "All".to_string(), "All");
                        });
                    });
                    ui.separator();
                    
                    let filtered_items: Vec<MediaItem> = self.library_items.iter().cloned().filter(|item| {
                        if self.library_filter == "Screenshots" {
                            !item.is_video
                        } else if self.library_filter == "Videos" {
                            item.is_video
                        } else {
                            true
                        }
                    }).collect();

                    if filtered_items.is_empty() {
                        ui.add_space(40.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("No media captures found.").color(Color32::from_rgb(0x6d, 0x63, 0x50)));
                        });
                    } else {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for item in filtered_items {
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        if !item.is_video {
                                            ui.add(egui::Image::new(format!("file://{}", item.path.display()))
                                                .fit_to_exact_size(Vec2::new(64.0, 36.0)));
                                        }
                                        let icon = if item.is_video { "🎥" } else { "📸" };
                                        ui.label(RichText::new(format!("{} {}", icon, item.name)).strong().color(Color32::from_rgb(0xec, 0xe5, 0xd6)));
                                        ui.label(RichText::new(format!("({})", item.size_str)).small().color(Color32::from_rgb(0x6d, 0x63, 0x50)));
                                        
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            let item_path = item.path.clone();
                                            if ui.button("🗑 Delete").clicked() {
                                                let _ = std::fs::remove_file(&item_path);
                                                self.show_toast("File deleted");
                                            }
                                            if ui.button("📂 Finder").clicked() {
                                                let _ = reveal_in_file_manager(&item_path);
                                            }
                                            if item.is_video {
                                                if ui.button("✂ Edit").clicked() {
                                                    self.edit_file = Some(item_path.clone());
                                                    self.current_tab = AppTab::Edit;
                                                    self.load_filmstrip(ctx, item_path);
                                                }
                                            } else {
                                                if ui.button("📋 Copy").clicked() {
                                                    self.copy_image_to_clipboard(&item_path);
                                                }
                                            }
                                        });
                                    });
                                });
                                ui.add_space(4.0);
                            }
                        });
                    }
                }

                AppTab::Edit => {
                    ui.heading(RichText::new("Interactive Video Editor & Exporter").color(Color32::from_rgb(0xf5, 0x9e, 0x4b)).strong());
                    ui.add_space(10.0);
                    
                    if ui.button("📂 Select Video").clicked() {
                        if let Some(path) = FileDialog::new().add_filter("Video", &["mp4", "mov"]).pick_file() {
                            self.edit_file = Some(path.clone());
                            self.load_filmstrip(ctx, path);
                        }
                    }
                    
                    let edit_file_opt = self.edit_file.clone();
                    if let Some(file) = &edit_file_opt {
                        ui.add_space(6.0);
                        ui.label(RichText::new(format!("Editing: {}", file.file_name().unwrap().to_str().unwrap())).color(Color32::from_rgb(0xec, 0xe5, 0xd6)));
                        ui.add_space(12.0);
                        
                        ui.group(|ui| {
                            ui.heading(RichText::new("Filmstrip Timeline").size(16.0).color(Color32::from_rgb(0xf5, 0x9e, 0x4b)));
                            ui.add_space(8.0);
                            
                            egui::ScrollArea::horizontal().show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if self.filmstrip.is_empty() {
                                        ui.label(RichText::new("Generating filmstrip...").color(Color32::from_rgb(0xa2, 0x95, 0x7f)));
                                    } else {
                                        for tex in &self.filmstrip {
                                            ui.image((tex.id(), Vec2::new(160.0, 90.0)));
                                        }
                                    }
                                });
                            });
                            
                            ui.add_space(15.0);
                            
                            ui.horizontal(|ui| {
                                ui.label("Start (HH:MM:SS):");
                                ui.text_edit_singleline(&mut self.trim_start);
                                ui.add_space(15.0);
                                ui.label("End (HH:MM:SS):");
                                ui.text_edit_singleline(&mut self.trim_end);
                            });
                            
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.label("Speed:");
                                ui.selectable_value(&mut self.export_speed, "0.5".to_string(), "0.5x");
                                ui.selectable_value(&mut self.export_speed, "1.0".to_string(), "1.0x");
                                ui.selectable_value(&mut self.export_speed, "1.5".to_string(), "1.5x");
                                ui.selectable_value(&mut self.export_speed, "2.0".to_string(), "2.0x");
                            });
                            
                            ui.add_space(15.0);
                            ui.horizontal(|ui| {
                                let file_clone = file.clone();
                                if ui.button(RichText::new("✂ Trim Video").color(Color32::from_rgb(0x1c, 0x14, 0x08)).strong()).clicked() {
                                    let out = file_clone.with_file_name(format!("trimmed_{}", file_clone.file_name().unwrap().to_str().unwrap()));
                                    self.spawn_ffmpeg_job(vec!["-y".into(), "-i".into(), file_clone.to_str().unwrap().into(), "-ss".into(), self.trim_start.clone(), "-to".into(), self.trim_end.clone(), "-c".into(), "copy".into(), out.to_str().unwrap().into()], "✂ Video trimmed!");
                                }
                                
                                if ui.button(RichText::new("🎞 Export Range to GIF").color(Color32::from_rgb(0xf5, 0x9e, 0x4b))).clicked() {
                                    let timestamp = Local::now().format("%H-%M-%S").to_string();
                                    let gif_out = file_clone.with_file_name(format!("clip_{}_{}.gif", self.trim_start.replace(":", "-"), timestamp));
                                    self.spawn_ffmpeg_job(vec!["-ss".into(), self.trim_start.clone(), "-to".into(), self.trim_end.clone(), "-i".into(), file_clone.to_str().unwrap().into(), "-vf".into(), "fps=15,scale=800:-1:flags=lanczos".into(), "-y".into(), gif_out.to_str().unwrap().into()], "🎞 GIF exported!");
                                }

                                if ui.button("🎵 Extract Audio").clicked() {
                                    let audio_out = file_clone.with_extension("m4a");
                                    self.spawn_ffmpeg_job(vec!["-y".into(), "-i".into(), file_clone.to_str().unwrap().into(), "-vn".into(), "-acodec".into(), "copy".into(), audio_out.to_str().unwrap().into()], "🎵 Audio extracted!");
                                }
                            });
                        });
                        
                        ui.add_space(10.0);
                        egui::CollapsingHeader::new(RichText::new("🎛 More video tools — extract frame, rotate, compress, mute, speed").color(Color32::from_rgb(0xf5, 0x9e, 0x4b)))
                            .default_open(false)
                            .show(ui, |ui| {
                                ui.label(RichText::new("Optional pro tools. Jobs report their real result when done — no fake success.").small().color(Color32::from_rgb(0x6d, 0x63, 0x50)));
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    let file_clone = file.clone();
                                    if ui.button("🖼 Extract Frame @ Start").clicked() {
                                        let out = file_clone.with_file_name(format!("frame_{}.jpg", self.trim_start.replace(":", "-")));
                                        self.spawn_ffmpeg_job(vec!["-ss".into(), self.trim_start.clone(), "-i".into(), file_clone.to_str().unwrap().into(), "-vframes".into(), "1".into(), "-q:v".into(), "2".into(), "-y".into(), out.to_str().unwrap().into()], "🖼 Frame extracted!");
                                    }
                                    if ui.button("🔇 Remove Audio").clicked() {
                                        let out = file_clone.with_file_name(format!("muted_{}", file_clone.file_name().unwrap().to_str().unwrap()));
                                        self.spawn_ffmpeg_job(vec!["-i".into(), file_clone.to_str().unwrap().into(), "-an".into(), "-c:v".into(), "copy".into(), "-y".into(), out.to_str().unwrap().into()], "🔇 Audio removed!");
                                    }
                                    if ui.button("🗜 Compress (CRF 28)").clicked() {
                                        let out = file_clone.with_file_name(format!("compressed_{}", file_clone.file_name().unwrap().to_str().unwrap()));
                                        self.spawn_ffmpeg_job(vec!["-i".into(), file_clone.to_str().unwrap().into(), "-c:v".into(), "libx264".into(), "-crf".into(), "28".into(), "-preset".into(), "medium".into(), "-c:a".into(), "aac".into(), "-b:a".into(), "96k".into(), "-y".into(), out.to_str().unwrap().into()], "🗜 Video compressed!");
                                    }
                                });
                                ui.horizontal(|ui| {
                                    let file_clone = file.clone();
                                    if ui.button("🔄 Rotate 90° CW").clicked() {
                                        let out = file_clone.with_file_name(format!("rot90_{}", file_clone.file_name().unwrap().to_str().unwrap()));
                                        self.spawn_ffmpeg_job(vec!["-i".into(), file_clone.to_str().unwrap().into(), "-vf".into(), "transpose=1".into(), "-y".into(), out.to_str().unwrap().into()], "🔄 Rotated 90° CW!");
                                    }
                                    if ui.button("🔄 Rotate 90° CCW").clicked() {
                                        let out = file_clone.with_file_name(format!("rot270_{}", file_clone.file_name().unwrap().to_str().unwrap()));
                                        self.spawn_ffmpeg_job(vec!["-i".into(), file_clone.to_str().unwrap().into(), "-vf".into(), "transpose=2".into(), "-y".into(), out.to_str().unwrap().into()], "🔄 Rotated 90° CCW!");
                                    }
                                    if ui.button("🔄 Rotate 180°").clicked() {
                                        let out = file_clone.with_file_name(format!("rot180_{}", file_clone.file_name().unwrap().to_str().unwrap()));
                                        self.spawn_ffmpeg_job(vec!["-i".into(), file_clone.to_str().unwrap().into(), "-vf".into(), "hflip,vflip".into(), "-y".into(), out.to_str().unwrap().into()], "🔄 Rotated 180°!");
                                    }
                                    if ui.button(format!("⏩ Apply {}x Speed", self.export_speed)).clicked() {
                                        let out = file_clone.with_file_name(format!("speed{}_{}", self.export_speed, file_clone.file_name().unwrap().to_str().unwrap()));
                                        self.spawn_ffmpeg_job(vec!["-i".into(), file_clone.to_str().unwrap().into(), "-filter:v".into(), format!("setpts=PTS/{}", self.export_speed), "-filter:a".into(), format!("atempo={}", self.export_speed), "-y".into(), out.to_str().unwrap().into()], "⏩ Speed change applied!");
                                    }
                                });
                            });

                        ui.add_space(15.0);
                        if ui.button("📂 Open Folder in Finder").clicked() {
                            let _ = reveal_in_file_manager(file);
                        }
                    } else {
                        ui.add_space(20.0);
                        ui.label(RichText::new("No video selected. Record a video or select one from the Media Library.").color(Color32::from_rgb(0x6d, 0x63, 0x50)));
                    }

                    ui.add_space(15.0);
                    egui::CollapsingHeader::new(RichText::new("🖼 Image tools — crop, rotate, resize, adjust").color(Color32::from_rgb(0xf5, 0x9e, 0x4b)))
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.label(RichText::new("Structural pic editing with live preview. Hidden by default to keep the studio clean.").small().color(Color32::from_rgb(0x6d, 0x63, 0x50)));
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                if ui.button("📂 Select Image").clicked() {
                                    if let Some(path) = FileDialog::new().add_filter("Image", &["jpg", "jpeg", "png", "gif", "webp"]).pick_file() {
                                        self.img_source_dims = image::image_dimensions(&path)
                                            .map(|(w, h)| format!("{}×{}", w, h))
                                            .unwrap_or_default();
                                        self.img_preview_params.clear();
                                        self.img_edit_file = Some(path);
                                    }
                                }
                                if let Some(p) = &self.img_edit_file {
                                    let dims = if self.img_source_dims.is_empty() { String::new() } else { format!(" · {} px", self.img_source_dims) };
                                    ui.label(RichText::new(format!("Editing: {}{}", p.file_name().unwrap().to_str().unwrap(), dims)).color(Color32::from_rgb(0xec, 0xe5, 0xd6)));
                                }
                            });
                            if self.img_edit_file.is_some() {
                                ui.horizontal(|ui| {
                                    ui.label("Rotate:");
                                    ui.selectable_value(&mut self.img_rotate, 0, "0°");
                                    ui.selectable_value(&mut self.img_rotate, 90, "90°");
                                    ui.selectable_value(&mut self.img_rotate, 180, "180°");
                                    ui.selectable_value(&mut self.img_rotate, 270, "270°");
                                    ui.checkbox(&mut self.img_flip_h, "Flip H");
                                    ui.checkbox(&mut self.img_flip_v, "Flip V");
                                    ui.checkbox(&mut self.img_grayscale, "Grayscale");
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Brightness:");
                                    ui.add(egui::Slider::new(&mut self.img_brightness, -100..=100));
                                    ui.label("Contrast:");
                                    ui.add(egui::Slider::new(&mut self.img_contrast, -100.0..=100.0));
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Blur:");
                                    ui.add(egui::Slider::new(&mut self.img_blur, 0.0..=10.0));
                                    ui.label("Resize %:");
                                    ui.add(egui::Slider::new(&mut self.img_resize_pct, 10..=200));
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Crop (pixels, optional):");
                                    ui.add(egui::TextEdit::singleline(&mut self.img_crop_x).hint_text("x").desired_width(50.0));
                                    ui.add(egui::TextEdit::singleline(&mut self.img_crop_y).hint_text("y").desired_width(50.0));
                                    ui.add(egui::TextEdit::singleline(&mut self.img_crop_w).hint_text("w").desired_width(50.0));
                                    ui.add(egui::TextEdit::singleline(&mut self.img_crop_h).hint_text("h").desired_width(50.0));
                                });
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut self.img_preview_on, "👁 Live preview");
                                    if ui.button("↺ Reset").clicked() {
                                        self.img_rotate = 0;
                                        self.img_flip_h = false;
                                        self.img_flip_v = false;
                                        self.img_grayscale = false;
                                        self.img_brightness = 0;
                                        self.img_contrast = 0.0;
                                        self.img_blur = 0.0;
                                        self.img_resize_pct = 100;
                                        self.img_crop_x.clear();
                                        self.img_crop_y.clear();
                                        self.img_crop_w.clear();
                                        self.img_crop_h.clear();
                                        self.img_preview_params.clear();
                                    }
                                });
                                if self.img_preview_on {
                                    self.refresh_img_preview(ui.ctx());
                                    if let Some(tex) = &self.img_preview_tex {
                                        let size = tex.size_vec2();
                                        let max_w = ui.available_width().min(480.0);
                                        let scale = (max_w / size.x).min(1.0);
                                        ui.image((tex.id(), size * scale));
                                    }
                                }
                                ui.add_space(6.0);
                                if ui.button(RichText::new("💾 Save Edited Image").color(Color32::from_rgb(0x1c, 0x14, 0x08)).strong()).clicked() {
                                    self.apply_image_edits();
                                }
                            }
                        });
                }

                AppTab::Feedback => {
                    if !self.feedback_scanned {
                        self.scan_feedback_requests();
                        self.feedback_scanned = true;
                    }

                    ui.horizontal(|ui| {
                        ui.heading(RichText::new("💬 Agent Feedback Inbox").color(Color32::from_rgb(0xf5, 0x9e, 0x4b)).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("🧹 Clear answered").clicked() {
                                self.clear_answered_feedback();
                            }
                            if ui.button("🔄 Refresh").clicked() {
                                self.scan_feedback_requests();
                            }
                        });
                    });
                    ui.label(RichText::new("Your AI assistant asks for your eyes here — answer now or later; it picks up your reply automatically.").small().color(Color32::from_rgb(0xa2, 0x95, 0x7f)));
                    ui.separator();

                    if self.feedback_requests.is_empty() {
                        ui.add_space(30.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("Nothing needs your eyes right now. When your AI assistant asks about a pic, GIF or video, it appears here.").color(Color32::from_rgb(0x6d, 0x63, 0x50)));
                        });
                    } else {
                        let requests = self.feedback_requests.clone();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for req in &requests {
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        let (icon, words) = if req.status == "answered" { ("✅", "Sent") } else { ("⏳", "Waiting for you") };
                                        ui.label(RichText::new(format!("{} {} — {}", icon, words, req.question)).strong().color(Color32::from_rgb(0xec, 0xe5, 0xd6)));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button("📂 Open Media").clicked() {
                                                let _ = open_path(std::path::Path::new(&req.media_path));
                                            }
                                            if req.status != "answered" {
                                                let lower = req.media_path.to_lowercase();
                                                let is_image = [".jpg", ".jpeg", ".png", ".gif", ".webp"].iter().any(|e| lower.ends_with(e));
                                                if is_image && ui.button("✏ Annotate & Reply").on_hover_text("Draw arrows, text and badges on the image, then send it back").clicked() {
                                                    self.annotating_feedback_id = Some(req.id.clone());
                                                    self.annotate_media(ui.ctx(), PathBuf::from(&req.media_path));
                                                }
                                                if ui.button("📝 Answer").clicked() {
                                                    self.feedback_selected = Some(req.id.clone());
                                                }
                                            }
                                        });
                                    });
                                    let fname = std::path::Path::new(&req.media_path)
                                        .file_name().map(|f| f.to_string_lossy().to_string())
                                        .unwrap_or_else(|| req.media_path.clone());
                                    ui.label(RichText::new(format!("{} · {}", req.created_at, fname)).small().color(Color32::from_rgb(0x6d, 0x63, 0x50)))
                                        .on_hover_text(&req.media_path);

                                    if req.status == "answered" {
                                        if !self.feedback_reply_cache.contains_key(&req.id) {
                                            if let Ok(s) = std::fs::read_to_string(feedback_responses_dir().join(format!("{}.json", req.id))) {
                                                if let Ok(resp) = serde_json::from_str::<FeedbackResponse>(&s) {
                                                    let mut txt = resp.feedback_text.clone();
                                                    if !resp.annotated_media_path.is_empty() { txt.push_str(&format!("\n🎨 {}", resp.annotated_media_path)); }
                                                    if !resp.voice_note_path.is_empty() { txt.push_str(&format!("\n🎙 {}", resp.voice_note_path)); }
                                                    self.feedback_reply_cache.insert(req.id.clone(), txt);
                                                }
                                            }
                                        }
                                        if let Some(reply) = self.feedback_reply_cache.get(&req.id).cloned() {
                                            egui::CollapsingHeader::new(RichText::new("View your reply").small().color(Color32::from_rgb(0xa2, 0x95, 0x7f)))
                                                .id_source(format!("reply_{}", req.id))
                                                .show(ui, |ui| {
                                                    ui.label(RichText::new(reply).color(Color32::from_rgb(0xec, 0xe5, 0xd6)));
                                                });
                                        }
                                    }

                                    if self.feedback_selected.as_deref() == Some(req.id.as_str()) && req.status != "answered" {
                                        ui.add_space(6.0);
                                        ui.label(RichText::new("Your feedback (sent back to the agent):").small().color(Color32::from_rgb(0xa2, 0x95, 0x7f)));
                                        ui.add(egui::TextEdit::multiline(&mut self.feedback_draft)
                                            .hint_text("Say what you see, what's off, or what to change…"));
                                        ui.horizontal(|ui| {
                                            let voice_label = if self.is_recording_voice_memo {
                                                RichText::new("🔴 Stop Voice Note").color(Color32::WHITE).strong()
                                            } else {
                                                RichText::new("🎙 Voice Note").color(Color32::from_rgb(0x5e, 0xc2, 0x6a)).strong()
                                            };
                                            if ui.button(voice_label).clicked() {
                                                let was_recording = self.is_recording_voice_memo;
                                                self.toggle_voice_memo();
                                                if !was_recording {
                                                    self.feedback_voice_note = self.active_voice_memo_path.clone();
                                                }
                                            }
                                            if let Some(p) = &self.feedback_voice_note {
                                                ui.label(RichText::new(format!("🎙 {}", p.file_name().unwrap_or_default().to_string_lossy())).small());
                                            }
                                        });
                                        ui.horizontal(|ui| {
                                            if ui.button(RichText::new("✅ Submit Feedback").color(Color32::from_rgb(0x1c, 0x14, 0x08)).strong()).clicked() {
                                                self.submit_feedback_response(&req.id);
                                            }
                                            if ui.button("Cancel").clicked() {
                                                self.feedback_selected = None;
                                            }
                                        });
                                    }
                                });
                                ui.add_space(4.0);
                            }
                        });
                    }
                }

                AppTab::Settings => {
                    ui.heading(RichText::new("Settings & Preferences").color(Color32::from_rgb(0xf5, 0x9e, 0x4b)).strong());
                    ui.add_space(15.0);
                    
                    ui.group(|ui| {
                        ui.label(RichText::new("SAVE LOCATION").small().color(Color32::from_rgb(0x6d, 0x63, 0x50)));
                        ui.add_space(4.0);
                        ui.label(format!("{}", self.save_dir.display()));
                        ui.add_space(8.0);
                        if ui.button("📂 Change Save Directory").clicked() {
                            if let Some(path) = FileDialog::new().pick_folder() {
                                self.save_dir = path;
                                self.refresh_library();
                                self.show_toast("Directory updated!");
                            }
                        }
                    });
                    
                    ui.add_space(15.0);
                    ui.group(|ui| {
                        ui.label(RichText::new("RECORDING").small().color(Color32::from_rgb(0x6d, 0x63, 0x50)));
                        ui.add_space(4.0);
                        ui.label(RichText::new("Framerate").small().color(Color32::from_rgb(0xa2, 0x95, 0x7f)));
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.fps_target, 30, "30 FPS (Balanced)");
                            ui.radio_value(&mut self.fps_target, 60, "60 FPS (Pro High-FPS)");
                        });
                        ui.add_space(6.0);
                        ui.checkbox(&mut self.capture_audio, "Include audio when recording");
                        ui.label(
                            RichText::new("Video-only by default. Enable audio for mic/system sound (platform-dependent).")
                                .small()
                                .color(Color32::from_rgb(0x6d, 0x63, 0x50)),
                        );
                    });
                    
                    ui.add_space(15.0);
                    ui.group(|ui| {
                        ui.label(RichText::new("SHORTCUTS & TRAY").small().color(Color32::from_rgb(0x6d, 0x63, 0x50)));
                        ui.add_space(4.0);
                        ui.label("In app (window focused)");
                        ui.label("  S  — Screenshot");
                        ui.label("  R  — Start / stop recording");
                        ui.add_space(4.0);
                        ui.label("Global (works from tray / other apps)");
                        ui.label("  Ctrl + Shift + 3  — Screenshot");
                        ui.label("  Ctrl + Shift + 2  — Start / stop recording");
                        ui.add_space(4.0);
                        ui.label("Tray menu: Screenshot · Record (shows live timer) · Feedback · Quit");
                        ui.label("Close window → hide to tray (not quit)");
                    });

                    ui.add_space(15.0);
                    ui.group(|ui| {
                        ui.label(RichText::new("🤖 AGENT SESSION & BUDGET").small().color(Color32::from_rgb(0x6d, 0x63, 0x50)));
                        ui.add_space(4.0);
                        if !self.budget_loaded {
                            let cfg = load_budget();
                            self.budget_frames_input = cfg.max_frames.to_string();
                            self.budget_mb_input = format!("{:.1}", cfg.max_mb);
                            self.budget_minutes_input = cfg.max_minutes.to_string();
                            self.budget_tier = cfg.analysis_tier.clone();
                            self.budget_loaded = true;
                        }
                        ui.label(RichText::new("Intensive frame analysis can be expensive — these caps control agent spending. Agents adjust them via their budget tool; you can override here any time. 0 = no limit. When a cap is hit, the agent's stream stops and it's told the budget is spent.").small().color(Color32::from_rgb(0xa2, 0x95, 0x7f)));
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label("Max frames:");
                            ui.add(egui::TextEdit::singleline(&mut self.budget_frames_input).desired_width(55.0));
                            ui.label("Max MB:");
                            ui.add(egui::TextEdit::singleline(&mut self.budget_mb_input).desired_width(60.0));
                            ui.label("Max minutes:");
                            ui.add(egui::TextEdit::singleline(&mut self.budget_minutes_input).desired_width(55.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Analysis tier:");
                            ui.selectable_value(&mut self.budget_tier, "eco".to_string(), "🌱 Eco — a still every few seconds");
                            ui.selectable_value(&mut self.budget_tier, "standard".to_string(), "⚖ Standard — short GIF ~3s");
                            ui.selectable_value(&mut self.budget_tier, "intensive".to_string(), "🔥 Intensive — 1s, expensive");
                        });
                        ui.horizontal(|ui| {
                            if ui.button("💾 Save Budget").clicked() {
                                let frames_p = self.budget_frames_input.trim().parse::<u32>();
                                let mb_p = self.budget_mb_input.trim().parse::<f64>();
                                let mins_p = self.budget_minutes_input.trim().parse::<u32>();
                                match (frames_p, mb_p, mins_p) {
                                    (Ok(f), Ok(mb), Ok(m)) if mb.is_finite() && mb >= 0.0 => {
                                        let cfg = BudgetConfig {
                                            max_frames: f,
                                            max_mb: mb,
                                            max_minutes: m,
                                            analysis_tier: self.budget_tier.clone(),
                                        };
                                        match save_budget(&cfg) {
                                            Ok(_) => self.show_toast("💾 Budget saved — agents follow these caps."),
                                            Err(e) => self.show_toast(&format!("❌ Could not save budget: {}", e)),
                                        }
                                    }
                                    _ => self.show_toast("❌ Budget values must be non-negative whole numbers (0 = no limit) — not saved."),
                                }
                            }
                            if ui.button("🔄 Reload").clicked() {
                                self.budget_loaded = false;
                            }
                        });
                        ui.add_space(6.0);
                        let live_dir = default_live_dir().display().to_string();
                        let (frames, mb, _) = live_usage_snapshot(&live_dir);
                        let cfg_now = load_budget();
                        let frames_cap = if cfg_now.max_frames == 0 { "∞".to_string() } else { cfg_now.max_frames.to_string() };
                        let mb_cap = if cfg_now.max_mb <= 0.0 { "∞".to_string() } else { format!("{:.0}", cfg_now.max_mb) };
                        ui.label(RichText::new(format!("Live session now: {}/{} frames · {:.1}/{} MB · tier {}", frames, frames_cap, mb, mb_cap, cfg_now.analysis_tier)).small().color(Color32::from_rgb(0xa2, 0x95, 0x7f)));
                    });
                }
            }
        });

        if let Some((msg, time)) = &self.toast_message {
            if time.elapsed() < Duration::from_secs(4) {
                let ctx = ctx.clone();
                egui::TopBottomPanel::bottom("toast_panel")
                    .frame(Frame::none().fill(Color32::from_rgb(0xf5, 0x9e, 0x4b)).inner_margin(6.0))
                    .show(&ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(msg).color(Color32::from_rgb(0x1c, 0x14, 0x08)).strong());
                        });
                    });
            }
        }
    }
}

fn get_dir_size_bytes(dir_path: &str) -> (u64, usize) {
    let mut total_size = 0u64;
    let mut count = 0usize;
    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total_size += meta.len();
                    count += 1;
                }
            }
        }
    }
    (total_size, count)
}

fn run_mcp_server() {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() { continue; }

        let parsed: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = parsed.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = parsed.get("id").cloned();

        match method {
            "initialize" => {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "vibecap",
                            "version": "0.1.0"
                        }
                    }
                });
                let _ = writeln!(handle, "{}", response.to_string());
                let _ = handle.flush();
            }
            "notifications/initialized" => {}
            "ping" => {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {}
                });
                let _ = writeln!(handle, "{}", response.to_string());
                let _ = handle.flush();
            }
            "tools/list" => {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": [
                            {
                                "name": "vibecap_capture",
                                "description": "Captures a full-screen screenshot to the Vibecap media folder (Videos/Vibecap or ~/Movies/Vibecap). Optionally focuses an app first. For pen/arrow/step-badge annotation, open the desktop app Annotation Studio.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "app_name": {
                                            "type": "string",
                                            "description": "Optional application name to focus before capture (e.g. iTerm, Google Chrome, Simulator)"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "vibecap_record_video",
                                "description": "Records continuous video clip of an application or screen for duration_secs and exports MP4 + motion GIF",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "app_name": {
                                            "type": "string",
                                            "description": "Optional application name to focus before recording (e.g. Google Chrome, iTerm, Simulator)"
                                        },
                                        "duration_secs": {
                                            "type": "number",
                                            "description": "Duration to record video in seconds (default: 5)"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "vibecap_export_gif",
                                "description": "Extracts high-FPS GIF around start/end timeline timestamps",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "video_path": {
                                            "type": "string",
                                            "description": "Path to input video file"
                                        },
                                        "start_time": {
                                            "type": "string",
                                            "description": "Start timestamp (HH:MM:SS)"
                                        },
                                        "end_time": {
                                            "type": "string",
                                            "description": "End timestamp (HH:MM:SS)"
                                        }
                                    },
                                    "required": ["video_path", "start_time", "end_time"]
                                }
                            },
                            {
                                "name": "vibecap_start_live_inspection",
                                "description": "Starts continuous background live inspection recording emitting rolling frames (gif, jpg, or mp4) every N seconds into a repo temp directory so AI agent can inspect user actions live while keeping user aware of disk storage usage.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "app_name": {
                                            "type": "string",
                                            "description": "Optional application name to focus before starting live stream (e.g. Google Chrome, iTerm, Simulator)"
                                        },
                                        "format": {
                                            "type": "string",
                                            "description": "Media format to emit: 'gif' (animated clip, default), 'jpg' (fast screenshot), or 'mp4' (video chunk)"
                                        },
                                        "interval_secs": {
                                            "type": "number",
                                            "description": "Frequency/interval in seconds between live frame emissions (default: 3)"
                                        },
                                        "output_dir": {
                                            "type": "string",
                                            "description": "Target output directory (default: platform media folder /live, e.g. Videos/Vibecap/live)"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "vibecap_get_live_frame",
                                "description": "Fetches the file path of the latest live emitted frame along with current session disk storage usage",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {}
                                }
                            },
                            {
                                "name": "vibecap_stop_live_inspection",
                                "description": "Stops the active continuous background live inspection stream and reports final disk storage summary",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {}
                                }
                            },
                            {
                                "name": "vibecap_set_budget",
                                "description": "Sets agent spending controls for frame/media analysis: caps on frames captured, storage MB, and session minutes, plus an analysis tier (eco/standard/intensive). Intensive frame analysis can be expensive — use eco when exploring. Live inspection auto-stops and new streams are refused once a cap is reached. Shared with the Vibecap app (Settings → Agent Session & Budget).",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "max_frames": {
                                            "type": "number",
                                            "description": "Maximum frames the live session may capture before auto-stop (0 = unlimited)"
                                        },
                                        "max_mb": {
                                            "type": "number",
                                            "description": "Maximum live-session storage in megabytes before auto-stop (0 = unlimited)"
                                        },
                                        "max_minutes": {
                                            "type": "number",
                                            "description": "Maximum live-session minutes before auto-stop (0 = unlimited)"
                                        },
                                        "analysis_tier": {
                                            "type": "string",
                                            "enum": ["eco", "standard", "intensive"],
                                            "description": "eco = jpg @ >=5s intervals (cheapest), standard = gif @ ~3s (balanced), intensive = gif/mp4 @ 1s (richest, most expensive frame analysis)"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "vibecap_get_spending",
                                "description": "Reports current session spending: frames captured, storage MB, elapsed minutes, the active caps, analysis tier, and whether the budget is exhausted. Call before and during intensive analysis to control costs.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {}
                                }
                            },
                            {
                                "name": "vibecap_request_feedback",
                                "description": "Requests human-in-the-loop feedback on a screenshot, GIF, or video. The request appears in the Vibecap app Feedback Inbox; the human can answer live in-session or any time after you submit. Returns a request_id to poll with vibecap_get_feedback.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "media_path": {
                                            "type": "string",
                                            "description": "Absolute path to the pic/gif/video the human should review"
                                        },
                                        "question": {
                                            "type": "string",
                                            "description": "The specific question for the human (e.g. 'Does this animation look right?')"
                                        }
                                    },
                                    "required": ["media_path", "question"]
                                }
                            },
                            {
                                "name": "vibecap_get_feedback",
                                "description": "Retrieves the human's feedback for a request_id previously created with vibecap_request_feedback. Returns pending status if the human has not answered yet.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "request_id": {
                                            "type": "string",
                                            "description": "The request_id returned by vibecap_request_feedback"
                                        }
                                    },
                                    "required": ["request_id"]
                                }
                            }
                        ]
                    }
                });
                let _ = writeln!(handle, "{}", response.to_string());
                let _ = handle.flush();
            }
            "tools/call" => {
                let tool_name = parsed.get("params")
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");

                let (content_text, is_error) = match tool_name {
                    "vibecap_capture" => {
                        let app_name = parsed.get("params")
                            .and_then(|p| p.get("arguments"))
                            .and_then(|a| a.get("app_name"))
                            .and_then(|s| s.as_str());

                        if let Some(app) = app_name {
                            let _ = focus_app(app);
                        }

                        match capture_screenshot_to_media_dir() {
                            Ok(out) => (format!("Captured screenshot successfully to {}", out.display()), false),
                            Err(e) => (e, true),
                        }
                    }
                    "vibecap_record_video" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let app_name = args.and_then(|a| a.get("app_name")).and_then(|s| s.as_str());
                        let raw_duration = args.and_then(|a| a.get("duration_secs")).and_then(|v| v.as_u64()).unwrap_or(5);
                        let duration_secs = raw_duration.min(600);
                        let clamp_note = if raw_duration > 600 { " (clamped from your request — 600s max per clip)" } else { "" };

                        let home_live = mcp_live_dir().display().to_string();
                        if let Some(reason) = budget_exceeded_reason(&home_live) {
                            (format!("⚠️ BUDGET EXHAUSTED — recording refused: {}. Raise caps with vibecap_set_budget or ask the human to adjust them in the Vibecap app.", reason), true)
                        } else {
                        if let Some(app) = app_name {
                            let _ = focus_app(app);
                        }

                        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                        let media = default_media_dir();
                        // Unique filenames so concurrent agent instances never clash.
                        let pid = std::process::id();
                        let out_mp4 = media.join(format!("video_{}_{}.mp4", timestamp, pid));
                        let out_gif = media.join(format!("video_{}_{}_clip.gif", timestamp, pid));

                        match record_screen_clip(&out_mp4, duration_secs) {
                            Ok(()) => {
                                let gif_s = out_gif.display().to_string();
                                let mp4_s = out_mp4.display().to_string();
                                // Companion motion GIF for the whole clip (ignore range export failures).
                                let _ = Command::new("ffmpeg")
                                    .args([
                                        "-i", &mp4_s,
                                        "-vf", "fps=15,scale=800:-1:flags=lanczos",
                                        "-y", &gif_s,
                                    ])
                                    .status();
                                (format!("Successfully recorded {}s video to {} and exported GIF to {}{}", duration_secs, mp4_s, gif_s, clamp_note), false)
                            }
                            Err(e) => (format!("Failed to record video: {}", e), true),
                        }
                        }
                    }
                    "vibecap_export_gif" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let video_path = args.and_then(|a| a.get("video_path")).and_then(|s| s.as_str()).unwrap_or("");
                        let start_time = args.and_then(|a| a.get("start_time")).and_then(|s| s.as_str()).unwrap_or("00:00:00");
                        let end_time = args.and_then(|a| a.get("end_time")).and_then(|s| s.as_str()).unwrap_or("00:00:05");

                        let gif_out = format!("{}_clip.gif", video_path.trim_end_matches(".mp4"));
                        match export_gif_clip(video_path, start_time, end_time, &gif_out) {
                            Ok(()) => (format!("Exported timeline GIF to {}", gif_out), false),
                            Err(e) => (format!("Failed to export GIF snippet: {}", e), true),
                        }
                    }
                    "vibecap_start_live_inspection" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let app_name = args.and_then(|a| a.get("app_name")).and_then(|s| s.as_str()).map(|s| s.to_string());
                        // When format/interval are omitted, the analysis tier drives the defaults —
                        // this makes eco/standard/intensive mechanically real, not just advisory.
                        let budget_now = load_budget();
                        let format_choice = args.and_then(|a| a.get("format")).and_then(|s| s.as_str()).map(|s| s.to_lowercase())
                            .unwrap_or_else(|| if budget_now.analysis_tier == "eco" { "jpg".to_string() } else { "gif".to_string() });
                        let interval_secs = args.and_then(|a| a.get("interval_secs")).and_then(|v| v.as_u64())
                            .unwrap_or_else(|| match budget_now.analysis_tier.as_str() { "eco" => 5, "intensive" => 1, _ => 3 });
                        
                        // Per-process session dir so several MCP servers can stream at once.
                        let default_dir = mcp_live_dir().display().to_string();
                        let live_dir = args.and_then(|a| a.get("output_dir")).and_then(|s| s.as_str()).unwrap_or(&default_dir).to_string();

                        if LIVE_INSPECTION_RUNNING.load(Ordering::SeqCst) {
                            ("Live inspection is already running in this MCP process! Call vibecap_get_live_frame, or vibecap_stop_live_inspection. Other agent instances may run their own streams in parallel.".to_string(), false)
                        } else if let Some(reason) = budget_exceeded_reason(&live_dir) {
                            (format!("⚠️ BUDGET EXHAUSTED — live inspection refused: {}. Raise the caps with vibecap_set_budget, ask the human to adjust them in the Vibecap app (Settings → Agent Session & Budget), or clean up {}.", reason, live_dir), true)
                        } else {
                            LIVE_INSPECTION_RUNNING.store(true, Ordering::SeqCst);
                            if let Ok(mut l) = get_live_started_mutex().lock() { *l = Some(Instant::now()); }
                            if let Ok(mut n) = get_budget_note_mutex().lock() { n.clear(); }
                            let _ = std::fs::create_dir_all(&live_dir);

                            if let Some(app) = &app_name {
                                let _ = focus_app(app);
                            }

                            let dir_clone = live_dir.clone();
                            let fmt_clone = format_choice.clone();
                            std::thread::spawn(move || {
                                let live_fmt = LiveFormat::from_str_loose(&fmt_clone);
                                while LIVE_INSPECTION_RUNNING.load(Ordering::SeqCst) {
                                    // Budget enforcement: auto-stop the stream when any cap is reached.
                                    if let Some(reason) = budget_exceeded_reason(&dir_clone) {
                                        LIVE_INSPECTION_RUNNING.store(false, Ordering::SeqCst);
                                        if let Ok(mut l) = get_live_started_mutex().lock() { *l = None; }
                                        if let Ok(mut n) = get_budget_note_mutex().lock() {
                                            *n = format!("BUDGET_EXHAUSTED — auto-stopped: {}", reason);
                                        }
                                        break;
                                    }

                                    let (latest_frame, timestamped_frame) =
                                        match capture_live_frame(&dir_clone, live_fmt, interval_secs) {
                                            Ok(pair) => pair,
                                            Err(_) => (String::new(), String::new()),
                                        };

                                    if !timestamped_frame.is_empty() {
                                        if let Ok(mut lock) = get_latest_live_gif_mutex().lock() {
                                            *lock = format!("{}|{}|{}", fmt_clone, latest_frame, timestamped_frame);
                                        }
                                    }

                                    if live_fmt == LiveFormat::Jpg {
                                        std::thread::sleep(Duration::from_secs(interval_secs));
                                    }
                                }
                            });

                            (format!("Started live inspection (format: {}, frequency: {}s, output_dir: {}).\n⚠️ STORAGE AWARENESS: Live frames are being stored in {}. Remember to inform the user about storage usage and call vibecap_stop_live_inspection when done.\n{}", format_choice, interval_secs, live_dir, live_dir, budget_status_line(&live_dir)), false)
                        }
                    }
                    "vibecap_get_live_frame" => {
                        let is_running = LIVE_INSPECTION_RUNNING.load(Ordering::SeqCst);
                        let state = get_latest_live_gif_mutex().lock().map(|l| l.clone()).unwrap_or_default();
                        
                        let parts: Vec<&str> = state.split('|').collect();
                        let (fmt, latest_frame, ts_frame) = if parts.len() == 3 {
                            (parts[0], parts[1], parts[2])
                        } else {
                            ("unknown", "", "")
                        };

                        let default_dir = mcp_live_dir().display().to_string();
                        let target_dir = if !ts_frame.is_empty() {
                            std::path::Path::new(ts_frame).parent().and_then(|p| p.to_str()).unwrap_or(&default_dir)
                        } else {
                            &default_dir
                        };

                        let (bytes, count) = get_dir_size_bytes(target_dir);
                        let mb = (bytes as f64) / (1024.0 * 1024.0);

                        if !is_running && ts_frame.is_empty() {
                            ("Live inspection is not running in this MCP process. Call vibecap_start_live_inspection first.".to_string(), true)
                        } else {
                            (format!("Status: live_running={}, format={}, latest_frame={}, timestamped_frame={}\n📊 STORAGE AWARENESS: Total session storage used: {:.2} MB across {} frame files in {}\n{}", is_running, fmt, latest_frame, ts_frame, mb, count, target_dir, budget_status_line(target_dir)), false)
                        }
                    }
                    "vibecap_stop_live_inspection" => {
                        LIVE_INSPECTION_RUNNING.store(false, Ordering::SeqCst);
                        if let Ok(mut l) = get_live_started_mutex().lock() { *l = None; }
                        let state = get_latest_live_gif_mutex().lock().map(|l| l.clone()).unwrap_or_default();
                        let parts: Vec<&str> = state.split('|').collect();
                        let ts_frame = if parts.len() == 3 { parts[2] } else { "" };
                        
                        let default_dir = mcp_live_dir().display().to_string();
                        let target_dir = if !ts_frame.is_empty() {
                            std::path::Path::new(ts_frame).parent().and_then(|p| p.to_str()).unwrap_or(&default_dir)
                        } else {
                            &default_dir
                        };

                        let (bytes, count) = get_dir_size_bytes(target_dir);
                        let mb = (bytes as f64) / (1024.0 * 1024.0);

                        (format!("Stopped live inspection stream.\n📊 FINAL STORAGE SUMMARY: Captured {} frames occupying {:.2} MB in {}. Inform the user so they can review or clean up temporary storage if desired.\n{}", count, mb, target_dir, budget_status_line(target_dir)), false)
                    }
                    "vibecap_set_budget" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let mut cfg = load_budget();
                        let mut notes: Vec<String> = Vec::new();
                        let mut invalid = false;
                        if let Some(a) = args {
                            if let Some(v) = a.get("max_frames") {
                                match v.as_u64() {
                                    Some(n) => cfg.max_frames = u32::try_from(n).unwrap_or_else(|_| { notes.push("max_frames clamped to u32::MAX".to_string()); u32::MAX }),
                                    None => invalid = true,
                                }
                            }
                            if let Some(v) = a.get("max_mb") {
                                match v.as_f64() {
                                    Some(n) if n.is_finite() && n >= 0.0 => cfg.max_mb = n,
                                    _ => invalid = true,
                                }
                            }
                            if let Some(v) = a.get("max_minutes") {
                                match v.as_u64() {
                                    Some(n) => cfg.max_minutes = u32::try_from(n).unwrap_or_else(|_| { notes.push("max_minutes clamped to u32::MAX".to_string()); u32::MAX }),
                                    None => invalid = true,
                                }
                            }
                            if let Some(v) = a.get("analysis_tier") {
                                match v.as_str() {
                                    Some(t) => {
                                        let t = t.to_lowercase();
                                        if t == "eco" || t == "standard" || t == "intensive" { cfg.analysis_tier = t; } else { invalid = true; }
                                    }
                                    None => invalid = true,
                                }
                            }
                        }
                        if invalid {
                            ("Invalid budget arguments: analysis_tier must be eco|standard|intensive and caps must be non-negative numbers. Nothing was saved.".to_string(), true)
                        } else if let Err(e) = save_budget(&cfg) {
                            (format!("Failed to save budget: {}", e), true)
                        } else {
                            let tier_guidance = match cfg.analysis_tier.as_str() {
                                "eco" => "eco: defaults to format='jpg' at 5s intervals — fewest frames, cheapest analysis.",
                                "intensive" => "intensive: defaults to 1s gif/mp4 — richest motion detail, but frame analysis is EXPENSIVE. Poll vibecap_get_spending and downshift when exploring.",
                                _ => "standard: defaults to gif at ~3s intervals — balanced detail vs cost.",
                            };
                            let notes_txt = if notes.is_empty() { String::new() } else { format!("\n⚠️ {}", notes.join("; ")) };
                            (format!("Budget updated: max_frames={} (0=unlimited), max_mb={:.1} (0=unlimited), max_minutes={} (0=unlimited), analysis_tier={}.\n💡 TIER GUIDANCE: {}\nCaps are enforced live: the stream auto-stops and new streams are refused once a cap is hit. The same budget is visible to the human in the Vibecap app (Settings → Agent Session & Budget).{}", cfg.max_frames, cfg.max_mb, cfg.max_minutes, cfg.analysis_tier, tier_guidance, notes_txt), false)
                        }
                    }
                    "vibecap_get_spending" => {
                        let state = get_latest_live_gif_mutex().lock().map(|l| l.clone()).unwrap_or_default();
                        let parts: Vec<&str> = state.split('|').collect();
                        let ts_frame = if parts.len() == 3 { parts[2] } else { "" };
                        let default_dir = mcp_live_dir().display().to_string();
                        let target_dir = if !ts_frame.is_empty() {
                            std::path::Path::new(ts_frame).parent().and_then(|p| p.to_str()).unwrap_or(&default_dir)
                        } else {
                            &default_dir
                        };
                        let (frames, mb, minutes) = live_usage_snapshot(target_dir);
                        let cfg = load_budget();
                        let frames_cap = if cfg.max_frames == 0 { "unlimited".to_string() } else { cfg.max_frames.to_string() };
                        let mb_cap = if cfg.max_mb <= 0.0 { "unlimited".to_string() } else { format!("{:.1}", cfg.max_mb) };
                        let min_cap = if cfg.max_minutes == 0 { "unlimited".to_string() } else { cfg.max_minutes.to_string() };
                        let status = match budget_exceeded_reason(target_dir) {
                            Some(r) => format!("⚠️ BUDGET EXHAUSTED: {}", r),
                            None => "within budget".to_string(),
                        };
                        let tier_note = if cfg.analysis_tier == "intensive" { " — frame analysis at this tier is expensive; downshift to eco when just exploring" } else { "" };
                        (format!("📊 SESSION SPENDING\nFrames captured: {} (cap: {})\nStorage: {:.2} MB (cap: {})\nElapsed: {:.1} min (cap: {})\nAnalysis tier: {}{}\nStatus: {}", frames, frames_cap, mb, mb_cap, minutes, min_cap, cfg.analysis_tier, tier_note, status), false)
                    }
                    "vibecap_request_feedback" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let media_path = args.and_then(|a| a.get("media_path")).and_then(|s| s.as_str()).unwrap_or("");
                        let question = args.and_then(|a| a.get("question")).and_then(|s| s.as_str()).unwrap_or("");
                        if media_path.is_empty() || question.is_empty() {
                            ("Missing required arguments: media_path and question".to_string(), true)
                        } else if !std::path::Path::new(media_path).exists() {
                            (format!("media_path does not exist: {}", media_path), true)
                        } else {
                            let id = format!("fb_{}", Local::now().format("%Y%m%d_%H%M%S%3f"));
                            let req = FeedbackRequest {
                                id: id.clone(),
                                media_path: media_path.to_string(),
                                question: question.to_string(),
                                created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                                status: "pending".to_string(),
                            };
                            let req_path = feedback_requests_dir().join(format!("{}.json", id));
                            match serde_json::to_string_pretty(&req).map_err(|e| e.to_string())
                                .and_then(|s| write_json_atomic(&req_path, &s)) {
                                Ok(_) => (format!("Feedback request '{}' submitted for {}.\n🧑 HUMAN-IN-THE-LOOP: The human can answer in the Vibecap app → 💬 Feedback Inbox — live in-session, or any time after you submit. Poll vibecap_get_feedback with request_id='{}' to pick up their answer.", id, media_path, id), false),
                                Err(e) => (format!("Failed to persist feedback request: {}", e), true),
                            }
                        }
                    }
                    "vibecap_get_feedback" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let request_id = args.and_then(|a| a.get("request_id")).and_then(|s| s.as_str()).unwrap_or("");
                        if request_id.is_empty() || !request_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                            ("Invalid request_id (allowed: letters, digits, _ and -)".to_string(), true)
                        } else {
                            let resp_path = feedback_responses_dir().join(format!("{}.json", request_id));
                            if let Ok(s) = std::fs::read_to_string(&resp_path) {
                                match serde_json::from_str::<FeedbackResponse>(&s) {
                                    Ok(resp) => {
                                        let mut extra = String::new();
                                        if !resp.annotated_media_path.is_empty() { extra.push_str(&format!("\n🎨 Annotated image: {}", resp.annotated_media_path)); }
                                        if !resp.voice_note_path.is_empty() { extra.push_str(&format!("\n🎙 Voice note: {}", resp.voice_note_path)); }
                                        (format!("✅ Human feedback for {}:\n\"{}\"{}", request_id, resp.feedback_text, extra), false)
                                    }
                                    Err(_) => ("Corrupt feedback response file".to_string(), true),
                                }
                            } else {
                                let req_path = feedback_requests_dir().join(format!("{}.json", request_id));
                                if req_path.exists() {
                                    (format!("⏳ Feedback request '{}' is still pending — the human has not answered yet. They can answer in-session or later via the Vibecap app Feedback Inbox; poll again shortly.", request_id), false)
                                } else {
                                    (format!("Unknown request_id: {}", request_id), true)
                                }
                            }
                        }
                    }
                    _ => (format!("Unknown tool: {}", tool_name), true),
                };

                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": content_text
                            }
                        ],
                        "isError": is_error
                    }
                });
                let _ = writeln!(handle, "{}", response.to_string());
                let _ = handle.flush();
            }
            _ => {
                if let Some(id_val) = id {
                    let response = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "error": {
                            "code": -32601,
                            "message": "Method not found"
                        }
                    });
                    let _ = writeln!(handle, "{}", response.to_string());
                    let _ = handle.flush();
                }
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
        println!("           request_feedback | get_feedback");
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

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_decorations(true)
            .with_transparent(false)
            .with_inner_size([760.0, 640.0])
            .with_min_inner_size([640.0, 560.0])
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
                match TrayController::try_new("Vibecap Studio — click to show") {
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
