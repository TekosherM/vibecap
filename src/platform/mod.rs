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
    even_screen_rect, export_gif_clip, export_gif_clip_ex, record_dry_run_line,
    record_screen_clip_opts, remux_to_clean_mp4, spawn_screen_recorder, spawn_screen_recorder_opts,
    spawn_voice_memo, verify_mp4, LiveFormat, ScreenRect,
};
pub use ffmpeg::{
    extract_preview_wav, ffmpeg_available, ffmpeg_command, ffmpeg_log_tail, ffmpeg_path,
    ffmpeg_recheck, format_timecode, list_audio_input_devices, parse_timecode, probe_duration,
    run_ffmpeg,
};
pub use notify::notify_agent_question;

/// F126 — loop the extracted preview WAV while the clip flipbook plays.
/// Windows uses winmm `PlaySoundW`; other platforms stay silent for now.
pub fn play_audio_preview(path: &std::path::Path) {
    #[cfg(windows)]
    win32::play_wav_loop(path);
    #[cfg(not(windows))]
    let _ = path;
}

/// Stop any looping preview audio.
pub fn stop_audio_preview() {
    #[cfg(windows)]
    win32::stop_sound();
}
pub use paths::{
    config_dir, is_portable, live_dir, live_session_dir, media_dir, media_dir_display,
    set_portable_marker,
};
pub use process::{cont_process, pause_supported, stop_process};
#[cfg(windows)]
pub(crate) use win32::{
    cursor_pos, disk_free_bytes, foreground_process_name, hide_studio_window, minimize_studio,
    monitor_at_point, pid_alive, restore_studio_to_taskbar, run_at_login_enabled_native,
    set_run_at_login_native, set_studio_capture_excluded, set_title_capture_excluded,
    studio_is_minimized, terminate_process, windows_at_point, ExcludeStatus,
};

/// Free bytes on the volume containing `dir` (Windows native; None elsewhere).
pub fn disk_free_bytes_for(dir: &std::path::Path) -> Option<u64> {
    #[cfg(windows)]
    {
        disk_free_bytes(dir)
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        None
    }
}
#[cfg(target_os = "windows")]
pub use shell::window_rect_on_screen;
pub use shell::{
    activate_own_app, explorer_verb_enabled, focus_app, frontmost_app_name, list_capture_windows,
    list_capture_windows_cached, list_monitors, list_running_apps, on_battery, open_path,
    open_screen_recording_settings, open_with, os_apps_dark, request_screen_recording_access,
    reveal_in_file_manager, run_at_login_enabled, screen_capture_allowed, set_explorer_verb,
    set_run_at_login, set_url_scheme, start_file_drag, url_scheme_enabled, window_tools_hint,
};
pub use source::{default_output_dir_display, resolve_output_dir, CaptureOpts};

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
