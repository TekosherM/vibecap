use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// Best-effort list of running application names for the window picker / MCP.
/// Sorted, de-duplicated, system helpers filtered out where practical.
pub fn list_running_apps() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        let script = r#"
tell application "System Events"
  set names to name of every process whose background only is false
end tell
set AppleScript's text item delimiters to linefeed
return names as text
"#;
        let out = Command::new("osascript").args(["-e", script]).output();
        if let Ok(o) = out {
            if o.status.success() {
                return parse_app_lines(&String::from_utf8_lossy(&o.stdout));
            }
        }
        Vec::new()
    }

    #[cfg(target_os = "windows")]
    {
        // tasklist CSV: "Image Name","PID",...
        let out = Command::new("tasklist")
            .args(["/FO", "CSV", "/NH"])
            .output();
        if let Ok(o) = out {
            if o.status.success() {
                let mut names = Vec::new();
                for line in String::from_utf8_lossy(&o.stdout).lines() {
                    // "chrome.exe","1234",...
                    if let Some(rest) = line.strip_prefix('"') {
                        if let Some(end) = rest.find('"') {
                            let exe = &rest[..end];
                            let label = exe.trim_end_matches(".exe").trim_end_matches(".EXE");
                            if !label.is_empty()
                                && !matches!(
                                    label.to_ascii_lowercase().as_str(),
                                    "system" | "svchost" | "explorer" | "tasklist" | "conhost"
                                )
                            {
                                names.push(label.to_string());
                            }
                        }
                    }
                }
                names.sort();
                names.dedup();
                return names;
            }
        }
        Vec::new()
    }

    #[cfg(target_os = "linux")]
    {
        // Prefer wmctrl -l  → 0x…  desktop  host  Window Title
        if let Ok(o) = Command::new("wmctrl").args(["-l"]).output() {
            if o.status.success() {
                let mut names = Vec::new();
                for line in String::from_utf8_lossy(&o.stdout).lines() {
                    let parts: Vec<_> = line.splitn(4, ' ').collect();
                    if parts.len() >= 4 {
                        let title = parts[3].trim();
                        if !title.is_empty() {
                            // Use last token-ish app-ish chunk
                            names.push(title.to_string());
                        }
                    }
                }
                names.sort();
                names.dedup();
                if !names.is_empty() {
                    return names;
                }
            }
        }
        Vec::new()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Vec::new()
    }
}

fn parse_app_lines(s: &str) -> Vec<String> {
    let skip = [
        "loginwindow",
        "WindowServer",
        "SystemUIServer",
        "Dock",
        "ControlCenter",
        "NotificationCenter",
        "Spotlight",
        "universalaccessd",
        "AirPlayUIAgent",
        "Vibecap",
        "vibecap",
    ];
    let mut names: Vec<String> = s
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .filter(|l| !skip.iter().any(|s| l.eq_ignore_ascii_case(s)))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Name of the frontmost GUI app (best-effort). Empty if unknown.
pub fn frontmost_app_name() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let script = r#"
tell application "System Events"
  set n to name of first application process whose frontmost is true
end tell
return n
"#;
        let out = Command::new("osascript").args(["-e", script]).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    #[cfg(target_os = "windows")]
    {
        // No reliable lightweight frontmost query without extra crates.
        None
    }

    #[cfg(target_os = "linux")]
    {
        // xdotool getactivewindow getwindowname
        let out = Command::new("xdotool")
            .args(["getactivewindow", "getwindowname"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

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

/// Open macOS System Settings → Privacy → Screen Recording (best-effort).
pub fn open_screen_recording_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // Ventura+ deep link, then older pref pane.
        let urls = [
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
            "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_ScreenCapture",
        ];
        for u in urls {
            if Command::new("open").arg(u).status().map(|s| s.success()).unwrap_or(false) {
                return Ok(());
            }
        }
        // Fallback: open Privacy & Security root
        Command::new("open")
            .arg("/System/Library/PreferencePanes/Security.prefPane")
            .status()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Screen Recording settings are only relevant on macOS".into())
    }
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
