use std::path::PathBuf;

/// Config root for budget + feedback inbox (`dirs::config_dir()/vibecap`).
pub fn config_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        })
        .join("vibecap");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Default media folder for screenshots, videos, GIFs.
///
/// Prefer the platform Videos directory (`dirs::video_dir()`), fall back to
/// `~/Movies/Vibecap` (macOS convention), then `~/Vibecap`.
pub fn media_dir() -> PathBuf {
    let dir = if let Some(videos) = dirs::video_dir() {
        videos.join("Vibecap")
    } else if let Some(home) = dirs::home_dir() {
        // Keep the historical macOS path when video_dir is unavailable.
        let movies = home.join("Movies").join("Vibecap");
        if cfg!(target_os = "macos") || movies.parent().map(|p| p.exists()).unwrap_or(false) {
            movies
        } else {
            home.join("Vibecap")
        }
    } else {
        PathBuf::from("Vibecap")
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Shared live-inspection root under the media folder (GUI status display).
pub fn live_dir() -> PathBuf {
    let dir = media_dir().join("live");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Per-process live session directory so multiple MCP servers / agents
/// can stream concurrently without overwriting each other's frames.
pub fn live_session_dir() -> PathBuf {
    let dir = live_dir().join(format!("session-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Short display string for help/toasts (tilde-style when under home).
pub fn media_dir_display() -> String {
    let path = media_dir();
    if let Some(home) = dirs::home_dir() {
        if let Ok(rel) = path.strip_prefix(&home) {
            return format!("~/{}", rel.display());
        }
    }
    path.display().to_string()
}
