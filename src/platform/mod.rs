//! OS abstraction for paths, app focus, screen capture, and file reveal.
//!
//! macOS uses native tools (`screencapture`, `open`). Windows/Linux prefer `ffmpeg`
//! (`gdigrab` / `x11grab`) so the same CLI/MCP surface works once ffmpeg is installed.
//!
//! GUI apps resolve ffmpeg via [`ffmpeg_path`] (not bare PATH — Finder launches omit Homebrew).

mod capture;
mod ffmpeg;
mod notify;
mod paths;
mod process;
mod shell;
mod source;
#[cfg(windows)]
mod win32;

// Re-exported capture surface (binary crate: some helpers are only used by
// headless/embedding call sites, not the GUI — keep them warning-free).
#[allow(unused_imports)]
pub use capture::{
    capture_live_frame, capture_screenshot, capture_screenshot_interactive,
    capture_screenshot_opts, capture_screenshot_region, capture_to_dir, crop_image_file,
    even_screen_rect, export_gif_clip, export_gif_clip_ex, record_screen_clip_opts, remux_to_clean_mp4,
    spawn_screen_recorder, spawn_screen_recorder_opts, spawn_voice_memo, LiveFormat, ScreenRect,
};
pub use ffmpeg::{
    ffmpeg_available, ffmpeg_command, ffmpeg_log_tail, ffmpeg_path, format_timecode,
    list_audio_input_devices, parse_timecode, probe_duration, run_ffmpeg,
};
pub use notify::notify_agent_question;
pub use paths::{config_dir, live_dir, live_session_dir, media_dir, media_dir_display};
pub use process::{cont_process, pause_supported, stop_process};
#[cfg(windows)]
pub(crate) use win32::{minimize_studio, restore_studio_to_taskbar, run_at_login_enabled_native, set_run_at_login_native};
pub use shell::{
    activate_own_app, focus_app, frontmost_app_name, list_capture_windows,
    list_capture_windows_cached, list_monitors,
    list_running_apps, open_path, open_screen_recording_settings, request_screen_recording_access,
    reveal_in_file_manager, run_at_login_enabled, screen_capture_allowed, set_run_at_login,
    window_tools_hint,
};
#[cfg(target_os = "windows")]
pub use shell::window_rect_on_screen;
pub use source::{
    default_output_dir_display, resolve_output_dir, CaptureOpts,
};

/// Human-readable platform capture backend label (for docs/help).
pub fn capture_backend_label() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macOS screencapture + ffmpeg"
    }
    #[cfg(target_os = "windows")]
    {
        "Windows ffmpeg gdigrab"
    }
    #[cfg(target_os = "linux")]
    {
        "Linux ffmpeg x11grab (supported agent backend; set DISPLAY). Wayland stills: grim fallback"
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "unsupported platform"
    }
}
