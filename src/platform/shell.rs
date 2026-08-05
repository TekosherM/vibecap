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
    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }

    #[cfg(target_os = "macos")]
    {
        // Prefer AppleScript → real Finder. `open -R` can fail silently when
        // NSFileViewer points at a missing third-party app (e.g. Path Finder).
        let posix = path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
        let script_reveal = format!(
            "tell application \"Finder\" to reveal POSIX file \"{}\"",
            posix
        );
        let status = Command::new("osascript")
            .args(["-e", &script_reveal, "-e", "tell application \"Finder\" to activate"])
            .status();
        if status.map(|s| s.success()).unwrap_or(false) {
            return Ok(());
        }

        // Fallback: open -R with OsStr path (no UTF-8 loss)
        let status = Command::new("open").arg("-R").arg(path).status();
        if status.map(|s| s.success()).unwrap_or(false) {
            return Ok(());
        }

        // Last resort: open containing folder in Finder
        if let Some(parent) = path.parent() {
            let _ = Command::new("open").arg("-a").arg("Finder").arg(parent).status();
            return Ok(());
        }
        return Err("could not reveal file in Finder".into());
    }

    #[cfg(target_os = "windows")]
    {
        // explorer /select needs a path without extra quoting quirks
        let status = Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .status();
        if status.map(|s| s.success()).unwrap_or(false) {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            let _ = Command::new("explorer").arg(parent).status();
            return Ok(());
        }
        return Err("could not reveal file in Explorer".into());
    }

    #[cfg(target_os = "linux")]
    {
        // Try dbus file manager interface, then open parent
        if let Some(uri) = path.to_str().map(|p| format!("file://{p}")) {
            let ok = Command::new("dbus-send")
                .args([
                    "--session",
                    "--dest=org.freedesktop.FileManager1",
                    "--type=method_call",
                    "/org/freedesktop/FileManager1",
                    "org.freedesktop.FileManager1.ShowItems",
                    &format!("array:string:{}", uri),
                    "string:",
                ])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                return Ok(());
            }
        }
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
