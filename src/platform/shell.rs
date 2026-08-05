use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// Bring an application to the foreground by name when the OS supports it.
pub fn focus_app(app_name: &str) -> Result<(), String> {
    if app_name.trim().is_empty() {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-a", app_name])
            .spawn()
            .map_err(|e| format!("open -a failed: {}", e))?;
        std::thread::sleep(Duration::from_millis(400));
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        // Best-effort: launch/focus by executable or start-menu name.
        let _ = Command::new("cmd")
            .args(["/C", "start", "", app_name])
            .spawn();
        std::thread::sleep(Duration::from_millis(400));
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        // Try wmctrl window raise, then gtk-launch / bare command.
        if Command::new("wmctrl")
            .args(["-a", app_name])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            std::thread::sleep(Duration::from_millis(300));
            return Ok(());
        }
        if Command::new("gtk-launch")
            .arg(app_name)
            .spawn()
            .is_ok()
        {
            std::thread::sleep(Duration::from_millis(400));
            return Ok(());
        }
        let _ = Command::new(app_name).spawn();
        std::thread::sleep(Duration::from_millis(400));
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = app_name;
        Err("focus_app is not supported on this platform".into())
    }
}

/// Open a file or directory with the default handler.
pub fn open_path(path: &Path) -> Result<(), String> {
    open::that(path).map_err(|e| format!("open failed: {}", e))
}

/// Reveal a file in the system file manager (Finder / Explorer / file manager).
pub fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", path.to_str().unwrap_or("")])
            .spawn()
            .map_err(|e| format!("open -R failed: {}", e))?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let arg = format!("/select,{}", path.display());
        Command::new("explorer")
            .arg(arg)
            .spawn()
            .map_err(|e| format!("explorer failed: {}", e))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = path.parent() {
            return open_path(parent);
        }
        open_path(path)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        open_path(path)
    }
}
