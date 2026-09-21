//! Optional single-instance lock for the GUI (MCP/CLI stay multi-process).

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use super::io::vibecap_config_dir;

fn lock_path() -> PathBuf {
    vibecap_config_dir().join("gui.lock")
}

fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        // E238 — native OpenProcess probe, no tasklist spawn.
        crate::platform::pid_alive(pid)
    }
    #[cfg(not(any(unix, windows)))]
    {
        true
    }
}

/// If another GUI is alive, return its pid. Otherwise take the lock.
pub fn acquire_gui_lock() -> Result<(), u32> {
    let path = lock_path();
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(".")));
    if let Ok(s) = std::fs::read_to_string(&path) {
        if let Ok(pid) = s.trim().parse::<u32>() {
            if pid != std::process::id() && pid_alive(pid) {
                return Err(pid);
            }
        }
    }
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
    {
        let _ = write!(f, "{}", std::process::id());
    }
    Ok(())
}

pub fn release_gui_lock() {
    let path = lock_path();
    if let Ok(s) = std::fs::read_to_string(&path) {
        if s.trim() == std::process::id().to_string() {
            let _ = std::fs::remove_file(&path);
        }
    }
}
