//! Shared atomic JSON write for budget + feedback files.

use std::path::{Path, PathBuf};

use crate::platform::config_dir as platform_config_dir;

pub fn vibecap_config_dir() -> PathBuf {
    platform_config_dir()
}

/// Write-then-rename so a concurrent reader never sees a partial file.
pub fn write_json_atomic(path: &PathBuf, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, contents).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Marker written by the screenshot worker so the UI can restore even if
/// the window was orderOut and the channel was missed.
fn pending_still_path() -> PathBuf {
    vibecap_config_dir().join("pending_still.path")
}

pub fn write_pending_still(path: &Path) {
    let _ = std::fs::create_dir_all(vibecap_config_dir());
    let _ = std::fs::write(pending_still_path(), path.to_string_lossy().as_bytes());
}

pub fn write_pending_still_error(msg: &str) {
    let _ = std::fs::create_dir_all(vibecap_config_dir());
    let _ = std::fs::write(
        pending_still_path(),
        format!("ERROR\n{msg}").as_bytes(),
    );
}

/// Returns Ok(path) or Err(message). Clears the marker.
pub fn take_pending_still() -> Option<Result<PathBuf, String>> {
    let p = pending_still_path();
    let raw = std::fs::read_to_string(&p).ok()?;
    let _ = std::fs::remove_file(&p);
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(msg) = raw.strip_prefix("ERROR\n") {
        return Some(Err(msg.to_string()));
    }
    if let Some(msg) = raw.strip_prefix("ERROR:") {
        return Some(Err(msg.trim().to_string()));
    }
    let path = PathBuf::from(raw);
    if path.exists() {
        Some(Ok(path))
    } else {
        Some(Err(format!("Capture file missing: {}", path.display())))
    }
}
