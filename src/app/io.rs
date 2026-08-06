//! Shared atomic JSON write for budget + feedback files.

use std::path::PathBuf;

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
