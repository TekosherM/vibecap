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

pub use capture::{
    capture_live_frame, capture_screenshot, capture_screenshot_interactive, capture_screenshot_opts,
    capture_to_dir, capture_to_media_dir, export_gif_clip, record_screen_clip,
    record_screen_clip_opts, spawn_screen_recorder, spawn_screen_recorder_opts, spawn_voice_memo,
    LiveFormat,
};
pub use ffmpeg::{ffmpeg_available, ffmpeg_command, ffmpeg_missing_message, ffmpeg_path};
pub use notify::{notify_agent_question, notify_desktop};
pub use paths::{config_dir, live_dir, live_session_dir, media_dir, media_dir_display};
pub use process::{cont_process, stop_process};
pub use shell::{
    focus_app, frontmost_app_name, list_running_apps, open_path, open_screen_recording_settings,
    reveal_in_file_manager,
};
pub use source::{
    default_output_dir_display, normalize_display, parse_xdpyinfo_dimensions, parse_xdotool_shell,
    resolve_grab, resolve_output_dir, x11grab_record_args, x11grab_still_args, CaptureOpts,
    GrabSpec, WindowGeom,
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
