#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod platform;
mod tray_ui;
mod ui;

use chrono::{Local, Timelike};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use egui::{
    Align2, Color32, FontId, Frame, Pos2, Rect, RichText, Stroke, UserAttentionType, Vec2,
    ViewportBuilder, ViewportCommand, ViewportId,
};
use global_hotkey::GlobalHotKeyEvent;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tray_icon::{menu::MenuEvent, TrayIconEvent};

use platform::{
    capture_screenshot, capture_screenshot_interactive, capture_screenshot_opts, cont_process,
    crop_image_file, even_screen_rect, export_gif_clip, focus_app, frontmost_app_name,
    list_running_apps, notify_agent_question, open_screen_recording_settings,
    record_screen_clip_opts, request_screen_recording_access, reveal_in_file_manager,
    screen_capture_allowed, spawn_screen_recorder_opts, spawn_voice_memo, stop_process,
    CaptureOpts, ScreenRect,
};
use tray_ui::{TrayAction, TrayController, TrayLiveState};
use ui::theme;
use ui::{
    apply_celestial_theme, apply_current_theme, apply_graphite_theme, funnel_stripe, loop_rail,
    overlay_rect_to_pixels, show_capture_toast, show_countdown_bubble, show_palette,
    show_region_selector, show_toast_card, status_strip, CaptureToastAction, Density, LoopStage,
    PaletteAction, RegionHudResult, StatusSnapshot, ThemeMode, ToastLevel,
};

use app::annotation_baker::{snap_annotation_point, AnnotationAction, AnnotationTool};
use app::io::vibecap_config_dir;
use app::session::{density_from_str, density_to_str, load_session, save_session, SessionState};
use app::{
    budget_exceeded_reason, default_live_dir, default_media_dir, extract_filmstrip_rgba,
    feedback_requests_dir, feedback_responses_dir, filter_items, finalize_recorder,
    format_feedback_answer, get_dir_size_bytes, kill_recorder, live_usage_snapshot, load_budget,
    mcp_live_dir, parse_args, run_headless, run_mcp_server, scan_media_dir, take_pending_still,
    write_json_atomic, write_pending_still, write_pending_still_error, CliAction, FeedbackRequest,
    FeedbackResponse, MediaCategory, MediaItem, LIBRARY_PAGE_SIZE,
};

/// Cached live-dir stats for the Capture tab's proof-of-life row.
#[derive(Clone)]
pub(crate) struct LiveStats {
    pub count: usize,
    pub mb: f64,
    pub frames_cap: u32,
    pub mb_cap: f64,
    pub minutes_cap: u32,
    pub tier: String,
    pub over: Option<String>,
}

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
    pub(crate) fn to_loop(self) -> LoopStage {
        match self {
            Self::Capture => LoopStage::Shutter,
            Self::Library => LoopStage::Media,
            Self::Clip | Self::Still => LoopStage::Review,
            Self::Feedback => LoopStage::Inbox,
            Self::Settings => LoopStage::Settings,
        }
    }

    /// Session/palette key for the stage — stable across renames.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Library => "library",
            Self::Clip => "clip",
            Self::Still => "still",
            Self::Feedback => "feedback",
            Self::Settings => "settings",
        }
    }

    /// E50 — position in the visual rail (stage, slot within stage) so tab
    /// transitions can slide in the direction the rail implies.
    fn rail_pos(self) -> (usize, usize) {
        let stage = match self.to_loop() {
            crate::ui::components::LoopStage::Shutter => 0,
            crate::ui::components::LoopStage::Review => 1,
            crate::ui::components::LoopStage::Media => 2,
            crate::ui::components::LoopStage::Inbox => 3,
            crate::ui::components::LoopStage::Settings => 4,
        };
        let slot = match self {
            Self::Clip => 0,
            Self::Still => 1,
            _ => 0,
        };
        (stage, slot)
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Capture => "Capture",
            Self::Library => "Library",
            Self::Clip => "Review · Clip",
            Self::Still => "Review · Still",
            Self::Feedback => "Inbox",
            Self::Settings => "Settings",
        }
    }

    /// One-line hint under the stage title — tells you what this screen is for.
    pub(crate) fn subtitle(self) -> &'static str {
        match self {
            Self::Capture => "Grab it, mark it, ship it",
            Self::Library => "Everything you've captured",
            Self::Clip => "Trim it, GIF it, ship it",
            Self::Still => "Mark it up, copy it out",
            Self::Feedback => "Your agent is waiting on you",
            Self::Settings => "",
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum CaptureTarget {
    Fullscreen,
    Region,
    Window,
}

impl CaptureTarget {
    /// E51 — stable name for per-target memory persistence.
    fn name(self) -> &'static str {
        match self {
            Self::Fullscreen => "Fullscreen",
            Self::Region => "Region",
            Self::Window => "Window",
        }
    }
    fn from_name(name: &str) -> Self {
        match name {
            "Region" => Self::Region,
            "Window" => Self::Window,
            _ => Self::Fullscreen,
        }
    }
}

/// What the most recent finished capture was — drives the app-level Ctrl+C
/// "copy last capture" shortcut from any tab.
#[derive(Clone)]
enum LastCapture {
    Still(PathBuf),
    Clip(PathBuf),
}

/// Still export encoder — drives the EXPORT group (format chips + quality).
#[derive(PartialEq, Clone, Copy, Default)]
pub(crate) enum StillExportFmt {
    #[default]
    Jpg,
    Png,
    WebP,
}

impl StillExportFmt {
    pub(crate) fn ext(self) -> &'static str {
        match self {
            StillExportFmt::Jpg => "jpg",
            StillExportFmt::Png => "png",
            StillExportFmt::WebP => "webp",
        }
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            StillExportFmt::Jpg => "JPG",
            StillExportFmt::Png => "PNG",
            StillExportFmt::WebP => "WebP",
        }
    }
    /// Lossy formats expose a quality slider; PNG/WebP are lossless here.
    pub(crate) fn lossy(self) -> bool {
        matches!(self, StillExportFmt::Jpg)
    }
}

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum RegionPickKind {
    Screenshot,
    Record,
    /// Click-to-pick a window inside the region overlay (Windows) — crops the
    /// same freeze snap, so no focus juggling is needed at all.
    WindowPick,
    /// Same overlay, but the picked rect becomes the recording crop — capture
    /// a window on video without naming it or focusing it.
    WindowRecord,
}

/// Events raised outside the GUI loop — global hotkeys and tray menu clicks.
/// A minimized winit window receives no WM_PAINT on Windows, so `update()`
/// stops ticking; these are queued by a pump thread and drained on the next
/// frame after the pump wakes the window.
#[derive(Clone, Copy, Debug)]
enum WakeEvent {
    /// Unconditional restore + foreground (summon fired while hidden).
    Show,
    /// `toggle_window` semantics (summon fired while the window was visible).
    ToggleWindow,
    Screenshot,
    RecordToggle,
    /// E50 — dedicated pause/resume hotkey (Ctrl+Shift+N, opt-in digit).
    PauseToggle,
    /// E86 — opt-in region-still hotkey: jumps straight into the picker.
    RegionStill,
    /// E86 — opt-in window-still hotkey: captures the picked window app.
    WindowStill,
    /// E86 — opt-in GIF-clip hotkey: 3 s record → gif.
    GifClip,
    /// E10 — Esc pressed while the region overlay was up but unfocused.
    RegionCancel,
    Tray(TrayAction),
}

/// Capture settings snapshot for the pump's minimized fast path — the app
/// state is not Sync, so `update()` mirrors the fields a worker needs.
#[derive(Clone)]
struct CaptureCfg {
    save_dir: PathBuf,
    name_pattern: String,
    monitor: Option<u32>,
    draw_mouse: bool,
    target: CaptureTarget,
    /// Last observed front app — used only for the file-name token.
    front_app: Option<String>,
    /// Snipping-Tool delay — mirrored so a hidden fast-path still honors it.
    delay_secs: u64,
}

#[derive(Default)]
struct WakeShared {
    queue: Mutex<VecDeque<WakeEvent>>,
    cfg: Mutex<Option<CaptureCfg>>,
    /// Cross-thread "a still is in flight" guard (GUI flag can't be read by
    /// the pump). Set by whichever side claims the capture.
    still_busy: AtomicBool,
    /// `MenuId → action` for tray menu translation (filled once the tray exists).
    tray_ids: Mutex<Vec<(tray_icon::menu::MenuId, TrayAction)>>,
    /// Mirror of `pre_capture_outer.is_some()` — while a capture owns the
    /// park, the pump must not un-hide the studio (it would enter its own
    /// shot); queued events wait for the worker's own restore.
    parked: AtomicBool,
    /// Region-pick instant overlay: 0 = waiting for update() to apply
    /// capture-exclusion to the "Vibecap Region" viewport, 1 = excluded
    /// (snap may proceed), 2 = give up waiting — snap anyway (overlay is
    /// closing or never appeared).
    region_overlay_state: AtomicU8,
    /// E250 — watch-folder mirror so the pump can import while parked.
    watch_dir: Mutex<Option<PathBuf>>,
    /// E250 — follow-OS mirror: pump repaints on the 3 s theme-poll cadence.
    follow_os: AtomicBool,
    /// E250 — files the pump's parked-side watch sweep moved; update()
    /// consumes the count → library refresh + toast.
    watch_moved: std::sync::atomic::AtomicUsize,
    /// E10 — region overlay is up; the pump polls for a global Esc so a
    /// focus-loss can't orphan the pick.
    region_open: AtomicBool,
}

impl WakeShared {
    fn push(&self, ev: WakeEvent) {
        if let Ok(mut q) = self.queue.lock() {
            q.push_back(ev);
        }
    }
}

/// A still triggered while the studio is already hidden: no hide-settle wait,
/// no focus yank — grab the desktop as-is, then wake the window so the next
/// `update()` consumes the pending marker (opens Still, copies, toasts).
fn spawn_pump_still(cfg: CaptureCfg, shared: Arc<WakeShared>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let seq = app::naming::next_seq(&cfg.save_dir, "");
        let stem = app::format_capture_stem(&cfg.name_pattern, cfg.front_app.as_deref(), seq);
        let shot = cfg.save_dir.join(format!("{stem}.jpg"));
        if cfg.delay_secs > 0 {
            std::thread::sleep(Duration::from_secs(cfg.delay_secs));
        }
        let result = capture_screenshot_opts(
            &shot,
            &CaptureOpts::default()
                .with_draw_mouse(cfg.draw_mouse)
                .with_monitor(crate::platform::resolve_monitor(cfg.monitor)),
        )
        .map(|_| shot);
        match &result {
            Ok(p) => write_pending_still(p),
            Err(e) => write_pending_still_error(e),
        }
        shared.still_busy.store(false, Ordering::SeqCst);
        #[cfg(windows)]
        crate::platform::restore_studio_to_taskbar();
        ctx.request_repaint();
    });
}

/// True when ≥90% of sampled pixels share one dark color — the tell-tale of
/// a region snap that froze our own un-excluded dim overlay. A real desktop
/// is never this uniform; a false hit only costs a retry toast. JPEG decode
/// noise means "equal" needs a per-channel tolerance.
fn snap_is_uniform_dim(pixels: &[u8]) -> bool {
    let stride = (pixels.len() / 4 / 128).max(1) * 4;
    let mut n = 0usize;
    let mut same = 0usize;
    let mut first = [0u8; 3];
    for (i, px) in pixels.chunks(stride).enumerate() {
        if px.len() < 4 {
            break;
        }
        let c = [px[0], px[1], px[2]];
        if i == 0 {
            first = c;
        }
        n += 1;
        if c.iter()
            .zip(first)
            .all(|(a, b)| (*a as i32 - b as i32).abs() <= 10)
        {
            same += 1;
        }
    }
    let dark = (first[0] as u32 + first[1] as u32 + first[2] as u32) < 200;
    n > 0 && same * 100 >= n * 90 && dark
}

/// Pump daemon: blocks on the global-hotkey channel and, in 250 ms slices,
/// drains tray icon + menu channels. For every event it pushes a `WakeEvent`
/// and makes sure the GUI loop can actually tick to consume it — on Windows a
/// minimized window gets no WM_PAINT, so the pump restores the HWND itself.
/// Screenshots fired while already hidden take the fast path: captured
/// entirely on a worker (the window never flashes up), then restored on done.
fn spawn_wake_pump(
    rx: Receiver<GlobalHotKeyEvent>,
    id_shot: u32,
    id_rec: u32,
    id_summon: u32,
    id_pause: u32,
    id_prtscn: u32,
    id_region: u32,
    id_window: u32,
    id_gif: u32,
    shared: Arc<WakeShared>,
    ctx: egui::Context,
) {
    std::thread::spawn(move || {
        let mut last_slow = Instant::now() - Duration::from_secs(4);
        loop {
            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(event) => {
                    if event.state != global_hotkey::HotKeyState::Pressed {
                        // release events: ignore, but still drain tray below
                    } else if event.id == id_shot {
                        pump_still_event(&shared, &ctx);
                    } else if event.id == id_rec {
                        pump_event(&shared, &ctx, WakeEvent::RecordToggle);
                    } else if event.id == id_summon {
                        pump_summon(&shared, &ctx);
                    } else if event.id == id_pause {
                        pump_event(&shared, &ctx, WakeEvent::PauseToggle);
                    } else if event.id == id_prtscn {
                        // E204 — bare PrtScn behaves like the screenshot hotkey.
                        pump_still_event(&shared, &ctx);
                    } else if event.id == id_region && id_region != 0 {
                        pump_event(&shared, &ctx, WakeEvent::RegionStill);
                    } else if event.id == id_window && id_window != 0 {
                        pump_event(&shared, &ctx, WakeEvent::WindowStill);
                    } else if event.id == id_gif && id_gif != 0 {
                        pump_event(&shared, &ctx, WakeEvent::GifClip);
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            }
            drain_tray_channels(&shared, &ctx);
            // E10 — while the region overlay is up, poll for a global Esc:
            // if the overlay lost focus its own Esc handling never fires.
            #[cfg(windows)]
            if shared.region_open.load(Ordering::SeqCst) && crate::platform::esc_pressed_edge() {
                pump_event(&shared, &ctx, WakeEvent::RegionCancel);
            }
            pump_housekeeping(&shared, &ctx, &mut last_slow);
        }
    });
}

/// E250 — slow-lane housekeeping for an event-driven idle app. `update()`
/// only ticks when a repaint is requested now, so the pump's 250 ms slice
/// watches the poke marker and, every 3 s, nudges the frame loop for the
/// OS-theme follow + watch-folder poll. While parked the sweep runs here
/// instead (a hidden window can't be trusted to tick) — imports land
/// silently and the toast waits for the next wake.
fn pump_housekeeping(shared: &Arc<WakeShared>, ctx: &egui::Context, last_slow: &mut Instant) {
    let parked = shared.parked.load(Ordering::SeqCst);
    // CLI poke + deep-link markers: cheap stat each slice; a parked window
    // must be restored for update() to consume them.
    if app::pending_cmd_waiting() || app::pending_deep_waiting() {
        if parked {
            pump_event(shared, ctx, WakeEvent::Show);
        } else {
            ctx.request_repaint();
        }
    }
    if last_slow.elapsed() < Duration::from_secs(3) {
        return;
    }
    *last_slow = Instant::now();
    // OS-theme follow + watch-folder ticks only need a frame.
    if shared.follow_os.load(Ordering::SeqCst) && !parked {
        ctx.request_repaint();
    }
    let watch = shared
        .watch_dir
        .lock()
        .ok()
        .and_then(|w| w.as_ref().cloned());
    if let Some(dir) = watch {
        if parked {
            // Hidden windows may not tick — sweep on this thread; the GUI
            // picks up `watch_moved` + refreshes on its next frame.
            let media = shared
                .cfg
                .lock()
                .ok()
                .and_then(|c| c.as_ref().map(|c| c.save_dir.clone()));
            if let Some(media) = media {
                let (moved, _) = crate::app::library::watch_sweep(&dir, &media);
                if moved > 0 {
                    shared.watch_moved.fetch_add(moved, Ordering::SeqCst);
                    ctx.request_repaint();
                }
            }
        } else {
            ctx.request_repaint();
        }
    }
}

/// Should a queued event un-hide the HWND so `update()` can consume it?
/// While a capture owns the park the answer is usually no — restoring the
/// studio mid-still would put it inside its own shot. But when *recording*
/// owns the park (`parked` without `still_busy`), stop/show/quit intents must
/// still wake the loop: they cannot wait for a REC-bar repaint that may never
/// come. Stop already restores the window via `show_window`, so an early wake
/// shows the same frames the tail of the recording would anyway.
fn pump_needs_wake(ev: &WakeEvent, parked: bool, still_busy: bool) -> bool {
    if !parked {
        return true;
    }
    if still_busy {
        return false; // a still owns the park — its worker restores on done
    }
    // A recording owns the park — only explicit user actions may surface it.
    matches!(
        ev,
        WakeEvent::RecordToggle
            | WakeEvent::PauseToggle
            | WakeEvent::Show
            | WakeEvent::Tray(TrayAction::ToggleRecord)
            | WakeEvent::Tray(TrayAction::TogglePause)
            | WakeEvent::Tray(TrayAction::Show)
            | WakeEvent::Tray(TrayAction::DoubleClick)
            | WakeEvent::Tray(TrayAction::Quit)
    )
}

/// Push an event and wake the OS window if it is hidden — without the wake the
/// queue is only consumed when the user next clicks the taskbar. While a
/// capture owns the park (`parked`), `pump_needs_wake` decides whether this
/// event may surface the window early (record stop/show/quit) or must wait
/// for the capture worker's own restore (anything during a still).
fn pump_event(shared: &Arc<WakeShared>, ctx: &egui::Context, ev: WakeEvent) {
    shared.push(ev);
    #[cfg(windows)]
    if crate::platform::studio_is_minimized()
        && pump_needs_wake(
            &ev,
            shared.parked.load(Ordering::SeqCst),
            shared.still_busy.load(Ordering::SeqCst),
        )
    {
        crate::platform::restore_studio_to_taskbar();
    }
    ctx.request_repaint();
}

fn pump_summon(shared: &Arc<WakeShared>, ctx: &egui::Context) {
    #[cfg(windows)]
    {
        if crate::platform::studio_is_minimized() {
            if shared.parked.load(Ordering::SeqCst) {
                // A capture owns the park — drop the key, same as the old
                // toggle_window guard (deferred toggles could hide the window
                // right after the capture pops it back up).
                return;
            }
            // Intent is unambiguously "show" — decide now, before the restore
            // flips the minimized flag and `toggle_window` would hide again.
            shared.push(WakeEvent::Show);
            crate::platform::restore_studio_to_taskbar();
            ctx.request_repaint();
            return;
        }
    }
    pump_event(shared, ctx, WakeEvent::ToggleWindow);
}

/// Minimized + Fullscreen + idle → the pump worker can grab the desktop
/// directly; anything else goes through the GUI path (window focus, region
/// overlay) or is already busy.
fn fast_still_allowed(minimized: bool, target: Option<CaptureTarget>, still_busy: bool) -> bool {
    minimized && matches!(target, Some(CaptureTarget::Fullscreen)) && !still_busy
}

/// Screenshot while hidden → capture on a worker, never flash the window.
/// Visible / non-Fullscreen target → queue for the normal GUI path.
fn pump_still_event(shared: &Arc<WakeShared>, ctx: &egui::Context) {
    #[cfg(windows)]
    {
        let minimized = crate::platform::studio_is_minimized();
        let cfg = shared.cfg.lock().ok().and_then(|c| c.clone());
        let target = cfg.as_ref().map(|c| c.target);
        // `swap` is the authoritative claim — the `load` inside the gate is
        // just a fast reject before it.
        if fast_still_allowed(minimized, target, shared.still_busy.load(Ordering::SeqCst))
            && !shared.still_busy.swap(true, Ordering::SeqCst)
        {
            if let Some(cfg) = cfg {
                spawn_pump_still(cfg, shared.clone(), ctx.clone());
                return;
            }
            shared.still_busy.store(false, Ordering::SeqCst);
        }
    }
    pump_event(shared, ctx, WakeEvent::Screenshot);
}

/// Tray icon clicks and menu items use the same global channels the tray
/// controller polls — but that poll only runs inside `update()`. Drain them
/// here too so clicks work while the window is parked.
fn drain_tray_channels(shared: &Arc<WakeShared>, ctx: &egui::Context) {
    let mut woke = false;
    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        match event {
            TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } => {
                if let Some(a) = tray_ui::tray_click_up() {
                    pump_event(shared, ctx, WakeEvent::Tray(a));
                    woke = true;
                }
            }
            TrayIconEvent::DoubleClick {
                button: tray_icon::MouseButton::Left,
                ..
            } => {
                pump_event(shared, ctx, WakeEvent::Tray(tray_ui::tray_double_click()));
                woke = true;
            }
            _ => {}
        }
    }
    // E206 — a parked click that outlived the double-click window was a real
    // single click; the 250 ms pump tick checks this even while hidden.
    if let Some(a) = tray_ui::tray_click_expired() {
        pump_event(shared, ctx, WakeEvent::Tray(a));
        woke = true;
    }
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        let action = shared
            .tray_ids
            .lock()
            .ok()
            .and_then(|ids| ids.iter().find(|(id, _)| *id == event.id).map(|(_, a)| *a));
        match action {
            Some(TrayAction::Screenshot) => pump_still_event(shared, ctx),
            Some(TrayAction::Show) => pump_event(shared, ctx, WakeEvent::Show),
            Some(TrayAction::ToggleRecord) => pump_event(shared, ctx, WakeEvent::RecordToggle),
            Some(a) => pump_event(shared, ctx, WakeEvent::Tray(a)),
            None => {}
        }
        woke = true;
    }
    if woke {
        ctx.request_repaint();
    }
}

#[derive(Default)]
pub(crate) struct VibecapApp {
    current_tab: AppTab,
    capture_target: CaptureTarget,
    capture_audio: bool,
    /// E59 — picked DirectShow audio device (empty = auto-resolve).
    audio_device: String,
    /// E49 — optional second dshow device mixed into recordings (loopback).
    audio_mix_device: String,
    /// E36 — newest dBFS peak from the recorder's astats meter (via .ffmpeg.log).
    audio_level_db: f32,
    audio_meter_at: Option<Instant>,
    fps_target: u32,
    /// C55 — recording quality (libx264 `-crf`): 18 sharp / 23 balanced / 28 small.
    record_crf: u8,
    /// E72 — time-lapse interval in seconds (0 = real-time recording).
    timelapse_secs: u32,
    draw_mouse: bool,
    capture_monitor: Option<u32>,
    name_pattern: String,
    inbox_snippets: Vec<String>,
    /// Inbox quiet mode — badge/tray still count, but no OS notify / toast /
    /// attention bounce / auto-open on new agent questions.
    inbox_quiet: bool,
    library_search: String,
    library_last_click: Option<PathBuf>,
    annotation_undo: Vec<Vec<AnnotationAction>>,
    /// Redo stack — cleared whenever a new stroke begins.
    annotation_redo: Vec<Vec<AnnotationAction>>,
    still_zoom: f32,
    /// Set by the `1` key; resolved to true-100% inside the canvas block
    /// where the fit scale is known.
    still_zoom_to_100: bool,
    /// E44 — thirds/quarters composition grid over the still canvas.
    still_grid: bool,
    clip_loop: bool,
    gif_fps: u32,
    gif_width: u32,
    /// Boomerang loop — export plays forward then reverse (#135).
    gif_pingpong: bool,
    /// E58 — end-of-loop hold (ms): clone the last frame via tpad.
    gif_hold_ms: u32,
    /// Editable clip notes — persisted to `<file>.notes.txt` beside the media.
    clip_notes: String,
    record_markers: Vec<f64>,
    shutter_flash_until: Option<Instant>,
    #[allow(dead_code)]
    region_await_enter: bool,
    audio_devices: Vec<String>,
    /// Worker → main: ffmpeg -list_devices probe (DirectShow spawn is ~1s).
    pub(crate) audio_devices_rx: Option<Receiver<Vec<String>>>,
    window_list_at: Option<Instant>,
    /// Worker → main: running-app list (PowerShell spawn must not block UI).
    window_list_rx: Option<Receiver<Vec<String>>>,
    /// Worker → main: frontmost-app probe (PowerShell ~200-500ms on Windows).
    front_app_rx: Option<Receiver<Option<String>>>,
    /// Status strip cache — dir walks must not run per frame.
    status_cache: Option<(StatusSnapshot, Instant)>,
    /// Capture-tab live-stats cache — same reason (dir walk + budget file).
    live_stats_cache: Option<(LiveStats, Instant)>,
    /// Last active Review editor (Still or Clip) — rail's Review stage returns here.
    last_review_tab: Option<AppTab>,
    /// Stored handle so worker threads can wake the UI when a drain lands.
    ui_ctx: Option<egui::Context>,
    is_recording: bool,
    /// Hide UI then spawn ffmpeg on a worker — true while countdown / spawn in flight.
    recording_arming: bool,
    /// User cancelled during arming (worker result is discarded / killed).
    recording_cancel_armed: bool,
    /// Windows: the studio is on screen during this recording but excluded via
    /// WDA_EXCLUDEFROMCAPTURE — release on every recording exit path.
    record_excluded: bool,
    /// Bounded retries while excluding the floating "Vibecap Recorder" bar —
    /// its HWND can take a frame to exist after show_viewport_immediate.
    rec_bar_exclude_attempts: u8,
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
    /// Worker → main: ffmpeg child after hide delay (recording starts even if UI was minimized).
    record_spawn_rx: Option<crossbeam_channel::Receiver<Result<(Child, PathBuf), String>>>,
    /// Worker → main: recorder exit after Stop (ffmpeg moov write can take seconds).
    record_finalize_rx: Option<Receiver<Result<(), String>>>,
    /// True while ffmpeg finalizes the MP4 on a worker — UI must not block on it.
    pub(crate) recording_finalizing: bool,
    /// Worker → main: voice memo finalize (same blocking wait shape as video).
    voice_finalize_rx: Option<Receiver<Result<(), String>>>,

    // File paths & Media Library
    save_dir: PathBuf,
    current_mp4_file: Option<PathBuf>,
    latest_screenshot: Option<PathBuf>,
    library_items: Vec<MediaItem>,
    /// Worker → main: media dir scan (never scan on the UI thread).
    library_scan_rx: Option<Receiver<Vec<MediaItem>>>,
    /// A scan was requested while one was in flight — rescan after drain.
    library_scan_pending: bool,
    /// Recent-capture thumbs on the Capture stage. The worker runs
    /// ensure_thumb + image decode; the UI only turns results into textures.
    pub(crate) recent_thumbs: Vec<(PathBuf, bool, egui::TextureHandle)>,
    pub(crate) recent_thumbs_rx: Option<Receiver<Vec<(PathBuf, bool, egui::ColorImage)>>>,
    pub(crate) recent_key: String,
    /// E53 — which recent tile was hovered last frame; drives the grow
    /// animation + scrub play (hover must lag the size change by a frame).
    pub(crate) recent_hover: Option<PathBuf>,
    /// "All" | category labels from MediaCategory::label()
    library_filter: String,
    /// Grid ordering (LibrarySort). Date sorts keep the group headers.
    library_sort: crate::app::LibrarySort,
    /// Tile width preset: 0 = S, 1 = M, 2 = L.
    library_tile_size: u8,
    /// How many filtered items to show (starts at LIBRARY_PAGE_SIZE).
    library_show_limit: usize,
    /// Paths selected for bulk open/delete.
    library_selected: std::collections::HashSet<PathBuf>,
    /// Pending confirm for clear-all in current category.
    library_confirm_clear: bool,
    /// E150 — deleting a clip with unsaved trims asks once: paths parked
    /// here while the confirm modal is up; `delete_guard_ok` remembers the
    /// "delete anyway" answer per path for the session.
    confirm_delete: Option<Vec<PathBuf>>,
    delete_guard_ok: std::collections::HashSet<PathBuf>,

    // Edit tab & Video Processing
    trim_start: String,
    trim_end: String,
    export_speed: String,
    edit_file: Option<PathBuf>,
    /// Probed playtime of the loaded clip in seconds (0 = unknown).
    clip_duration_secs: f64,
    filmstrip: Vec<egui::TextureHandle>,
    /// Extraction rate of `filmstrip` frames (frames per second of real time).
    filmstrip_fps: f64,
    filmstrip_loading: bool,
    filmstrip_error: Option<String>,
    /// Dead-air detection on the loaded clip: `(content_start_s, content_end_s)`
    /// when the head/tail is frozen — shown as a one-tap trim offer.
    dead_air_hint: Option<(f64, f64)>,
    dead_air_dismissed: bool,
    // In-app preview player state
    player_playing: bool,
    player_pos: f64,
    player_last_time: Option<f64>,
    /// Auto-play the preview when filmstrip frames land (Settings toggle).
    clip_autoplay: bool,
    /// B54 — auto-apply detected dead-air bounds to the trim on clip load.
    auto_dead_air: bool,
    /// E214 — opt-in: check GitHub Releases once at launch.
    update_check_on_launch: bool,
    /// D70 — jump to Review after a capture lands (off = toast only).
    auto_open_review: bool,
    /// B52 — remembered REC bar window position (screen px).
    rec_bar_pos: Option<(i32, i32)>,
    /// Half-width filmstrip — faster extraction, softer preview.
    filmstrip_low_res: bool,
    /// E24 — region overlay dim alpha (session-backed).
    region_dim: u8,
    /// E154 — favorited library file names (session-backed).
    library_favorites: std::collections::HashSet<String>,
    /// E83 — "needs attention" flags on library items (file names, persisted).
    library_flagged: std::collections::HashSet<String>,
    /// E76 — library tags: file name → tags (session-backed).
    library_tags: std::collections::BTreeMap<String, Vec<String>>,
    /// E76 — active tag chip (ANDs with the category filter).
    library_tag_filter: Option<String>,
    /// E76 — tag editor popup: target item name + edit buffer.
    library_tag_edit: Option<String>,
    library_tag_edit_buf: String,
    /// E79 — retention rule (session): 0 off / 1 older-than-days / 2 newest-N.
    retention_mode: u8,
    retention_value: u32,
    retention_auto: bool,
    /// E79 — one auto-sweep per launch at most.
    retention_swept: bool,
    /// E157 — compact list view instead of the tile grid.
    library_list_view: bool,
    /// E159 — hover-scrub frame cache: clip path → decoded strip textures.
    scrub_cache: std::collections::HashMap<PathBuf, Vec<egui::TextureHandle>>,
    /// E159 — clips with a scrub-strip extraction in flight.
    scrub_pending: std::collections::HashSet<PathBuf>,
    /// E159 — persistent extraction channel (tx cloned per worker spawn).
    scrub_tx: Option<Sender<(PathBuf, Result<Vec<(u32, u32, Vec<u8>)>, String>)>>,
    scrub_rx: Option<Receiver<(PathBuf, Result<Vec<(u32, u32, Vec<u8>)>, String>)>>,
    /// E167 — regenerable bytes under the media root (cache/scratch dirs).
    library_reclaimable: u64,
    /// E180 — pending-question key last pushed to the tray quick slots.
    tray_quick_key: String,
    /// E180 — request ids bound to the 3 tray quick-reply slots, synced
    /// whenever `set_quick_pending` pushes labels.
    tray_quick_ids: Vec<String>,
    /// E189 — Inbox "new since last visit" watermark (`%Y-%m-%d %H:%M:%S`,
    /// lexicographically comparable to `created_at`).
    inbox_seen_stamp: String,
    /// E188 — collapsed agent groups in the Inbox pending list.
    inbox_collapsed_agents: std::collections::HashSet<String>,
    /// E244 — session writes are debounced: `persist_session` only marks
    /// dirty; update() flushes at a 500 ms cadence, exit paths flush sync.
    session_dirty: std::cell::Cell<bool>,
    session_last_write: std::cell::Cell<Option<Instant>>,
    /// E256 — post-stop MP4 readability probe (worker → drain).
    mp4_verify_rx: Option<Receiver<(PathBuf, Result<Option<PathBuf>, String>)>>,
    /// Filmstrip decode progress `(done, total)` for the determinate label.
    filmstrip_progress: (usize, usize),
    filmstrip_progress_rx: Option<Receiver<(usize, usize)>>,

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

    /// Pump-thread mailbox: hotkey + tray events queued while the GUI loop
    /// was parked (a minimized Windows window gets no WM_PAINT → no update()).
    wake_shared: Arc<WakeShared>,
    /// Arm-cancel flag shared with the recording-spawn worker — a cancel
    /// during the REC-bar grace window must stop the worker's own minimize.
    arm_cancel: Arc<AtomicBool>,
    /// Kept alive so the global hotkey stays registered (drop unregisters).
    #[allow(dead_code)]
    hotkey_manager: Option<GlobalHotKeyManager>,
    /// Hotkey id → start/stop recording (Ctrl+Shift+2).
    hotkey_id_record: u32,
    /// Hotkey id → screenshot (Ctrl+Shift+3).
    hotkey_id_screenshot: u32,
    /// Hotkey id → summon/hide the window (Ctrl+Alt+V).
    hotkey_id_summon: u32,
    hotkey_id_pause: u32,
    hotkey_id_region: u32,
    hotkey_id_window: u32,
    hotkey_id_gif: u32,
    hotkey_id_prtscn: u32,

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
    /// True while a real drag is live in the region overlay — lets a
    /// missed drag_stopped still confirm on pointer release.
    region_was_dragging: bool,
    /// Frames to keep re-asserting foreground after the region overlay closes —
    /// the closing child viewport can steal focus back on its way out.
    region_refocus_frames: u8,
    selected_region: Option<Rect>,
    /// Ghost outline for next region select (session-persisted).
    last_region: Option<Rect>,
    pending_region_kind: Option<RegionPickKind>,
    /// Studio is capture-excluded (WDA_EXCLUDEFROMCAPTURE) rather than hidden
    /// for this pick — must be released on every exit path.
    region_affinity: bool,
    /// Frames spent waiting for the region overlay's own capture-exclusion
    /// before degrading to the delayed-overlay path.
    region_excl_attempts: u8,
    /// Aspect-ratio lock for the region drag (None = free). Persisted across
    /// picks so the HUD chip stays where the user left it.
    region_aspect_lock: Option<f32>,
    /// E96 — confirmed region rects this session; Ctrl+Z in the HUD pops one.
    region_history: Vec<Rect>,
    /// E95 — lifetime completed picks (session); HUD hints hide after 3.
    region_pick_count: u32,
    /// E94 — region HUD toolbar docks bottom when set (session).
    hud_toolbar_bottom: bool,
    /// Seconds to wait after the hide before grabbing (menu/tooltip shots,
    /// Snipping-Tool parity). Applies to non-interactive stills only.
    capture_delay_secs: u64,
    /// Copy the still and discard the file — never touches the library.
    clipboard_only: bool,
    /// Suppress non-error toasts + the shutter flash.
    silent_mode: bool,
    /// `?` / F1 shortcut cheat sheet.
    cheatsheet_open: bool,
    /// Window-pick mode: topmost window under the cursor (title + OS-px rect).
    window_pick_hover: Option<(String, i32, i32, i32, i32)>,
    /// E22 — hwnd the hover tuple represents (feeds the thumbnail grab).
    window_pick_hwnd: Option<u64>,
    /// E22 — hwnd → live thumbnail texture cache for the pick card.
    pick_thumbs: std::collections::HashMap<u64, egui::TextureHandle>,
    /// E22 — in-flight thumbnail grab (hwnd is what the worker is painting).
    pick_thumb_rx: Option<(u64, Receiver<Option<(u32, u32, Vec<u8>)>>)>,
    /// Hover is dead space → the pick target is the whole monitor, not a window.
    window_pick_hover_monitor: bool,
    /// Scroll-wheel Z-cycle index into the overlapping windows under the cursor.
    window_pick_cycle: usize,
    /// Last cursor point — a move resets the cycle to the topmost hit.
    window_pick_last_pos: Option<(i32, i32)>,
    window_pick_poll_at: Option<Instant>,
    /// Pixel crop (w,h,x,y) mapped from the region overlay / snapshot.
    selected_screen_rect: Option<(i32, i32, i32, i32)>,
    /// E22 — named region rects `(w, h, x, y)` in physical pixels.
    saved_regions: Vec<(String, [i32; 4])>,
    /// E275 — opt-in local capture counters (persisted; never transmitted).
    stats_opt_in: bool,
    stat_shots_ok: u32,
    stat_shots_fail: u32,
    stat_recs_ok: u32,
    stat_recs_fail: u32,
    region_backdrop: Option<egui::TextureHandle>,
    region_backdrop_px: (u32, u32),
    region_backdrop_rgba: Option<(u32, u32, Vec<u8>)>,
    /// The shown backdrop is the *previous* pick's snap — instant feedback
    /// while the fresh one lands. Confirms are blocked until it swaps in
    /// (crop pixels must map to the snap that will be cropped).
    region_backdrop_stale: bool,
    /// When the current backdrop snap was captured — a re-pick within 2 s
    /// skips the re-grab entirely and reuses it (J230).
    region_backdrop_at: Option<Instant>,
    still_crop_mode: bool,
    still_pan: Vec2,
    text_edit_at: Option<Pos2>,
    crop_drag: Option<(Pos2, Pos2)>,
    feedback_pinned: std::collections::HashSet<String>,
    feedback_snooze_until: std::collections::HashMap<String, Instant>,
    inbox_search: String,
    /// Which thread buckets the Inbox list shows (#199).
    inbox_filter: crate::ui::inbox_tab::InboxFilter,
    budget_warned: bool,
    update_status: String,
    /// E214 — in-flight GitHub check; Some = a worker is fetching.
    update_rx: Option<crossbeam_channel::Receiver<Result<crate::app::update::ReleaseInfo, String>>>,
    /// E215 — in-flight release download (worker → staged exe path).
    update_dl_rx: Option<crossbeam_channel::Receiver<Result<std::path::PathBuf, String>>>,
    /// E215 — a staged `<exe>.new` ready to swap on restart.
    update_staged: Option<std::path::PathBuf>,
    /// Last successful release payload (drives the Download button + notes).
    update_info: Option<crate::app::update::ReleaseInfo>,
    /// E222 — in-flight orphan-recording remux from a dead previous run.
    recovery_rx: Option<crossbeam_channel::Receiver<Result<String, String>>>,
    /// E222 — draft fingerprint + debounce for unsaved Review annotations.
    review_draft_fp: u64,
    review_draft_changed_at: Option<std::time::Instant>,
    /// E216 — cached "Annotate with Vibecap" registry state for Settings.
    explorer_verb_state: Option<bool>,
    /// E187 — vibecap:// URL-scheme registration state (mirrors registry).
    url_scheme_state: Option<bool>,
    hotkey_shot_digit: u8,
    hotkey_rec_digit: u8,
    /// Digits currently registered with the OS — rebind unregisters these,
    /// not the (possibly edited) pending digits.
    hotkey_shot_digit_prev: u8,
    hotkey_rec_digit_prev: u8,
    /// E50 — opt-in pause/resume hotkey digit (None = unbound).
    hotkey_pause_digit: Option<u8>,
    hotkey_pause_digit_prev: Option<u8>,
    /// E86 — opt-in per-mode digits (Ctrl+Shift+N): jump to region pick / fire a window still.
    hotkey_region_digit: Option<u8>,
    hotkey_region_digit_prev: Option<u8>,
    hotkey_window_digit: Option<u8>,
    hotkey_window_digit_prev: Option<u8>,
    hotkey_gif_digit: Option<u8>,
    hotkey_gif_digit_prev: Option<u8>,
    /// E51 — hour-of-day → last target; a morning region picker gets Region
    /// back at launch instead of whatever the evening session used last.
    target_hours: std::collections::BTreeMap<u8, String>,
    /// E204 — opt-in bare PrtScn still (steals the OS key while running).
    hotkey_prtscn: bool,
    hotkey_prtscn_prev: bool,
    /// E12 — attach the file's PNG/JPEG encode to the clipboard next to
    /// the raw bitmap copy.
    clipboard_encode: String,
    /// E202 — subtle click when a still lands; session-backed, off default.
    shutter_sound: bool,
    /// E64 — last screenshot-hotkey press; a second tap inside 600 ms opens
    /// the region picker instead of a second fullscreen shot.
    last_shot_hk_at: Option<Instant>,
    /// E64 — second tap landed while a still was in flight: open the picker
    /// as soon as `finish_screenshot` restores the studio.
    region_after_still: bool,
    /// E74 — clipboard watcher (session-backed, off default): a fresh image
    /// on the system clipboard opens Still review.
    clipboard_watcher: bool,
    /// Last observed clipboard sequence; we refresh it after our own writes
    /// so a capture we copied doesn't bounce back through the watcher.
    clipboard_seq_seen: u32,
    clipboard_poll_at: Option<Instant>,
    /// E171 — throttle for the stale-sidecar sweep (10 min cadence).
    last_sidecar_sweep: Option<Instant>,
    /// E71 — stills saved in the current Shift-drag batch overlay session.
    batch_shot_count: u32,
    /// E73 — a still delay ≥60 s becomes a real schedule: the studio stays
    /// interactive and the shot fires when this Instant passes. The card
    /// shows a countdown + Cancel.
    scheduled_shot_at: Option<Instant>,
    /// Set while firing a scheduled shot so the delay doesn't re-arm it.
    scheduled_fire_now: bool,
    /// E39 — zen mode: rail + status strip hidden; palette/hotkeys only.
    zen_mode: bool,
    /// E249 — decoded still keyed by (path, mtime): slider tweaks re-run ops
    /// on this instead of re-decoding the file every frame of the drag.
    still_decode_cache: Option<(PathBuf, u64, image::DynamicImage)>,
    region_snap_path: Option<PathBuf>,
    region_snap_rx: Option<Receiver<Result<(PathBuf, u32, u32, Vec<u8>), String>>>,
    /// E93 — this snap is a live-backdrop refresh, not the pick's freeze.
    region_snap_is_refresh: bool,
    /// E93 — live backdrop while picking (requires capture exclusion; the
    /// overlay must not photograph itself). Off = the classic frozen frame.
    region_live_backdrop: bool,
    /// E93 — last live refresh kick; re-fires every ~1.2 s while picking.
    region_live_at: Option<Instant>,
    brand_logo: Option<egui::TextureHandle>,
    filmstrip_rx: Option<Receiver<Result<(Vec<(u32, u32, Vec<u8>)>, f64, f64), String>>>,

    // Notification toast (message, shown_at, severity)
    toast_message: Option<(String, Instant, ToastLevel)>,
    /// Last error toast — persistent surface in Settings + tray tooltip (K253).
    last_error: Option<String>,
    /// Post-capture action card (path + shown_at); mutually preferred over simple toast.
    /// Last fresh capture card — `bool` is whether auto-copy already landed
    /// on the clipboard, so the card title can say "Copied" honestly.
    capture_toast: Option<(PathBuf, Instant, bool)>,
    /// Most recent finished capture — target of the app-level Ctrl+C.
    last_capture: Option<LastCapture>,

    // Phase 1d: palette, density, undo trash
    palette_open: bool,
    palette_query: String,
    palette_selected: usize,
    /// E285 — Settings left-rail selection ("all" scrolls everything).
    settings_nav: String,
    /// E284 — Settings filter box contents.
    settings_filter: String,
    /// E292 — tag/notes of the just-applied update; cleared when dismissed.
    whats_new_tag: String,
    whats_new_notes: String,
    /// E49 — off = always open on Capture; on = resume the last stage.
    restore_tab: bool,
    /// E33 — remembered window size per stage.
    window_sizes: std::collections::HashMap<String, [f32; 2]>,
    /// E26 — icon-only rail.
    rail_collapsed: bool,
    /// E29 — persisted rail stage order (labels); empty = canonical.
    rail_order: Vec<String>,
    /// E46 — horizontal tab strip across the top instead of the left rail.
    top_tabs: bool,
    /// E38 — docked metadata inspector on Review stages.
    inspector_open: bool,
    /// E23 — flatten pulses/hover-grow.
    reduce_motion: bool,
    /// E3 — celestial accent-hue offset (degrees, ±40).
    aurora_hue: f32,
    /// Recently-run palette actions (most recent first, max 3).
    palette_mru: Vec<PaletteAction>,
    density: Density,
    /// Soft-deleted paths staged for undo (restore before expiry).
    undo_trash: Option<(Vec<PathBuf>, Instant, PathBuf)>,

    // Feedback Inbox (agent human-in-the-loop)
    feedback_requests: Vec<FeedbackRequest>,
    feedback_scanned: bool,
    feedback_selected: Option<String>,
    /// True once the user explicitly picked a thread — suppress silent auto-select.
    feedback_user_picked: bool,
    /// Set when a brand-new request arrives so one auto-select is allowed again.
    feedback_new_arrived: bool,
    feedback_draft: String,
    /// Quick-choice chip selected for the open reply (maps to selected_option).
    feedback_choice: String,

    // Agent Budget panel (shared with MCP via ~/.config/vibecap/budget.json)
    budget_frames_input: String,
    budget_mb_input: String,
    budget_minutes_input: String,
    budget_tier: String,
    budget_loaded: bool,
    /// E295 — per-session utilization samples (0..1, worst of
    /// frames/MB/minutes caps) for the Inbox sparkline.
    budget_samples: Vec<f32>,
    budget_sample_at: Option<Instant>,

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
    /// Source pixel dims of the loaded still (for the export size readout).
    img_src_wh: (u32, u32),
    /// Export surface: format / quality / uniform pad (F117, F118, F122).
    export_fmt: StillExportFmt,
    export_quality: u8,
    export_pad_px: u32,
    export_pad_color: egui::Color32,
    /// E30 — style for the next step badge (0 filled circle, 1 outline
    /// circle, 2 filled square, 3 outline square).
    badge_style: u8,
    /// E35 — export edge treatment: 0 none, 1 border, 2 drop shadow.
    edge_fx: u8,
    edge_fx_px: u32,
    edge_color: egui::Color32,
    /// Strokes panel selection (E123) — index into `annotation_actions`.
    annotation_selected: Option<usize>,
    /// Corner-watermark text field (E121).
    watermark_text: String,
    /// Filmstrip indices marked for removal on export (F136).
    filmstrip_cut: std::collections::HashSet<usize>,
    /// Clip in/out frame compare split (F146).
    clip_compare: bool,
    /// Extracted preview WAV for the loaded clip (F126).
    preview_audio_path: Option<PathBuf>,
    preview_audio_rx: Option<crossbeam_channel::Receiver<Option<PathBuf>>>,
    preview_audio_playing: bool,

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

    /// E206 — tray double-click action: "open" | "screenshot" | "record".
    tray_dblclick: String,
    /// E211 — folder polled for media to move into the library; "" = off.
    watch_folder: String,
    watch_last_scan: Option<std::time::Instant>,
    /// E225 — follow Windows light/dark for the theme.
    theme_follow_os: bool,
    /// E6 — scheduled themes: Light by day, dark pick at night.
    theme_schedule: bool,
    /// E225 — dark theme used when the OS is dark (name string).
    theme_dark_pick: String,
    os_dark_seen: Option<bool>,
    os_theme_poll_at: Option<std::time::Instant>,

    // First-run wizard (Phase 3)
    wizard_open: bool,
    wizard_step: u8,
    wizard_done: bool,
    wizard_budget_touched: bool,
    /// Wizard "start at login" choice (defaults on; applied on finish).
    wizard_autostart: bool,
    /// E83 — in-flight/last result of the wizard's one-click test capture
    /// (bytes written on success).
    wizard_test_rx: Option<crossbeam_channel::Receiver<Result<u64, String>>>,
    wizard_test_done: Option<Result<u64, String>>,
    /// E288 — mic enumeration runs on a worker (ffmpeg spawn stalls ~300 ms);
    /// `Some(None)` = probe failed, `Some(Some(list))` = device names.
    wizard_audio_rx: Option<crossbeam_channel::Receiver<Option<Vec<String>>>>,
    wizard_audio_devices: Option<Option<Vec<String>>>,
    /// E224 — keyboard-walkthrough bits: 1=S pressed, 2=R, 4=Esc.
    wizard_keys_hit: u8,

    /// Back/forward stacks for Alt+← / Alt+→ stage navigation.
    tab_back: Vec<AppTab>,
    tab_fwd: Vec<AppTab>,
    /// Last rendered tab — per-frame diff feeds `tab_back`.
    prev_tab: AppTab,
    /// E50 — tab-change slide: (start, direction) — content eases in from
    /// ±60 px over ~160 ms, matching rail order.
    tab_slide: Option<(Instant, f32)>,
    /// Run-at-login state for the Settings toggle; None = not probed yet.
    autostart_state: Option<bool>,
    /// Left stage rail — hidden by default; the funnel column is the home UX.
    pub(crate) rail_open: bool,

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
    /// After first GUI open we probe Screen Recording once (macOS TCC dialog).
    screen_permission_prompted: bool,
    /// Persisted: probe produced a capture that looks allowed.
    screen_permission_ok: bool,
    /// Set true for one frame after open so we can run the probe off the UI thread.
    screen_permission_pending: bool,
    /// True while a screenshot worker is running (keep event loop alive while parked).
    screenshot_in_flight: bool,
    /// Geometry before parking off-screen for capture (orderOut breaks restore on macOS).
    pre_capture_outer: Option<Pos2>,
    pre_capture_size: Option<Vec2>,
    /// Modal: first-run / failed Screen Recording gate (blocks until dismissed).
    screen_perm_modal: bool,
    /// Last observed window inner size (persisted so relaunch restores it).
    window_size: Vec2,
    /// Last probe result for the modal copy (None = still running).
    screen_perm_probe_ok: Option<bool>,
    /// One-shot receiver for async permission probe.
    #[allow(clippy::type_complexity)]
    screen_perm_rx: Option<std::sync::mpsc::Receiver<bool>>,
}

impl Default for AppTab {
    fn default() -> Self {
        AppTab::Capture
    }
}

impl Default for CaptureTarget {
    fn default() -> Self {
        CaptureTarget::Fullscreen
    }
}

impl VibecapApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        apply_graphite_theme(&cc.egui_ctx);

        // Hotkeys are best-effort: a second GUI may fail to claim them.
        // Ctrl+Shift+2 = record toggle · Ctrl+Shift+3 = screenshot (digits from session).
        let (hotkey_manager, hotkey_receiver) = match GlobalHotKeyManager::new() {
            Ok(manager) => (Some(manager), Some(GlobalHotKeyEvent::receiver().clone())),
            Err(_) => (None, None),
        };

        let default_dir = default_media_dir();

        let (ffmpeg_tx, ffmpeg_rx) = crossbeam_channel::unbounded();

        let mut app = Self {
            current_tab: AppTab::Capture,
            capture_target: CaptureTarget::Fullscreen,
            capture_audio: false, // video-only by default; user can enable audio
            fps_target: 30,
            record_crf: 23,
            timelapse_secs: 0,
            draw_mouse: false,
            name_pattern: app::DEFAULT_PATTERN.to_string(),
            inbox_snippets: vec![
                "Looks good".into(),
                "Blur the token".into(),
                "Re-record 16:9".into(),
            ],
            inbox_quiet: false,
            still_zoom: 1.0,
            still_zoom_to_100: false,
            still_grid: false,
            gif_fps: 15,
            gif_width: 800,
            gif_hold_ms: 0,
            hotkey_shot_digit: 3,
            hotkey_rec_digit: 2,
            hotkey_shot_digit_prev: 3,
            hotkey_rec_digit_prev: 2,
            hotkey_pause_digit: None,
            hotkey_pause_digit_prev: None,
            hotkey_region_digit: None,
            hotkey_region_digit_prev: None,
            hotkey_window_digit: None,
            hotkey_window_digit_prev: None,
            hotkey_gif_digit: None,
            hotkey_gif_digit_prev: None,
            target_hours: std::collections::BTreeMap::new(),
            hotkey_prtscn: false,
            hotkey_prtscn_prev: false,
            shutter_sound: false,
            last_shot_hk_at: None,
            region_after_still: false,
            clipboard_encode: "png".into(),
            clipboard_watcher: false,
            clipboard_seq_seen: crate::platform::clipboard_seq(),
            clipboard_poll_at: None,
            last_sidecar_sweep: None,
            batch_shot_count: 0,
            scheduled_shot_at: None,
            scheduled_fire_now: false,
            zen_mode: false,
            still_decode_cache: None,
            region_history: Vec::new(),
            region_pick_count: 0,
            hud_toolbar_bottom: false,
            save_dir: default_dir,
            wake_shared: Arc::new(WakeShared::default()),
            arm_cancel: Arc::new(AtomicBool::new(false)),
            hotkey_manager,
            hotkey_id_record: 0,
            hotkey_id_screenshot: 0,
            hotkey_id_summon: 0,
            hotkey_id_pause: 0,
            hotkey_id_region: 0,
            hotkey_id_window: 0,
            hotkey_id_gif: 0,
            hotkey_id_prtscn: 0,
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
            ffmpeg_tx: Some(ffmpeg_tx),
            ffmpeg_rx: Some(ffmpeg_rx),
            library_filter: "All".to_string(),
            library_sort: crate::app::LibrarySort::Newest,
            library_tile_size: 1,
            library_show_limit: LIBRARY_PAGE_SIZE,
            library_selected: std::collections::HashSet::new(),
            library_confirm_clear: false,
            confirm_delete: None,
            delete_guard_ok: std::collections::HashSet::new(),
            budget_frames_input: "0".to_string(),
            budget_mb_input: "0.0".to_string(),
            budget_minutes_input: "0".to_string(),
            budget_tier: "standard".to_string(),
            budget_samples: Vec::new(),
            budget_sample_at: None,
            img_resize_pct: 100,
            img_src_wh: (0, 0),
            export_fmt: StillExportFmt::Jpg,
            export_quality: 90,
            export_pad_px: 0,
            export_pad_color: egui::Color32::WHITE,
            badge_style: 0,
            edge_fx: 0,
            edge_fx_px: 4,
            edge_color: egui::Color32::from_gray(32),
            annotation_selected: None,
            watermark_text: String::new(),
            filmstrip_cut: std::collections::HashSet::new(),
            clip_compare: false,
            preview_audio_path: None,
            preview_audio_rx: None,
            preview_audio_playing: false,
            allow_exit: false,
            start_hidden: false,
            recording_arming: false,
            recording_cancel_armed: false,
            recording_finalizing: false,
            record_finalize_rx: None,
            voice_finalize_rx: None,
            library_scan_rx: None,
            library_scan_pending: false,
            recent_thumbs: Vec::new(),
            recent_thumbs_rx: None,
            recent_key: String::new(),
            recent_hover: None,
            window_list_rx: None,
            front_app_rx: None,
            audio_devices_rx: None,
            status_cache: None,
            live_stats_cache: None,
            last_review_tab: None,
            ui_ctx: None,
            pending_arm_record: false,
            filmstrip_error: None,
            dead_air_hint: None,
            dead_air_dismissed: false,
            palette_open: false,
            palette_query: String::new(),
            palette_selected: 0,
            settings_nav: "all".to_string(),
            settings_filter: String::new(),
            whats_new_tag: String::new(),
            whats_new_notes: String::new(),
            restore_tab: true,
            window_sizes: std::collections::HashMap::new(),
            rail_collapsed: false,
            rail_order: Vec::new(),
            top_tabs: false,
            reduce_motion: false,
            aurora_hue: 0.0,
            density: Density::Comfortable,
            undo_trash: None,
            capture_toast: None,
            last_capture: None,
            wizard_autostart: true,
            tab_back: Vec::new(),
            tab_fwd: Vec::new(),
            prev_tab: AppTab::Capture,
            tab_slide: None,
            autostart_state: None,
            rail_open: false,
            ..Default::default() // wizard_* default closed / not done
        };

        let session = load_session();
        app.apply_session(session);
        app.bind_global_hotkeys();
        if let Some(rx) = hotkey_receiver {
            spawn_wake_pump(
                rx,
                app.hotkey_id_screenshot,
                app.hotkey_id_record,
                app.hotkey_id_summon,
                app.hotkey_id_pause,
                app.hotkey_id_prtscn,
                app.hotkey_id_region,
                app.hotkey_id_window,
                app.hotkey_id_gif,
                app.wake_shared.clone(),
                cc.egui_ctx.clone(),
            );
        }
        // Home is always Capture — a capture ends in Review, but the next
        // launch must reopen the funnel at the top, not the last editor.
        app.current_tab = AppTab::Capture;
        app.prev_tab = AppTab::Capture;
        // Re-apply visuals if session asked for light (graphite was applied above).
        apply_current_theme(&cc.egui_ctx);
        app.brand_logo = load_brand_logo(&cc.egui_ctx);
        app.refresh_library();
        // E222 — recover unsaved annotations + orphaned frag recordings.
        app.recover_on_launch(&cc.egui_ctx);
        // E214 — opt-in launch check; a newer release surfaces as a toast.
        if app.update_check_on_launch {
            app.start_update_check();
        }
        // E215 — a previous run may have staged <exe>.new (survives
        // restarts); and a successful swap leaves <exe>.old to delete.
        crate::app::update::cleanup_old_binary();
        if crate::app::update::staged_update_exists() {
            app.update_staged = crate::app::update::staged_update_path();
            app.update_status = "update staged — restart to apply".into();
        }
        app
    }

    fn digit_code(d: u8) -> Code {
        match d {
            1 => Code::Digit1,
            2 => Code::Digit2,
            3 => Code::Digit3,
            4 => Code::Digit4,
            5 => Code::Digit5,
            6 => Code::Digit6,
            7 => Code::Digit7,
            8 => Code::Digit8,
            9 => Code::Digit9,
            0 => Code::Digit0,
            _ => Code::Digit2,
        }
    }

    fn bind_global_hotkeys(&mut self) {
        let Some(manager) = self.hotkey_manager.as_ref() else {
            return;
        };
        let rec = self.hotkey_rec_digit.clamp(0, 9);
        let shot = self.hotkey_shot_digit.clamp(0, 9);
        let hk_rec = HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::SHIFT),
            Self::digit_code(rec),
        );
        let hk_shot = HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::SHIFT),
            Self::digit_code(shot),
        );
        // Ctrl+Shift+V would steal "paste plain text" in other apps; Ctrl+Alt+V is free.
        let hk_summon = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyV);
        self.hotkey_id_record = hk_rec.id();
        self.hotkey_id_screenshot = hk_shot.id();
        self.hotkey_id_summon = hk_summon.id();
        let _ = manager.register(hk_rec);
        let _ = manager.register(hk_shot);
        let _ = manager.register(hk_summon);
        self.hotkey_rec_digit_prev = rec;
        self.hotkey_shot_digit_prev = shot;
        self.bind_extra_hotkeys();
    }

    /// E50/E204 — opt-in extras: pause digit (Ctrl+Shift+N) and bare PrtScn.
    /// Registers + records ids; shared by `bind` and `rebind`.
    fn bind_extra_hotkeys(&mut self) {
        self.hotkey_id_pause = 0;
        self.hotkey_id_prtscn = 0;
        self.hotkey_id_region = 0;
        self.hotkey_id_window = 0;
        self.hotkey_id_gif = 0;
        let Some(manager) = self.hotkey_manager.as_ref() else {
            return;
        };
        if let Some(d) = self.hotkey_pause_digit {
            let hk = HotKey::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Self::digit_code(d.clamp(0, 9)),
            );
            self.hotkey_id_pause = hk.id();
            let _ = manager.register(hk);
        }
        if self.hotkey_prtscn {
            let hk = HotKey::new(None, Code::PrintScreen);
            self.hotkey_id_prtscn = hk.id();
            let _ = manager.register(hk);
        }
        // E86 — per-mode digits share the Ctrl+Shift+N bank; skip any that
        // would collide with a sibling slot (register would fail anyway,
        // but a stale prev-id would mis-dispatch after rebind).
        let mut taken: Vec<u8> = vec![
            self.hotkey_shot_digit.clamp(0, 9),
            self.hotkey_rec_digit.clamp(0, 9),
        ];
        if let Some(d) = self.hotkey_pause_digit {
            taken.push(d.clamp(0, 9));
        }
        for (digit, slot) in [
            (self.hotkey_region_digit, &mut self.hotkey_id_region),
            (self.hotkey_window_digit, &mut self.hotkey_id_window),
            (self.hotkey_gif_digit, &mut self.hotkey_id_gif),
        ] {
            let Some(d) = digit else { continue };
            let d = d.clamp(0, 9);
            if taken.contains(&d) {
                continue;
            }
            let hk = HotKey::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Self::digit_code(d),
            );
            *slot = hk.id();
            let _ = manager.register(hk);
            taken.push(d);
        }
        self.hotkey_pause_digit_prev = self.hotkey_pause_digit;
        self.hotkey_prtscn_prev = self.hotkey_prtscn;
        self.hotkey_region_digit_prev = self.hotkey_region_digit;
        self.hotkey_window_digit_prev = self.hotkey_window_digit;
        self.hotkey_gif_digit_prev = self.hotkey_gif_digit;
    }

    /// Unregister the extras as previously bound (ids are deterministic
    /// hashes of mods+code, so rebuild them from the `_prev` fields).
    fn unbind_extra_hotkeys(&mut self) {
        let Some(manager) = self.hotkey_manager.as_ref() else {
            return;
        };
        if let Some(d) = self.hotkey_pause_digit_prev {
            let _ = manager.unregister(HotKey::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Self::digit_code(d.clamp(0, 9)),
            ));
        }
        for d in [
            self.hotkey_region_digit_prev,
            self.hotkey_window_digit_prev,
            self.hotkey_gif_digit_prev,
        ]
        .into_iter()
        .flatten()
        {
            let _ = manager.unregister(HotKey::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Self::digit_code(d.clamp(0, 9)),
            ));
        }
        if self.hotkey_prtscn_prev {
            let _ = manager.unregister(HotKey::new(None, Code::PrintScreen));
        }
    }

    /// Re-register global hotkeys after the user changes the digit in
    /// Settings — no restart needed (I201). Returns Err listing which
    /// bindings failed (e.g. another app owns the combo).
    pub(crate) fn rebind_global_hotkeys(&mut self) -> Result<(), String> {
        self.unbind_extra_hotkeys();
        let Some(manager) = self.hotkey_manager.as_ref() else {
            return Err("global hotkey manager unavailable".into());
        };
        // Reconstruct the previously registered HotKeys — unregister matches
        // on HotKey::id(), which is a deterministic hash of mods+code.
        let prev = [
            HotKey::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Self::digit_code(self.hotkey_rec_digit_prev),
            ),
            HotKey::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Self::digit_code(self.hotkey_shot_digit_prev),
            ),
            HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyV),
        ];
        for hk in prev {
            let _ = manager.unregister(hk);
        }
        let mut failed = Vec::new();
        let rec = self.hotkey_rec_digit.clamp(0, 9);
        let shot = self.hotkey_shot_digit.clamp(0, 9);
        let hk_rec = HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::SHIFT),
            Self::digit_code(rec),
        );
        let hk_shot = HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::SHIFT),
            Self::digit_code(shot),
        );
        let hk_summon = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyV);
        if manager.register(hk_rec).is_err() {
            failed.push(format!("Ctrl+Shift+{rec}"));
        }
        if manager.register(hk_shot).is_err() {
            failed.push(format!("Ctrl+Shift+{shot}"));
        }
        if manager.register(hk_summon).is_err() {
            failed.push("Ctrl+Alt+V".to_string());
        }
        self.hotkey_id_record = hk_rec.id();
        self.hotkey_id_screenshot = hk_shot.id();
        self.hotkey_id_summon = hk_summon.id();
        self.hotkey_rec_digit_prev = rec;
        self.hotkey_shot_digit_prev = shot;
        self.bind_extra_hotkeys();
        if failed.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{} already taken by another app",
                failed.join(", ")
            ))
        }
    }

    fn apply_session(&mut self, s: SessionState) {
        self.density = density_from_str(&s.density);
        // K258: a stale/hand-edited filter value would render the library as
        // an unexplained empty grid — whitelist to real categories.
        let valid_filters = [
            "All",
            "★ Favorites",
            MediaCategory::Screenshot.label(),
            MediaCategory::Video.label(),
            MediaCategory::Gif.label(),
            MediaCategory::Audio.label(),
            MediaCategory::Note.label(),
        ];
        if valid_filters.contains(&s.library_filter.as_str()) {
            self.library_filter = s.library_filter;
        } else if !s.library_filter.is_empty() {
            self.library_filter = "All".into();
        }
        // E49 — "always Capture" ignores the saved stage.
        self.restore_tab = s.restore_tab;
        self.current_tab = if s.restore_tab {
            match s.tab.as_str() {
                "library" | "media" => AppTab::Library,
                "edit" | "studio" | "clip" => AppTab::Clip,
                "still" | "image" | "review" => AppTab::Still,
                "feedback" | "inbox" => AppTab::Feedback,
                "settings" => AppTab::Settings,
                _ => AppTab::Capture,
            }
        } else {
            AppTab::Capture
        };
        self.window_sizes = s.window_sizes;
        self.rail_collapsed = s.rail_collapsed;
        self.rail_order = s.rail_order.clone();
        self.top_tabs = s.top_tabs;
        self.inspector_open = s.inspector_open;
        self.region_live_backdrop = s.region_live_backdrop;
        self.reduce_motion = s.reduce_motion;
        self.aurora_hue = s.aurora_hue;
        if let Some(p) = s.edit_file {
            let path = PathBuf::from(p);
            if path.exists() {
                let is_image = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| {
                        matches!(
                            e.to_ascii_lowercase().as_str(),
                            "jpg" | "jpeg" | "png" | "gif" | "webp"
                        )
                    })
                    .unwrap_or(false);
                if is_image || matches!(self.current_tab, AppTab::Still) {
                    self.img_edit_file = Some(path.clone());
                    self.latest_screenshot = Some(path.clone());
                } else {
                    self.edit_file = Some(path);
                }
            }
        }
        self.wizard_done = s.wizard_done;
        self.wizard_open = !s.wizard_done;
        self.wizard_step = 0;
        theme::set_theme_mode(theme::theme_mode_from_str(&s.theme));
        // K258: reject inverted/garbage rects — they'd feed a corrupt ghost
        // or crop bounds downstream.
        self.last_region = s.last_region.and_then(|a| {
            if a[2] > a[0] && a[3] > a[1] && a.iter().all(|v| v.is_finite()) {
                Some(Rect::from_min_max(
                    Pos2::new(a[0], a[1]),
                    Pos2::new(a[2], a[3]),
                ))
            } else {
                None
            }
        });
        self.record_countdown_secs = match s.record_countdown_secs {
            3 | 5 => s.record_countdown_secs,
            _ => 0,
        };
        if !s.name_pattern.trim().is_empty() {
            self.name_pattern = s.name_pattern;
        }
        if let Some([w, h, x, y]) = s.last_screen_rect {
            // K258: clamp to sane capture dims — a corrupt value must not
            // become a gdigrab rect.
            if (1..=32768).contains(&w) && (1..=32768).contains(&h) {
                self.selected_screen_rect = Some((w, h, x, y));
            }
        }
        self.draw_mouse = s.draw_mouse;
        if s.fps == 24 || s.fps == 30 || s.fps == 60 {
            self.fps_target = s.fps;
        }
        self.timelapse_secs = s.timelapse_secs.min(3600);
        self.record_crf = match s.record_crf {
            15..=40 => s.record_crf,
            _ => 23,
        };
        self.capture_monitor = s.monitor;
        self.audio_device = s.audio_device.clone();
        self.audio_mix_device = s.audio_mix_device.clone();
        self.window_app = s.window_app.clone();
        if !s.inbox_snippets.is_empty() {
            self.inbox_snippets = s.inbox_snippets;
        }
        if s.hotkey_shot_digit <= 9 {
            self.hotkey_shot_digit = s.hotkey_shot_digit;
        }
        if s.hotkey_rec_digit <= 9 {
            self.hotkey_rec_digit = s.hotkey_rec_digit;
        }
        self.hotkey_pause_digit = s.hotkey_pause_digit.filter(|d| *d <= 9);
        self.hotkey_region_digit = s.hotkey_region_digit.filter(|d| *d <= 9);
        self.hotkey_window_digit = s.hotkey_window_digit.filter(|d| *d <= 9);
        self.hotkey_gif_digit = s.hotkey_gif_digit.filter(|d| *d <= 9);
        // E51 — per-target memory: this hour's habit wins over the raw last pick.
        self.target_hours = s.target_hours;
        let hour = Local::now().hour() as u8;
        let remembered = self
            .target_hours
            .get(&hour)
            .or(if s.capture_target_name.is_empty() {
                None
            } else {
                Some(&s.capture_target_name)
            });
        if let Some(name) = remembered {
            self.capture_target = CaptureTarget::from_name(name);
        }
        self.hotkey_prtscn = s.hotkey_prtscn;
        self.shutter_sound = s.shutter_sound;
        self.clipboard_encode = s.clipboard_encode.clone();
        self.clipboard_watcher = s.clipboard_watcher;
        if self.clipboard_watcher {
            self.clipboard_seq_seen = crate::platform::clipboard_seq();
        }
        self.screen_permission_prompted = s.screen_permission_prompted;
        self.screen_permission_ok = s.screen_permission_ok;
        self.rail_open = s.rail_open;
        self.inbox_quiet = s.inbox_quiet;
        self.clip_autoplay = s.clip_autoplay;
        self.auto_dead_air = s.auto_dead_air;
        self.update_check_on_launch = s.update_check_on_launch;
        self.auto_open_review = s.auto_open_review;
        self.rec_bar_pos = s.rec_bar_pos.and_then(|[x, y]| {
            // Reject stale/off-screen positions (monitor unplugged etc.).
            (-8000..=16000)
                .contains(&x)
                .then_some((x, y))
                .filter(|_| (-2000..=16000).contains(&y))
        });
        self.filmstrip_low_res = s.filmstrip_low_res;
        self.region_dim = s.region_dim.min(200);
        self.region_pick_count = s.region_pick_count;
        self.hud_toolbar_bottom = s.hud_toolbar_bottom;
        self.library_favorites = s.library_favorites.iter().cloned().collect();
        self.library_flagged = s.library_flagged.iter().cloned().collect();
        self.library_tags = s.library_tags.clone();
        self.retention_mode = s.retention_mode;
        self.retention_value = s.retention_value.max(1);
        self.retention_auto = s.retention_auto;
        self.library_list_view = s.library_list_view;
        self.tray_dblclick = match s.tray_dblclick.as_str() {
            "screenshot" | "record" => s.tray_dblclick.clone(),
            _ => "open".into(),
        };
        self.watch_folder = s.watch_folder;
        self.theme_follow_os = s.theme_follow_os;
        self.theme_schedule = s.theme_schedule;
        self.saved_regions = s.saved_regions.clone();
        self.stats_opt_in = s.stats_opt_in;
        self.stat_shots_ok = s.stat_shots_ok;
        self.stat_shots_fail = s.stat_shots_fail;
        self.stat_recs_ok = s.stat_recs_ok;
        self.stat_recs_fail = s.stat_recs_fail;
        self.stats_opt_in = s.stats_opt_in;
        self.stat_shots_ok = s.stat_shots_ok;
        self.stat_shots_fail = s.stat_shots_fail;
        self.stat_recs_ok = s.stat_recs_ok;
        self.stat_recs_fail = s.stat_recs_fail;
        self.theme_dark_pick = match s.theme_dark_pick.as_str() {
            "carbon" | "dark" | "celestial" | "celestial-pink" => s.theme_dark_pick.clone(),
            _ => "dark".into(),
        };
        self.inbox_seen_stamp = s.inbox_seen_at.clone();
        self.whats_new_tag = s.whats_new_tag;
        self.whats_new_notes = s.whats_new_notes;
        // Re-check with a cheap, prompt-free preflight on the next frame.
        // The modal is shown by `update` only when the preflight actually fails —
        // never unconditionally, so granted users are not re-asked on cold start.
        self.screen_permission_pending = true;
        self.screen_perm_modal = false;
        // Restore last window size (floor keeps relaunch comfortably large).
        self.window_size = Vec2::new(s.window_w.max(1024.0), s.window_h.max(700.0));
    }

    /// Build the durable session snapshot — shared by `persist_session`
    /// and E213 profile export.
    pub(crate) fn session_snapshot(&self) -> SessionState {
        let tab = match self.current_tab {
            AppTab::Capture => "capture",
            AppTab::Library => "library",
            AppTab::Clip => "clip",
            AppTab::Still => "still",
            AppTab::Feedback => "feedback",
            AppTab::Settings => "settings",
        };
        let last_region = self
            .last_region
            .map(|r| [r.min.x, r.min.y, r.max.x, r.max.y]);
        // Prefer the path matching the active studio tab.
        let edit_file = match self.current_tab {
            AppTab::Still => self
                .img_edit_file
                .as_ref()
                .or(self.edit_file.as_ref())
                .map(|p| p.display().to_string()),
            _ => self.edit_file.as_ref().map(|p| p.display().to_string()),
        };
        SessionState {
            schema_version: crate::app::session::SESSION_SCHEMA,
            tab: tab.into(),
            edit_file,
            density: density_to_str(self.density).into(),
            library_filter: self.library_filter.clone(),
            window_w: self.window_size.x,
            window_h: self.window_size.y,
            wizard_done: self.wizard_done,
            theme: theme::theme_mode_to_str(theme::theme_mode()).into(),
            last_region,
            record_countdown_secs: self.record_countdown_secs,
            name_pattern: self.name_pattern.clone(),
            last_screen_rect: self.selected_screen_rect.map(|(w, h, x, y)| [w, h, x, y]),
            draw_mouse: self.draw_mouse,
            fps: self.fps_target,
            record_crf: self.record_crf,
            timelapse_secs: self.timelapse_secs,
            audio_device: self.audio_device.clone(),
            audio_mix_device: self.audio_mix_device.clone(),
            window_app: self.window_app.clone(),
            monitor: self.capture_monitor,
            inbox_snippets: self.inbox_snippets.clone(),
            hotkey_shot_digit: self.hotkey_shot_digit,
            hotkey_rec_digit: self.hotkey_rec_digit,
            hotkey_pause_digit: self.hotkey_pause_digit,
            hotkey_region_digit: self.hotkey_region_digit,
            hotkey_window_digit: self.hotkey_window_digit,
            hotkey_gif_digit: self.hotkey_gif_digit,
            capture_target_name: self.capture_target.name().to_string(),
            target_hours: self.target_hours.clone(),
            hotkey_prtscn: self.hotkey_prtscn,
            shutter_sound: self.shutter_sound,
            clipboard_encode: self.clipboard_encode.clone(),
            clipboard_watcher: self.clipboard_watcher,
            screen_permission_prompted: self.screen_permission_prompted,
            screen_permission_ok: self.screen_permission_ok,
            rail_open: self.rail_open,
            inbox_quiet: self.inbox_quiet,
            clip_autoplay: self.clip_autoplay,
            auto_dead_air: self.auto_dead_air,
            update_check_on_launch: self.update_check_on_launch,
            auto_open_review: self.auto_open_review,
            rec_bar_pos: self.rec_bar_pos.map(|(x, y)| [x, y]),
            filmstrip_low_res: self.filmstrip_low_res,
            region_dim: self.region_dim,
            region_pick_count: self.region_pick_count,
            hud_toolbar_bottom: self.hud_toolbar_bottom,
            library_favorites: self.library_favorites.iter().cloned().collect(),
            library_flagged: self.library_flagged.iter().cloned().collect(),
            inbox_seen_at: self.inbox_seen_stamp.clone(),
            library_tags: self.library_tags.clone(),
            retention_mode: self.retention_mode,
            retention_value: self.retention_value,
            retention_auto: self.retention_auto,
            library_list_view: self.library_list_view,
            tray_dblclick: self.tray_dblclick.clone(),
            watch_folder: self.watch_folder.clone(),
            theme_follow_os: self.theme_follow_os,
            theme_schedule: self.theme_schedule,
            saved_regions: self.saved_regions.clone(),
            stats_opt_in: self.stats_opt_in,
            stat_shots_ok: self.stat_shots_ok,
            stat_shots_fail: self.stat_shots_fail,
            stat_recs_ok: self.stat_recs_ok,
            stat_recs_fail: self.stat_recs_fail,
            theme_dark_pick: self.theme_dark_pick.clone(),
            whats_new_tag: self.whats_new_tag.clone(),
            whats_new_notes: self.whats_new_notes.clone(),
            restore_tab: self.restore_tab,
            window_sizes: self.window_sizes.clone(),
            rail_collapsed: self.rail_collapsed,
            rail_order: self.rail_order.clone(),
            top_tabs: self.top_tabs,
            inspector_open: self.inspector_open,
            region_live_backdrop: self.region_live_backdrop,
            reduce_motion: self.reduce_motion,
            aurora_hue: self.aurora_hue,
        }
    }

    /// E244 — callers just mark the session dirty; update() coalesces
    /// bursts of state changes into one write per 500 ms window instead of
    /// serializing + writing on every slider tick / toast / tab switch.
    pub(crate) fn persist_session(&self) {
        self.session_dirty.set(true);
        // Guarantee an update() tick so the flush can't starve under
        // repaint-on-demand.
        if let Some(c) = &self.ui_ctx {
            c.request_repaint_after(Duration::from_millis(600));
        }
    }

    /// E244 — synchronous flush for exit paths; the debounce must not lose
    /// the last write on quit.
    fn flush_session_now(&self) {
        if self.session_dirty.replace(false) {
            save_session(&self.session_snapshot());
        }
    }

    /// E244 — the debounced flush, called each frame.
    fn tick_session_write(&mut self) {
        if !self.session_dirty.get() {
            return;
        }
        let due = self
            .session_last_write
            .get()
            .map(|t| t.elapsed() >= Duration::from_millis(500))
            .unwrap_or(true);
        if due {
            save_session(&self.session_snapshot());
            self.session_last_write.set(Some(Instant::now()));
            self.session_dirty.set(false);
        }
    }

    /// Load a still into Still studio and select that tab.
    fn open_still_from_path(&mut self, path: PathBuf) {
        self.load_still_from_path(&path);
        self.current_tab = AppTab::Still;
    }

    /// Load-only half of `open_still_from_path` — D70 lets a capture stage
    /// the editor without yanking the user over to it.
    fn load_still_from_path(&mut self, path: &PathBuf) {
        self.img_src_wh = image::image_dimensions(path).unwrap_or((0, 0));
        self.img_source_dims = image::image_dimensions(path)
            .map(|(w, h)| format!("{}×{}", w, h))
            .unwrap_or_default();
        self.img_preview_params.clear();
        self.img_preview_on = true;
        self.img_edit_file = Some(path.clone());
        self.latest_screenshot = Some(path.clone());
    }

    pub fn save_current_still(&mut self) {
        self.save_still_to(None);
    }

    pub fn save_current_still_copy(&mut self) {
        let Some(path) = self.img_edit_file.clone() else {
            self.show_toast("No image loaded to save");
            return;
        };
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "still".into());
        let dest = path.with_file_name(format!("{stem}_annotated.jpg"));
        self.save_still_to(Some(dest));
    }

    /// Edited pixels with annotations baked in — shared by save/copy/export.
    fn edited_baked_image(&mut self) -> Result<image::DynamicImage, String> {
        let Some(path) = self.img_edit_file.clone() else {
            return Err("No image loaded".into());
        };
        let mut dyn_img = match self.compute_edited_image(None) {
            Ok(img) => img,
            Err(_) => image::open(&path).map_err(|e| format!("Could not read image: {e}"))?,
        };
        app::bake_annotations(
            &mut dyn_img,
            &self.annotation_actions,
            self.annotation_canvas_rect,
        );
        // E35 — edge treatment sits inside the export pad: pad = outer
        // margin, border/shadow hugs the image edge.
        Ok(app::apply_edge_fx(
            dyn_img,
            self.edge_fx,
            self.edge_fx_px,
            self.edge_color,
        ))
    }

    /// Final pixel dims after crop/rotate/resize/pad — the EXPORT readout (F118).
    pub(crate) fn edited_output_dims(&self) -> Option<(u32, u32)> {
        let (mut w, mut h) = self.img_src_wh;
        if w == 0 || h == 0 {
            return None;
        }
        if let (Ok(cx), Ok(cy), Ok(cw), Ok(ch)) = (
            self.img_crop_x.parse::<u32>(),
            self.img_crop_y.parse::<u32>(),
            self.img_crop_w.parse::<u32>(),
            self.img_crop_h.parse::<u32>(),
        ) {
            if cw > 0 && ch > 0 && cx + cw <= w && cy + ch <= h {
                w = cw;
                h = ch;
            }
        }
        if self.img_rotate == 90 || self.img_rotate == 270 {
            std::mem::swap(&mut w, &mut h);
        }
        if self.img_resize_pct != 100 && self.img_resize_pct > 0 {
            w = (w as f32 * self.img_resize_pct as f32 / 100.0).max(1.0) as u32;
            h = (h as f32 * self.img_resize_pct as f32 / 100.0).max(1.0) as u32;
        }
        Some((w, h))
    }

    /// Export dims include the uniform pad (F122) — pad is export-only, applied
    /// post-bake so annotation mapping stays aligned.
    pub(crate) fn export_output_dims(&self) -> Option<(u32, u32)> {
        self.edited_output_dims().map(|(w, h)| {
            // E35 — border/shadow grows the canvas before the pad margin.
            let px = self.edge_fx_px.max(1);
            let (w, h) = match self.edge_fx {
                1 => (w + px * 2, h + px * 2),
                2 => {
                    let m = px * 2;
                    let off = (px / 2).max(2);
                    (w + m * 2 + off, h + m * 2 + off)
                }
                _ => (w, h),
            };
            let pad = self.export_pad_px.saturating_mul(2);
            (w.saturating_add(pad), h.saturating_add(pad))
        })
    }

    /// F117/F122 — export with explicit format, quality and uniform pad.
    pub fn export_still_as(&mut self) {
        let Some(path) = self.img_edit_file.clone() else {
            self.show_toast("No image loaded to export");
            return;
        };
        let mut img = match self.edited_baked_image() {
            Ok(i) => i,
            Err(e) => {
                self.show_toast(&format!("❌ {e}"));
                return;
            }
        };
        if self.export_pad_px > 0 {
            let pad = self.export_pad_px;
            let c = self.export_pad_color;
            let mut canvas = image::RgbaImage::from_pixel(
                img.width() + pad * 2,
                img.height() + pad * 2,
                image::Rgba([c.r(), c.g(), c.b(), 255]),
            );
            image::imageops::overlay(&mut canvas, &img.to_rgba8(), pad.into(), pad.into());
            img = image::DynamicImage::ImageRgba8(canvas);
        }
        let ext = self.export_fmt.ext();
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "still".into());
        let Some(dest) = rfd::FileDialog::new()
            .set_file_name(format!("{stem}.{ext}"))
            .add_filter(self.export_fmt.label(), &[ext])
            .set_directory(path.parent().unwrap_or_else(|| std::path::Path::new("")))
            .save_file()
        else {
            return;
        };
        let dest = if dest.extension().is_some() {
            dest
        } else {
            dest.with_extension(ext)
        };
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        use image::ImageEncoder;
        let encoded: Result<Vec<u8>, image::ImageError> = (|| {
            let mut buf = std::io::Cursor::new(Vec::new());
            match self.export_fmt {
                StillExportFmt::Jpg => image::codecs::jpeg::JpegEncoder::new_with_quality(
                    &mut buf,
                    self.export_quality,
                )
                .write_image(
                    img.to_rgb8().as_raw(),
                    w,
                    h,
                    image::ExtendedColorType::Rgb8,
                ),
                StillExportFmt::Png => image::codecs::png::PngEncoder::new(&mut buf).write_image(
                    rgba.as_raw(),
                    w,
                    h,
                    image::ExtendedColorType::Rgba8,
                ),
                StillExportFmt::WebP => image::codecs::webp::WebPEncoder::new_lossless(&mut buf)
                    .write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8),
            }
            .map(|_| buf.into_inner())
        })();
        match encoded.and_then(|b| std::fs::write(&dest, b).map_err(image::ImageError::IoError)) {
            Ok(_) => {
                self.show_toast(format!(
                    "Exported {} · {}",
                    self.export_fmt.label(),
                    dest.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| dest.display().to_string())
                ));
                self.refresh_library();
            }
            Err(e) => self.show_toast(&format!("❌ Export failed: {e}")),
        }
    }

    /// D68 — `file:///…` URI for the loaded still.
    pub fn copy_still_file_uri(&mut self) {
        let Some(path) = self.img_edit_file.clone() else {
            self.show_toast("No image loaded");
            return;
        };
        match arboard::Clipboard::new().and_then(|mut b| b.set_text(app::file_uri(&path))) {
            Ok(_) => self.show_toast("file:// URI copied"),
            Err(e) => self.show_toast(&format!("❌ Clipboard: {e}")),
        }
    }

    /// D68 — `data:<mime>;base64,…` for the loaded still (capped at 8 MB).
    pub fn copy_still_data_uri(&mut self) {
        let Some(path) = self.img_edit_file.clone() else {
            self.show_toast("No image loaded");
            return;
        };
        match app::data_uri(&path, 8_000_000) {
            Ok(uri) => match arboard::Clipboard::new().and_then(|mut b| b.set_text(uri)) {
                Ok(_) => self.show_toast("data: URI copied"),
                Err(e) => self.show_toast(&format!("❌ Clipboard: {e}")),
            },
            Err(e) => self.show_toast(&format!("⚠ {e}")),
        }
    }

    /// D72 — JPEG q85 re-encode, shrinking 0.8× per pass until under
    /// Discord's 8 MB. Writes `<stem>_discord.jpg` beside the original.
    pub fn export_still_for_discord(&mut self) {
        let Some(path) = self.img_edit_file.clone() else {
            self.show_toast("No image loaded to export");
            return;
        };
        let img = match self.edited_baked_image() {
            Ok(i) => i,
            Err(e) => {
                self.show_toast(&format!("❌ {e}"));
                return;
            }
        };
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "still".into());
        let dest = path.with_file_name(format!("{stem}_discord.jpg"));
        const CAP: u64 = 8_000_000;
        use image::ImageEncoder;
        let mut cur = img.clone();
        let mut scale = 1.0f32;
        let mut out_bytes = Vec::new();
        for _ in 0..6 {
            let rgba = cur.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            let mut buf = std::io::Cursor::new(Vec::new());
            if image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 85)
                .write_image(cur.to_rgb8().as_raw(), w, h, image::ExtendedColorType::Rgb8)
                .is_err()
            {
                self.show_toast("❌ Encode failed");
                return;
            }
            out_bytes = buf.into_inner();
            if out_bytes.len() as u64 <= CAP {
                break;
            }
            scale *= 0.8;
            let nw = ((img.width() as f32 * scale).round() as u32).max(1);
            let nh = ((img.height() as f32 * scale).round() as u32).max(1);
            cur = img.resize(nw, nh, image::imageops::FilterType::Lanczos3);
        }
        match std::fs::write(&dest, &out_bytes) {
            Ok(_) => {
                let mb = out_bytes.len() as f64 / 1e6;
                let over = if out_bytes.len() as u64 > CAP {
                    " — still over 8 MB"
                } else {
                    ""
                };
                self.show_toast(format!(
                    "💾 {mb:.1} MB{over} · {}",
                    dest.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                ));
                self.refresh_library();
            }
            Err(e) => self.show_toast(&format!("❌ Export failed: {e}")),
        }
    }

    fn save_still_to(&mut self, dest: Option<PathBuf>) {
        let Some(path) = self.img_edit_file.clone() else {
            self.show_toast("No image loaded to save");
            return;
        };
        let out = dest.unwrap_or_else(|| path.clone());
        let dyn_img = match self.edited_baked_image() {
            Ok(img) => img,
            Err(e) => {
                self.show_toast(&format!("❌ {e}"));
                return;
            }
        };
        match dyn_img.save(&out) {
            Ok(_) => {
                if out == path {
                    self.show_toast("Image & annotations saved (overwrite)");
                } else {
                    self.show_toast(format!(
                        "Saved copy · {}",
                        out.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| out.display().to_string())
                    ));
                    self.img_edit_file = Some(out);
                }
                self.refresh_library();
            }
            Err(e) => self.show_toast(&format!("❌ Save failed: {}", e)),
        }
    }

    pub fn copy_current_still_to_clipboard(&mut self) {
        if self.img_edit_file.is_none() {
            self.show_toast("No image loaded to copy");
            return;
        }
        let dyn_img = match self.edited_baked_image() {
            Ok(img) => img,
            Err(e) => {
                self.show_toast(&format!("❌ {e}"));
                return;
            }
        };

        let rgba = dyn_img.to_rgba8();
        let (w, h) = (rgba.width() as usize, rgba.height() as usize);
        if let Ok(mut board) = arboard::Clipboard::new() {
            let img_data = arboard::ImageData {
                width: w,
                height: h,
                bytes: std::borrow::Cow::Borrowed(rgba.as_raw()),
            };
            if board.set_image(img_data).is_ok() {
                self.clipboard_seq_seen = crate::platform::clipboard_seq();
                self.show_toast("📋 Image copied to system clipboard!");
            } else {
                self.show_toast("❌ Clipboard copy failed");
            }
        }
    }

    /// Blocking overlay: Screen Recording onboarding / recovery.
    fn draw_screen_perm_modal(&mut self, ctx: &egui::Context) {
        let mut dismiss = false;
        let mut retest = false;
        let mut open_settings = false;

        egui::Area::new(egui::Id::new("screen_perm_dim"))
            .order(egui::Order::Middle)
            .fixed_pos(egui::pos2(0.0, 0.0))
            .interactable(false)
            .show(ctx, |ui| {
                let rect = ui.ctx().screen_rect();
                ui.painter().rect_filled(rect, 0.0, theme::OVERLAY_DIM());
            });

        egui::Area::new(egui::Id::new("screen_perm_modal"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(theme::SURFACE())
                    .stroke(Stroke::new(1.0_f32, theme::BORDER()))
                    .rounding(theme::rounding_lg())
                    .inner_margin(egui::Margin::symmetric(28.0, 24.0))
                    .show(ui, |ui| {
                        ui.set_width(440.0);
                        ui.label(
                            RichText::new("Screen Recording")
                                .size(20.0)
                                .strong()
                                .color(theme::TEXT()),
                        );
                        ui.add_space(theme::SP_2);
                        match self.screen_perm_probe_ok {
                            None => {
                                ui.label(
                                    RichText::new(
                                        "Checking Screen Recording permission. If macOS shows a dialog, click Allow so Vibecap can capture windows and screens.",
                                    )
                                    .color(theme::TEXT_MUTED()),
                                );
                                ui.add_space(theme::SP_2);
                                ui.label(
                                    RichText::new("Waiting for system check…")
                                        .small()
                                        .color(theme::TEXT_DIM()),
                                );
                            }
                            Some(true) => {
                                ui.label(
                                    RichText::new(
                                        "Capture looks allowed. You can start screenshotting and recording.",
                                    )
                                    .color(theme::SUCCESS()),
                                );
                            }
                            Some(false) => {
                                ui.label(
                                    RichText::new(
                                        "Capture is not fully allowed yet (often looks like bare desktop wallpaper only).",
                                    )
                                    .color(theme::WARN()),
                                );
                                ui.add_space(theme::SP_2);
                                ui.label(
                                    RichText::new(
                                        "1. Open Settings → enable only “Vibecap”\n\
                                         2. Quit this app completely (tray Quit)\n\
                                         3. Reopen from Applications, then Test again",
                                    )
                                    .color(theme::TEXT_MUTED()),
                                );
                            }
                        }
                        ui.add_space(theme::SP_4);
                        ui.horizontal(|ui| {
                            if ui
                                .button(RichText::new("Open Settings…").strong())
                                .clicked()
                            {
                                open_settings = true;
                            }
                            if ui.button("Test again").clicked() {
                                retest = true;
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let can_continue = matches!(self.screen_perm_probe_ok, Some(true))
                                        || matches!(self.screen_perm_probe_ok, Some(false));
                                    if ui
                                        .add_enabled(
                                            can_continue,
                                            egui::Button::new(
                                                RichText::new(if self.screen_perm_probe_ok == Some(true) {
                                                    "Continue"
                                                } else {
                                                    "Continue anyway"
                                                })
                                                .strong(),
                                            ),
                                        )
                                        .clicked()
                                    {
                                        dismiss = true;
                                    }
                                },
                            );
                        });
                    });
            });

        if open_settings {
            let _ = open_screen_recording_settings();
        }
        if retest {
            self.screen_perm_probe_ok = None;
            self.screen_permission_ok = false;
            let (tx, rx) = std::sync::mpsc::channel();
            let ctx_probe = ctx.clone();
            std::thread::spawn(move || {
                let ok = request_screen_recording_access().unwrap_or(false);
                let _ = tx.send(ok);
                ctx_probe.request_repaint();
            });
            self.screen_perm_rx = Some(rx);
        }
        if dismiss {
            self.screen_perm_modal = false;
            if self.screen_perm_probe_ok == Some(true) {
                self.screen_permission_ok = true;
            }
            self.persist_session();
        }
    }

    pub(crate) fn set_theme(&mut self, ctx: &egui::Context, mode: ThemeMode) {
        match mode {
            ThemeMode::Carbon => theme::apply_carbon_theme(ctx),
            ThemeMode::Dark => apply_graphite_theme(ctx),
            ThemeMode::Light => theme::apply_light_theme(ctx),
            ThemeMode::Celestial => apply_celestial_theme(ctx),
            ThemeMode::CelestialPink => theme::apply_celestial_pink_theme(ctx),
        }
        // E225 — a manually-picked dark theme becomes the follow-OS dark
        // target (Light is the OS-light target, not a pick).
        if mode != ThemeMode::Light && self.theme_follow_os {
            self.theme_dark_pick = theme::theme_mode_to_str(mode).to_string();
        }
        self.persist_session();
    }

    /// E225 — live dark-mode follow: poll AppsUseLightTheme every 3 s and
    /// re-theme on transitions (and once on enable, to converge).
    /// E6 — scheduled themes share this tick: when on, local hour decides
    /// dark instead of the OS probe (Light 07:00–19:00, else dark pick).
    fn tick_os_theme(&mut self, ctx: &egui::Context) {
        if !self.theme_follow_os && !self.theme_schedule {
            self.os_dark_seen = None;
            return;
        }
        let now = std::time::Instant::now();
        if self
            .os_theme_poll_at
            .map(|t| now.duration_since(t) < std::time::Duration::from_secs(3))
            .unwrap_or(false)
        {
            return;
        }
        self.os_theme_poll_at = Some(now);
        let dark = if self.theme_schedule {
            let h = chrono::Local::now().hour();
            !(7..19).contains(&h)
        } else {
            let Some(dark) = crate::platform::os_apps_dark() else {
                return;
            };
            dark
        };
        let want = if dark {
            theme::theme_mode_from_str(&self.theme_dark_pick)
        } else {
            ThemeMode::Light
        };
        let changed = match self.os_dark_seen {
            None => theme::theme_mode() != want, // converge on launch/enable
            Some(prev) => prev != dark,          // OS toggled
        };
        self.os_dark_seen = Some(dark);
        if changed {
            self.set_theme(ctx, want);
            self.show_toast(format!(
                "{} → {}",
                if self.theme_schedule {
                    "Theme schedule"
                } else {
                    "Theme followed Windows"
                },
                theme::theme_mode_label(want)
            ));
        }
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

    /// Refresh the window-target list on a worker — `list_running_apps` shells
    /// out to PowerShell on Windows (~300-800ms), so it must not run on the UI
    /// thread. Results land in drain_window_list; the worker also primes the
    /// capture-window cache the ComboBox reads.
    pub(crate) fn refresh_window_list(&mut self) {
        self.window_list_scanned = true;
        if self.window_list_rx.is_some() {
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.window_list_rx = Some(rx);
        let ctx_clone = self.ui_ctx.clone();
        std::thread::spawn(move || {
            let apps = list_running_apps();
            // Prime the shared cache so the picker ComboBox never blocks on PS.
            let _ = crate::platform::list_capture_windows();
            let _ = tx.send(apps);
            if let Some(c) = ctx_clone {
                c.request_repaint();
            }
        });
    }

    fn drain_window_list(&mut self) {
        let Some(rx) = self.window_list_rx.as_ref() else {
            return;
        };
        let Ok(apps) = rx.try_recv() else {
            return;
        };
        self.window_list_rx = None;
        self.window_app_list = apps;
        if self.window_app.is_empty() {
            if let Some(first) = self.window_app_list.first() {
                self.window_app = first.clone();
            }
        }
        // Keep a typed name even if not in the list — user may have typed it.
    }

    /// Track the app the user was in before focusing Vibecap (for Fullscreen capture).
    fn poll_frontmost_app(&mut self) {
        // Drain a finished probe first.
        if let Some(rx) = self.front_app_rx.as_ref() {
            match rx.try_recv() {
                Ok(Some(name)) => {
                    self.front_app_rx = None;
                    let lower = name.to_ascii_lowercase();
                    if !lower.contains("vibecap") {
                        self.last_front_app = Some(name);
                    }
                }
                Ok(None) => self.front_app_rx = None,
                Err(_) => {}
            }
        }
        // Windows resolves this via PowerShell (~200-500ms spawn) — run it on a
        // worker and poll rarely.
        let throttle_ms = if cfg!(target_os = "windows") {
            2000
        } else {
            400
        };
        let due = self
            .last_front_poll
            .map(|t| t.elapsed() > Duration::from_millis(throttle_ms))
            .unwrap_or(true);
        if !due || self.front_app_rx.is_some() {
            return;
        }
        self.last_front_poll = Some(Instant::now());
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.front_app_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(frontmost_app_name());
        });
    }

    /// Which editor the rail's Review stage opens: whichever has content,
    /// preferring the one the user last used when both are loaded.
    fn review_tab(&self) -> AppTab {
        match (self.img_edit_file.is_some(), self.edit_file.is_some()) {
            (true, false) => AppTab::Still,
            (false, true) => AppTab::Clip,
            _ => self.last_review_tab.unwrap_or(AppTab::Still),
        }
    }

    /// Rail stage → concrete tab. Review dispatches to the right editor.
    fn tab_for_loop(&self, stage: LoopStage) -> AppTab {
        match stage {
            LoopStage::Shutter => AppTab::Capture,
            LoopStage::Review => self.review_tab(),
            LoopStage::Media => AppTab::Library,
            LoopStage::Inbox => AppTab::Feedback,
            LoopStage::Settings => AppTab::Settings,
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
        if self.is_recording
            || self.recording_arming
            || self.recording_finalizing
            || self.countdown_deadline.is_some()
        {
            return;
        }
        // Window target with no app chosen would silently become fullscreen —
        // refuse instead so the user picks a real target.
        if self.capture_target == CaptureTarget::Window && self.capture_focus_target().is_none() {
            self.show_toast("Pick a window app first — or switch to Full.");
            return;
        }
        // E265 — a CLI/MCP agent recording already holds the capture lock;
        // refuse rather than spawn a second ffmpeg fighting for the desktop.
        if let Some(s) = app::agent_record::load_record_state() {
            if app::agent_record::record_pid_alive(s.pid) {
                self.show_toast(format!(
                    "⚠️ A CLI recording is already running → {} — `vibecap record stop` first.",
                    s.mp4
                ));
                return;
            }
        }
        if let Some(app) = self.capture_focus_target() {
            if let Err(e) = focus_app(&app) {
                self.show_toast(format!(
                    "⚠️ Could not focus “{}” — recording whatever is on screen. {}",
                    app, e
                ));
            }
        }
        let secs = self.record_countdown_secs;
        if secs == 0 {
            self.arm_recording(ctx);
        } else {
            self.countdown_deadline = Some(Instant::now() + Duration::from_secs(secs as u64));
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

        let mut gif: Option<PathBuf> = None;
        match self.retro.dump_gif(&self.save_dir) {
            Ok(g) => {
                parts.push(
                    g.file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| "retro.gif".into()),
                );
                gif = Some(g);
            }
            Err(_) => {
                // Retro empty / off — still is enough; hint to enable buffer next time.
                parts.push("(no retro — enable buffer for last-N GIF)".into());
            }
        }

        // E297 — one attachable bundle: the capture artifacts + doctor
        // report + live config + newest ffmpeg stderr log, zipped STORED.
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        if let Ok(b) = std::fs::read(&shot) {
            entries.push((
                shot.file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| "screenshot.jpg".into()),
                b,
            ));
        }
        if let Some(g) = &gif {
            if let Ok(b) = std::fs::read(g) {
                entries.push(("retro.gif".into(), b));
            }
        }
        entries.push((
            "doctor.json".into(),
            crate::app::doctor::doctor_json().into_bytes(),
        ));
        entries.push((
            "system.txt".into(),
            format!(
                "vibecap {} on {}\nexe={}\nsave_dir={}\ntime={}\n",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::current_exe()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "?".into()),
                self.save_dir.display(),
                Local::now().format("%Y-%m-%d %H:%M:%S %Z"),
            )
            .into_bytes(),
        ));
        for cfg in [
            "session.json",
            "budget.json",
            "review_draft.json",
            "crash.log",
        ] {
            let p = crate::app::io::vibecap_config_dir().join(cfg);
            if let Ok(b) = std::fs::read(&p) {
                entries.push((cfg.to_string(), b));
            }
        }
        // Newest recording stderr — ffmpeg flags live here, not in a log file.
        if let Ok(rd) = std::fs::read_dir(&self.save_dir) {
            let newest_log = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().ends_with(".ffmpeg.log"))
                        .unwrap_or(false)
                })
                .max_by_key(|p| {
                    p.metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                });
            if let Some(log) = newest_log {
                if let Ok(b) = std::fs::read(&log) {
                    entries.push((
                        log.file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_else(|| "ffmpeg.log".into()),
                        b,
                    ));
                }
            }
        }

        let zip = self.save_dir.join(format!("bug_{}.zip", stamp));
        match crate::app::zip::write_zip_entries(&zip, &entries) {
            Ok(()) => parts.insert(
                0,
                zip.file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| "bug.zip".into()),
            ),
            Err(e) => parts.push(format!("(zip failed: {e})")),
        }

        self.refresh_library();
        self.show_window(ctx);
        self.show_toast(format!("🐛 Bug pack saved — {}", parts.join(" · ")));
    }

    fn run_palette_action(&mut self, ctx: &egui::Context, action: PaletteAction) {
        // Media rows are positional — the index is only meaningful for the
        // list that rendered them, so they never join the MRU.
        if !matches!(
            action,
            PaletteAction::OpenMedia(_) | PaletteAction::ApplyRegion(_)
        ) {
            self.palette_mru.retain(|a| *a != action);
            self.palette_mru.insert(0, action);
            self.palette_mru.truncate(3);
        }
        match action {
            PaletteAction::GoShutter => self.current_tab = AppTab::Capture,
            PaletteAction::GoMedia => self.current_tab = AppTab::Library,
            PaletteAction::GoReview => self.current_tab = self.review_tab(),
            PaletteAction::GoInbox => self.current_tab = AppTab::Feedback,
            PaletteAction::GoSettings => self.current_tab = AppTab::Settings,
            PaletteAction::Screenshot => self.trigger_capture(ctx, true),
            PaletteAction::RepeatLast => self.repeat_last_capture(ctx),
            PaletteAction::CopyLastMarkdown => match self.last_capture.clone() {
                Some(LastCapture::Still(p) | LastCapture::Clip(p)) => {
                    let md = format!("![]({})", p.display());
                    if arboard::Clipboard::new()
                        .and_then(|mut b| b.set_text(md))
                        .is_ok()
                    {
                        self.show_toast("📋 Markdown copied");
                    } else {
                        self.show_toast("❌ Could not copy");
                    }
                }
                None => self.show_toast("Nothing captured yet"),
            },
            PaletteAction::CopyLastPath => match self.last_capture.clone() {
                Some(LastCapture::Still(p) | LastCapture::Clip(p)) => {
                    if arboard::Clipboard::new()
                        .and_then(|mut b| b.set_text(p.display().to_string()))
                        .is_ok()
                    {
                        self.show_toast("📋 Path copied");
                    } else {
                        self.show_toast("❌ Could not copy");
                    }
                }
                None => self.show_toast("Nothing captured yet"),
            },
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
            PaletteAction::ToggleZen => {
                self.zen_mode = !self.zen_mode;
                self.show_toast(if self.zen_mode {
                    "Zen mode — Ctrl+K to bring chrome back"
                } else {
                    "Zen mode off"
                });
            }
            PaletteAction::ToggleDensity => {
                self.density = match self.density {
                    Density::Comfortable => Density::Compact,
                    Density::Compact => Density::Comfortable,
                };
                self.persist_session();
                self.show_toast(format!("Density: {}", density_to_str(self.density)));
            }
            PaletteAction::ToggleTheme => {
                let order = &theme::THEME_ORDER;
                let cur = order
                    .iter()
                    .position(|m| *m == theme::theme_mode())
                    .unwrap_or(0);
                let next = order[(cur + 1) % order.len()];
                self.set_theme(ctx, next);
                self.show_toast(format!("Theme: {}", theme::theme_mode_label(next)));
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
            PaletteAction::QuitApp => {
                self.quit_app();
            }
            // E22 — a saved region applies instantly: set the pixel rect
            // and fire the same direct re-grab path as repeat-last.
            PaletteAction::ApplyRegion(i) => {
                if let Some((name, r)) = self.saved_regions.get(i).cloned() {
                    self.capture_target = CaptureTarget::Region;
                    self.capture_rect_still(ctx, (r[0], r[1], r[2], r[3]));
                    self.show_toast(format!("▦ {name} — capturing"));
                }
            }
            PaletteAction::OpenMedia(i) => {
                if let Some(item) = self.library_items.get(i) {
                    let p = item.path.clone();
                    let is_video =
                        matches!(item.category, MediaCategory::Video | MediaCategory::Gif);
                    let copied = arboard::Clipboard::new()
                        .and_then(|mut b| b.set_text(p.display().to_string()))
                        .is_ok();
                    if is_video {
                        self.edit_file = Some(p);
                        self.current_tab = AppTab::Clip;
                    } else {
                        self.open_still_from_path(p);
                    }
                    self.show_toast(if copied {
                        "Opened · path copied"
                    } else {
                        "Opened"
                    });
                }
            }
        }
        self.persist_session();
    }

    /// Snapshot current strokes before a mutation (drag start / clear / text).
    fn annotation_push_undo(&mut self) {
        self.annotation_undo.push(self.annotation_actions.clone());
        if self.annotation_undo.len() > 40 {
            self.annotation_undo.remove(0);
        }
        self.annotation_redo.clear();
    }

    fn annotation_do_undo(&mut self) {
        if let Some(prev) = self.annotation_undo.pop() {
            self.annotation_redo
                .push(std::mem::take(&mut self.annotation_actions));
            self.annotation_actions = prev;
            self.step_counter = app::renumber_step_badges(&mut self.annotation_actions);
        }
    }

    fn annotation_do_redo(&mut self) {
        if let Some(next) = self.annotation_redo.pop() {
            self.annotation_undo
                .push(std::mem::take(&mut self.annotation_actions));
            self.annotation_actions = next;
            self.step_counter = app::renumber_step_badges(&mut self.annotation_actions);
        }
    }

    /// Delete one stroke from the STROKES panel / Del key (E123). Undoable;
    /// step badges renumber so the sequence stays contiguous.
    pub fn remove_annotation(&mut self, idx: usize) {
        if idx >= self.annotation_actions.len() {
            return;
        }
        self.annotation_push_undo();
        self.annotation_actions.remove(idx);
        self.step_counter = app::renumber_step_badges(&mut self.annotation_actions);
        self.annotation_selected = match self.annotation_selected {
            Some(s) if s == idx => None,
            Some(s) if s > idx => Some(s - 1),
            s => s,
        };
    }

    /// Keep strokes glued to image content when the canvas rect moves
    /// (zoom, pan, layout splits): points live in canvas coordinates, so a
    /// moved rect re-projects them through the old→new mapping.
    pub(crate) fn sync_annotation_canvas(&mut self, new_rect: Rect) {
        if let Some(old) = self.annotation_canvas_rect {
            let moved = (old.min.x - new_rect.min.x).abs() > 0.5
                || (old.min.y - new_rect.min.y).abs() > 0.5
                || (old.width() - new_rect.width()).abs() > 0.5
                || (old.height() - new_rect.height()).abs() > 0.5;
            if moved {
                let map = |p: Pos2| -> Pos2 {
                    let u = (p.x - old.min.x) / old.width().max(1.0);
                    let v = (p.y - old.min.y) / old.height().max(1.0);
                    Pos2::new(
                        new_rect.min.x + u * new_rect.width(),
                        new_rect.min.y + v * new_rect.height(),
                    )
                };
                for a in self
                    .annotation_actions
                    .iter_mut()
                    .chain(self.current_action.iter_mut())
                {
                    for p in &mut a.points {
                        *p = map(*p);
                    }
                }
                if let Some(p) = self.text_edit_at {
                    self.text_edit_at = Some(map(p));
                }
            }
        }
        self.annotation_canvas_rect = Some(new_rect);
    }

    /// E119 — paste the system clipboard image onto the canvas as a movable
    /// sticker. Anchored center; drag to move while it's the selected stroke.
    pub fn paste_sticker_from_clipboard(&mut self, ctx: &egui::Context) {
        let Ok(mut board) = arboard::Clipboard::new() else {
            self.show_toast("❌ Clipboard unavailable");
            return;
        };
        let img = match board.get_image() {
            Ok(i) => i,
            Err(_) => {
                self.show_toast("Clipboard has no image");
                return;
            }
        };
        let Some(rgba) =
            image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())
        else {
            self.show_toast("❌ Could not decode clipboard image");
            return;
        };
        let rect = self
            .annotation_canvas_rect
            .unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0)));
        // Display at native image-pixel size: canvas px per source px.
        let scale = rect.width() / (self.img_src_wh.0.max(1) as f32);
        let disp = Vec2::new(rgba.width() as f32 * scale, rgba.height() as f32 * scale);
        let anchor = rect.center() - disp / 2.0;
        let size = [rgba.width() as usize, rgba.height() as usize];
        let ci = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        let tex = ctx.load_texture("sticker", ci, egui::TextureOptions::LINEAR);
        self.annotation_push_undo();
        self.annotation_actions.push(AnnotationAction {
            tool: AnnotationTool::Sticker,
            badge_style: 0,
            color: self.current_color,
            stroke_width: 1.0,
            points: vec![anchor],
            text_content: String::new(),
            badge_number: 0,
            sticker: Some(app::annotation_baker::Sticker {
                rgba: std::sync::Arc::new(rgba),
                tex,
            }),
        });
        self.annotation_selected = Some(self.annotation_actions.len() - 1);
        self.show_toast("📋 Image pasted — drag it while selected, Del removes");
    }

    /// Selected stroke is a movable sticker?
    pub(crate) fn selected_sticker(&self) -> Option<usize> {
        self.annotation_selected.filter(|&i| {
            self.annotation_actions
                .get(i)
                .map(|a| a.tool == AnnotationTool::Sticker)
                .unwrap_or(false)
        })
    }

    /// E121 — drop the watermark text as a Text annotation in the canvas'
    /// bottom-right corner, in the current brush color.
    pub fn add_watermark(&mut self) {
        let text = self.watermark_text.trim().to_string();
        if text.is_empty() {
            self.show_toast("Type watermark text first");
            return;
        }
        let rect = self
            .annotation_canvas_rect
            .unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0)));
        // Anchor bottom-right; estimate the pill width so it hugs the edge.
        let est_w = text.len() as f32 * 9.0;
        let margin = 18.0_f32.min(rect.width() * 0.05);
        let pos = Pos2::new(
            (rect.max.x - margin - est_w).max(rect.min.x + margin),
            (rect.max.y - margin - 20.0).max(rect.min.y + margin),
        );
        self.annotation_push_undo();
        self.annotation_actions.push(AnnotationAction {
            tool: AnnotationTool::Text,
            color: self.current_color,
            stroke_width: 1.0,
            points: vec![pos],
            text_content: text,
            badge_number: 0,
            badge_style: 0,
            sticker: None,
        });
        self.annotation_selected = Some(self.annotation_actions.len() - 1);
        self.show_toast("Watermark added");
    }

    /// Copy the still to the clipboard without baked annotations (E115).
    pub fn copy_still_original_to_clipboard(&mut self) {
        let Some(path) = self.img_edit_file.clone() else {
            self.show_toast("No image loaded to copy");
            return;
        };
        match image::open(&path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                if let Ok(mut board) = arboard::Clipboard::new() {
                    if board
                        .set_image(arboard::ImageData {
                            width: w,
                            height: h,
                            bytes: std::borrow::Cow::Borrowed(rgba.as_raw()),
                        })
                        .is_ok()
                    {
                        self.clipboard_seq_seen = crate::platform::clipboard_seq();
                        self.show_toast("📋 Original copied (no markup)");
                    } else {
                        self.show_toast("❌ Could not copy the image");
                    }
                }
            }
            Err(e) => self.show_toast(format!("❌ Could not read image: {e}")),
        }
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

    /// Guaranteed exit — ViewportCommand::Close can no-op when the window is
    /// minimized/hidden, so tray Quit must not depend on it.
    fn quit_app(&mut self) -> ! {
        self.allow_exit = true;
        if let Some(child) = self.child_process.take() {
            kill_recorder(child, self.is_paused);
        }
        if let Some(mut c) = self.voice_memo_child.take() {
            let _ = c.kill();
        }
        self.persist_session();
        self.flush_session_now();
        app::instance::release_gui_lock();
        std::process::exit(0);
    }

    fn show_window(&mut self, ctx: &egui::Context) {
        app::capture_flow::restore_parked(
            ctx,
            &mut self.pre_capture_outer,
            &mut self.pre_capture_size,
        );
        self.wake_shared.parked.store(false, Ordering::SeqCst);
    }

    /// Ctrl+Alt+V: focused window → hide to tray; minimized/unfocused →
    /// restore + foreground. While parked mid-capture, ignore the key —
    /// restoring early would put the studio in its own screenshot.
    fn toggle_window(&mut self, ctx: &egui::Context) {
        // Mid-capture park or mid-region-select: hiding the owner would take
        // the overlay viewport down with it.
        if self.pre_capture_outer.is_some() || self.is_selecting_region {
            return;
        }
        let (minimized, focused) = ctx.input(|i| {
            (
                i.viewport().minimized.unwrap_or(false),
                i.viewport().focused.unwrap_or(false),
            )
        });
        if minimized || !focused {
            self.show_window(ctx);
            #[cfg(windows)]
            crate::platform::restore_studio_to_taskbar();
        } else {
            self.hide_to_tray(ctx);
        }
    }

    fn hide_to_tray(&self, ctx: &egui::Context) {
        // Windows: minimize so the taskbar button stays (Visible(false) drops it
        // and can stall the event loop — Inbox/feedback then looks dead).
        #[cfg(windows)]
        {
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            crate::platform::minimize_studio();
            ctx.request_repaint();
            return;
        }
        #[cfg(not(windows))]
        {
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            ctx.request_repaint();
        }
    }

    /// Hide the studio window so it is not in the shot.
    ///
    /// Windows: SW_HIDE removes the window from the compositor synchronously —
    /// no minimize animation, so the grab can start after ~100 ms (Snagit-class
    /// hide). Recording-arm is the exception: it keeps `snapshot_park_geometry`
    /// + worker `minimize_studio` because the taskbar button must persist for
    /// the whole recording.
    /// Other platforms park off-screen (no native instant hide).
    fn hide_for_capture(&mut self, ctx: &egui::Context) {
        self.wake_shared.parked.store(true, Ordering::SeqCst);
        #[cfg(windows)]
        app::capture_flow::park_hidden(
            ctx,
            &mut self.pre_capture_outer,
            &mut self.pre_capture_size,
        );
        #[cfg(not(windows))]
        app::capture_flow::park_offscreen(
            ctx,
            &mut self.pre_capture_outer,
            &mut self.pre_capture_size,
        );
    }

    fn handle_tray_actions(&mut self, ctx: &egui::Context) {
        let actions: Vec<TrayAction> = self
            .tray
            .as_ref()
            .map(|t| t.poll_actions())
            .unwrap_or_default();
        for action in actions {
            self.on_tray_action(ctx, action);
        }
    }

    fn on_tray_action(&mut self, ctx: &egui::Context, action: TrayAction) {
        match action {
            TrayAction::Show => self.show_window(ctx),
            TrayAction::Hide => self.hide_to_tray(ctx),
            // E206 — configurable double-click (default: open studio).
            TrayAction::DoubleClick => match self.tray_dblclick.as_str() {
                "screenshot" => self.trigger_capture(ctx, true),
                "record" => self.on_tray_action(ctx, TrayAction::ToggleRecord),
                _ => self.show_window(ctx),
            },
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
            TrayAction::TogglePause => self.toggle_pause(),
            TrayAction::RepeatLast => self.repeat_last_capture(ctx),
            TrayAction::GoShutter => {
                self.current_tab = AppTab::Capture;
                self.show_window(ctx);
            }
            TrayAction::GoMedia => {
                self.current_tab = AppTab::Library;
                self.refresh_library();
                self.show_window(ctx);
            }
            TrayAction::GoReview => {
                self.current_tab = self.review_tab();
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
            // E180 — per-request quick reply: slot i answers the i-th
            // pending question (first option = approve, last = deny).
            TrayAction::ApproveIdx(i) => self.reply_pending_slot(i as usize, true),
            TrayAction::DenyIdx(i) => self.reply_pending_slot(i as usize, false),
            TrayAction::OpenRecent(i) => {
                // Tray recents mirror library order (newest first).
                if let Some(item) = self.library_items.get(i as usize) {
                    let _ = platform::open_path(&item.path);
                } else {
                    self.refresh_library();
                    self.show_toast("Library refreshed — try again");
                }
            }
            TrayAction::BugReport => {
                self.show_window(ctx);
                self.bug_report_pack(ctx);
            }
            TrayAction::Quit => self.quit_app(),
        }
    }

    fn sync_tray_recording_progress(&mut self) {
        let state = if self.is_recording {
            TrayLiveState::Recording {
                elapsed_secs: self.recording_elapsed_secs(),
                paused: self.is_paused,
            }
        } else if self.recording_finalizing {
            TrayLiveState::Finalizing
        } else if self.recording_arming || self.countdown_deadline.is_some() {
            TrayLiveState::Arming
        } else {
            TrayLiveState::Idle
        };
        let inbox = self.feedback_pending_count;
        if let Some(tray) = self.tray.as_mut() {
            tray.set_live_state(state, inbox, self.last_error.as_deref());
        }
    }

    /// E214 — release check on a worker; the UI never blocks on curl.
    /// E215 — download the platform asset and stage `<exe>.new` on a
    /// worker. The swap itself happens in `apply_staged_and_restart`.
    fn start_update_download(&mut self, url: String) {
        if self.update_dl_rx.is_some() {
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx_clone = self.ui_ctx.clone();
        std::thread::spawn(move || {
            let res = crate::app::update::download_and_stage(&url);
            let _ = tx.send(res);
            if let Some(c) = ctx_clone {
                c.request_repaint();
            }
        });
        self.update_dl_rx = Some(rx);
        self.update_status = "downloading update…".into();
    }

    /// E215 — drain the download worker; on success the staged path lands
    /// in `update_staged` and the Settings card offers Restart-to-apply.
    fn drain_update_download(&mut self) {
        let Some(rx) = self.update_dl_rx.as_ref() else {
            return;
        };
        let Ok(res) = rx.try_recv() else {
            return;
        };
        self.update_dl_rx = None;
        match res {
            Ok(p) => {
                self.update_staged = Some(p);
                self.update_status = "update staged — restart to apply".into();
            }
            Err(e) => {
                self.update_status = format!("update download failed: {e}");
            }
        }
    }

    fn start_update_check(&mut self) {
        if self.update_rx.is_some() {
            return;
        }
        let (tx, rx) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            let _ = tx.send(crate::app::update::check_latest_release());
        });
        self.update_rx = Some(rx);
        self.update_status = "checking…".into();
    }

    /// E273 — the saved display pick can outlive the display (undock).
    /// Re-target to the default and say so once, rather than capturing a
    /// dead index forever.
    fn resolved_monitor(&mut self) -> Option<u32> {
        let want = self.capture_monitor?;
        match crate::platform::resolve_monitor(Some(want)) {
            some @ Some(_) => some,
            None => {
                self.capture_monitor = None;
                self.persist_session();
                self.show_toast("Saved display unplugged — capture retargeted to primary");
                None
            }
        }
    }

    /// E222 — launch recovery: unsaved Review annotations + a frag-MP4
    /// orphaned by a killed recorder. Both restore quietly; failures toast.
    fn recover_on_launch(&mut self, ctx: &egui::Context) {
        if let Some(draft) = app::read_review_draft() {
            let still = PathBuf::from(&draft.still_path);
            if still.exists() {
                let n = draft.actions.len();
                let actions = app::actions_from_draft(ctx, &draft);
                self.load_still_from_path(&still);
                self.annotation_canvas_rect = draft.canvas_rect.map(|[x0, y0, x1, y1]| {
                    Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
                });
                self.annotation_actions = actions;
                self.step_counter = app::renumber_step_badges(&mut self.annotation_actions);
                self.review_draft_fp = app::actions_fingerprint(&self.annotation_actions);
                self.show_toast(format!(
                    "Recovered {n} unsaved annotation{} — open Review to continue",
                    if n == 1 { "" } else { "s" }
                ));
            } else {
                // Source still is gone — the draft can't be meaningfully
                // restored; drop it so it doesn't re-nag every launch.
                app::clear_review_draft();
            }
        }
        if let Some(mp4) = app::agent_record::orphaned_frag_mp4() {
            let (tx, rx) = crossbeam_channel::unbounded();
            std::thread::spawn(move || {
                let clean = mp4.with_extension("clean.mp4");
                let res = crate::platform::remux_to_clean_mp4(&mp4, &clean).map(|()| {
                    let kept = if clean.exists() { clean } else { mp4 };
                    kept.display().to_string()
                });
                app::agent_record::discard_orphaned_state();
                let _ = tx.send(res);
            });
            self.recovery_rx = Some(rx);
        }
    }

    /// E222 — debounced draft write: fingerprint the action list each frame;
    /// 800 ms after the last change, persist (empty list clears the draft).
    fn tick_review_draft(&mut self) {
        let fp = app::actions_fingerprint(&self.annotation_actions);
        if fp != self.review_draft_fp {
            self.review_draft_fp = fp;
            self.review_draft_changed_at = Some(std::time::Instant::now());
        }
        let Some(at) = self.review_draft_changed_at else {
            return;
        };
        if at.elapsed() < std::time::Duration::from_millis(800) {
            return;
        }
        self.review_draft_changed_at = None;
        if self.annotation_actions.is_empty() {
            app::clear_review_draft();
        } else if let Some(still) = self.img_edit_file.clone() {
            app::write_review_draft(
                &still,
                self.annotation_canvas_rect,
                &self.annotation_actions,
            );
        }
    }

    fn drain_recovery(&mut self) {
        let done = if let Some(rx) = &self.recovery_rx {
            match rx.try_recv() {
                Ok(res) => {
                    match res {
                        Ok(path) => {
                            self.show_toast(format!("Recovered recording → {path}"));
                            self.refresh_library();
                        }
                        Err(e) => self.show_toast(format!("⚠ Recording recovery: {e}")),
                    }
                    true
                }
                Err(crossbeam_channel::TryRecvError::Empty) => false,
                Err(crossbeam_channel::TryRecvError::Disconnected) => true,
            }
        } else {
            false
        };
        if done {
            self.recovery_rx = None;
        }
    }

    /// E213 — save the durable profile (settings/hotkeys/library sets) to a
    /// `.vcap-profile` zip chosen via the file dialog.
    fn export_profile_dialog(&mut self) {
        let Some(dest) = rfd::FileDialog::new()
            .set_file_name("vibecap-profile.vcap-profile")
            .add_filter("Vibecap profile", &["vcap-profile", "zip"])
            .save_file()
        else {
            return;
        };
        match app::export_profile(&dest, &self.session_snapshot()) {
            Ok(()) => self.show_toast(format!("Profile exported → {}", dest.display())),
            Err(e) => self.show_toast(format!("⚠ Profile export: {e}")),
        }
    }

    /// E213 — load a `.vcap-profile`: serde defaults fill fields the file
    /// predates, then rebind hotkeys + re-theme so it applies live.
    fn import_profile_dialog(&mut self, ctx: &egui::Context) {
        let Some(src) = rfd::FileDialog::new()
            .add_filter("Vibecap profile", &["vcap-profile", "zip"])
            .pick_file()
        else {
            return;
        };
        match app::import_profile(&src) {
            Ok(mut s) => {
                // A profile carries prefs, not session UI state — keep the
                // importer's tab, open editors, window size, wizard status
                // and permission probes instead of taking the exporter's.
                s.tab = String::new();
                s.edit_file = None;
                s.window_w = self.window_size.x;
                s.window_h = self.window_size.y;
                s.wizard_done = self.wizard_done;
                s.screen_permission_prompted = self.screen_permission_prompted;
                s.screen_permission_ok = self.screen_permission_ok;
                s.library_filter = self.library_filter.clone();
                self.apply_session(s);
                self.current_tab = AppTab::Settings;
                self.persist_session();
                let _ = self.rebind_global_hotkeys();
                crate::ui::theme::apply_current_theme(ctx);
                self.show_toast("Profile imported — settings applied");
            }
            Err(e) => self.show_toast(format!("⚠ Profile import: {e}")),
        }
    }

    fn show_toast(&mut self, message: impl Into<String>) {
        let message = message.into();
        let level = ToastLevel::from_message(&message);
        // K253: errors persist — Settings + tray tooltip show the last one.
        if matches!(level, ToastLevel::Error) {
            self.last_error = Some(message.clone());
        }
        // Silent mode suppresses feedback noise — errors still surface.
        if self.silent_mode && !matches!(level, ToastLevel::Error) {
            return;
        }
        self.toast_message = Some((message, Instant::now(), level));
    }

    fn toggle_voice_memo(&mut self) {
        if self.is_recording_voice_memo {
            if let Some(child) = self.voice_memo_child.take() {
                self.is_recording_voice_memo = false;
                let (tx, rx) = crossbeam_channel::bounded(1);
                self.voice_finalize_rx = Some(rx);
                let ctx_clone = self.ui_ctx.clone();
                std::thread::spawn(move || {
                    let res = finalize_recorder(child)
                        .map(|_| ())
                        .map_err(|e| e.to_string());
                    let _ = tx.send(res);
                    if let Some(c) = ctx_clone {
                        c.request_repaint();
                    }
                });
                self.show_toast("🎙 Saving voice note…");
            }
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

    /// Drain the voice-memo finalize worker — toast + rescan once ffmpeg exits.
    fn drain_voice_finalize(&mut self) {
        let Some(rx) = self.voice_finalize_rx.as_ref() else {
            return;
        };
        let Ok(res) = rx.try_recv() else {
            return;
        };
        self.voice_finalize_rx = None;
        match res {
            Ok(()) => self.show_toast("🎙 Voice Note saved!"),
            Err(e) => self.show_toast(format!("⚠️ Voice note may be truncated: {e}")),
        }
        self.refresh_library();
    }

    /// Kick a media-dir scan on a worker — never on the UI thread. Startup and
    /// post-capture both go through here; results land in drain_library_scan.
    /// E159 — queue a scrub-strip extraction for a hovered clip tile.
    /// No-op while one is already in flight for the same path or cached.
    pub(crate) fn request_scrub(&mut self, path: PathBuf, ctx: &egui::Context) {
        if self.scrub_cache.contains_key(&path) || !self.scrub_pending.insert(path.clone()) {
            return;
        }
        if self.scrub_tx.is_none() {
            let (tx, rx) = crossbeam_channel::unbounded();
            self.scrub_tx = Some(tx);
            self.scrub_rx = Some(rx);
        }
        let Some(tx) = self.scrub_tx.clone() else {
            return;
        };
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            let r = crate::app::recording::extract_scrub_frames(&path, 8, 192);
            let _ = tx.send((path, r));
            ctx_clone.request_repaint();
        });
    }

    /// E159 — land decoded scrub strips as textures; drop failures quietly
    /// (the static thumb stays). Cache is capped — clears fully past 32.
    fn drain_scrub(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.scrub_rx.as_ref() else {
            return;
        };
        while let Ok((path, res)) = rx.try_recv() {
            self.scrub_pending.remove(&path);
            if let Ok(frames) = res {
                let textures: Vec<egui::TextureHandle> = frames
                    .into_iter()
                    .map(|(w, h, rgba)| {
                        ctx.load_texture(
                            "scrub",
                            egui::ColorImage::from_rgba_unmultiplied(
                                [w as usize, h as usize],
                                &rgba,
                            ),
                            egui::TextureOptions::LINEAR,
                        )
                    })
                    .collect();
                if !textures.is_empty() {
                    self.scrub_cache.insert(path, textures);
                }
            }
        }
        if self.scrub_cache.len() > 32 {
            self.scrub_cache.clear();
        }
    }

    fn refresh_library(&mut self) {
        self.library_selected.retain(|p| p.exists());
        if self.library_scan_rx.is_some() {
            self.library_scan_pending = true;
            return;
        }
        let dir = self.save_dir.clone();
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.library_scan_rx = Some(rx);
        let ctx_clone = self.ui_ctx.clone();
        std::thread::spawn(move || {
            let items = scan_media_dir(&dir);
            let _ = tx.send(items);
            if let Some(c) = ctx_clone {
                c.request_repaint();
            }
        });
    }

    /// Apply a finished media-dir scan, warm thumbs off-thread, rescan if dirty.
    fn drain_library_scan(&mut self) {
        let Some(rx) = self.library_scan_rx.as_ref() else {
            return;
        };
        let Ok(items) = rx.try_recv() else {
            return;
        };
        self.library_scan_rx = None;
        app::thumbs::cleanup_frames_temp(&self.save_dir);
        // E167 — reclaimable scratch/cache bytes for the storage bar.
        self.library_reclaimable = crate::app::library::reclaimable_bytes(&self.save_dir);
        let warmup: Vec<PathBuf> = items.iter().take(40).map(|i| i.path.clone()).collect();
        app::thumbs::warmup_thumbs(warmup);
        // Orphaned thumbs (media deleted via Explorer) + LRU byte cap.
        app::thumbs::sweep_thumbs(
            self.save_dir.clone(),
            items.iter().map(|i| i.name.clone()).collect(),
        );
        // Keep the tray's "Recent captures" slots in sync with the scan
        // (newest-first order — the menu mirrors it verbatim).
        if let Some(tray) = self.tray.as_mut() {
            let names: Vec<String> = items.iter().take(5).map(|i| i.name.clone()).collect();
            tray.set_recents(&names);
        }
        self.library_items = items;
        // E79 — auto retention sweep once per launch (opt-in; files go to a
        // persistent retention_trash dir, never hard-deleted).
        if self.retention_auto && self.retention_mode != 0 && !self.retention_swept {
            self.retention_swept = true;
            self.apply_retention(true);
        }
        if self.library_scan_pending {
            self.library_scan_pending = false;
            self.refresh_library();
        }
    }

    /// Apply decoded recent-capture thumbs (worker → textures here).
    fn drain_recent_thumbs(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.recent_thumbs_rx.as_ref() else {
            return;
        };
        let Ok(items) = rx.try_recv() else {
            return;
        };
        self.recent_thumbs_rx = None;
        self.recent_thumbs = items
            .into_iter()
            .map(|(p, is_video, img)| {
                let tex = ctx.load_texture(
                    format!("recent:{}", p.display()),
                    img,
                    egui::TextureOptions::LINEAR,
                );
                (p, is_video, tex)
            })
            .collect();
    }

    /// E154 — toggle a library item's ★ (keyed by file name, session-persisted).
    pub(crate) fn toggle_library_favorite(&mut self, name: &str) {
        if !self.library_favorites.remove(name) {
            self.library_favorites.insert(name.to_string());
        }
        self.persist_session();
    }

    /// E83 — toggle a library item's ⚑ review-queue flag (same persistence
    /// keying as favorites — file name survives a moved media dir).
    pub(crate) fn toggle_library_flag(&mut self, name: &str) {
        if !self.library_flagged.remove(name) {
            self.library_flagged.insert(name.to_string());
        }
        self.persist_session();
    }

    /// E77 — write the selected library items to a store-only ZIP.
    pub(crate) fn export_selection_zip(&mut self) {
        if self.library_selected.is_empty() {
            self.show_toast("Nothing selected");
            return;
        }
        let Some(dest) = rfd::FileDialog::new()
            .set_file_name("vibecap_export.zip")
            .add_filter("ZIP archive", &["zip"])
            .save_file()
        else {
            return;
        };
        let dest = if dest.extension().is_some() {
            dest
        } else {
            dest.with_extension("zip")
        };
        let paths: Vec<PathBuf> = self.library_selected.iter().cloned().collect();
        match app::write_zip(&dest, &paths) {
            Ok(names) => {
                self.show_toast(format!(
                    "📦 Exported {} file(s) → {}",
                    names.len(),
                    dest.display()
                ));
            }
            Err(e) => self.show_toast(format!("❌ ZIP failed: {e}")),
        }
    }

    /// E76 — set tags for item name(s) from a comma-separated buffer
    /// (lowercased, trimmed, deduped; empty clears the entry).
    pub(crate) fn set_library_tags(&mut self, names: &[String], raw: &str) {
        let mut tags: Vec<String> = Vec::new();
        for t in raw.split(',') {
            let t = t.trim().to_ascii_lowercase();
            if !t.is_empty() && !tags.contains(&t) {
                tags.push(t);
            }
        }
        for name in names {
            if tags.is_empty() {
                self.library_tags.remove(name);
            } else {
                self.library_tags.insert(name.clone(), tags.clone());
            }
        }
        self.persist_session();
    }

    /// E76 — every tag present in the library, most-used first (chip row).
    pub(crate) fn library_tag_index(&self) -> Vec<(String, usize)> {
        let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
        for item in &self.library_items {
            if let Some(tags) = self.library_tags.get(&item.name) {
                for t in tags {
                    *counts.entry(t.clone()).or_default() += 1;
                }
            }
        }
        let mut v: Vec<_> = counts.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        v
    }

    /// E79 — files the current retention rule would sweep.
    fn retention_candidates(&self) -> Vec<PathBuf> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        app::retention_pick(
            &self.library_items,
            self.retention_mode,
            self.retention_value,
            now,
        )
        .into_iter()
        .map(|i| self.library_items[i].path.clone())
        .collect()
    }

    /// E79 — move retention candidates into a *persistent* trash folder
    /// (recoverable — deliberately not the 12s undo window).
    pub(crate) fn apply_retention(&mut self, auto: bool) {
        let cands = self.retention_candidates();
        if cands.is_empty() {
            if !auto {
                self.show_toast("Retention: nothing to sweep");
            }
            return;
        }
        let trash_dir = vibecap_config_dir()
            .join("retention_trash")
            .join(Local::now().format("%Y%m%d_%H%M%S%3f").to_string());
        let _ = std::fs::create_dir_all(&trash_dir);
        let mut n = 0usize;
        for p in &cands {
            let Some(name) = p.file_name() else { continue };
            let dest = trash_dir.join(name);
            if std::fs::rename(p, &dest).is_ok()
                || (std::fs::copy(p, &dest).is_ok() && std::fs::remove_file(p).is_ok())
            {
                n += 1;
                self.library_selected.remove(p);
            }
        }
        if n > 0 {
            self.refresh_library();
            self.show_toast(format!(
                "Retention swept {n} file(s) → {}",
                trash_dir.display()
            ));
        }
    }

    fn library_filtered(&self) -> Vec<&MediaItem> {
        let q = self.library_search.trim().to_ascii_lowercase();
        // E154 — the ★ chip is a pseudo-category over favorited file names.
        // E83 — ⚑ Review is the same pattern over flagged names.
        let base: Vec<&MediaItem> = if self.library_filter == "★ Favorites" {
            self.library_items
                .iter()
                .filter(|i| self.library_favorites.contains(&i.name))
                .collect()
        } else if self.library_filter == "⚑ Review" {
            self.library_items
                .iter()
                .filter(|i| self.library_flagged.contains(&i.name))
                .collect()
        } else {
            filter_items(&self.library_items, &self.library_filter)
        };
        // E76 — a selected tag chip ANDs onto the category filter.
        let base: Vec<&MediaItem> = match &self.library_tag_filter {
            Some(tag) => base
                .into_iter()
                .filter(|i| {
                    self.library_tags
                        .get(&i.name)
                        .map(|ts| ts.iter().any(|t| t == tag))
                        .unwrap_or(false)
                })
                .collect(),
            None => base,
        };
        let mut out: Vec<&MediaItem> = base
            .into_iter()
            .filter(|i| {
                if q.is_empty() || i.name.to_ascii_lowercase().contains(&q) {
                    return true;
                }
                // Sidecar search (G152): notes + legacy txt transcripts.
                for ext in ["notes.txt", "txt"] {
                    let side = i.path.with_extension(ext);
                    if side.exists()
                        && std::fs::read_to_string(&side)
                            .map(|t| t.to_ascii_lowercase().contains(&q))
                            .unwrap_or(false)
                    {
                        return true;
                    }
                }
                false
            })
            .collect();
        self.library_sort.apply(&mut out);
        out
    }

    /// Chrome-only snapshot for the bottom status strip (no new backends).
    /// Per-frame accessor. Expensive fields (media dir walk, live-dir walk,
    /// budget file read) are cached ~2s; live fields (REC clock, inbox) are
    /// overlaid every call so the strip stays true.
    fn status_snapshot(&mut self) -> StatusSnapshot {
        const STATUS_TTL: Duration = Duration::from_secs(2);
        let stale = self
            .status_cache
            .as_ref()
            .map(|(_, at)| at.elapsed() >= STATUS_TTL)
            .unwrap_or(true);
        if stale {
            let snap = self.compute_status_snapshot();
            self.status_cache = Some((snap, Instant::now()));
        }
        let mut snap = self.status_cache.as_ref().unwrap().0.clone();
        snap.pending_inbox = self.feedback_pending_count;
        snap.rec_live = self.is_recording || self.recording_arming || self.recording_finalizing;
        snap.rec_label = if self.is_recording {
            let e = self.recording_elapsed_secs();
            format!("REC {:02}:{:02}", e / 60, e % 60)
        } else if self.recording_arming {
            "Starting…".into()
        } else if self.recording_finalizing {
            "Saving…".into()
        } else {
            String::new()
        };
        snap
    }

    /// Capture-tab live-stats row — the live-dir walk and budget file reads
    /// are cached ~2s; the row previously re-scanned every frame.
    pub(crate) fn live_stats_snapshot(&mut self) -> LiveStats {
        const TTL: Duration = Duration::from_secs(2);
        if let Some((s, at)) = &self.live_stats_cache {
            if at.elapsed() < TTL {
                return s.clone();
            }
        }
        let live_dir = default_live_dir().display().to_string();
        let (bytes, count) = get_dir_size_bytes(&live_dir);
        let cfg = load_budget();
        let over = budget_exceeded_reason(&live_dir);
        let s = LiveStats {
            count,
            mb: bytes as f64 / (1024.0 * 1024.0),
            frames_cap: cfg.max_frames,
            mb_cap: cfg.max_mb,
            minutes_cap: cfg.max_minutes,
            tier: cfg.analysis_tier.clone(),
            over,
        };
        self.live_stats_cache = Some((s.clone(), Instant::now()));
        s
    }

    fn compute_status_snapshot(&self) -> StatusSnapshot {
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

        StatusSnapshot {
            storage_label,
            budget_tier,
            budget_usage,
            ffmpeg_ok,
            // Live fields are overlaid per call by status_snapshot().
            pending_inbox: self.feedback_pending_count,
            rec_live: false,
            rec_label: String::new(),
        }
    }

    fn delete_library_paths(&mut self, paths: &[PathBuf]) {
        if paths.is_empty() {
            return;
        }
        // E150 — a clip with unsaved cuts/trims asks once before trashing.
        if self.confirm_delete.is_none() {
            let guarded: Vec<PathBuf> = paths
                .iter()
                .filter(|p| {
                    self.edit_file.as_ref() == Some(*p)
                        && (!self.filmstrip_cut.is_empty() || self.trim_start != "00:00:00")
                        && !self.delete_guard_ok.contains(*p)
                })
                .cloned()
                .collect();
            if !guarded.is_empty() {
                self.confirm_delete = Some(guarded);
                return;
            }
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
            if std::fs::rename(p, &dest).is_ok()
                || (std::fs::copy(p, &dest).is_ok() && std::fs::remove_file(p).is_ok())
            {
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
        self.feedback_last_poll = Some(Instant::now());
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
        // E180 — mirror the first-3 pending questions into tray quick slots.
        // `tray_quick_ids` pins each slot to a request id so a rescan between
        // label render and menu click can't retarget the answer.
        let pairs: Vec<(String, String)> = self
            .feedback_requests
            .iter()
            .filter(|r| r.status == "pending")
            .take(3)
            .map(|r| (r.id.clone(), r.question.clone()))
            .collect();
        let key = pairs
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>()
            .join("\u{1}");
        if key != self.tray_quick_key {
            self.tray_quick_key = key;
            self.tray_quick_ids = pairs.iter().map(|(id, _)| id.clone()).collect();
            let questions: Vec<String> = pairs.into_iter().map(|(_, q)| q).collect();
            if let Some(tray) = self.tray.as_mut() {
                tray.set_quick_pending(&questions);
            }
        }
    }

    /// Detect newly pending agent questions and make them unmissable:
    /// OS notification · Dock bounce · tray title · toast · open Inbox.
    fn surface_new_feedback(&mut self, ctx: &egui::Context) {
        // Snoozed threads aren't "pending" for surfacing — dropping them from
        // notified_ids means the snooze expiry re-fires the notification,
        // which is the whole point of snoozing.
        let now = std::time::Instant::now();
        let pending: Vec<FeedbackRequest> = self
            .feedback_requests
            .iter()
            .filter(|r| r.status == "pending")
            .filter(|r| {
                self.feedback_snooze_until
                    .get(&r.id)
                    .map(|t| *t <= now)
                    .unwrap_or(true)
            })
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

        // Quiet mode: mark the ids notified (so un-quieting doesn't re-fire
        // everything) and update badge/tray — but skip the loud channel.
        if self.inbox_quiet {
            for r in &new_ones {
                self.feedback_notified_ids.insert(r.id.clone());
            }
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
        let q: String = first.question.chars().take(90).collect();
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

        let composing = !self.feedback_draft.trim().is_empty() || !self.feedback_choice.is_empty();
        if !composing {
            self.current_tab = AppTab::Feedback;
            self.feedback_selected = Some(first.id.clone());
            self.feedback_user_picked = false;
            self.feedback_new_arrived = true;
            self.show_window(ctx);
        } else {
            self.feedback_new_arrived = true;
        }

        self.feedback_pending_count = pending_ids.len();
        // Force tray title refresh immediately (don't wait for next tick).
        let live = if self.is_recording {
            TrayLiveState::Recording {
                elapsed_secs: self.recording_elapsed_secs(),
                paused: self.is_paused,
            }
        } else if self.recording_arming || self.countdown_deadline.is_some() {
            TrayLiveState::Arming
        } else {
            TrayLiveState::Idle
        };
        let inbox_n = self.feedback_pending_count;
        if let Some(tray) = self.tray.as_mut() {
            // Reset debounce so Idle+Inbox title always applies.
            tray.force_live_state(live, inbox_n, self.last_error.as_deref());
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

    /// E180 — answer the i-th pending question from the tray without
    /// opening the window. Approve picks the first choice chip (or
    /// "approved" for text-only); deny picks the last chip (or "denied").
    /// Resolves by the request id pinned when the label was pushed, so a
    /// rescan between render and click can't retarget the answer.
    fn reply_pending_slot(&mut self, idx: usize, approve: bool) {
        self.scan_feedback_requests();
        let Some(id) = self.tray_quick_ids.get(idx).cloned() else {
            self.show_toast("No pending question in that slot");
            return;
        };
        let options = self
            .feedback_requests
            .iter()
            .find(|r| r.id == id && r.status == "pending")
            .map(|r| r.options.clone());
        let Some(options) = options else {
            self.show_toast("That question was already answered");
            return;
        };
        let answer = if approve {
            options
                .first()
                .cloned()
                .unwrap_or_else(|| "approved".into())
        } else {
            options.last().cloned().unwrap_or_else(|| "denied".into())
        };
        let response = FeedbackResponse {
            id: id.clone(),
            feedback_text: answer.clone(),
            voice_note_path: String::new(),
            annotated_media_path: String::new(),
            answered_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            selected_option: answer.clone(),
        };
        let resp_path = feedback_responses_dir().join(format!("{id}.json"));
        if serde_json::to_string_pretty(&response)
            .ok()
            .and_then(|s| write_json_atomic(&resp_path, &s).ok())
            .is_some()
        {
            self.mark_feedback_status(&id, "answered");
            self.scan_feedback_requests();
            self.show_toast(format!(
                "{} from tray — {}",
                if approve {
                    "✅ Approved"
                } else {
                    "✗ Denied"
                },
                answer
            ));
        } else {
            self.show_toast("❌ Could not save feedback — check disk permissions.");
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
        // Allow the inbox to advance to the next pending thread.
        self.feedback_user_picked = false;
        self.scan_feedback_requests();
        self.show_toast("✅ Feedback submitted — the agent can pick it up now!");
    }

    /// E190 — approve the given pending threads using each thread's first
    /// choice-chip option. Callers pass the currently visible set so an
    /// active search filter scopes the bulk action. Text-only threads are
    /// skipped — bulk approval never fabricates a free-form answer.
    pub(crate) fn approve_all_pending(&mut self, ids: &[String]) {
        let picks: Vec<(String, String)> = ids
            .iter()
            .filter_map(|id| {
                self.feedback_requests
                    .iter()
                    .find(|r| &r.id == id && r.status == "pending" && !r.options.is_empty())
                    .map(|r| (r.id.clone(), r.options[0].clone()))
            })
            .collect();
        if picks.is_empty() {
            self.show_toast("No pending choice threads to approve");
            return;
        }
        let n = picks.len();
        let stamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        for (id, opt) in picks {
            let response = FeedbackResponse {
                id: id.clone(),
                feedback_text: opt.clone(),
                voice_note_path: String::new(),
                annotated_media_path: String::new(),
                answered_at: stamp.clone(),
                selected_option: opt,
            };
            let resp_path = feedback_responses_dir().join(format!("{id}.json"));
            if serde_json::to_string_pretty(&response)
                .ok()
                .and_then(|s| write_json_atomic(&resp_path, &s).ok())
                .is_some()
            {
                self.mark_feedback_status(&id, "answered");
            }
        }
        self.feedback_selected = None;
        self.scan_feedback_requests();
        self.show_toast(format!("✅ Approved {n} thread(s)"));
    }

    /// E182 — search matches the question, id, media filename, and (for
    /// closed threads) the recorded answer text.
    pub(crate) fn inbox_matches(&mut self, r: &FeedbackRequest, q: &str) -> bool {
        if q.is_empty() {
            return true;
        }
        if r.question.to_ascii_lowercase().contains(q)
            || r.id.to_ascii_lowercase().contains(q)
            || r.media_path.to_ascii_lowercase().contains(q)
        {
            return true;
        }
        if r.status != "pending" {
            if !self.feedback_reply_cache.contains_key(&r.id) {
                if let Ok(s) =
                    std::fs::read_to_string(feedback_responses_dir().join(format!("{}.json", r.id)))
                {
                    if let Ok(resp) = serde_json::from_str::<FeedbackResponse>(&s) {
                        self.feedback_reply_cache
                            .insert(r.id.clone(), format_feedback_answer(&r.id, &resp));
                    }
                }
            }
            if let Some(reply) = self.feedback_reply_cache.get(&r.id) {
                return reply.to_ascii_lowercase().contains(q);
            }
        }
        false
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
            self.feedback_user_picked = false;
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

    /// Decode the still once per (path, mtime) — slider tweaks re-run ops on
    /// the cached decode instead of re-reading + re-decoding the file (E249).
    fn source_still(&mut self) -> Result<image::DynamicImage, String> {
        let path = self.img_edit_file.clone().ok_or("No image selected")?;
        let mtime = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Some((p, mt, img)) = &self.still_decode_cache {
            if *p == path && *mt == mtime {
                return Ok(img.clone());
            }
        }
        let img = image::open(&path).map_err(|e| format!("Could not open image: {}", e))?;
        self.still_decode_cache = Some((path, mtime, img.clone()));
        Ok(img)
    }

    /// `max_dim` caps the working image after crop — the Still preview passes
    /// 1600 so >25 MP sources stay responsive; export passes `None` (E249).
    fn compute_edited_image(
        &mut self,
        max_dim: Option<u32>,
    ) -> Result<image::DynamicImage, String> {
        let mut img = self.source_still()?;
        if img.width() as u64 * img.height() as u64 > 50_000_000 {
            return Err("Image too large (>50 MP) — refusing to edit.".to_string());
        }
        let any_crop = !self.img_crop_x.trim().is_empty()
            || !self.img_crop_y.trim().is_empty()
            || !self.img_crop_w.trim().is_empty()
            || !self.img_crop_h.trim().is_empty();
        if any_crop {
            let (cx, cy, cw, ch) = (
                self.img_crop_x.trim().parse::<u32>().unwrap_or(0),
                self.img_crop_y.trim().parse::<u32>().unwrap_or(0),
                self.img_crop_w.trim().parse::<u32>().unwrap_or(0),
                self.img_crop_h.trim().parse::<u32>().unwrap_or(0),
            );
            if cw == 0
                || ch == 0
                || (cx as u64 + cw as u64) > img.width() as u64
                || (cy as u64 + ch as u64) > img.height() as u64
            {
                return Err("Crop exceeds image bounds — nothing was cropped.".to_string());
            }
            img = img.crop_imm(cx, cy, cw, ch);
        }
        if let Some(cap) = max_dim {
            if img.width().max(img.height()) > cap {
                img = img.thumbnail(cap, cap);
            }
        }
        img = match self.img_rotate {
            90 => img.rotate90(),
            180 => img.rotate180(),
            270 => img.rotate270(),
            _ => img,
        };
        if self.img_flip_h {
            img = img.fliph();
        }
        if self.img_flip_v {
            img = img.flipv();
        }
        if self.img_resize_pct != 100 && self.img_resize_pct > 0 {
            let w = (img.width() as f32 * self.img_resize_pct as f32 / 100.0).max(1.0) as u32;
            let h = (img.height() as f32 * self.img_resize_pct as f32 / 100.0).max(1.0) as u32;
            img = img.resize(w, h, image::imageops::FilterType::Triangle);
        }
        if self.img_grayscale {
            img = img.grayscale();
        }
        if self.img_brightness != 0 {
            img = img.brighten(self.img_brightness);
        }
        if self.img_contrast != 0.0 {
            img = img.adjust_contrast(self.img_contrast);
        }
        if self.img_blur > 0.05 {
            img = img.blur(self.img_blur);
        }
        Ok(img)
    }

    fn refresh_img_preview(&mut self, ctx: &egui::Context) {
        let params = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.img_rotate,
            self.img_flip_h,
            self.img_flip_v,
            self.img_grayscale,
            self.img_brightness,
            self.img_contrast,
            self.img_blur,
            self.img_resize_pct,
            self.img_crop_x,
            self.img_crop_y,
            self.img_crop_w,
            self.img_crop_h
        );
        if params == self.img_preview_params || self.img_edit_file.is_none() {
            return;
        }
        self.img_preview_params = params;
        if let Ok(img) = self.compute_edited_image(Some(1600)) {
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
        self.spawn_ffmpeg_job_ex(args, ok_msg, None);
    }

    /// E64 — batch re-export: every selected video/GIF becomes a GIF at the
    /// clip editor's fps/width settings. One worker runs the jobs serially —
    /// a 20-pick batch must not spawn 20 concurrent encoders.
    pub(crate) fn batch_gif_export(&mut self) {
        let files: Vec<PathBuf> = self
            .library_items
            .iter()
            .filter(|i| {
                self.library_selected.contains(&i.path)
                    && matches!(i.category, MediaCategory::Video | MediaCategory::Gif)
            })
            .map(|i| i.path.clone())
            .collect();
        if files.is_empty() {
            self.show_toast("Select video or GIF items first");
            return;
        }
        let Some(tx) = self.ffmpeg_tx.clone() else {
            return;
        };
        let fps = self.gif_fps.clamp(4, 30);
        let w = self.gif_width.clamp(160, 1920).min(720);
        let n = files.len();
        self.show_toast(format!("🎞 Exporting {n} GIF(s) in the background…"));
        std::thread::spawn(move || {
            let mut ok_n = 0u32;
            let mut fail = String::new();
            for f in &files {
                let stem = f
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "clip".into());
                let out = f.with_file_name(format!("{stem}_720p.gif"));
                let args = [
                    "-y".to_string(),
                    "-i".to_string(),
                    f.to_string_lossy().to_string(),
                    "-vf".to_string(),
                    format!("fps={fps},scale={w}:-1:flags=lanczos"),
                    "-loop".to_string(),
                    "0".to_string(),
                    out.to_string_lossy().to_string(),
                ];
                let good = platform::ffmpeg_command()
                    .map(|mut c| {
                        c.args(&args)
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::piped())
                            .output()
                    })
                    .map(|r| {
                        r.map(|o| {
                            if !o.status.success() {
                                fail = String::from_utf8_lossy(&o.stderr)
                                    .lines()
                                    .last()
                                    .unwrap_or("ffmpeg error")
                                    .trim()
                                    .to_string();
                            }
                            o.status.success()
                        })
                        .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if good {
                    ok_n += 1;
                }
            }
            let failed = n as u32 - ok_n;
            let msg = if failed == 0 {
                format!("Batch GIF export — {ok_n}/{n} done")
            } else {
                format!("Batch GIF export — {ok_n}/{n} done · {failed} failed ({fail})")
            };
            let _ = tx.send((failed == 0, msg));
        });
    }

    /// ffmpeg job with an optional post-run verifier — returns a warning
    /// string appended to the success toast (e.g. trim duration drift).
    fn spawn_ffmpeg_job_ex(
        &mut self,
        args: Vec<String>,
        ok_msg: &str,
        verify: Option<Box<dyn FnOnce() -> Option<String> + Send>>,
    ) {
        let Some(tx) = self.ffmpeg_tx.clone() else {
            return;
        };
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
                        Ok(out) if out.status.success() => {
                            let extra = verify.and_then(|v| v());
                            let msg = match extra {
                                Some(w) => format!("{ok_msg} — ⚠ {w}"),
                                None => ok_msg,
                            };
                            (true, msg)
                        }
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
            // E249 — canvas texture is capped; export still bakes full-res.
            let img = if img.width().max(img.height()) > 4096 {
                img.thumbnail(4096, 4096)
            } else {
                img
            };
            let size = [img.width() as _, img.height() as _];
            let image_buffer = img.to_rgba8();
            let pixels = image_buffer.as_flat_samples();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            self.annotation_texture =
                Some(ctx.load_texture("screenshot", color_image, Default::default()));
        }
        self.annotation_actions.clear();
        self.annotation_selected = None;
        self.step_counter = 1;
    }

    /// Handles the ViewportCommand::Screenshot reply: crops to the annotation canvas and saves it,
    /// producing a flattened image with annotations baked in.
    fn check_annotated_save(&mut self, ctx: &egui::Context) {
        let Some((target, requested_at)) = self.pending_annotated_save.clone() else {
            return;
        };
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
        let Some(img) = found else {
            return;
        };

        let (w, h) = (img.width(), img.height());
        let raw: Vec<u8> = img
            .pixels
            .iter()
            .flat_map(|c| [c.r(), c.g(), c.b(), c.a()])
            .collect();
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

    fn copy_image_to_clipboard(&mut self, path: &PathBuf) -> bool {
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
                    // E12 — PNG stays sharp pasted into browsers/Office:
                    // attach an encode of the same pixels under the registered
                    // format next to the raw DIB arboard just wrote.
                    let (fmt, image_fmt) = match self.clipboard_encode.as_str() {
                        "jpeg" => ("JFIF", image::ImageFormat::Jpeg),
                        "png" => ("PNG", image::ImageFormat::Png),
                        _ => ("", image::ImageFormat::Png),
                    };
                    if !fmt.is_empty() {
                        let mut buf = std::io::Cursor::new(Vec::new());
                        if img.write_to(&mut buf, image_fmt).is_ok() {
                            let _ = crate::platform::clipboard_add_encoded(buf.get_ref(), fmt);
                        }
                    }
                    // Re-read after the encoded attach — SetClipboardData
                    // bumps the sequence, and the watcher must not treat
                    // our own write as a fresh external image.
                    self.clipboard_seq_seen = crate::platform::clipboard_seq();
                    self.show_toast("📋 Image copied to system clipboard!");
                    return true;
                }
            }
        }
        false
    }

    fn recording_elapsed_secs(&self) -> u64 {
        let current = self
            .segment_start
            .map(|s| s.elapsed())
            .unwrap_or(Duration::ZERO);
        (self.accumulated_duration + current).as_secs()
    }

    /// E51 — one-line "what am I recording" for the REC bar caption.
    fn record_source_line(&self) -> String {
        let target = match self.capture_target {
            CaptureTarget::Fullscreen => self
                .capture_monitor
                .map(|m| format!("Display {}", m + 1))
                .unwrap_or_else(|| "Full screen".to_string()),
            CaptureTarget::Region => self
                .selected_screen_rect
                .map(|(w, h, _, _)| format!("Region {w}×{h}"))
                .unwrap_or_else(|| "Region".to_string()),
            CaptureTarget::Window => {
                let name = self.window_app.trim();
                let name = if name.is_empty() {
                    self.last_front_app.as_deref().unwrap_or("window")
                } else {
                    name
                };
                let short: String = name.chars().take(18).collect();
                format!("Window: {short}")
            }
        };
        // Audio flag shows wherever recording honors it — dshow input on
        // Windows, avfoundation on macOS. Time-lapse never records audio.
        let audio = if self.capture_audio && self.timelapse_secs == 0 {
            " · mic"
        } else {
            ""
        };
        let lapse = if self.timelapse_secs > 0 {
            format!(" · lapse {}s", self.timelapse_secs)
        } else {
            String::new()
        };
        let marks = if self.record_markers.is_empty() {
            String::new()
        } else {
            format!(" · ⚑ {}", self.record_markers.len())
        };
        format!("{target}{audio}{lapse}{marks}")
    }

    fn toggle_pause(&mut self) {
        if !self.is_recording {
            return;
        }
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
            self.arm_cancel.store(true, Ordering::SeqCst);
            self.recording_arming = false;
            // Drop receiver; worker may still finish — drain_record_spawn kills it.
            self.record_spawn_rx = None;
            app::agent_record::clear_gui_record_state();
            self.release_record_exclusion();
            self.show_window(ctx);
            self.show_toast("❌ Recording cancelled");
            return;
        }

        if self.recording_finalizing {
            // ffmpeg is finalizing the MP4 — let it finish; deleting mid-write corrupts it.
            self.show_toast("💾 Still saving — wait for ffmpeg to finish.");
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

        app::agent_record::clear_gui_record_state();
        self.release_record_exclusion();
        self.show_window(ctx);
        self.show_toast("❌ Recording cancelled");
    }

    fn stop_recording(&mut self, ctx: &egui::Context) {
        if self.recording_arming {
            // Nothing to save yet — treat as cancel.
            self.cancel_recording(ctx);
            return;
        }
        if self.recording_finalizing {
            // ffmpeg is still writing the moov atom — a second Stop must not pile on.
            return;
        }

        if let Some(child) = self.child_process.take() {
            if self.is_paused {
                cont_process(child.id());
            }
            self.recording_finalizing = true;
            let (tx, rx) = crossbeam_channel::bounded(1);
            self.record_finalize_rx = Some(rx);
            let ctx_clone = ctx.clone();
            std::thread::spawn(move || {
                let res = finalize_recorder(child)
                    .map(|_| ())
                    .map_err(|e| e.to_string());
                let _ = tx.send(res);
                ctx_clone.request_repaint();
            });
            self.show_toast("💾 Saving recording…");
        }

        // Stop the REC clock UI now; drain_record_finalize runs the post-stop
        // steps once the MP4 is complete (probing a file still being muxed fails).
        self.is_recording = false;
        self.is_paused = false;
        self.accumulated_duration = Duration::ZERO;
        self.segment_start = None;
        self.recording_arming = false;

        // Always surface the main window (Visible + unminimize) so Editor is usable after tray/hidden rec.
        self.show_window(ctx);

        if !self.recording_finalizing {
            self.finish_stop_recording(ctx);
        }
    }

    /// Drain the recorder-finalize worker, then run post-stop steps.
    fn drain_record_finalize(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.record_finalize_rx.as_ref() else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        self.record_finalize_rx = None;
        self.recording_finalizing = false;
        if let Err(e) = result {
            self.show_toast(format!("⚠️ Recorder did not exit cleanly: {e}"));
        }
        self.finish_stop_recording(ctx);
    }

    /// Post-stop steps — safe only after ffmpeg has written the moov atom.
    fn finish_stop_recording(&mut self, ctx: &egui::Context) {
        app::agent_record::clear_gui_record_state();
        self.release_record_exclusion();
        if self.shutter_sound {
            crate::platform::record_tone(false);
        }
        if let Some(mp4) = self.current_mp4_file.clone() {
            // E245 — keep the recorder's stderr tail in-process for doctor.
            crate::platform::remember_ffmpeg_log(&mp4);
            if !self.record_markers.is_empty() {
                let side = mp4.with_extension("markers.txt");
                let body = self
                    .record_markers
                    .iter()
                    .map(|t| format!("{t:.3}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let _ = std::fs::write(side, body);
            }
            let bytes = std::fs::metadata(&mp4).map(|m| m.len()).unwrap_or(0);
            self.edit_file = Some(mp4.clone());
            // Load the preview either way — D70 only gates the tab jump;
            // visiting Review later must not find an empty player.
            self.load_filmstrip(ctx, mp4.clone());
            if self.auto_open_review {
                self.current_tab = AppTab::Clip;
            }
            self.refresh_library();
            if bytes < 512 {
                if self.stats_opt_in {
                    self.stat_recs_fail = self.stat_recs_fail.saturating_add(1);
                }
                self.show_toast(format!(
                    "⚠️ Saved {} but file looks empty ({bytes} bytes) — check Screen Recording permission.",
                    mp4.file_name().and_then(|n| n.to_str()).unwrap_or("video")
                ));
            } else {
                if self.stats_opt_in {
                    self.stat_recs_ok = self.stat_recs_ok.saturating_add(1);
                }
                // Same rule as stills: the fresh clip's path goes straight to
                // the clipboard so it is pasteable without opening Vibecap.
                let copied = arboard::Clipboard::new()
                    .and_then(|mut b| b.set_text(mp4.display().to_string()))
                    .is_ok();
                self.last_capture = Some(LastCapture::Clip(mp4.clone()));
                let name = mp4
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("video.mp4");
                self.show_toast(if copied {
                    format!("💾 Video saved — path copied · {name}")
                } else {
                    format!("💾 Video saved · {name}")
                });
            }
        } else {
            self.refresh_library();
            self.show_toast("⚠️ Stopped but no video path was set.");
        }
        // E256 — prove the MP4 is readable before calling the stop a
        // success: a worker decodes one frame; a missing moov (killed
        // mid-write) triggers a remux repair instead of a silent bad file.
        if let Some(mp4) = self.current_mp4_file.clone() {
            if self.mp4_verify_rx.is_none() {
                let (tx, rx) = crossbeam_channel::bounded(1);
                self.mp4_verify_rx = Some(rx);
                let ctx_clone = ctx.clone();
                std::thread::spawn(move || {
                    let res = if crate::platform::verify_mp4(&mp4) {
                        Ok(None)
                    } else {
                        let clean = mp4.with_extension("repaired.mp4");
                        match crate::platform::remux_to_clean_mp4(&mp4, &clean) {
                            Ok(()) => Ok(Some(clean)),
                            Err(e) => Err(e),
                        }
                    };
                    let _ = tx.send((mp4, res));
                    ctx_clone.request_repaint();
                });
            }
        }
        // B52 — a dragged REC bar position lands on disk with the session.
        self.persist_session();
    }

    /// E256 — land the post-stop readability probe: verified → silent;
    /// remux repaired → point Review/recents at the clean file;
    /// unrepairable → loud toast (never a quiet corrupt file).
    fn drain_mp4_verify(&mut self) {
        let Some(rx) = self.mp4_verify_rx.as_ref() else {
            return;
        };
        let Ok((mp4, res)) = rx.try_recv() else {
            return;
        };
        self.mp4_verify_rx = None;
        match res {
            Ok(None) => {
                // E94 — clean stop: the recorder log is noise, not signal.
                // Kept only when verify/repair says the MP4 is suspect.
                let _ = std::fs::remove_file(mp4.with_extension("ffmpeg.log"));
            }
            Ok(Some(clean)) => {
                if self.edit_file.as_ref() == Some(&mp4) {
                    self.edit_file = Some(clean.clone());
                }
                if self.current_mp4_file.as_ref() == Some(&mp4) {
                    self.current_mp4_file = Some(clean.clone());
                }
                if matches!(&self.last_capture, Some(LastCapture::Clip(p)) if *p == mp4) {
                    self.last_capture = Some(LastCapture::Clip(clean.clone()));
                }
                let name = clean
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("repaired.mp4");
                self.show_toast(format!("⚠ Recording was unreadable — repaired → {name}"));
                self.refresh_library();
            }
            Err(e) => {
                self.show_toast(format!(
                    "⚠ Recording may be unreadable (repair failed: {e})"
                ));
            }
        }
    }

    fn load_filmstrip(&mut self, ctx: &egui::Context, file: PathBuf) {
        self.filmstrip.clear();
        self.filmstrip_error = None;
        self.dead_air_hint = None;
        self.dead_air_dismissed = false;
        self.filmstrip_loading = true;
        self.clip_duration_secs = 0.0;
        self.player_playing = false;
        self.player_pos = 0.0;
        self.player_last_time = None;
        self.filmstrip_cut.clear();
        // F126 — stop any playing preview audio and re-extract for this clip.
        if self.preview_audio_playing {
            crate::platform::stop_audio_preview();
            self.preview_audio_playing = false;
        }
        self.preview_audio_path = None;
        {
            let (atx, arx) = crossbeam_channel::bounded(1);
            self.preview_audio_rx = Some(arx);
            let audio_src = file.clone();
            std::thread::spawn(move || {
                let _ = atx.send(crate::platform::extract_preview_wav(&audio_src));
            });
        }
        self.record_markers = load_marker_sidecar(&file);
        self.clip_notes =
            std::fs::read_to_string(file.with_extension("notes.txt")).unwrap_or_default();
        self.filmstrip_progress = (0, 0);

        let (tx, rx) = crossbeam_channel::bounded(1);
        let (ptx, prx) = crossbeam_channel::unbounded();
        self.filmstrip_rx = Some(rx);
        self.filmstrip_progress_rx = Some(prx);
        let ctx_clone = ctx.clone();
        let low_res = self.filmstrip_low_res;
        std::thread::spawn(move || {
            // Let ffmpeg finish the moov atom before probing / extracting.
            std::thread::sleep(Duration::from_millis(200));
            let result = extract_filmstrip_rgba(
                &file,
                Some(&|i, n| {
                    let _ = ptx.send((i, n));
                }),
                low_res,
            );
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
        ctx.request_repaint_after(Duration::from_millis(50));
    }

    fn drain_filmstrip(&mut self, ctx: &egui::Context) {
        // Decode progress is fire-and-forget — drain whatever arrived.
        if let Some(prx) = self.filmstrip_progress_rx.as_ref() {
            while let Ok(p) = prx.try_recv() {
                self.filmstrip_progress = p;
            }
        }
        // Preview-audio extraction result (F126) lands independently.
        if let Some(arx) = self.preview_audio_rx.as_ref() {
            if let Ok(res) = arx.try_recv() {
                self.preview_audio_path = res;
                self.preview_audio_rx = None;
            }
        }
        // E214 — update check result; the worker keeps curl off the UI thread.
        if let Some(urx) = self.update_rx.as_ref() {
            if let Ok(res) = urx.try_recv() {
                match res {
                    Ok(info) => {
                        self.update_status = if info.newer {
                            format!(
                                "update available: {} (running v{})",
                                info.tag,
                                env!("CARGO_PKG_VERSION")
                            )
                        } else {
                            format!("up to date (v{})", env!("CARGO_PKG_VERSION"))
                        };
                        // E214 — changelog toast on launch checks: only when
                        // a newer release exists (quiet otherwise).
                        if info.newer {
                            self.show_toast(format!(
                                "Vibecap {} is out — Settings → Download",
                                info.tag
                            ));
                        }
                        self.update_info = Some(info);
                    }
                    Err(e) => {
                        self.update_status = e;
                        self.update_info = None;
                    }
                }
                self.update_rx = None;
            }
        }
        let Some(rx) = self.filmstrip_rx.as_ref() else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        self.filmstrip_rx = None;
        self.filmstrip_progress_rx = None;
        match result {
            Ok((frames, fps, duration)) => {
                self.filmstrip_fps = fps;
                self.clip_duration_secs = duration;
                // Dead-air scan on the raw RGBA before it goes to the GPU —
                // once frames are textures the pixels are unreachable.
                self.dead_air_hint = app::recording::dead_air_bounds(&frames, fps);
                // B54 — auto-apply instead of offering when the user opted in.
                if self.auto_dead_air {
                    if let Some((cs, ce)) = self.dead_air_hint {
                        self.trim_start = crate::platform::format_timecode(cs);
                        self.trim_end = crate::platform::format_timecode(ce);
                        self.dead_air_dismissed = true;
                        self.show_toast(format!(
                            "✂ Auto-trimmed dead air → {}–{}",
                            crate::platform::format_timecode(cs),
                            crate::platform::format_timecode(ce)
                        ));
                    }
                }
                for (i, (w, h, pixels)) in frames.into_iter().enumerate() {
                    let expected = w as usize * h as usize * 4;
                    if w == 0 || h == 0 || pixels.len() != expected {
                        continue;
                    }
                    let color_image =
                        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
                    let tex =
                        ctx.load_texture(format!("thumb_{i}"), color_image, Default::default());
                    self.filmstrip.push(tex);
                }
                if self.filmstrip.is_empty() {
                    self.filmstrip_error =
                        Some("No frames extracted — video may be corrupt or too short.".into());
                } else {
                    // Land playing: a still first frame reads as "won't start"
                    // (unless the user turned autoplay off in Settings).
                    self.player_playing = self.clip_autoplay;
                    self.player_last_time = None;
                }
            }
            Err(e) => {
                self.filmstrip_error = Some(e);
            }
        }
        self.filmstrip_loading = false;
    }
    /// F136 — re-encode the clip with the right-click-marked filmstrip
    /// sections dropped (`select` on time windows; audio mirrors via
    /// `aselect`). GIF sources re-encode to GIF, everything else to MP4.
    pub fn export_without_cuts(&mut self, file: &std::path::Path) {
        if self.filmstrip_cut.is_empty() || self.filmstrip_fps <= 0.0 {
            return;
        }
        let mut idx: Vec<usize> = self.filmstrip_cut.iter().copied().collect();
        idx.sort_unstable();
        // Merge adjacent marks into contiguous time ranges.
        let fps = self.filmstrip_fps;
        let mut ranges: Vec<(f64, f64)> = Vec::new();
        for i in idx {
            let (s, e) = (i as f64 / fps, (i as f64 + 1.0) / fps);
            match ranges.last_mut() {
                Some(last) if s <= last.1 + 0.001 => last.1 = e,
                _ => ranges.push((s, e)),
            }
        }
        let expr = ranges
            .iter()
            .map(|(a, b)| format!("between(t,{a:.3},{b:.3})"))
            .collect::<Vec<_>>()
            .join("+");
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("clip.mp4");
        let is_gif = file
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("gif"))
            .unwrap_or(false);
        let vf = format!("select='not({expr})',setpts=N/FRAME_RATE/TB");
        let args = if is_gif {
            let out = file.with_file_name(format!("cut_{name}"));
            (
                vec![
                    "-i".into(),
                    file.to_str().unwrap_or_default().into(),
                    "-vf".into(),
                    format!("{vf},fps=15,scale=480:-1:flags=lanczos"),
                    "-y".into(),
                    out.to_str().unwrap_or_default().into(),
                ],
                "GIF re-encoded without cuts",
            )
        } else {
            let out = file.with_file_name(format!("cut_{name}"));
            (
                vec![
                    "-i".into(),
                    file.to_str().unwrap_or_default().into(),
                    "-vf".into(),
                    vf,
                    "-af".into(),
                    format!("aselect='not({expr})',asetpts=N/SR/TB"),
                    "-c:v".into(),
                    "libx264".into(),
                    "-crf".into(),
                    "23".into(),
                    "-c:a".into(),
                    "aac".into(),
                    "-y".into(),
                    out.to_str().unwrap_or_default().into(),
                ],
                "Exported without cut sections",
            )
        };
        self.spawn_ffmpeg_job(args.0, args.1);
        self.filmstrip_cut.clear();
    }

    fn arm_recording(&mut self, ctx: &egui::Context) {
        if self.is_recording || self.recording_arming || self.recording_finalizing {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let mp4_file = self.save_dir.join(format!("video_{}.mp4", timestamp));
        let fps = self.fps_target.max(1);
        let with_audio = self.capture_audio;
        // Region recordings use the pixel rect mapped from the region overlay
        // (`selected_screen_rect`). The raw overlay points are egui units, not
        // desktop pixels (HiDPI), so they must never be used directly here.
        let crop = if self.capture_target == CaptureTarget::Region {
            match self.selected_screen_rect {
                Some(rect) => Some(rect),
                None => {
                    self.show_toast("Select a region first — drag a rectangle, then record.");
                    return;
                }
            }
        } else {
            None
        };
        // Window recordings resolve the window rectangle inside the recorder;
        // pass the target through so they never silently become fullscreen.
        let record_monitor = self.resolved_monitor();
        let record_opts = match self.capture_target {
            CaptureTarget::Window => CaptureOpts::from_parts(
                None,
                Some(
                    self.capture_focus_target()
                        .unwrap_or_else(|| self.window_app.clone()),
                ),
            )
            .with_monitor(record_monitor),
            _ => CaptureOpts::default().with_monitor(record_monitor),
        }
        .with_crf(self.record_crf)
        .with_audio_device(&self.audio_device)
        .with_audio_mix_device(&self.audio_mix_device)
        .with_timelapse(self.timelapse_secs);

        let (tx, rx) = crossbeam_channel::bounded(1);
        self.record_spawn_rx = Some(rx);
        self.recording_arming = true;
        self.recording_cancel_armed = false;
        self.arm_cancel.store(false, Ordering::SeqCst);
        self.current_mp4_file = Some(mp4_file.clone());
        // E265 — announce to `vibecap record start` that the studio owns the
        // capture lock; cleared on every terminal path below.
        app::agent_record::write_gui_record_state(&mp4_file);

        // Windows: keep the studio on screen but invisible to the capture
        // (WDA_EXCLUDEFROMCAPTURE). A minimized window gets no WM_PAINT, so
        // update() stalls while minimized — drain_record_spawn then never
        // adopts the ffmpeg child, recording_arming sticks at "Starting…",
        // and the recorder runs headless until the user reopens the window.
        // Excluded-but-visible keeps the event loop alive and leaves a REC
        // surface with a Stop button on screen. Fall back to parking when
        // the API refuses (pre-Win10-2004).
        #[cfg(windows)]
        let arm_excluded = {
            let excluded = crate::platform::set_studio_capture_excluded(true);
            self.record_excluded = excluded;
            if !excluded {
                app::capture_flow::snapshot_park_geometry(
                    ctx,
                    &mut self.pre_capture_outer,
                    &mut self.pre_capture_size,
                );
                self.wake_shared.parked.store(true, Ordering::SeqCst);
            }
            ctx.request_repaint();
            excluded
        };
        #[cfg(not(windows))]
        self.hide_for_capture(ctx);

        let arm_cancel = self.arm_cancel.clone();
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            // Let the compositor hide our UI before the grabber starts.
            #[cfg(windows)]
            {
                if arm_excluded {
                    // Affinity applies synchronously; a short settle lets DWM
                    // drop us from the composed frame before gdigrab reads it.
                    std::thread::sleep(Duration::from_millis(120));
                } else {
                    std::thread::sleep(Duration::from_millis(150));
                    if !arm_cancel.load(Ordering::SeqCst) {
                        crate::platform::minimize_studio();
                    }
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
            #[cfg(not(windows))]
            std::thread::sleep(Duration::from_millis(350));
            if arm_cancel.load(Ordering::SeqCst) {
                // Cancelled during arm — do not spawn a headless recorder.
                #[cfg(windows)]
                crate::platform::restore_studio_to_taskbar();
                ctx_clone.request_repaint();
                return;
            }
            let result =
                spawn_screen_recorder_opts(&mp4_file, fps, with_audio, crop, &record_opts, false)
                    .map(|child| (child, mp4_file));
            if let Err(unsent) = tx.send(result) {
                // Receiver dropped by a cancel that raced the spawn — a Child
                // dropped without kill keeps writing a headless MP4.
                if let Ok((mut child, path)) = unsent.into_inner() {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = std::fs::remove_file(path);
                }
            }
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
            app::agent_record::clear_gui_record_state();
            self.release_record_exclusion();
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
                if self.shutter_sound {
                    crate::platform::record_tone(true);
                }
                // Excluded path keeps the studio visible as the REC surface;
                // the floating bar + tray still cover the parked fallback.
                if cfg!(target_os = "windows") {
                    self.show_toast(if self.record_excluded {
                        "Recording — this window is hidden from the capture. Stop here, in the tray, or Ctrl+Shift+2"
                    } else {
                        "Recording — stop from the REC bar, tray, or Ctrl+Shift+2"
                    });
                }
                ctx.request_repaint();
            }
            Err(e) => {
                self.current_mp4_file = None;
                app::agent_record::clear_gui_record_state();
                self.release_record_exclusion();
                self.show_window(ctx);
                self.show_toast(format!("❌ Record failed: {e}"));
            }
        }
    }

    fn trigger_capture(&mut self, ctx: &egui::Context, is_screenshot: bool) {
        // E51 — record the target under the current hour before dispatch;
        // even a region-pick opening counts as "user wanted Region".
        self.target_hours.insert(
            Local::now().hour() as u8,
            self.capture_target.name().to_string(),
        );
        self.persist_session();
        if !is_screenshot {
            if self.capture_target == CaptureTarget::Region {
                if self.selected_screen_rect.is_none() {
                    self.start_region_pick(ctx, RegionPickKind::Record);
                    return;
                }
            }
            self.begin_recording(ctx);
            return;
        }

        // Ignore a second trigger while a capture worker is already in flight
        // (hotkey + tray + button pressed together would otherwise race on
        // focus/hide and restore). `still_busy` also covers the pump thread's
        // hidden-capture fast path, which bypasses this function.
        if self.screenshot_in_flight || self.wake_shared.still_busy.load(Ordering::SeqCst) {
            return;
        }

        if self.capture_target == CaptureTarget::Region {
            // macOS stills use the native interactive picker; elsewhere we freeze a
            // snapshot and crop in-app (Windows cannot host a transparent overlay HUD).
            if cfg!(target_os = "macos") {
                self.start_macos_interactive_still(ctx);
            } else {
                self.start_region_pick(ctx, RegionPickKind::Screenshot);
            }
            return;
        }

        // Screenshot flow:
        // - hide main window so it is not in the shot
        // - worker writes pending_still.path as durable handoff
        // - main opens Still + restores geometry when marker appears
        self.poll_frontmost_app();

        // Fullscreen / Window capture restores a real app before the shot.
        // On macOS a shot with no known target is bare-desktop wallpaper
        // (TCC-gated capture yields wallpaper-only) — refuse with guidance.
        // On Windows/Linux a desktop grab is still a real capture, so proceed.
        if cfg!(target_os = "macos") && self.capture_focus_target().is_none() {
            self.show_toast(
                "No app to capture — click the app you want in the shot, then press S again.",
            );
            return;
        }

        let focus_target = self.capture_focus_target();
        let is_window = self.capture_target == CaptureTarget::Window;
        if is_window && focus_target.is_none() {
            self.show_toast("Pick a window app first — or switch to Full.");
            return;
        }
        // E73 — a minute-plus delay is a *schedule*, not a park: keep the
        // studio interactive and fire when the Instant lands. The card shows
        // the countdown + a Cancel button until then.
        if self.capture_delay_secs >= 60 && !self.scheduled_fire_now {
            let mins = self.capture_delay_secs / 60;
            self.scheduled_shot_at =
                Some(Instant::now() + Duration::from_secs(self.capture_delay_secs));
            self.show_toast(format!(
                "⏰ Capture scheduled — fires in {mins}m · Cancel on the Capture card"
            ));
            return;
        }
        self.scheduled_fire_now = false;

        self.screenshot_in_flight = true;
        self.wake_shared.still_busy.store(true, Ordering::SeqCst);
        // Flash now — the press visibly registered even though the file lands
        // ~0.5–1 s later (hide settle + grab). finish_screenshot flashes again.
        self.shutter_flash_until = Some(Instant::now() + Duration::from_millis(140));
        self.hide_for_capture(ctx);

        let ctx_clone = ctx.clone();
        let save_dir = self.save_dir.clone();
        let draw_mouse = self.draw_mouse;
        let monitor = self.resolved_monitor();
        let pattern = self.name_pattern.clone();
        let app_token = focus_target.clone();
        let mut delay_ms = self.capture_delay_secs.saturating_mul(1000);
        // E19 — a menu is already open: give it a beat to stay rendered.
        #[cfg(windows)]
        if crate::platform::foreground_is_menu() {
            delay_ms = delay_ms.max(1000);
        }

        std::thread::spawn(move || {
            // Resolve the output name while the compositor is still hiding our
            // window — the directory walk overlaps the settle, not adds to it.
            let seq = app::naming::next_seq(&save_dir, "");
            let stem = app::format_capture_stem(&pattern, app_token.as_deref(), seq);
            let shot_file = save_dir.join(format!("{stem}.jpg"));
            // SW_HIDE removes the window from the compositor synchronously —
            // a couple of frames of propagation is plenty. The old 450 ms
            // covered the minimize ANIMATION, which no longer happens. Any
            // configured delay rides on top (menu/tooltip shots: the window is
            // already gone, the wait just lets the user open things first).
            let hide_ms = if cfg!(target_os = "windows") {
                100
            } else {
                450
            };
            std::thread::sleep(Duration::from_millis(hide_ms + delay_ms));
            if is_window {
                // Window path focuses + crops inside capture_screenshot_opts;
                // do not pre-focus here (double focus races the grab).
                let name = focus_target.clone().unwrap_or_default();
                let result = capture_screenshot_opts(
                    &shot_file,
                    &CaptureOpts::from_parts(None, Some(name))
                        .with_draw_mouse(draw_mouse)
                        .with_monitor(monitor),
                )
                .map(|_| shot_file);
                match &result {
                    Ok(p) => write_pending_still(p),
                    Err(e) => write_pending_still_error(e),
                }
                // A minimized winit window gets no WM_PAINT, so update() may be
                // asleep — unminimize the HWND directly so the pending marker is
                // consumed now, not when the user clicks the taskbar.
                #[cfg(windows)]
                crate::platform::restore_studio_to_taskbar();
                ctx_clone.request_repaint();
                return;
            }
            if let Some(app) = &focus_target {
                // After our hide, Windows usually returns foreground to the
                // app that was under us — when that already IS the target its
                // content is up, so skip the refocus and the redraw settle.
                #[cfg(windows)]
                let already_front = crate::platform::foreground_process_name()
                    .map(|n| {
                        let n = n.to_ascii_lowercase();
                        let t = app.to_ascii_lowercase();
                        !t.is_empty() && (n.contains(&t) || t.contains(&n))
                    })
                    .unwrap_or(false);
                #[cfg(not(windows))]
                let already_front = false;
                if !already_front {
                    // If focus fails, the shot would be bare desktop — abort
                    // with the reason instead of saving a useless image.
                    if let Err(e) = focus_app(app) {
                        write_pending_still_error(&format!(
                            "{} — click the app you want in the shot first, then capture again",
                            e
                        ));
                        #[cfg(windows)]
                        crate::platform::restore_studio_to_taskbar();
                        ctx_clone.request_repaint();
                        return;
                    }
                    // Native focus verifies GetForegroundWindow before returning;
                    // the remaining settle covers the target app's redraw before
                    // the grab reads the frame.
                    let focus_ms = if cfg!(target_os = "windows") {
                        350
                    } else {
                        700
                    };
                    std::thread::sleep(Duration::from_millis(focus_ms));
                }
            } else if !cfg!(target_os = "windows") {
                std::thread::sleep(Duration::from_millis(500));
            }
            let result = capture_screenshot_opts(
                &shot_file,
                &CaptureOpts::from_parts(None, None)
                    .with_draw_mouse(draw_mouse)
                    .with_monitor(monitor),
            )
            .map(|_| shot_file);
            match &result {
                Ok(p) => write_pending_still(p),
                Err(e) => write_pending_still_error(e),
            }
            // A minimized winit window gets no WM_PAINT, so update() may be
            // asleep — unminimize the HWND directly so the pending marker is
            // consumed now, not when the user clicks the taskbar.
            #[cfg(windows)]
            crate::platform::restore_studio_to_taskbar();
            // No activation from this worker thread: AppKit calls belong on the
            // main thread, and the repaint below + screenshot_in_flight cadence
            // wake `update`, which restores geometry via `finish_screenshot`.
            ctx_clone.request_repaint();
        });
        ctx.request_repaint_after(Duration::from_millis(50));
    }

    pub(crate) fn trigger_gif_clip(&mut self, ctx: &egui::Context) {
        if self.screenshot_in_flight
            || self.wake_shared.still_busy.load(Ordering::SeqCst)
            || self.is_recording
            || self.recording_arming
        {
            return;
        }
        self.screenshot_in_flight = true;
        self.wake_shared.still_busy.store(true, Ordering::SeqCst);
        self.hide_for_capture(ctx);
        let ctx_clone = ctx.clone();
        let save_dir = self.save_dir.clone();
        let pattern = self.name_pattern.clone();
        let app_token = self.capture_focus_target();
        let opts =
            CaptureOpts::from_parts(None, app_token.clone()).with_monitor(self.resolved_monitor());
        std::thread::spawn(move || {
            // SW_HIDE is synchronous — ~100 ms covers compositor propagation.
            let hide_ms = if cfg!(target_os = "windows") {
                100
            } else {
                350
            };
            std::thread::sleep(Duration::from_millis(hide_ms));
            let seq = app::naming::next_seq(&save_dir, "");
            let stem = app::format_capture_stem(&pattern, app_token.as_deref(), seq);
            let mp4 = save_dir.join(format!("{stem}.mp4"));
            let gif = save_dir.join(format!("{stem}.gif"));
            let result = record_screen_clip_opts(&mp4, 3, &opts).and_then(|_| {
                export_gif_clip(
                    &mp4.display().to_string(),
                    "00:00:00",
                    "00:00:03",
                    &gif.display().to_string(),
                )
                .map(|_| gif)
            });
            match &result {
                Ok(p) => write_pending_still(p),
                Err(e) => write_pending_still_error(e),
            }
            #[cfg(windows)]
            crate::platform::restore_studio_to_taskbar();
            ctx_clone.request_repaint();
        });
        ctx.request_repaint_after(Duration::from_millis(50));
    }

    fn start_macos_interactive_still(&mut self, ctx: &egui::Context) {
        if self.screenshot_in_flight {
            return;
        }
        self.screenshot_in_flight = true;
        self.wake_shared.still_busy.store(true, Ordering::SeqCst);
        self.hide_for_capture(ctx);
        let ctx_clone = ctx.clone();
        let save_dir = self.save_dir.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
            let shot_file = save_dir.join(format!("screenshot_{}.jpg", timestamp));
            let result = capture_screenshot_interactive(&shot_file, true).map(|_| shot_file);
            match &result {
                Ok(p) => write_pending_still(p),
                Err(e) => write_pending_still_error(e),
            }
            ctx_clone.request_repaint();
        });
        ctx.request_repaint_after(Duration::from_millis(50));
    }

    /// Repeat the last capture without re-picking: re-arm the same recording
    /// (region crops persist in `selected_screen_rect`), re-grab the
    /// remembered rect as a still, or fall back to a plain fullscreen still.
    /// Palette/tray verb — inside the overlay `R` / ghost double-click covers
    /// the same idea by confirming the ghost rect.
    fn repeat_last_capture(&mut self, ctx: &egui::Context) {
        if self.screenshot_in_flight
            || self.is_selecting_region
            || self.region_snap_rx.is_some()
            || self.is_recording
            || self.recording_arming
            || self.recording_finalizing
        {
            return;
        }
        if matches!(self.last_capture, Some(LastCapture::Clip(_))) {
            self.trigger_capture(ctx, false);
            return;
        }
        let Some(rect) = self.selected_screen_rect else {
            self.trigger_capture(ctx, true);
            return;
        };
        self.capture_rect_still(ctx, rect);
    }

    /// Direct still of a pixel rect (w,h,x,y) — shared by repeat-last and
    /// E22 saved regions. Excludes the studio on Windows instead of hiding
    /// so it stays close to instant.
    fn capture_rect_still(&mut self, ctx: &egui::Context, rect: (i32, i32, i32, i32)) {
        if self.screenshot_in_flight
            || self.is_selecting_region
            || self.region_snap_rx.is_some()
            || self.is_recording
            || self.recording_arming
            || self.recording_finalizing
        {
            return;
        }
        let (w, h, x, y) = rect;
        self.selected_screen_rect = Some(rect);
        self.screenshot_in_flight = true;
        self.wake_shared.still_busy.store(true, Ordering::SeqCst);
        self.shutter_flash_until = Some(Instant::now() + Duration::from_millis(140));
        // Same trick as region pick — exclude rather than hide on Windows,
        // so a repeat is as close to instant as the snap allows.
        #[cfg(windows)]
        let excluded = crate::platform::set_studio_capture_excluded(true);
        #[cfg(not(windows))]
        self.hide_for_capture(ctx);

        let ctx_clone = ctx.clone();
        let save_dir = self.save_dir.clone();
        let draw_mouse = self.draw_mouse;
        let pattern = self.name_pattern.clone();
        std::thread::spawn(move || {
            let seq = app::naming::next_seq(&save_dir, "");
            let stem = app::format_capture_stem(&pattern, None, seq);
            let dest = save_dir.join(format!("{stem}.jpg"));
            let snap = std::env::temp_dir().join(format!("vibecap_repeat_{stem}.jpg"));
            // A beat for DWM to drop us from the composed frame.
            let settle_ms = if cfg!(target_os = "windows") {
                100
            } else {
                450
            };
            std::thread::sleep(Duration::from_millis(settle_ms));
            let region = ScreenRect { x, y, w, h };
            let result =
                capture_screenshot_opts(&snap, &CaptureOpts::default().with_draw_mouse(draw_mouse))
                    .and_then(|_| crop_image_file(&snap, &dest, region))
                    .map(|_| dest);
            let _ = std::fs::remove_file(&snap);
            match &result {
                Ok(p) => write_pending_still(p),
                Err(e) => write_pending_still_error(e),
            }
            #[cfg(windows)]
            {
                if excluded {
                    crate::platform::set_studio_capture_excluded(false);
                }
                crate::platform::restore_studio_to_taskbar();
            }
            ctx_clone.request_repaint();
        });
    }

    fn start_region_pick(&mut self, ctx: &egui::Context, kind: RegionPickKind) {
        if self.region_snap_rx.is_some() || self.screenshot_in_flight {
            return;
        }
        self.pending_region_kind = Some(kind);
        self.selected_region = None;
        // Keep `selected_screen_rect` so the overlay can ghost last pixels.
        self.region_start = None;
        self.region_end = None;
        // Pre-warm: keep the previous snap as an instant backdrop (stamped
        // "refreshing…") until this pick's snap swaps in — Snagit shows the
        // last freeze immediately rather than a blank dim.
        self.region_backdrop_stale = self.region_backdrop.is_some();
        self.is_selecting_region = false;
        self.region_snap_is_refresh = false;
        self.region_live_at = None;
        self.window_pick_hover = None;
        self.window_pick_hover_monitor = false;
        self.window_pick_hwnd = None;
        self.window_pick_cycle = 0;
        self.window_pick_last_pos = None;
        self.window_pick_poll_at = None;
        self.pick_thumbs.clear();
        self.pick_thumb_rx = None;
        self.batch_shot_count = 0;

        // macOS: live transparent overlay. Windows/Linux: freeze a still first
        // (transparent overlays do not composite; the overlay viewport is opaque).
        if cfg!(target_os = "macos") {
            self.is_selecting_region = true;
            ctx.request_repaint();
            return;
        }

        // J230: a snap taken <2 s ago is still the screen the user just saw —
        // reopen the pick on the same freeze instead of re-grabbing. The
        // fullscreen overlay covers the studio either way, so no hide needed.
        let snap_fresh = self.region_backdrop.is_some()
            && self
                .region_snap_path
                .as_ref()
                .map(|p| p.exists())
                .unwrap_or(false)
            && self
                .region_backdrop_at
                .map(|t| t.elapsed() < Duration::from_secs(2))
                .unwrap_or(false);
        if snap_fresh {
            self.region_backdrop_stale = false;
            self.is_selecting_region = true;
            self.wake_shared
                .region_overlay_state
                .store(1, Ordering::SeqCst);
            ctx.request_repaint();
            return;
        }

        // Snagit-style instant pick on Windows: exclude the studio from the
        // grab (WDA_EXCLUDEFROMCAPTURE) instead of hiding it — and show the
        // selector immediately (dim until the freeze lands). The overlay gets
        // its own exclusion in update() before the snap fires, so it cannot
        // freeze into its own backdrop. Falls back to the SW_HIDE +
        // delayed-overlay path when the API is unavailable (pre-Win10-2004).
        #[cfg(windows)]
        {
            self.region_affinity = crate::platform::set_studio_capture_excluded(true);
            if self.region_affinity {
                self.is_selecting_region = true;
                self.wake_shared
                    .region_overlay_state
                    .store(0, Ordering::SeqCst);
                self.region_excl_attempts = 0;
            } else {
                self.hide_for_capture(ctx);
            }
        }
        #[cfg(not(windows))]
        self.hide_for_capture(ctx);
        self.screenshot_in_flight = true;
        self.wake_shared.still_busy.store(true, Ordering::SeqCst);

        let (tx, rx) = crossbeam_channel::bounded(1);
        self.region_snap_rx = Some(rx);
        let ctx_clone = ctx.clone();
        let affinity_used = self.region_affinity;
        let shared = self.wake_shared.clone();
        std::thread::spawn(move || {
            // Track whether the overlay's exclusion status is genuinely
            // unknown — only then can it contaminate the snap.
            let mut overlay_unknown = false;
            let wait_ms = if affinity_used {
                // Instant-overlay path: the selector may already be up — wait
                // until update() has excluded its HWND (1) or given up (2)
                // before reading the screen, else our own dim freezes into
                // the backdrop.
                let mut st = 0u8;
                for _ in 0..80 {
                    st = shared.region_overlay_state.load(Ordering::SeqCst);
                    if st != 0 {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                overlay_unknown = st == 0;
                // Excluded → one more beat for DWM to drop it from the frame;
                // otherwise the overlay is closing/absent — a hair longer.
                if st == 1 {
                    60
                } else {
                    160
                }
            } else if cfg!(target_os = "windows") {
                100
            } else {
                350
            };
            std::thread::sleep(Duration::from_millis(wait_ms));
            let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
            let snap = std::env::temp_dir().join(format!("vibecap_region_snap_{}.jpg", timestamp));
            let result = capture_screenshot(&snap).and_then(|_| {
                let img = image::open(&snap).map_err(|e| format!("could not read snap: {e}"))?;
                let rgba = img.to_rgba8();
                let (w, h) = (rgba.width(), rgba.height());
                let px = rgba.into_raw();
                if overlay_unknown && snap_is_uniform_dim(&px) {
                    return Err("snap caught our own overlay — please retry".into());
                }
                Ok((snap, w, h, px))
            });
            let _ = tx.send(result);
            // Unminimize the HWND directly: while minimized the GUI loop is
            // asleep, and the region overlay cannot appear until it wakes.
            // (Affinity path never hid — nothing to restore.)
            #[cfg(windows)]
            if !affinity_used {
                crate::platform::restore_studio_to_taskbar();
            }
            ctx_clone.request_repaint();
        });
        ctx.request_repaint_after(Duration::from_millis(50));
    }

    /// E93 — while the overlay is open in live mode, re-grab the backdrop
    /// every ~1.2 s. Only on the capture-exclusion path: without it the
    /// opaque overlay would photograph itself into an infinite dim mirror.
    fn tick_live_backdrop(&mut self, ctx: &egui::Context) {
        if !self.is_selecting_region
            || !self.region_live_backdrop
            || !self.region_affinity
            || self.region_snap_rx.is_some()
        {
            return;
        }
        if self
            .region_live_at
            .map(|t| t.elapsed() < Duration::from_millis(1200))
            .unwrap_or(false)
        {
            return;
        }
        self.region_live_at = Some(Instant::now());
        self.region_snap_is_refresh = true;
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.region_snap_rx = Some(rx);
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            let ts = Local::now().format("%Y-%m-%d_%H-%M-%S-%3f").to_string();
            let snap = std::env::temp_dir().join(format!("vibecap_region_live_{ts}.jpg"));
            let result = capture_screenshot(&snap).and_then(|_| {
                let img = image::open(&snap).map_err(|e| format!("could not read snap: {e}"))?;
                let rgba = img.to_rgba8();
                Ok((snap, rgba.width(), rgba.height(), rgba.into_raw()))
            });
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }

    /// E22 — land a worker's thumbnail into the hwnd-keyed texture cache.
    fn drain_pick_thumb(&mut self, ctx: &egui::Context) {
        let Some((hwnd, rx)) = self.pick_thumb_rx.as_ref() else {
            return;
        };
        let Ok(res) = rx.try_recv() else {
            return;
        };
        let hwnd = *hwnd;
        self.pick_thumb_rx = None;
        if let Some((w, h, px)) = res {
            let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &px);
            let tex = ctx.load_texture(format!("pick_thumb_{hwnd}"), img, Default::default());
            self.pick_thumbs.insert(hwnd, tex);
        }
    }

    fn drain_region_snap(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.region_snap_rx.as_ref() else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        self.region_snap_rx = None;
        self.screenshot_in_flight = false;
        self.wake_shared.still_busy.store(false, Ordering::SeqCst);
        let is_refresh = std::mem::take(&mut self.region_snap_is_refresh);
        // E93 — a refresh result only swaps the backdrop; selection state,
        // affinity, and window placement stay exactly as they are.
        if is_refresh {
            if let Ok((path, w, h, pixels)) = result {
                let expected = w as usize * h as usize * 4;
                if w > 0 && h > 0 && pixels.len() == expected {
                    let color_image =
                        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
                    let tex = ctx.load_texture("region_backdrop", color_image, Default::default());
                    self.region_backdrop = Some(tex);
                    self.region_backdrop_px = (w, h);
                    self.region_backdrop_rgba = Some((w, h, pixels));
                    self.region_backdrop_stale = false;
                    self.region_backdrop_at = Some(Instant::now());
                    if let Some(old) = self.region_snap_path.replace(path.clone()) {
                        if old != path {
                            let _ = std::fs::remove_file(old);
                        }
                    }
                } else {
                    let _ = std::fs::remove_file(&path);
                }
            }
            return;
        }
        match result {
            Ok((path, w, h, pixels)) => {
                // A cancel/Esc during the instant-overlay phase cleared the
                // kind — the snap is stale; don't resurrect the selector.
                if self.pending_region_kind.is_none() {
                    let _ = std::fs::remove_file(&path);
                    self.end_region_affinity();
                    self.show_window(ctx);
                    return;
                }
                let expected = w as usize * h as usize * 4;
                if w == 0 || h == 0 || pixels.len() != expected {
                    let _ = std::fs::remove_file(&path);
                    self.pending_region_kind = None;
                    self.end_region_affinity();
                    self.show_window(ctx);
                    self.show_toast("❌ Region snap was empty");
                    return;
                }
                let color_image =
                    egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
                let tex = ctx.load_texture("region_backdrop", color_image, Default::default());
                self.region_backdrop = Some(tex);
                self.region_backdrop_px = (w, h);
                self.region_backdrop_rgba = Some((w, h, pixels));
                self.region_backdrop_stale = false;
                self.region_backdrop_at = Some(Instant::now());
                // Bound temp-dir snaps to the current one.
                if let Some(old) = self.region_snap_path.replace(path.clone()) {
                    if old != path {
                        let _ = std::fs::remove_file(old);
                    }
                }
                self.is_selecting_region = true;
                // Restore the owner so the child overlay can take the mouse —
                // but only when it needs restoring: on the affinity path the
                // studio stayed visible and show_window would focus-steal the
                // already-open overlay.
                #[cfg(windows)]
                let needs_restore =
                    self.pre_capture_outer.is_some() || crate::platform::studio_is_minimized();
                #[cfg(not(windows))]
                let needs_restore = self.pre_capture_outer.is_some();
                if needs_restore {
                    self.show_window(ctx);
                }
                ctx.request_repaint();
            }
            Err(e) => {
                self.pending_region_kind = None;
                self.end_region_affinity();
                self.show_window(ctx);
                self.show_toast(format!("❌ {e}"));
            }
        }
    }

    /// Release WDA_EXCLUDEFROMCAPTURE if this pick used it (Windows only).
    /// No-op on the hide path and on other platforms.
    /// WDA_EXCLUDEFROMCAPTURE has two owners — the region overlay
    /// (`region_affinity`) and recording (`record_excluded`). Re-derive the
    /// flag from both so one owner releasing can never drop the other's
    /// exclusion mid-use.
    fn sync_capture_exclusion(&self) {
        #[cfg(windows)]
        crate::platform::set_studio_capture_excluded(self.region_affinity || self.record_excluded);
    }

    fn release_record_exclusion(&mut self) {
        if self.record_excluded {
            self.record_excluded = false;
            self.rec_bar_exclude_attempts = 0;
            self.sync_capture_exclusion();
        }
    }

    fn end_region_affinity(&mut self) {
        if self.region_affinity {
            self.region_affinity = false;
            self.sync_capture_exclusion();
        }
    }

    /// Pickable window under the cursor for window-pick mode: the Z-cycle
    /// index scrolls deeper through overlapping windows (a moved cursor
    /// resets to topmost), and dead space offers the whole monitor.
    /// Off-Windows the pick UI is unreachable — always None.
    #[cfg(windows)]
    fn poll_window_pick(&mut self, drill: bool) -> Option<(String, i32, i32, i32, i32)> {
        let (x, y) = crate::platform::cursor_pos()?;
        if self
            .window_pick_last_pos
            .map(|(lx, ly)| (lx - x).abs() + (ly - y).abs() > 6)
            .unwrap_or(true)
        {
            self.window_pick_cycle = 0;
        }
        self.window_pick_last_pos = Some((x, y));
        let hits = crate::platform::windows_at_point(x, y);
        if !hits.is_empty() {
            let w = &hits[self.window_pick_cycle % hits.len()];
            // E90 — Alt drills into the hovered app's child windows: the
            // smallest visible child under the cursor becomes the pick rect
            // (toolbars, panes, tooltip popups as their own region).
            if drill {
                if let Some((cx, cy, cw, ch)) = crate::platform::child_window_at(w.hwnd, x, y) {
                    let mut label = if w.title.is_empty() {
                        w.process.clone()
                    } else {
                        w.title.clone()
                    };
                    label = format!("{label} · child");
                    self.window_pick_hover_monitor = false;
                    self.window_pick_hwnd = Some(w.hwnd); // child → parent's pixels
                    return Some((label, cx, cy, cw, ch));
                }
            }
            // E17 — pick card shows title + process + which display it's on.
            let mut label = if w.title.is_empty() {
                w.process.clone()
            } else {
                w.title.clone()
            };
            if !w.title.is_empty()
                && !w.process.is_empty()
                && !w.title.to_lowercase().contains(&w.process.to_lowercase())
            {
                label = format!("{label} · {}", w.process);
            }
            let (cx, cy) = (w.x + w.w / 2, w.y + w.h / 2);
            if let Some(m) = crate::platform::list_monitors()
                .iter()
                .find(|m| cx >= m.x && cx < m.x + m.w && cy >= m.y && cy < m.y + m.h)
            {
                label = format!("{label} · Display {}", m.index + 1);
            }
            self.window_pick_hover_monitor = false;
            self.window_pick_hwnd = Some(w.hwnd);
            return Some((label, w.x, w.y, w.w, w.h));
        }
        self.window_pick_hover_monitor = true;
        self.window_pick_hwnd = None;
        crate::platform::monitor_at_point(x, y).map(|m| ("Display".to_string(), m.x, m.y, m.w, m.h))
    }
    #[cfg(not(windows))]
    fn poll_window_pick(&mut self, _drill: bool) -> Option<(String, i32, i32, i32, i32)> {
        None
    }

    fn exit_region_overlay(&mut self, ctx: &egui::Context) {
        self.end_region_affinity();
        self.is_selecting_region = false;
        self.region_was_dragging = false;
        self.region_refocus_frames = 4;
        self.region_start = None;
        self.region_end = None;
        // Keep backdrop + snap: the next pick's pre-warm shows this freeze
        // instantly, and a re-pick within 2 s reuses it entirely (J230).
        self.window_pick_hover = None;
        self.window_pick_hover_monitor = false;
        self.window_pick_cycle = 0;
        self.window_pick_last_pos = None;
        self.window_pick_poll_at = None;
        self.window_pick_hwnd = None;
        self.pick_thumb_rx = None;
        // E94 — toolbar dock choice is a session pref.
        self.persist_session();
        self.show_window(ctx);
    }

    fn confirm_region_pick(&mut self, ctx: &egui::Context, selected: Rect, overlay: Rect) {
        let (img_w, img_h) = self.region_backdrop_px;
        let crop = if img_w > 0 && img_h > 0 {
            overlay_rect_to_pixels(selected, overlay, img_w, img_h)
        } else {
            // Live overlay (macOS): map points to physical pixels.
            let ppp = ctx.pixels_per_point();
            even_screen_rect(
                (selected.min.x * ppp).round() as i32,
                (selected.min.y * ppp).round() as i32,
                (selected.width() * ppp).round() as i32,
                (selected.height() * ppp).round() as i32,
            )
            .as_whxy()
        };
        self.selected_region = Some(selected);
        self.last_region = Some(selected);
        self.selected_screen_rect = Some(crop);
        // E96 — Ctrl+Z in the HUD steps back through these.
        self.region_history.push(selected);
        if self.region_history.len() > 32 {
            self.region_history.remove(0);
        }
        // E95 — first-run HUD hints hide after 3 completed picks.
        self.region_pick_count = self.region_pick_count.saturating_add(1);
        self.persist_session();

        let kind = self.pending_region_kind.take();
        // Window pick: remember the chosen window so CaptureTarget::Window
        // aims at it next run (exit_region_overlay clears the hover state).
        // A dead-space monitor pick has no window to aim at — skip adoption.
        if matches!(
            kind,
            Some(RegionPickKind::WindowPick) | Some(RegionPickKind::WindowRecord)
        ) && !self.window_pick_hover_monitor
        {
            if let Some((name, ..)) = self.window_pick_hover.take() {
                self.window_app = name;
                self.persist_session();
            }
        }
        self.exit_region_overlay(ctx);

        match kind {
            Some(RegionPickKind::Screenshot) | Some(RegionPickKind::WindowPick) => {
                if let Some(snap) = self.region_snap_path.clone() {
                    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                    let dest = self.save_dir.join(format!("screenshot_{}.jpg", timestamp));
                    let (w, h, x, y) = crop;
                    let region = ScreenRect { x, y, w, h };
                    // Crop from the hidden-window snap only. A live re-grab
                    // here would include our own restored window.
                    match crop_image_file(&snap, &dest, region) {
                        Ok(()) => {
                            // Keep the snap — a re-pick within 2 s reuses it
                            // as a fresh backdrop (J230).
                            self.finish_screenshot(ctx, Ok(dest));
                        }
                        Err(e) => {
                            self.show_window(ctx);
                            self.show_toast(format!("❌ {e}"));
                        }
                    }
                } else {
                    self.show_toast("❌ Region snap missing");
                }
            }
            Some(RegionPickKind::Record) | Some(RegionPickKind::WindowRecord) => {
                if kind == Some(RegionPickKind::WindowRecord) {
                    // The pick already resolved to exact pixels — record the
                    // rect, not the window name (it may move mid-record).
                    self.capture_target = CaptureTarget::Region;
                }
                // Keep the snap — a re-pick within 2 s reuses it (J230).
                self.pending_arm_record = true;
                ctx.request_repaint();
            }
            None => {}
        }
    }

    /// Apply a finished screenshot (from channel or disk marker).
    fn finish_screenshot(&mut self, ctx: &egui::Context, result: Result<PathBuf, String>) {
        self.screenshot_in_flight = false;
        self.wake_shared.still_busy.store(false, Ordering::SeqCst);
        match result {
            Ok(shot_file) => {
                // E275 — local opt-in counter only; nothing leaves the box.
                if self.stats_opt_in {
                    self.stat_shots_ok = self.stat_shots_ok.saturating_add(1);
                }
                self.shutter_flash_until = Some(Instant::now() + Duration::from_millis(140));
                if self.shutter_sound {
                    crate::platform::shutter_click();
                }
                // Snipping-Tool rule: a fresh capture is immediately pasteable —
                // no need to open the app and press Ctrl+C first. The card
                // reports the copy result honestly ("Copied" vs "Captured").
                let copied = self.copy_image_to_clipboard(&shot_file);
                if self.clipboard_only {
                    // Clipboard-only stills never touch the library — copy,
                    // discard the file, skip Review.
                    let _ = std::fs::remove_file(&shot_file);
                    self.last_capture = None;
                    self.refresh_library();
                    self.show_window(ctx);
                    self.persist_session();
                    self.show_toast(if copied {
                        "📋 Copied — file not saved"
                    } else {
                        "❌ Copy to clipboard failed"
                    });
                    return;
                }
                // D70 — auto-open in Review is opt-out; off means the
                // capture lands silently (toast still fires). The editor is
                // staged either way so a later visit finds it loaded.
                if !self.is_annotating {
                    if self.auto_open_review {
                        self.open_still_from_path(shot_file.clone());
                    } else {
                        self.load_still_from_path(&shot_file);
                    }
                }
                self.refresh_library();
                self.toast_message = None;
                self.capture_toast = Some((shot_file.clone(), Instant::now(), copied));
                self.last_capture = Some(LastCapture::Still(shot_file.clone()));
                self.show_window(ctx);
                self.persist_session();
                let ready = if copied {
                    "📋 Screenshot copied — ready in Still"
                } else {
                    "Screenshot ready in Still"
                };
                if self.is_annotating {
                    self.show_toast("Screenshot saved — finish this markup first");
                } else {
                    // D72 — warn when a still can't go straight into Discord.
                    let mb = std::fs::metadata(&shot_file)
                        .map(|m| m.len() as f64 / 1e6)
                        .unwrap_or(0.0);
                    if mb > 8.0 {
                        self.show_toast(format!(
                            "⚠ {mb:.1} MB — over Discord's 8 MB limit · Still ⋯ → Export for Discord"
                        ));
                    } else {
                        self.show_toast(ready);
                    }
                }
            }
            Err(e) => {
                if self.stats_opt_in {
                    self.stat_shots_fail = self.stat_shots_fail.saturating_add(1);
                }
                let msg = e.to_string();
                self.show_window(ctx);
                self.show_toast(format!("❌ {msg}"));
                #[cfg(target_os = "macos")]
                if msg.contains("Screen Recording") || msg.contains("empty") {
                    // Capture failed on permission — surface the guidance modal,
                    // but do not force-open System Settings (user-initiated only).
                    self.screen_permission_ok = false;
                    self.screen_perm_modal = true;
                    self.screen_perm_probe_ok = Some(false);
                    self.persist_session();
                }
            }
        }
        // E64 — a hotkey double-tap during this shot asked for the picker.
        if self.region_after_still {
            self.region_after_still = false;
            if !self.is_annotating && !self.is_selecting_region {
                self.start_region_pick(ctx, RegionPickKind::Screenshot);
            }
        }
    }

    /// Disk marker wins over a missed channel — call early every frame.
    fn poll_pending_still(&mut self, ctx: &egui::Context) {
        if self.is_selecting_region || self.region_snap_rx.is_some() {
            return;
        }
        if let Some(result) = take_pending_still() {
            self.finish_screenshot(ctx, result);
        }
    }

    /// E210 — `vibecap poke <cmd>` handoff: a CLI process (or a second
    /// launch that lost the instance lock) drops a command marker here.
    fn poll_pending_cmd(&mut self, ctx: &egui::Context) {
        let Some(cmd) = app::take_pending_cmd() else {
            return;
        };
        match cmd.as_str() {
            "show" => self.show_window(ctx),
            "hide" => self.hide_to_tray(ctx),
            "screenshot" => self.trigger_capture(ctx, true),
            "record" => self.on_tray_action(ctx, TrayAction::ToggleRecord),
            "stop" => {
                if self.is_recording {
                    self.stop_recording(ctx);
                } else {
                    self.show_toast("Not recording");
                }
            }
            other => self.show_toast(format!("Unknown poke: {other}")),
        }
    }

    /// E187 — deep links: `vibecap://feedback/<id>` selects that thread in
    /// the Inbox; `vibecap://open` just summons the studio. Unknown routes
    /// land a toast rather than failing silently.
    fn poll_pending_deep(&mut self, ctx: &egui::Context) {
        let Some(url) = app::take_pending_deep() else {
            return;
        };
        let route = url.trim_start_matches("vibecap://").trim_matches('/');
        if let Some(id) = route.strip_prefix("feedback/") {
            self.scan_feedback_requests();
            let id = id.to_string();
            if self.feedback_requests.iter().any(|r| r.id == id) {
                self.current_tab = AppTab::Feedback;
                self.feedback_selected = Some(id.clone());
                self.feedback_user_picked = true;
                self.show_window(ctx);
                self.show_toast(format!("Opened thread {id}"));
            } else {
                self.current_tab = AppTab::Feedback;
                self.show_window(ctx);
                self.show_toast(format!("Thread {id} not found — showing Inbox"));
            }
        } else if route == "open" || route.is_empty() {
            self.show_window(ctx);
        } else {
            self.show_toast(format!("Unknown link: {url}"));
            self.show_window(ctx);
        }
    }

    /// E211 — watch-folder intake: files that settle (mtime ≥2s ago) move
    /// into the media dir and land in Library.
    fn tick_watch_folder(&mut self) {
        let dir = self.watch_folder.trim().to_string();
        // E250 — while parked the pump's slow lane owns the sweep; a tick
        // here would race its renames and report false failures.
        if dir.is_empty() || self.wake_shared.parked.load(Ordering::SeqCst) {
            return;
        }
        let now = std::time::Instant::now();
        if self
            .watch_last_scan
            .map(|t| now.duration_since(t) < std::time::Duration::from_secs(3))
            .unwrap_or(false)
        {
            return;
        }
        self.watch_last_scan = Some(now);
        let media = self.save_dir.clone();
        let (moved, failed) = crate::app::library::watch_sweep(&PathBuf::from(dir), &media);
        if moved > 0 {
            self.refresh_library();
            self.show_toast(format!(
                "Watch folder imported {moved} file{}",
                if moved == 1 { "" } else { "s" }
            ));
        }
        if failed > 0 {
            self.show_toast(format!(
                "⚠ Watch folder: {failed} file(s) could not be moved"
            ));
        }
    }

    /// E74 — clipboard watcher: while enabled, a *new* image on the system
    /// clipboard is saved to media and opened in Still. The sequence counter
    /// keeps each poll to one user32 read; `clipboard_seq_seen` is refreshed
    /// after our own `set_image` calls so captures don't echo back in.
    fn tick_clipboard_watcher(&mut self, ctx: &egui::Context) {
        if !self.clipboard_watcher
            || self.is_annotating
            || self.is_selecting_region
            || self.screenshot_in_flight
            || self.region_snap_rx.is_some()
            || self.is_recording
            || self.recording_arming
        {
            return;
        }
        let now = Instant::now();
        if self
            .clipboard_poll_at
            .map(|t| now.duration_since(t) < Duration::from_millis(800))
            .unwrap_or(false)
        {
            return;
        }
        self.clipboard_poll_at = Some(now);
        let seq = crate::platform::clipboard_seq();
        if seq == 0 || seq == self.clipboard_seq_seen {
            return;
        }
        self.clipboard_seq_seen = seq;
        let Ok(mut board) = arboard::Clipboard::new() else {
            return;
        };
        let Ok(img) = board.get_image() else {
            return;
        };
        let Some(rgba) =
            image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.to_vec())
        else {
            return;
        };
        let seq_n = app::naming::next_seq(&self.save_dir, "");
        let stem = app::format_capture_stem(&self.name_pattern, Some("clipboard"), seq_n);
        let path = self.save_dir.join(format!("{stem}.png"));
        if image::DynamicImage::ImageRgba8(rgba).save(&path).is_err() {
            self.show_toast("❌ Clipboard image could not be saved");
            return;
        }
        self.refresh_library();
        self.last_capture = Some(LastCapture::Still(path.clone()));
        self.show_window(ctx);
        self.open_still_from_path(path);
        self.show_toast("📋 Clipboard image opened in Still");
    }

    /// E171 — stale `.ffmpeg.log` / `.clean.mp4` / `frames_temp*` orphans,
    /// swept on a 10-minute cadence (idle >1 h inside, live writes safe).
    fn tick_sidecar_sweep(&mut self) {
        let now = Instant::now();
        if self
            .last_sidecar_sweep
            .map(|t| now.duration_since(t) < Duration::from_secs(600))
            .unwrap_or(false)
        {
            return;
        }
        self.last_sidecar_sweep = Some(now);
        let removed = crate::app::library::sweep_stale_sidecars(&self.save_dir);
        if removed > 0 {
            self.refresh_library();
        }
    }

    fn show_annotation(&mut self, ui: &mut egui::Ui) {
        if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.is_annotating = false;
            self.current_tab = if self.img_edit_file.is_some() {
                AppTab::Still
            } else {
                AppTab::Capture
            };
            return;
        }
        // Ctrl+Z undo / Ctrl+Shift+Z or Ctrl+Y redo — skip while a text
        // field owns the keyboard (its own undo should win).
        if !ui.ctx().wants_keyboard_input() {
            let (undo, redo) = ui.ctx().input(|i| {
                let cmd = i.modifiers.command || i.modifiers.ctrl;
                (
                    cmd && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
                    (cmd && i.modifiers.shift && i.key_pressed(egui::Key::Z))
                        || (cmd && i.key_pressed(egui::Key::Y)),
                )
            });
            if undo {
                self.annotation_do_undo();
            } else if redo {
                self.annotation_do_redo();
            }
        }
        ui.horizontal(|ui| {
            ui.heading(
                RichText::new("Annotation Studio")
                    .color(theme::ACCENT())
                    .strong(),
            );
            ui.separator();
            ui.radio_value(&mut self.current_tool, AnnotationTool::Pen, "✏ Pen");
            ui.radio_value(&mut self.current_tool, AnnotationTool::Arrow, "➡ Arrow");
            ui.radio_value(&mut self.current_tool, AnnotationTool::Rectangle, "🔲 Rect");
            ui.radio_value(&mut self.current_tool, AnnotationTool::Ellipse, "⬭ Ellipse");
            ui.radio_value(
                &mut self.current_tool,
                AnnotationTool::Highlight,
                "🖍 Highlight",
            );
            ui.radio_value(&mut self.current_tool, AnnotationTool::Text, "🔤 Text");
            ui.radio_value(&mut self.current_tool, AnnotationTool::Blur, "💧 Blur");
            ui.radio_value(&mut self.current_tool, AnnotationTool::Spotlight, "🔦 Spot");
            ui.radio_value(
                &mut self.current_tool,
                AnnotationTool::Measure,
                "📐 Measure",
            );
            ui.radio_value(
                &mut self.current_tool,
                AnnotationTool::StepBadge,
                "🔢 Badge",
            );

            ui.separator();
            ui.color_edit_button_srgba(&mut self.current_color);
            ui.add(egui::Slider::new(&mut self.current_stroke_width, 1.0..=10.0).text("Size"));

            if self.current_tool == AnnotationTool::Text {
                ui.separator();
                ui.label("Text:");
                ui.text_edit_singleline(&mut self.pending_text);
            }
            // E30 — badge look presets apply to the next badge and restyle
            // the selected one.
            if self.current_tool == AnnotationTool::StepBadge {
                ui.separator();
                for (v, glyph) in [(0u8, "●"), (1, "○"), (2, "■"), (3, "□")] {
                    if ui.selectable_label(self.badge_style == v, glyph).clicked() {
                        self.badge_style = v;
                        if let Some(i) = self.annotation_selected {
                            if matches!(
                                self.annotation_actions.get(i).map(|a| a.tool),
                                Some(AnnotationTool::StepBadge)
                            ) {
                                self.annotation_push_undo();
                                if let Some(a) = self.annotation_actions.get_mut(i) {
                                    a.badge_style = v;
                                }
                            }
                        }
                    }
                }
            }

            ui.separator();
            if ui.button("↩ Undo").on_hover_text("Ctrl+Z").clicked() {
                self.annotation_do_undo();
            }
            if ui
                .button("↪ Redo")
                .on_hover_text("Ctrl+Shift+Z / Ctrl+Y")
                .clicked()
            {
                self.annotation_do_redo();
            }
            if ui.button("🗑 Clear").clicked() {
                self.annotation_push_undo();
                self.annotation_actions.clear();
                self.annotation_selected = None;
                self.step_counter = 1;
            }

            ui.separator();
            let voice_btn_text = if self.is_recording_voice_memo {
                RichText::new("🔴 Stop Voice Note")
                    .color(theme::ON_SOLID())
                    .strong()
            } else {
                RichText::new("🎙 Voice Note")
                    .color(theme::SUCCESS())
                    .strong()
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

            if ui
                .button(
                    RichText::new("💾 Save & Close")
                        .color(theme::ACCENT_INK())
                        .strong(),
                )
                .clicked()
            {
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
                        let stem = shot
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let target = shot.with_file_name(format!("{}_annotated.png", stem));
                        annotated_path = target.display().to_string();
                        self.pending_annotated_save = Some((target, Instant::now()));
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Screenshot);
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
            ui.label(
                RichText::new("Optional note to attach with this capture:")
                    .small()
                    .color(theme::TEXT_MUTED()),
            );
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
            let canvas = response.rect;
            let tex_px_w = tex.size()[0].max(1) as f32;
            painter.image(
                tex.id(),
                response.rect,
                egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                theme::ON_SOLID(),
            );

            let draw_action = |painter: &egui::Painter, action: &AnnotationAction| {
                if action.points.is_empty() {
                    return;
                }
                let mut color = action.color;
                if action.tool == AnnotationTool::Highlight {
                    color = color.linear_multiply(0.4);
                }
                let stroke = Stroke::new(action.stroke_width, color);

                match action.tool {
                    AnnotationTool::Pen | AnnotationTool::Highlight => {
                        for i in 1..action.points.len() {
                            painter.line_segment([action.points[i - 1], action.points[i]], stroke);
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
                    AnnotationTool::Ellipse => {
                        if action.points.len() >= 2 {
                            let start = action.points[0];
                            let end = *action.points.last().unwrap();
                            let rect = Rect::from_two_pos(start, end);
                            painter.add(egui::Shape::Ellipse(egui::epaint::EllipseShape::stroke(
                                rect.center(),
                                Vec2::new(rect.width() / 2.0, rect.height() / 2.0),
                                stroke,
                            )));
                        }
                    }
                    AnnotationTool::Blur => {
                        if action.points.len() >= 2 {
                            let start = action.points[0];
                            let end = *action.points.last().unwrap();
                            let rect = Rect::from_two_pos(start, end);
                            painter.rect_filled(rect, 0.0, theme::OVERLAY_BLUR());
                            painter.rect_stroke(
                                rect,
                                0.0,
                                Stroke::new(1.0_f32, theme::NEUTRAL_STROKE()),
                            );
                        }
                    }
                    AnnotationTool::Spotlight => {
                        if action.points.len() >= 2 {
                            let hole = Rect::from_two_pos(
                                action.points[0],
                                *action.points.last().unwrap(),
                            );
                            let dim = Color32::from_black_alpha(120);
                            for band in [
                                Rect::from_min_max(canvas.min, Pos2::new(canvas.max.x, hole.min.y)),
                                Rect::from_min_max(Pos2::new(canvas.min.x, hole.max.y), canvas.max),
                                Rect::from_min_max(
                                    Pos2::new(canvas.min.x, hole.min.y),
                                    Pos2::new(hole.min.x, hole.max.y),
                                ),
                                Rect::from_min_max(
                                    Pos2::new(hole.max.x, hole.min.y),
                                    Pos2::new(canvas.max.x, hole.max.y),
                                ),
                            ] {
                                painter.rect_filled(band, 0.0, dim);
                            }
                            painter.rect_stroke(
                                hole,
                                0.0,
                                Stroke::new(1.0_f32, theme::NEUTRAL_STROKE()),
                            );
                        }
                    }
                    AnnotationTool::Measure => {
                        if action.points.len() >= 2 {
                            let start = action.points[0];
                            let end = *action.points.last().unwrap();
                            painter.line_segment([start, end], stroke);
                            let px_scale = tex_px_w / canvas.width().max(1.0);
                            let dist = (end - start).length() * px_scale;
                            let deg = (end - start).angle().to_degrees().abs() as i32;
                            painter.text(
                                end + Vec2::new(8.0, -22.0),
                                Align2::LEFT_TOP,
                                format!("{dist:.0} px · {deg}°"),
                                FontId::proportional(12.0),
                                action.color,
                            );
                        }
                    }
                    AnnotationTool::Text => {
                        let pos = action.points[0];
                        painter.rect_filled(
                            Rect::from_min_size(
                                pos - Vec2::new(4.0, 2.0),
                                Vec2::new(action.text_content.len() as f32 * 10.0 + 8.0, 22.0),
                            ),
                            4.0,
                            theme::OVERLAY_LABEL(),
                        );
                        painter.text(
                            pos,
                            Align2::LEFT_TOP,
                            &action.text_content,
                            FontId::proportional(16.0),
                            action.color,
                        );
                    }
                    AnnotationTool::StepBadge => {
                        // E30 — preview mirrors the baker's style presets.
                        let pos = action.points[0];
                        let edge = Stroke::new(2.0_f32, action.color);
                        match action.badge_style {
                            1 => {
                                painter.circle_stroke(pos, 14.0, edge);
                            }
                            2 => {
                                painter.rect_filled(
                                    Rect::from_center_size(pos, Vec2::splat(24.0)),
                                    2.0,
                                    action.color,
                                );
                            }
                            3 => {
                                painter.rect_stroke(
                                    Rect::from_center_size(pos, Vec2::splat(24.0)),
                                    2.0,
                                    edge,
                                );
                            }
                            _ => {
                                painter.circle_filled(pos, 14.0, action.color);
                            }
                        }
                        let ink = if matches!(action.badge_style, 1 | 3) {
                            action.color
                        } else {
                            theme::ACCENT_INK()
                        };
                        painter.text(
                            pos,
                            Align2::CENTER_CENTER,
                            action.badge_number.to_string(),
                            FontId::proportional(14.0),
                            ink,
                        );
                    }
                    AnnotationTool::Sticker => {} // modal: paste lives in Still tab
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
                    self.annotation_push_undo();
                    let action = AnnotationAction {
                        tool: self.current_tool,
                        color: self.current_color,
                        stroke_width: self.current_stroke_width,
                        points: vec![pos],
                        text_content: self.pending_text.clone(),
                        badge_number: self.step_counter,
                        badge_style: self.badge_style,
                        sticker: None,
                    };

                    if self.current_tool == AnnotationTool::Text
                        || self.current_tool == AnnotationTool::StepBadge
                    {
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
                        // Shift snaps arrows to 15° and rects/blur to squares.
                        let pos = if ui.input(|i| i.modifiers.shift) {
                            snap_annotation_point(action.tool, action.points[0], pos)
                        } else {
                            pos
                        };
                        action.points.push(pos);
                    }
                }
            }
            if response.drag_stopped() {
                if let Some(mut action) = self.current_action.take() {
                    // E32 — a near-straight freehand becomes a clean line.
                    if action.tool == AnnotationTool::Pen {
                        crate::app::straighten_if_near_line(&mut action.points);
                    }
                    self.annotation_actions.push(action);
                }
            }
        }
    }
}

impl eframe::App for VibecapApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        app::thumbs::cleanup_frames_temp(&self.save_dir);
        // E189 — quitting on the Inbox also advances the seen watermark.
        if self.current_tab == AppTab::Feedback {
            self.inbox_seen_stamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        }
        self.persist_session();
        self.flush_session_now();
        // E265 — never leave a stale studio-record heartbeat behind.
        app::agent_record::clear_gui_record_state();
        app::instance::release_gui_lock();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Hand workers a wake handle (Context is an Arc clone — cheap).
        self.ui_ctx = Some(ctx.clone());
        // Mirror capture settings for the pump thread's hidden-capture path.
        if let Ok(mut cfg) = self.wake_shared.cfg.lock() {
            *cfg = Some(CaptureCfg {
                save_dir: self.save_dir.clone(),
                name_pattern: self.name_pattern.clone(),
                monitor: self.capture_monitor,
                draw_mouse: self.draw_mouse,
                target: self.capture_target,
                front_app: self.last_front_app.clone(),
                delay_secs: self.capture_delay_secs,
            });
        }
        // E250 — mirror the periodic-work flags the pump's slow lane needs.
        if let Ok(mut w) = self.wake_shared.watch_dir.lock() {
            let d = self.watch_folder.trim();
            *w = (!d.is_empty()).then(|| PathBuf::from(d));
        }
        self.wake_shared
            .follow_os
            .store(self.theme_follow_os, Ordering::SeqCst);
        // E10 — pump polls GetAsyncKeyState(Esc) while the overlay is up so
        // a focus-loss can't orphan the pick.
        self.wake_shared
            .region_open
            .store(self.is_selecting_region, Ordering::SeqCst);
        // E23 — animation sites read the flag without app plumbing.
        theme::set_reduce_motion(self.reduce_motion);
        // E3 — celestial accent-hue offset → aurora stop rotation.
        theme::set_aurora_hue(self.aurora_hue);
        // Remember which Review editor was last used (rail Review returns here).
        if matches!(self.current_tab, AppTab::Still | AppTab::Clip) {
            self.last_review_tab = Some(self.current_tab);
        }
        // Feed the back stack on any tab change (rail, palette, tray,
        // auto-advance). Direct nav sets prev_tab itself so it isn't re-pushed.
        if self.current_tab != self.prev_tab {
            // E189 — leaving the Inbox advances the "new since last visit"
            // watermark so arrivals seen this visit don't stay new forever.
            if self.prev_tab == AppTab::Feedback {
                self.inbox_seen_stamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                self.persist_session();
            }
            self.tab_back.push(self.prev_tab);
            if self.tab_back.len() > 32 {
                self.tab_back.remove(0);
            }
            self.tab_fwd.clear();
            // E33 — window-size memory per stage: stash the size we had on
            // the stage we're leaving, restore the remembered one (if any)
            // for the stage we're entering. Ignore park/restore leftovers.
            if self.pre_capture_outer.is_none()
                && self.window_size.x >= 640.0
                && self.window_size.y >= 400.0
            {
                self.window_sizes.insert(
                    self.prev_tab.key().to_string(),
                    [self.window_size.x, self.window_size.y],
                );
                if let Some(&[w, h]) = self.window_sizes.get(self.current_tab.key()) {
                    if (w - self.window_size.x).abs() > 20.0
                        || (h - self.window_size.y).abs() > 20.0
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                            w, h,
                        )));
                    }
                }
            }
            // E50 — directional slide: later rail stages enter from the
            // right, earlier from the left (skipped under reduce-motion).
            if !theme::reduce_motion() {
                let dir = match self.current_tab.rail_pos().cmp(&self.prev_tab.rail_pos()) {
                    std::cmp::Ordering::Greater => 1.0,
                    std::cmp::Ordering::Less => -1.0,
                    std::cmp::Ordering::Equal => 0.0,
                };
                self.tab_slide = (dir != 0.0).then_some((Instant::now(), dir));
            }
            self.prev_tab = self.current_tab;
        }
        // Track window size for session restore (skip park / tiny restore leftovers).
        if self.pre_capture_outer.is_none() {
            let s = ctx.screen_rect().size();
            if s.x >= 640.0 && s.y >= 400.0 {
                self.window_size = s;
            }
        }

        // First frame: honor --hidden (window already created; hide after paint setup).
        if self.start_hidden {
            self.start_hidden = false;
            self.hide_to_tray(ctx);
        }

        // Always reclaim a finished screenshot first (file marker is authoritative).
        self.poll_pending_still(ctx);
        // E210 — CLI pokes land as a marker file; dispatch each frame.
        self.poll_pending_cmd(ctx);
        // E187 — vibecap:// deep links (same marker mechanism).
        self.poll_pending_deep(ctx);
        // E222 — crash-recovery drains: orphan remux result + annotation
        // draft debounce.
        self.drain_recovery();
        self.tick_review_draft();
        // E206 — mirror the configured double-click action for both drains.
        tray_ui::set_dblclick_is_open(self.tray_dblclick == "open");
        // E211 — watch-folder intake poll (3 s cadence inside).
        self.tick_watch_folder();
        // E250 — files imported by the pump while the studio was parked.
        let parked_moved = self.wake_shared.watch_moved.swap(0, Ordering::SeqCst);
        if parked_moved > 0 {
            self.refresh_library();
            self.show_toast(format!(
                "Watch folder imported {parked_moved} file{}",
                if parked_moved == 1 { "" } else { "s" }
            ));
        }
        // E225 — OS dark-mode follow (3 s registry poll inside).
        self.tick_os_theme(ctx);
        // E74 — clipboard watcher (800 ms seq poll inside; the copy cost is
        // one user32 read until a change actually lands).
        self.tick_clipboard_watcher(ctx);
        // E36 — poll the recorder's meter output ~4×/s while audio is on.
        if self.is_recording && self.capture_audio {
            if self
                .audio_meter_at
                .map(|t| t.elapsed() > Duration::from_millis(250))
                .unwrap_or(true)
            {
                self.audio_meter_at = Some(Instant::now());
                if let Some(mp4) = &self.current_mp4_file {
                    let log = mp4.with_extension("ffmpeg.log");
                    if let Some(db) = crate::platform::audio_peak_db(&log) {
                        self.audio_level_db = db;
                    }
                }
            }
        } else {
            self.audio_level_db = -90.0;
            self.audio_meter_at = None;
        }
        // E73 — scheduled still: fires as soon as its Instant passes and no
        // capture/selection owns the screen.
        if let Some(t) = self.scheduled_shot_at {
            if Instant::now() >= t
                && !self.screenshot_in_flight
                && !self.is_selecting_region
                && self.region_snap_rx.is_none()
            {
                self.scheduled_shot_at = None;
                self.scheduled_fire_now = true;
                self.trigger_capture(ctx, true);
            }
        }
        // E171 — stale sidecar sweep (10 min cadence inside).
        self.tick_sidecar_sweep();
        // E159 — hover-scrub strip results land as textures.
        self.drain_scrub(ctx);

        // Startup Screen Recording check: cheap, prompt-free preflight first.
        // Granted users are never asked again; the system dialog appears only
        // while macOS state is still undetermined.
        if self.screen_permission_pending {
            self.screen_permission_pending = false;
            if screen_capture_allowed() {
                // Granted (or self-healed after a reinstall) — no dialog, no modal.
                if !self.screen_permission_ok {
                    self.show_toast("Screen Recording permission is active");
                }
                self.screen_permission_ok = true;
                self.screen_permission_prompted = true;
                self.screen_perm_modal = false;
                self.persist_session();
            } else {
                #[cfg(target_os = "macos")]
                {
                    // Not granted: ask the system once (silent if already denied).
                    self.screen_perm_probe_ok = None;
                    let ctx_probe = ctx.clone();
                    let (tx, rx) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        let ok = request_screen_recording_access().unwrap_or(false);
                        let _ = tx.send(ok);
                        ctx_probe.request_repaint();
                    });
                    self.screen_perm_rx = Some(rx);
                }
                #[cfg(not(target_os = "macos"))]
                {
                    self.screen_permission_ok = true;
                }
            }
        }

        // Drain the permission request. Never auto-open System Settings here —
        // the modal and Settings tab carry user-initiated buttons instead.
        if let Some(rx) = &self.screen_perm_rx {
            if let Ok(ok) = rx.try_recv() {
                self.screen_perm_rx = None;
                self.screen_perm_probe_ok = Some(ok);
                self.screen_permission_prompted = true;
                if ok {
                    self.screen_permission_ok = true;
                    self.screen_perm_modal = false;
                    self.show_toast("Screen Recording allowed — capture is ready");
                } else {
                    self.screen_permission_ok = false;
                    if self.wizard_open || !self.wizard_done {
                        // First run: block until acknowledged so capture is not
                        // silently broken from day one.
                        self.screen_perm_modal = true;
                    } else {
                        self.show_toast(
                            "⚠️ Screen Recording is off — captures will be empty. Fix in Settings → Permissions.",
                        );
                    }
                }
                self.persist_session();
            }
        }

        // Close button → hide to tray (multi-agent + human: keep app alive in menu bar).
        // Tray "Quit" sets allow_exit so the process can terminate.
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.tray.is_some() && !self.allow_exit {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.hide_to_tray(ctx);
                self.show_toast(if cfg!(target_os = "macos") {
                    "Hidden to tray — click the menu bar icon to show again."
                } else {
                    "Minimized to the taskbar — click the taskbar or tray icon to show again."
                });
            }
        }

        if self.silent_mode {
            self.shutter_flash_until = None;
        }
        // E25 — the HUD ⚡ chip suppresses the flash for one shot only.
        let no_flash_once = ctx
            .data_mut(|d| d.get_temp::<bool>(egui::Id::new("hud_no_flash")))
            .unwrap_or(false);
        if no_flash_once {
            self.shutter_flash_until = None;
            ctx.data_mut(|d| d.remove::<bool>(egui::Id::new("hud_no_flash")));
        }
        if let Some(until) = self.shutter_flash_until {
            if Instant::now() < until {
                let rect = ctx.screen_rect();
                ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("shutter_flash"),
                ))
                .rect_filled(rect, 0.0, Color32::from_white_alpha(90));
                ctx.request_repaint();
            } else {
                self.shutter_flash_until = None;
            }
        }

        self.handle_tray_actions(ctx);
        self.poll_frontmost_app();
        self.drain_record_spawn(ctx);
        self.drain_record_finalize(ctx);
        self.drain_voice_finalize();
        self.drain_library_scan();
        self.drain_recent_thumbs(ctx);
        self.drain_window_list();
        self.drain_region_snap(ctx);
        self.tick_live_backdrop(ctx);
        self.drain_filmstrip(ctx);
        self.drain_mp4_verify();
        self.drain_update_download();
        self.tick_session_write();
        // F126 — preview audio follows the flipbook: plays while the clip
        // preview runs, stops on pause / tab switch / clip unload.
        if self.preview_audio_playing
            && (!self.player_playing
                || self.current_tab != AppTab::Clip
                || self.preview_audio_path.is_none())
        {
            crate::platform::stop_audio_preview();
            self.preview_audio_playing = false;
        } else if self.player_playing
            && self.current_tab == AppTab::Clip
            && !self.preview_audio_playing
        {
            if let Some(wav) = self.preview_audio_path.clone() {
                crate::platform::play_audio_preview(&wav);
                self.preview_audio_playing = true;
            }
        }
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

        if let Ok(id) = std::env::var("VIBECAP_OPEN_FEEDBACK") {
            if !id.trim().is_empty() {
                std::env::remove_var("VIBECAP_OPEN_FEEDBACK");
                self.current_tab = AppTab::Feedback;
                self.feedback_selected = Some(id.trim().to_string());
                self.feedback_user_picked = true;
                self.scan_feedback_requests();
            }
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        for src in dropped {
            if let Some(name) = src.file_name() {
                let dest = self.save_dir.join(name);
                if std::fs::copy(&src, &dest).is_ok() {
                    self.show_toast(format!("Imported {}", name.to_string_lossy()));
                    self.refresh_library();
                }
            }
        }

        if self.is_recording
            && !self.is_annotating
            && ctx.input(|i| i.key_pressed(egui::Key::M) && !i.modifiers.any())
        {
            let t = self.recording_elapsed_secs() as f64;
            self.record_markers.push(t);
            self.show_toast(format!("Marker @ {t:.1}s"));
        }

        if !self.budget_warned {
            if let Some(reason) = budget_exceeded_reason(&default_live_dir().display().to_string())
            {
                self.budget_warned = true;
                self.show_toast(format!("Budget cap: {reason}"));
                if let Some(tray) = self.tray.as_mut() {
                    tray.force_live_state(
                        TrayLiveState::Idle,
                        self.feedback_pending_count,
                        self.last_error.as_deref(),
                    );
                }
            }
        }

        // E295 — 15 s samples of worst-cap utilization for the Inbox sparkline.
        let sample_due = self
            .budget_sample_at
            .map(|t| t.elapsed() >= Duration::from_secs(15))
            .unwrap_or(true);
        if sample_due {
            self.budget_sample_at = Some(Instant::now());
            let cfg = load_budget();
            let (frames, mb, mins) =
                crate::app::budget::live_usage_snapshot(&default_live_dir().display().to_string());
            let mut worst = 0.0f32;
            if cfg.max_frames > 0 {
                worst = worst.max(frames as f32 / cfg.max_frames as f32);
            }
            if cfg.max_mb > 0.0 {
                worst = worst.max((mb / cfg.max_mb) as f32);
            }
            if cfg.max_minutes > 0 {
                worst = worst.max((mins / cfg.max_minutes as f64) as f32);
            }
            self.budget_samples.push(worst.min(1.0));
            if self.budget_samples.len() > 120 {
                self.budget_samples.remove(0);
            }
        }

        self.sync_tray_recording_progress();
        // E233/E250 — repaint on demand: the loop only needs a fast cadence
        // while real work is in flight. The tray no longer forces a 10 fps
        // tick — its channels are pumped on the wake thread, and the pump's
        // slow lane nudges us for poke markers / watch-folder / OS theme.
        if self.is_recording
            || self.recording_arming
            || self.recording_finalizing
            || self.record_finalize_rx.is_some()
            || self.voice_finalize_rx.is_some()
            || self.library_scan_rx.is_some()
            || self.countdown_deadline.is_some()
            || self.feedback_pending_count > 0
            || self.screenshot_in_flight
            || self.screen_perm_modal
            || self.filmstrip_rx.is_some()
            || self.region_snap_rx.is_some()
            || self.is_selecting_region
            || self.mp4_verify_rx.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if self.retro.config().enabled {
            ctx.request_repaint_after(Duration::from_millis(500));
        } else if self.session_dirty.get() {
            // E244 — a pending debounced session write needs a tick within
            // its 500 ms window even when nothing else repaints.
            ctx.request_repaint_after(Duration::from_millis(500));
        } else if self.toast_message.is_some() || self.capture_toast.is_some() {
            // A toast/capture card expires on a timer — one slow tick retires
            // it; nothing else in the idle path needs a cadence.
            ctx.request_repaint_after(Duration::from_secs(1));
        }

        #[cfg(debug_assertions)]
        {
            let in_flight = app::capture_flow::capture_in_flight(
                self.screenshot_in_flight,
                self.is_recording,
                self.recording_arming,
                self.is_selecting_region,
                self.region_snap_rx.is_some(),
            );
            app::capture_flow::assert_unparked(&self.pre_capture_outer, in_flight);
        }

        // The region overlay just closed — its child viewport can steal focus
        // on teardown, so re-assert the studio's foreground for a few frames.
        if self.region_refocus_frames > 0 {
            self.region_refocus_frames -= 1;
            #[cfg(windows)]
            crate::platform::restore_studio_to_taskbar();
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            ctx.request_repaint();
        }

        // --- Capture HUD: region selection (thirds + handles + W×H) ---
        if self.is_selecting_region {
            // Window-pick mode: hit-test the top-level window under the cursor
            // (throttled — EnumWindows costs a few ms) so the HUD highlights
            // the pick target live.
            let picking_window = matches!(
                self.pending_region_kind,
                Some(RegionPickKind::WindowPick) | Some(RegionPickKind::WindowRecord)
            );
            if picking_window {
                let due = self
                    .window_pick_poll_at
                    .map(|t| t.elapsed() > Duration::from_millis(60))
                    .unwrap_or(true);
                if due {
                    self.window_pick_poll_at = Some(Instant::now());
                    // E90 — Alt held = drill into child windows under the cursor.
                    let drill = ctx.input(|i| i.modifiers.alt);
                    self.window_pick_hover = self.poll_window_pick(drill);
                    // E22 — kick a thumbnail grab for the newly-hovered hwnd.
                    if let Some(hwnd) = self.window_pick_hwnd {
                        let in_flight = self.pick_thumb_rx.as_ref().map(|(h, _)| *h) == Some(hwnd);
                        if !self.pick_thumbs.contains_key(&hwnd) && !in_flight {
                            let (tx, rx) = crossbeam_channel::bounded(1);
                            self.pick_thumb_rx = Some((hwnd, rx));
                            let ctx2 = ctx.clone();
                            std::thread::spawn(move || {
                                let _ = tx.send(crate::platform::window_thumb_rgba(hwnd, 192));
                                ctx2.request_repaint();
                            });
                        }
                    }
                }
                self.drain_pick_thumb(ctx);
                ctx.request_repaint_after(Duration::from_millis(60));
            }
            match show_region_selector(
                ctx,
                &mut self.region_start,
                &mut self.region_end,
                self.selected_screen_rect,
                self.last_region,
                self.region_backdrop.as_ref(),
                self.region_backdrop_rgba.as_ref(),
                self.region_backdrop_stale,
                &mut self.region_was_dragging,
                if picking_window {
                    self.window_pick_hover.as_ref()
                } else {
                    None
                },
                self.window_pick_hwnd.and_then(|h| self.pick_thumbs.get(&h)),
                &mut self.region_aspect_lock,
                &mut self.window_pick_cycle,
                self.region_dim,
                self.region_pick_count,
                &mut self.region_history,
                self.batch_shot_count,
                &mut self.hud_toolbar_bottom,
            ) {
                RegionHudResult::Continue => {}
                RegionHudResult::Confirmed { selected, overlay } => {
                    // A stale (pre-warm) backdrop is display-only — the crop
                    // must map to the snap that will actually be cropped.
                    // The selection stays up; the fresh backdrop lands in
                    // ~150 ms and the same rect still confirms.
                    if self.region_backdrop_stale {
                        self.show_toast("Refreshing…");
                    } else if ctx.input(|i| i.modifiers.shift)
                        && self.pending_region_kind == Some(RegionPickKind::Screenshot)
                        && self.region_snap_path.is_some()
                        && self.region_backdrop_px.0 > 0
                    {
                        // E71 — Shift+release keeps the overlay alive: each
                        // drag crops a fresh still from the same frozen
                        // backdrop. Esc ends the batch.
                        let (w, h, x, y) = overlay_rect_to_pixels(
                            selected,
                            overlay,
                            self.region_backdrop_px.0,
                            self.region_backdrop_px.1,
                        );
                        if let Some(snap) = self.region_snap_path.clone() {
                            let seq = app::naming::next_seq(&self.save_dir, "");
                            let stem = app::format_capture_stem(&self.name_pattern, None, seq);
                            let dest = self.save_dir.join(format!("{stem}.jpg"));
                            match crop_image_file(&snap, &dest, ScreenRect { x, y, w, h }) {
                                Ok(()) => {
                                    self.batch_shot_count += 1;
                                    self.last_capture = Some(LastCapture::Still(dest));
                                    self.region_history.push(selected);
                                    self.region_start = None;
                                    self.region_end = None;
                                    self.region_pick_count =
                                        self.region_pick_count.saturating_add(1);
                                    if self.shutter_sound {
                                        crate::platform::shutter_click();
                                    }
                                    self.persist_session();
                                }
                                Err(e) => self.show_toast(format!("❌ {e}")),
                            }
                        }
                    } else {
                        self.confirm_region_pick(ctx, selected, overlay);
                    }
                }
                RegionHudResult::Cancelled => {
                    // Keep the snap — a re-pick within 2 s reuses it (J230).
                    self.pending_region_kind = None;
                    self.exit_region_overlay(ctx);
                    // E25 — a cancelled pick must not eat the next flash.
                    ctx.data_mut(|d| d.remove::<bool>(egui::Id::new("hud_no_flash")));
                    self.show_toast("Region select cancelled");
                }
            }
            // Instant-overlay path: the selector is up showing the dim while
            // the freeze snap is in flight — exclude its HWND from that grab
            // or it freezes into its own backdrop. On Denied / persistent
            // NotFound, close the overlay so the worker's snap stays clean;
            // the delayed overlay returns when the snap arrives.
            #[cfg(windows)]
            if self.region_affinity
                && self.region_backdrop.is_none()
                && self.wake_shared.region_overlay_state.load(Ordering::SeqCst) == 0
            {
                match crate::platform::set_title_capture_excluded("Vibecap Region", true) {
                    crate::platform::ExcludeStatus::Applied => {
                        self.wake_shared
                            .region_overlay_state
                            .store(1, Ordering::SeqCst);
                    }
                    crate::platform::ExcludeStatus::Denied => {
                        self.wake_shared
                            .region_overlay_state
                            .store(2, Ordering::SeqCst);
                        self.is_selecting_region = false;
                    }
                    crate::platform::ExcludeStatus::NotFound => {
                        self.region_excl_attempts += 1;
                        if self.region_excl_attempts > 40 {
                            self.wake_shared
                                .region_overlay_state
                                .store(2, Ordering::SeqCst);
                            self.is_selecting_region = false;
                        } else {
                            // Viewport HWND not materialized yet — retry next frame.
                            ctx.request_repaint();
                        }
                    }
                }
            }
            return;
        }

        // --- Floating controller: arming countdown + active recording ---
        // Immediate viewport keeps the event loop awake while the main window is hidden.
        // Windows: opaque (transparent child viewports do not composite). Always
        // show it — tray-only stop is how recordings became unstoppable.
        #[cfg(windows)]
        if self.record_excluded && self.rec_bar_exclude_attempts < 8 {
            // The bar HWND can take a frame to exist — retry briefly, stop on
            // Applied or Denied (Denied never resolves on retry).
            self.rec_bar_exclude_attempts += 1;
            if !matches!(
                crate::platform::set_title_capture_excluded("Vibecap Recorder", true),
                crate::platform::ExcludeStatus::NotFound
            ) {
                self.rec_bar_exclude_attempts = 8;
            }
        }
        if self.is_recording || self.recording_arming || self.recording_finalizing {
            let mut builder = ViewportBuilder::default()
                .with_title("Vibecap Recorder")
                .with_decorations(false)
                .with_always_on_top()
                .with_inner_size([340.0, 68.0])
                .with_resizable(false)
                .with_transparent(!cfg!(target_os = "windows"))
                .with_visible(true);
            // B52 — reopen where the user left it.
            if let Some((x, y)) = self.rec_bar_pos {
                builder = builder.with_position([x as f32, y as f32]);
            }

            ctx.show_viewport_immediate(
                ViewportId::from_hash_of("recording_bar"),
                builder,
                |ctx, class| {
                    if class == egui::ViewportClass::Immediate {
                        // B52 — track the OS position so a drag persists.
                        if let Some(r) = ctx.input(|i| i.viewport().outer_rect) {
                            self.rec_bar_pos =
                                Some((r.min.x.round() as i32, r.min.y.round() as i32));
                        }
                        let bar_frame = Frame::none()
                            .fill(theme::SURFACE())
                            .rounding(theme::rounding_lg())
                            .stroke(Stroke::new(1.5_f32, theme::ACCENT()));

                        let bar_resp =
                            egui::CentralPanel::default()
                                .frame(bar_frame)
                                .show(ctx, |ui| {
                                    ui.vertical(|ui| {
                                        ui.add_space(2.0);
                                        ui.horizontal(|ui| {
                                            ui.add_space(6.0);

                                            let pulse =
                                                (ctx.input(|i| i.time) * 4.0).sin().abs() as f32;
                                            let dot_color = if self.recording_arming
                                                || self.recording_finalizing
                                            {
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
                                            } else if self.recording_finalizing {
                                                ui.label(
                                                    RichText::new("Saving…")
                                                        .strong()
                                                        .color(theme::TEXT()),
                                                );
                                            } else {
                                                let elapsed = self.recording_elapsed_secs();
                                                let mins = elapsed / 60;
                                                let secs = elapsed % 60;
                                                let status_text =
                                                    if self.is_paused { "PAUSED" } else { "REC" };
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

                                                    if !self.recording_arming
                                                        && !self.recording_finalizing
                                                    {
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

                                                        if crate::platform::pause_supported() {
                                                            let pause_icon = if self.is_paused {
                                                                "▶"
                                                            } else {
                                                                "⏸"
                                                            };
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

                                                        // E53 — chapter marker that works while the
                                                        // studio is parked; written to
                                                        // <clip>.markers.txt on finalize.
                                                        if ui
                                                        .button(
                                                            RichText::new("⚑")
                                                                .color(theme::ACCENT())
                                                                .strong(),
                                                        )
                                                        .on_hover_text(
                                                            "Drop a chapter marker at this moment",
                                                        )
                                                        .clicked()
                                                    {
                                                        let t =
                                                            self.recording_elapsed_secs() as f64;
                                                        self.record_markers.push(t);
                                                        self.show_toast(format!(
                                                            "Marker @ {t:.1}s"
                                                        ));
                                                    }
                                                    }
                                                },
                                            );
                                        });

                                        // E51 — what is being captured, at a glance.
                                        ui.horizontal(|ui| {
                                            ui.add_space(8.0);
                                            ui.label(
                                                RichText::new(self.record_source_line())
                                                    .size(9.5)
                                                    .color(theme::TEXT_MUTED()),
                                            );
                                        });
                                    });
                                });
                        // B52 — no titlebar: empty-space drags move the OS
                        // window; position persists via `rec_bar_pos`.
                        if bar_resp
                            .response
                            .interact(egui::Sense::drag())
                            .drag_started()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }
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

        // While a capture is in flight the main window is parked off-screen and
        // still receives ticks — poll the disk marker aggressively.
        if self.screenshot_in_flight {
            self.poll_pending_still(ctx);
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        if self.is_recording || self.is_recording_voice_memo || self.recording_arming {
            ctx.request_repaint();
        }

        // Hotkeys + tray actions queued by the wake pump. The pump owns the
        // OS event channels now and restores the window so this loop ticks —
        // draining here keeps working when the window was parked/minimized.
        let mut hotkey_shots = 0u32;
        let mut hotkey_recs = 0u32;
        let wake_events: Vec<WakeEvent> = self
            .wake_shared
            .queue
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default();
        for ev in wake_events {
            match ev {
                WakeEvent::Show => {
                    // Mid-still / mid-arm: restoring now would put the studio
                    // inside its own shot — the capture worker restores on
                    // done. While a recording owns the park an explicit Show
                    // is honored (the tail frames would show the restore that
                    // stop_recording performs anyway).
                    let capture_parked = self.screenshot_in_flight
                        || self.wake_shared.still_busy.load(Ordering::SeqCst)
                        || self.is_selecting_region
                        || self.region_snap_rx.is_some()
                        || self.recording_arming;
                    // is_selecting_region is checked separately: on the
                    // affinity path the studio was never parked, but a Show
                    // here would focus-steal the overlay mid-drag.
                    if (self.pre_capture_outer.is_none() || !capture_parked)
                        && !self.is_selecting_region
                    {
                        self.show_window(ctx);
                        #[cfg(windows)]
                        crate::platform::restore_studio_to_taskbar();
                    }
                }
                WakeEvent::ToggleWindow => self.toggle_window(ctx),
                WakeEvent::Screenshot => hotkey_shots += 1,
                WakeEvent::RecordToggle => hotkey_recs += 1,
                WakeEvent::PauseToggle => self.toggle_pause(),
                // E86 — per-mode hotkeys: region jumps into the picker,
                // window fires the window-target still path directly.
                WakeEvent::RegionStill => {
                    if !self.is_selecting_region && self.region_snap_rx.is_none() {
                        self.start_region_pick(ctx, RegionPickKind::Screenshot);
                    }
                }
                WakeEvent::WindowStill => {
                    let prev = self.capture_target;
                    self.capture_target = CaptureTarget::Window;
                    self.trigger_capture(ctx, true);
                    self.capture_target = prev;
                }
                WakeEvent::GifClip => self.trigger_gif_clip(ctx),
                WakeEvent::RegionCancel => {
                    if self.is_selecting_region {
                        self.pending_region_kind = None;
                        self.exit_region_overlay(ctx);
                        self.show_toast("Region select cancelled");
                    }
                }
                WakeEvent::Tray(action) => self.on_tray_action(ctx, action),
            }
        }
        for _ in 0..hotkey_shots {
            // E64 — second tap inside 600 ms means "I wanted a region":
            // mid-flight it arms the picker to open on restore; between
            // shots it jumps straight into the pick with last settings.
            let recent = self
                .last_shot_hk_at
                .map(|t| t.elapsed() < Duration::from_millis(600))
                .unwrap_or(false);
            self.last_shot_hk_at = Some(Instant::now());
            if !recent {
                self.trigger_capture(ctx, true);
            } else if self.screenshot_in_flight || self.region_snap_rx.is_some() {
                self.region_after_still = true;
            } else if !self.is_selecting_region {
                self.start_region_pick(ctx, RegionPickKind::Screenshot);
            }
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
        // S = screenshot · R = record · Z = undo delete · ⌘K/Ctrl+K = palette · ⌘I = inbox
        // Alt+←/→ = stage back/forward · Ctrl+1..5 = jump to a Loop stage.
        if !self.is_annotating && !self.palette_open && !self.wizard_open && !self.screen_perm_modal
        {
            let wants_text = ctx.wants_keyboard_input();
            let (
                press_s,
                press_r,
                press_z,
                press_palette,
                press_inbox,
                press_back,
                press_fwd,
                press_rail,
                press_help,
                stage_jump,
            ) = ctx.input(|i| {
                let mod_cmd = i.modifiers.command || i.modifiers.ctrl;
                let jump = if mod_cmd {
                    [
                        egui::Key::Num1,
                        egui::Key::Num2,
                        egui::Key::Num3,
                        egui::Key::Num4,
                        egui::Key::Num5,
                    ]
                    .iter()
                    .position(|k| i.key_pressed(*k))
                    .map(|p| p + 1)
                } else {
                    None
                };
                (
                    i.key_pressed(egui::Key::S) && !i.modifiers.any(),
                    i.key_pressed(egui::Key::R) && !i.modifiers.any(),
                    i.key_pressed(egui::Key::Z) && !i.modifiers.any(),
                    mod_cmd && i.key_pressed(egui::Key::K),
                    mod_cmd && i.key_pressed(egui::Key::I),
                    i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft),
                    i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight),
                    mod_cmd && i.key_pressed(egui::Key::B),
                    !wants_text
                        && (i.key_pressed(egui::Key::F1)
                            || (i.modifiers.shift && i.key_pressed(egui::Key::Slash))),
                    jump,
                )
            });
            if let Some(n) = stage_jump {
                let stages = LoopStage::all();
                if n <= stages.len() {
                    self.current_tab = self.tab_for_loop(stages[n - 1]);
                }
            } else if press_back {
                if let Some(t) = self.tab_back.pop() {
                    self.tab_fwd.push(self.current_tab);
                    self.prev_tab = t;
                    self.current_tab = t;
                }
            } else if press_fwd {
                if let Some(t) = self.tab_fwd.pop() {
                    self.tab_back.push(self.current_tab);
                    self.prev_tab = t;
                    self.current_tab = t;
                }
            } else if press_palette {
                self.palette_open = true;
                self.palette_query.clear();
                self.palette_selected = 0;
            } else if press_inbox {
                self.current_tab = AppTab::Feedback;
                self.scan_feedback_requests();
            } else if press_rail {
                self.rail_open = !self.rail_open;
            } else if press_help {
                self.cheatsheet_open = !self.cheatsheet_open;
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
            // Ctrl/Cmd+C outside Still copies the last fresh capture: still →
            // image pixels, clip → file path. Still has its own annotated
            // copy, and a focused text field keeps normal text copy.
            let press_copy = self.current_tab != AppTab::Still
                && !ctx.wants_keyboard_input()
                && ctx.input(|i| {
                    (i.modifiers.command || i.modifiers.ctrl)
                        && !i.modifiers.shift
                        && !i.modifiers.alt
                        && i.key_pressed(egui::Key::C)
                });
            if press_copy {
                match self.last_capture.clone() {
                    Some(LastCapture::Still(p)) => {
                        if !self.copy_image_to_clipboard(&p) {
                            self.show_toast("❌ Could not copy the image");
                        }
                    }
                    Some(LastCapture::Clip(p)) => {
                        if arboard::Clipboard::new()
                            .and_then(|mut b| b.set_text(p.display().to_string()))
                            .is_ok()
                        {
                            self.show_toast("📋 Clip path copied");
                        } else {
                            self.show_toast("❌ Could not copy the path");
                        }
                    }
                    None => {}
                }
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

        // Screen Recording gate (first open / failed capture) — above wizard.
        if self.screen_perm_modal {
            self.draw_screen_perm_modal(ctx);
            // Keep polling probe result while modal is open.
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }

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
            &self.palette_mru,
            &self.library_items,
            &self.saved_regions,
        ) {
            self.run_palette_action(ctx, action);
        }

        // ? / F1 shortcut cheat sheet
        ui::palette::show_cheatsheet(
            ctx,
            &mut self.cheatsheet_open,
            self.hotkey_shot_digit,
            self.hotkey_rec_digit,
            self.hotkey_pause_digit,
            self.hotkey_region_digit,
            self.hotkey_window_digit,
            self.hotkey_gif_digit,
            self.hotkey_prtscn,
        );

        // ── Stage rail — hidden by default; the funnel column is the home UX.
        //    ☰ in the header or Ctrl+B toggles it back on.
        // E39 — zen mode hides the rail too; Ctrl+B still flips the pref but
        // chrome stays out until zen is toggled off via the palette.
        if self.rail_open && !self.zen_mode && self.top_tabs {
            egui::TopBottomPanel::top("loop_tabs")
                .exact_height(40.0)
                .frame(
                    Frame::none()
                        .fill(theme::SURFACE())
                        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
                        .inner_margin(0.0),
                )
                .show(ctx, |ui| {
                    if theme::is_celestial() {
                        theme::paint_celestial_sky(ui.painter(), ui.clip_rect());
                    }
                    let rec_live =
                        self.is_recording || self.recording_arming || self.recording_finalizing;
                    if let Some(stage) = crate::ui::components::loop_tabs(
                        ui,
                        self.current_tab.to_loop(),
                        self.feedback_pending_count,
                        rec_live,
                        self.brand_logo.as_ref(),
                    ) {
                        self.current_tab = self.tab_for_loop(stage);
                    }
                });
        }
        if self.rail_open && !self.zen_mode && !self.top_tabs {
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
                    if theme::is_celestial() {
                        theme::paint_celestial_sky(ui.painter(), ui.clip_rect());
                    }
                    let rec_live =
                        self.is_recording || self.recording_arming || self.recording_finalizing;
                    if let Some(stage) = loop_rail(
                        ui,
                        self.current_tab.to_loop(),
                        self.feedback_pending_count,
                        rec_live,
                        self.brand_logo.as_ref(),
                        &mut self.rail_collapsed,
                        &mut self.rail_order,
                    ) {
                        self.current_tab = self.tab_for_loop(stage);
                    }
                });
        }

        // ── Status strip — only when something needs attention
        //    (recording, ffmpeg missing, inbox pending). No ambient trivia.
        {
            let snap = self.status_snapshot();
            let actionable =
                (snap.rec_live || !snap.ffmpeg_ok || snap.pending_inbox > 0) && !self.zen_mode;
            if actionable {
                egui::TopBottomPanel::bottom("status_strip")
                    .frame(
                        Frame::none()
                            .fill(theme::CANVAS())
                            .inner_margin(egui::Margin::symmetric(theme::SP_2, theme::SP_1)),
                    )
                    .show(ctx, |ui| {
                        if theme::is_celestial() {
                            theme::paint_celestial_sky(ui.painter(), ui.clip_rect());
                        }
                        // E37 — strip segments are jump links.
                        match status_strip(ui, &snap, self.current_tab != AppTab::Capture) {
                            Some(ui::StatusJump::Library) => self.current_tab = AppTab::Library,
                            Some(ui::StatusJump::Settings) => self.current_tab = AppTab::Settings,
                            Some(ui::StatusJump::Inbox) => {
                                self.current_tab = AppTab::Feedback;
                                self.scan_feedback_requests();
                            }
                            None => {}
                        }
                    });
            }
        }

        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(theme::CANVAS())
                    .inner_margin(theme::SP_4),
            )
            .show(ctx, |ui| {
                if theme::is_celestial() {
                    let clip = ui.clip_rect();
                    theme::paint_celestial_sky(ui.painter(), clip);
                    theme::paint_aurora_strip(
                        ui.painter(),
                        egui::Rect::from_min_size(clip.min, egui::Vec2::new(clip.width(), 2.0)),
                    );
                }
                if self.is_annotating {
                    self.show_annotation(ui);
                    return;
                }

                // ── Stage header ─────────────────────────────────────
                ui.horizontal(|ui| {
                    if ui::icon_btn(ui, "☰", "Stage rail (Ctrl+B)") {
                        self.rail_open = !self.rail_open;
                    }
                    // Ink logo mark — mono-ui topbar `.logo`.
                    let (logo, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(24.0), egui::Sense::hover());
                    let lp = ui.painter_at(logo);
                    if theme::is_celestial() {
                        theme::paint_aurora_button(&lp, logo, 6.0);
                    } else {
                        lp.rect_filled(logo, 6.0, theme::PRIMARY());
                    }
                    lp.text(
                        logo.center(),
                        egui::Align2::CENTER_CENTER,
                        "V",
                        egui::FontId::new(13.0, theme::font_bold()),
                        theme::PRIMARY_INK(),
                    );
                    ui.vertical(|ui| {
                        // E40 — the title names the thing you're working on,
                        // not just the stage.
                        let file_name = |p: &PathBuf| {
                            p.file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        };
                        let title: String = match self.current_tab {
                            AppTab::Clip => self
                                .edit_file
                                .as_ref()
                                .map(file_name)
                                .filter(|n| !n.is_empty())
                                .unwrap_or_else(|| self.current_tab.title().into()),
                            AppTab::Still => self
                                .img_edit_file
                                .as_ref()
                                .or(self.edit_file.as_ref())
                                .map(file_name)
                                .filter(|n| !n.is_empty())
                                .unwrap_or_else(|| self.current_tab.title().into()),
                            _ => self.current_tab.title().into(),
                        };
                        ui.horizontal(|ui| {
                            // E31 — breadcrumb root for review stages:
                            // Esc / back always lands in Library.
                            let in_review =
                                matches!(self.current_tab, AppTab::Clip | AppTab::Still)
                                    && (self.edit_file.is_some() || self.img_edit_file.is_some());
                            if in_review {
                                if ui
                                    .link(
                                        RichText::new("Library ›")
                                            .size(15.0)
                                            .color(theme::TEXT_DIM()),
                                    )
                                    .on_hover_text("Back to Library")
                                    .clicked()
                                {
                                    self.current_tab = AppTab::Library;
                                }
                            }
                            ui.label(
                                RichText::new(title)
                                    .font(egui::FontId::new(22.0, theme::font_semibold()))
                                    .color(theme::TEXT()),
                            );
                        });
                        // E41 — subtitle carries live context (unsaved marks,
                        // counts) before the static stage hint.
                        let sub: String = match self.current_tab {
                            AppTab::Still if !self.annotation_actions.is_empty() => {
                                format!("{} annotations on canvas", self.annotation_actions.len())
                            }
                            AppTab::Clip if !self.filmstrip_cut.is_empty() => {
                                format!("{} frames marked to cut", self.filmstrip_cut.len())
                            }
                            _ => self.current_tab.subtitle().into(),
                        };
                        if !sub.is_empty() {
                            ui.label(RichText::new(sub).size(11.0).color(theme::TEXT_DIM()));
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui::icon_btn(ui, "⌘K", "Command palette (Ctrl+K / ⌘K)") {
                            self.palette_open = true;
                            self.palette_query.clear();
                            self.palette_selected = 0;
                        }
                        if ui::icon_btn(ui, "⚙", "Settings (Ctrl+5)") {
                            self.current_tab = AppTab::Settings;
                        }
                        let inbox_label = if self.feedback_pending_count > 0 {
                            format!("🗳 {}", self.feedback_pending_count)
                        } else {
                            "🗳".to_string()
                        };
                        if ui::icon_btn(ui, &inbox_label, "Inbox (Ctrl+I)") {
                            self.current_tab = AppTab::Feedback;
                            self.scan_feedback_requests();
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
                        } else if self.recording_finalizing {
                            ui.label(
                                RichText::new("● Saving…")
                                    .color(theme::ACCENT())
                                    .strong()
                                    .small(),
                            );
                        } else if self.tray.is_some() {
                            ui.label(RichText::new("tray on").color(theme::TEXT_DIM()).small());
                        } else {
                            // E271 — no tray surface: say what close does.
                            ui.label(
                                RichText::new("no tray — close quits")
                                    .color(theme::WARN())
                                    .small(),
                            )
                            .on_hover_text(
                                "System tray unavailable; closing the window exits Vibecap",
                            );
                        }
                    });
                });
                ui.add_space(self.density.sp(theme::SP_2));

                // E50 — ease the incoming stage in from the rail direction.
                let mut slide_dx = 0.0;
                if let Some((at, dir)) = self.tab_slide {
                    let t = at.elapsed().as_secs_f32() / 0.16;
                    if t >= 1.0 {
                        self.tab_slide = None;
                    } else {
                        let e = 1.0 - (1.0 - t).powi(3); // ease-out cubic
                        slide_dx = dir * 60.0 * (1.0 - e);
                        ctx.request_repaint();
                    }
                }
                let body_rect = ui.available_rect_before_wrap();
                let mut render_body = |ui: &mut egui::Ui| match self.current_tab {
                    // ── The funnel: Capture → Review → Library, one column.
                    //    Active stage expands; the others collapse to stripes
                    //    (animated — the column visibly squeezes/reveals).
                    AppTab::Capture | AppTab::Still | AppTab::Clip | AppTab::Library => {
                        const STRIPE_H: f32 = 40.0;
                        let gap = theme::SP_2;
                        // The Review slot shows whichever editor is live — if we're
                        // already on Still/Clip, keep it so the active stage never
                        // collapses to a stripe mid-review.
                        let review = if matches!(self.current_tab, AppTab::Still | AppTab::Clip) {
                            self.current_tab
                        } else {
                            self.review_tab()
                        };
                        let order = [AppTab::Capture, review, AppTab::Library];
                        let avail_h = ui.available_height().max(120.0);
                        let expanded_h = (avail_h - 2.0 * (STRIPE_H + gap)).max(160.0);

                        let mut heights = [0.0_f32; 3];
                        let mut sum = 0.0_f32;
                        for (i, t) in order.iter().enumerate() {
                            let f = ctx.animate_value_with_time(
                                egui::Id::new("funnel_stage").with(i),
                                if self.current_tab == *t { 1.0 } else { 0.0 },
                                0.22,
                            );
                            let h = STRIPE_H + (expanded_h - STRIPE_H) * f;
                            heights[i] = h;
                            sum += h;
                        }
                        // Keep the column exactly avail_h during transitions.
                        let scale = if sum + 2.0 * gap > avail_h {
                            ((avail_h - 2.0 * gap) / sum).max(0.0)
                        } else {
                            1.0
                        };

                        let lib_n = self.library_items.len();
                        let mut switch_to: Option<AppTab> = None;
                        for (i, tab) in order.iter().enumerate() {
                            let h = (heights[i] * scale).max(30.0);
                            let w = ui.available_width();
                            ui.allocate_ui_with_layout(
                                Vec2::new(w, h),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    if self.current_tab == *tab {
                                        egui::ScrollArea::vertical()
                                            .id_source(("funnel_scroll", i))
                                            .auto_shrink([false; 2])
                                            .show(ui, |ui| {
                                                ui.set_width(ui.available_width());
                                                match tab {
                                                    AppTab::Capture => {
                                                        ui::capture_tab::show(self, ui, ctx)
                                                    }
                                                    AppTab::Library => {
                                                        ui::library_tab::show(self, ui, ctx)
                                                    }
                                                    AppTab::Clip => {
                                                        ui::clip_tab::show(self, ui, ctx)
                                                    }
                                                    AppTab::Still => {
                                                        ui::still_tab::show(self, ui, ctx)
                                                    }
                                                    _ => {}
                                                }
                                            });
                                    } else {
                                        let (icon, title, hint): (ui::icons::Icon, &str, String) =
                                            match tab {
                                                AppTab::Capture => (
                                                    ui::icons::Icon::Shutter,
                                                    "Capture",
                                                    "grab a shot or clip".to_string(),
                                                ),
                                                AppTab::Library => (
                                                    ui::icons::Icon::Media,
                                                    "Library",
                                                    if lib_n == 0 {
                                                        "your captures land here".to_string()
                                                    } else {
                                                        format!("{lib_n} items")
                                                    },
                                                ),
                                                _ => (
                                                    ui::icons::Icon::Still,
                                                    "Review",
                                                    "mark it · trim it · ship it".to_string(),
                                                ),
                                            };
                                        if funnel_stripe(ui, icon, title, &hint) {
                                            switch_to = Some(*tab);
                                        }
                                    }
                                },
                            );
                            if i < 2 {
                                ui.add_space(gap);
                            }
                        }
                        if let Some(t) = switch_to {
                            self.current_tab = t;
                        }
                    }
                    // Off-funnel stages: slim way back, then the content.
                    AppTab::Feedback | AppTab::Settings => {
                        if funnel_stripe(
                            ui,
                            ui::icons::Icon::Shutter,
                            "Capture",
                            "back to the funnel",
                        ) {
                            self.current_tab = AppTab::Capture;
                        } else {
                            match self.current_tab {
                                AppTab::Feedback => ui::inbox_tab::show(self, ui, ctx),
                                AppTab::Settings => ui::settings_tab::show(self, ui, ctx),
                                _ => {}
                            }
                        }
                    }
                };
                if slide_dx.abs() > 0.5 {
                    let shifted = body_rect.translate(egui::vec2(slide_dx, 0.0));
                    ui.allocate_ui_at_rect(shifted, |ui| render_body(ui));
                    ui.advance_cursor_after_rect(body_rect);
                } else {
                    render_body(ui);
                }
            });

        // Capture action toast takes priority over plain toasts.
        if let Some((path, at, copied)) = self.capture_toast.clone() {
            if at.elapsed() > Duration::from_secs(12) {
                self.capture_toast = None;
            } else if let Some(act) = show_capture_toast(ctx, &path, copied) {
                match act {
                    CaptureToastAction::Annotate => {
                        self.capture_toast = None;
                        self.open_still_from_path(path);
                    }
                    CaptureToastAction::Copy => {
                        self.img_edit_file = Some(path.clone());
                        self.copy_current_still_to_clipboard();
                        self.capture_toast = None;
                    }
                    CaptureToastAction::CopyPath => {
                        if let Ok(mut board) = arboard::Clipboard::new() {
                            let _ = board.set_text(path.display().to_string());
                            self.show_toast("Path copied");
                        }
                        self.capture_toast = None;
                    }
                    CaptureToastAction::Reveal => {
                        let _ = reveal_in_file_manager(&path);
                        self.capture_toast = None;
                    }
                    CaptureToastAction::Discard => {
                        self.delete_library_paths(&[path.clone()]);
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

        // E150 — one-time confirm when a delete targets the open clip with
        // unsaved cuts/trims.
        if let Some(paths) = self.confirm_delete.clone() {
            let mut keep = false;
            let mut del = false;
            egui::Window::new("Delete unsaved clip work?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "{} file(s) include the clip open in Review with unsaved trims.",
                        paths.len()
                    ));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Delete anyway").clicked() {
                            del = true;
                        }
                        if ui.button("Keep").clicked() {
                            keep = true;
                        }
                    });
                });
            if del {
                for p in &paths {
                    self.delete_guard_ok.insert(p.clone());
                }
                self.confirm_delete = None;
                self.delete_library_paths(&paths);
            } else if keep {
                self.confirm_delete = None;
            }
        }
        // E236 — first painted frame ≈ interactive; recorded once.
        app::note_first_frame();
    }
}

fn main() -> eframe::Result<()> {
    // E236 — t0 for the launch→interactive measurement.
    app::mark_app_start();
    // Panic capture (K249): append crash info to <config>/crash.log — a
    // windows-subsystem GUI dies silently otherwise.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let line = format!(
            "[{}] panic: {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            info
        );
        let log = vibecap_config_dir().join("crash.log");
        let _ = std::fs::create_dir_all(vibecap_config_dir());
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
        {
            use std::io::Write;
            let _ = f.write_all(line.as_bytes());
        }
        default_hook(info);
    }));

    let raw: Vec<String> = std::env::args().skip(1).collect();
    let cli = parse_args(&raw);

    if let Some(code) = run_headless(&cli) {
        if code == 0 {
            return Ok(());
        }
        std::process::exit(code);
    }

    if matches!(cli.action, CliAction::Mcp) {
        // Intentionally no process-wide lock: many agents may each spawn --mcp.
        eprintln!(
            "vibecap mcp ready (pid {}, live session {})",
            std::process::id(),
            mcp_live_dir().display()
        );
        run_mcp_server();
        return Ok(());
    }

    let (no_tray, start_hidden) = match cli.action {
        CliAction::Gui { hidden, no_tray } => (no_tray, hidden),
        _ => (false, false),
    };
    if let Some(id) = raw
        .iter()
        .find_map(|a| a.strip_prefix("vibecap://feedback/"))
    {
        std::env::set_var("VIBECAP_OPEN_FEEDBACK", id);
    }
    if let Err(_pid) = app::instance::acquire_gui_lock() {
        crate::platform::activate_own_app();
        eprintln!("vibecap GUI already running — focusing the existing window");
        return Ok(());
    }
    let enable_tray = !no_tray || start_hidden;

    // Brand dock / taskbar icon. Decode via `image` so RGB (non-RGBA) PNGs work;
    // eframe::from_png_bytes rejects those and unwrap_or_default() yielded a blank icon.
    let app_icon = window_icon_data();

    // Open at the last persisted size (bigger default: 1160×800 on first run).
    let sess = load_session();
    let mut win_w = sess.window_w.clamp(1024.0, 3200.0);
    let mut win_h = sess.window_h.clamp(700.0, 2200.0);
    // E48 — snap-layout friendly: on a true first run (no session.json yet)
    // the default window fits inside a Windows half-snap — never wider or
    // taller than half the primary display.
    if !sess.wizard_done {
        if let Some(m) = crate::platform::list_monitors().first() {
            win_w = win_w.min((m.w / 2) as f32).max(760.0);
            win_h = win_h.min(m.h as f32).max(560.0);
        }
    }

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_decorations(true)
            .with_transparent(false)
            .with_inner_size([win_w, win_h])
            .with_min_inner_size([760.0, 560.0])
            .with_icon(app_icon)
            // Multiple GUI instances are allowed (human + optional second window).
            .with_title(format!("Vibecap Studio · {}", std::process::id())),
        ..Default::default()
    };

    let result = eframe::run_native(
        "Vibecap Studio",
        options,
        Box::new(move |cc| {
            let mut app = VibecapApp::new(cc);
            app.start_hidden = start_hidden;
            if enable_tray {
                match TrayController::try_new("Vibecap — click to show") {
                    Ok(tray) => {
                        if let Ok(mut ids) = app.wake_shared.tray_ids.lock() {
                            *ids = tray.menu_action_map();
                        }
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
    );
    app::instance::release_gui_lock();
    result
}

fn load_marker_sidecar(file: &std::path::Path) -> Vec<f64> {
    let side = file.with_extension("markers.txt");
    let Ok(s) = std::fs::read_to_string(side) else {
        return Vec::new();
    };
    s.lines()
        .filter_map(|l| l.trim().parse::<f64>().ok())
        .collect()
}

fn window_icon_data() -> egui::IconData {
    match image::load_from_memory(include_bytes!("../assets/app_icon.png")) {
        Ok(img) => {
            let rgba = img.into_rgba8();
            egui::IconData {
                width: rgba.width(),
                height: rgba.height(),
                rgba: rgba.into_raw(),
            }
        }
        Err(_) => egui::IconData::default(),
    }
}

fn load_brand_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(include_bytes!("../assets/app_icon.png")).ok()?;
    let rgba = img.into_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture("brand_logo", color, Default::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_icon_is_rgba_and_nonzero() {
        let icon = window_icon_data();
        assert!(icon.width >= 16 && icon.height >= 16);
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
        assert!(icon.rgba.iter().any(|&b| b != 0));
    }

    #[test]
    fn fast_still_only_when_hidden_fullscreen_idle() {
        // The minimized fast path: grab the desktop on a worker, no window flash.
        assert!(fast_still_allowed(
            true,
            Some(CaptureTarget::Fullscreen),
            false
        ));
        // Visible window → normal hide→focus→grab GUI path.
        assert!(!fast_still_allowed(
            false,
            Some(CaptureTarget::Fullscreen),
            false
        ));
        // Region needs the overlay; Window needs focus+crop — both GUI path.
        for t in [CaptureTarget::Region, CaptureTarget::Window] {
            assert!(!fast_still_allowed(true, Some(t), false));
        }
        // No mirrored cfg yet → cannot know the target.
        assert!(!fast_still_allowed(true, None, false));
        // A still already in flight → do not double-capture.
        assert!(!fast_still_allowed(
            true,
            Some(CaptureTarget::Fullscreen),
            true
        ));
    }

    #[test]
    fn wake_rules_let_record_stop_surface_but_never_a_still() {
        // Not parked → everything wakes the window.
        for ev in [
            WakeEvent::Screenshot,
            WakeEvent::RecordToggle,
            WakeEvent::Show,
            WakeEvent::ToggleWindow,
            WakeEvent::Tray(TrayAction::Hide),
        ] {
            assert!(pump_needs_wake(&ev, false, false), "{ev:?} unparked");
        }
        // Still owns the park → nothing surfaces the studio into its own shot.
        for ev in [
            WakeEvent::Screenshot,
            WakeEvent::RecordToggle,
            WakeEvent::Show,
            WakeEvent::Tray(TrayAction::Show),
            WakeEvent::Tray(TrayAction::Quit),
        ] {
            assert!(!pump_needs_wake(&ev, true, true), "{ev:?} still-parked");
        }
        // Recording owns the park → stop/show/quit must wake the loop even if
        // the REC bar is not repainting; other events stay queued.
        for ev in [
            WakeEvent::RecordToggle,
            WakeEvent::Show,
            WakeEvent::Tray(TrayAction::ToggleRecord),
            WakeEvent::Tray(TrayAction::Show),
            WakeEvent::Tray(TrayAction::Quit),
        ] {
            assert!(pump_needs_wake(&ev, true, false), "{ev:?} record-parked");
        }
        for ev in [
            WakeEvent::Screenshot,
            WakeEvent::ToggleWindow,
            WakeEvent::Tray(TrayAction::GoSettings),
        ] {
            assert!(!pump_needs_wake(&ev, true, false), "{ev:?} record-parked");
        }
    }
}
