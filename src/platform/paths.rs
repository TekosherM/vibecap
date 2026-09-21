use std::path::PathBuf;

/// E212 — portable mode: when `<exe_dir>/vibecap.portable` exists, config +
/// media live under `<exe_dir>/portable/` instead of the OS profile.
/// Resolved once — the marker is a startup decision; toggling it takes
/// effect on relaunch.
pub fn portable_root() -> Option<PathBuf> {
    static ROOT: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))?;
        exe_dir
            .join("vibecap.portable")
            .exists()
            .then(|| exe_dir.join("portable"))
    })
    .clone()
}

/// True while running from a portable install (marker beside the exe).
pub fn is_portable() -> bool {
    portable_root().is_some()
}

/// Create/remove the portable marker beside the exe. Takes effect on next
/// launch — `portable_root` is resolved once at startup.
pub fn set_portable_marker(enable: bool) -> Result<(), String> {
    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|d| d.to_path_buf())
        .ok_or("could not resolve the executable directory")?;
    let marker = exe_dir.join("vibecap.portable");
    if enable {
        let _ = std::fs::create_dir_all(exe_dir.join("portable"));
        std::fs::write(&marker, b"portable\n").map_err(|e| e.to_string())
    } else {
        match std::fs::remove_file(&marker) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

/// Config root for budget + feedback inbox (`dirs::config_dir()/vibecap`,
/// or `<exe>/portable/config` in portable mode).
pub fn config_dir() -> PathBuf {
    let dir = if let Some(root) = portable_root() {
        root.join("config")
    } else {
        dirs::config_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
            })
            .join("vibecap")
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Default media folder for screenshots, videos, GIFs.
///
/// Prefer the platform Videos directory (`dirs::video_dir()`), fall back to
/// `~/Movies/Vibecap` (macOS convention), then `~/Vibecap`.
pub fn media_dir() -> PathBuf {
    let dir = if let Some(root) = portable_root() {
        root.join("media")
    } else if let Some(videos) = dirs::video_dir() {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// E212 — the portable marker is created/removed beside the exe and
    /// double-disable is a no-op. (`portable_root` itself is OnceLock-cached
    /// per process, so this exercises the file ops, not the cache.)
    #[test]
    fn portable_marker_roundtrip() {
        let exe_dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let marker = exe_dir.join("vibecap.portable");
        let _ = std::fs::remove_file(&marker); // clean slate
        set_portable_marker(true).unwrap();
        assert!(marker.exists());
        set_portable_marker(false).unwrap();
        assert!(!marker.exists());
        set_portable_marker(false).unwrap(); // idempotent
    }
}
