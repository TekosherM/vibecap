use std::path::Path;
use std::process::Command;
use std::time::Duration;

// ---------------------------------------------------------------------------
// macOS TCC (Screen Recording) + AppKit activation via direct FFI.
//
// Why not `osascript` / `open -b`:
// · AppleScript from this process to another app trips the Automation TCC
//   prompt ("… wants to control …") — the source of repeated Allow dialogs.
// · `open -b <bundle id>` can launch a *second* GUI instance when the running
//   copy has no matching registered bundle (e.g. `cargo run`).
// Direct CoreGraphics/AppKit calls have neither problem.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos_ffi {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_void};

    extern "C" {
        // libSystem — always linked.
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    // macOS `RTLD_DEFAULT`: search symbols already loaded into the process.
    const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;

    type CgBoolFn = unsafe extern "C" fn() -> bool;

    fn lookup(name: &str) -> Option<CgBoolFn> {
        let c = CString::new(name).ok()?;
        let sym = unsafe { dlsym(RTLD_DEFAULT, c.as_ptr()) };
        if sym.is_null() {
            None
        } else {
            Some(unsafe { std::mem::transmute::<*mut c_void, CgBoolFn>(sym) })
        }
    }

    /// Authoritative Screen Recording grant check (macOS 10.15+).
    /// Never prompts. Missing symbol (pre-10.15) → no TCC gate exists → true.
    pub fn screen_capture_preflight() -> bool {
        match lookup("CGPreflightScreenCaptureAccess") {
            Some(f) => unsafe { f() },
            None => true,
        }
    }

    /// Ask macOS for Screen Recording access (10.15+). Shows the system dialog
    /// only while the state is still undetermined; returns the grant state.
    pub fn screen_capture_request() -> bool {
        if screen_capture_preflight() {
            return true;
        }
        if let Some(f) = lookup("CGRequestScreenCaptureAccess") {
            unsafe { f() };
        }
        screen_capture_preflight()
    }

    // --- AppKit activation without subprocesses ---------------------------
    //
    // Do NOT call `-[NSApplication activateWithOptions:]` on winit's
    // `WinitApplication` subclass — on several macOS/winit combos that selector
    // is missing and throws, which aborts the process during the first paint.
    // Prefer `NSRunningApplication` (stable API) and only send selectors the
    // receiver actually implements.

    type Id = *mut c_void;
    type Sel = *mut c_void;
    type Bool = i8;

    #[link(name = "objc", kind = "dylib")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
    }

    unsafe fn msg_send_id(recv: Id, sel: Sel) -> Id {
        let f: unsafe extern "C" fn(Id, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        f(recv, sel)
    }

    unsafe fn msg_send_bool_sel(recv: Id, sel: Sel, arg: Sel) -> Bool {
        let f: unsafe extern "C" fn(Id, Sel, Sel) -> Bool =
            std::mem::transmute(objc_msgSend as *const ());
        f(recv, sel, arg)
    }

    unsafe fn msg_send_bool_options(recv: Id, sel: Sel, options: u64) -> Bool {
        let f: unsafe extern "C" fn(Id, Sel, u64) -> Bool =
            std::mem::transmute(objc_msgSend as *const ());
        f(recv, sel, options)
    }

    unsafe fn msg_send_void(recv: Id, sel: Sel) {
        let f: unsafe extern "C" fn(Id, Sel) = std::mem::transmute(objc_msgSend as *const ());
        f(recv, sel)
    }

    unsafe fn responds_to(recv: Id, sel: Sel) -> bool {
        if recv.is_null() || sel.is_null() {
            return false;
        }
        let rts = match CString::new("respondsToSelector:") {
            Ok(c) => sel_registerName(c.as_ptr()),
            Err(_) => return false,
        };
        msg_send_bool_sel(recv, rts, sel) != 0
    }

    /// Bring this process to the front without Apple Events / `open -b`.
    /// Must run on the main thread (all callers in this app do).
    pub fn activate_application() {
        unsafe {
            // Preferred: NSRunningApplication (not WinitApplication).
            // -[NSRunningApplication activateWithOptions:] is the supported path.
            const NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS: u64 = 1 << 1;
            if let Ok(cls_name) = CString::new("NSRunningApplication") {
                let cls = objc_getClass(cls_name.as_ptr());
                if !cls.is_null() {
                    if let Ok(cur_name) = CString::new("currentApplication") {
                        let cur_sel = sel_registerName(cur_name.as_ptr());
                        let running = msg_send_id(cls, cur_sel);
                        if !running.is_null() {
                            if let Ok(act_name) = CString::new("activateWithOptions:") {
                                let act_sel = sel_registerName(act_name.as_ptr());
                                if responds_to(running, act_sel) {
                                    let _ = msg_send_bool_options(
                                        running,
                                        act_sel,
                                        NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS,
                                    );
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            // Fallback: NSApp -activate (macOS 14+) if present — never use
            // activateWithOptions: on NSApplication/WinitApplication here.
            if let Ok(cls_name) = CString::new("NSApplication") {
                let cls = objc_getClass(cls_name.as_ptr());
                if cls.is_null() {
                    return;
                }
                if let Ok(shared_name) = CString::new("sharedApplication") {
                    let shared_sel = sel_registerName(shared_name.as_ptr());
                    let app = msg_send_id(cls, shared_sel);
                    if app.is_null() {
                        return;
                    }
                    if let Ok(act_name) = CString::new("activate") {
                        let act_sel = sel_registerName(act_name.as_ptr());
                        if responds_to(app, act_sel) {
                            msg_send_void(app, act_sel);
                        }
                    }
                }
            }
        }
    }
}

/// Best-effort list of running application names for the window picker / MCP.
/// Sorted, de-duplicated, system helpers filtered out where practical.
///
/// macOS: prefers `lsappinfo` so we do **not** poke System Events (avoids
/// repeated Automation / Accessibility prompts).
pub fn list_running_apps() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(names) = list_running_apps_lsappinfo() {
            return names;
        }
        // Fallback only if lsappinfo is unavailable (rare).
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
    let mut names: Vec<String> = s
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .filter(|l| !should_skip_app_name(l))
        .collect();
    names.sort();
    names.dedup();
    names
}

fn should_skip_app_name(name: &str) -> bool {
    let skip = [
        "loginwindow",
        "WindowServer",
        "SystemUIServer",
        "Dock",
        "ControlCenter",
        "Control Centre",
        "NotificationCenter",
        "Notification Centre",
        "Spotlight",
        "universalaccessd",
        "AirPlayUIAgent",
        "ViewBridgeAuxiliary",
        "Vibecap",
        "vibecap",
    ];
    skip.iter().any(|s| name.eq_ignore_ascii_case(s))
}

#[cfg(target_os = "macos")]
fn frontmost_app_lsappinfo() -> Option<String> {
    let front = Command::new("lsappinfo")
        .arg("front")
        .output()
        .ok()?;
    if !front.status.success() {
        return None;
    }
    let asn = String::from_utf8_lossy(&front.stdout).trim().to_string();
    if asn.is_empty() {
        return None;
    }
    let info = Command::new("lsappinfo")
        .args(["info", "-only", "name", &asn])
        .output()
        .ok()?;
    if !info.status.success() {
        return None;
    }
    // Typical: "LSDisplayName"="Safari"\n
    let text = String::from_utf8_lossy(&info.stdout);
    for part in ["LSDisplayName", "CFBundleName", "name"] {
        let key = format!("\"{part}\"=");
        if let Some(rest) = text.split(&key).nth(1) {
            let rest = rest.trim_start();
            if let Some(s) = rest.strip_prefix('"') {
                if let Some(end) = s.find('"') {
                    let name = s[..end].trim().to_string();
                    if !name.is_empty() && !should_skip_app_name(&name) {
                        return Some(name);
                    }
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn list_running_apps_lsappinfo() -> Option<Vec<String>> {
    let out = Command::new("lsappinfo").arg("list").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // Collect display names for Foreground apps; fall back to all APPL names.
    let mut foreground = Vec::new();
    let mut all_appl = Vec::new();
    let mut current: Option<String> = None;
    let mut is_foreground = false;
    let mut is_appl = false;

    let mut flush = |current: &mut Option<String>, is_fg: &mut bool, is_appl: &mut bool| {
        if let Some(n) = current.take() {
            if !should_skip_app_name(&n) {
                if *is_fg {
                    foreground.push(n.clone());
                }
                if *is_appl || *is_fg {
                    all_appl.push(n);
                }
            }
        }
        *is_fg = false;
        *is_appl = false;
    };

    for line in text.lines() {
        let t = line.trim();
        // New entry: `4) "LilyView" ASN:…`
        if let Some(idx) = t.find(") \"") {
            let prefix = t[..idx].trim();
            if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
                flush(&mut current, &mut is_foreground, &mut is_appl);
                if let Some(after) = t.get(idx + 3..) {
                    if let Some(end) = after.find('"') {
                        current = Some(after[..end].to_string());
                    }
                }
                continue;
            }
        }
        if t.contains("type=\"Foreground\"") {
            is_foreground = true;
        }
        if t.contains("fileType=\"APPL\"") || t.contains(".app\"") || t.contains(".app/") {
            is_appl = true;
        }
        if t.contains("type=\"BackgroundOnly\"") {
            // background helpers — drop unless also Foreground
            if !is_foreground {
                current = None;
            }
        }
    }
    flush(&mut current, &mut is_foreground, &mut is_appl);

    let mut names = if !foreground.is_empty() {
        foreground
    } else {
        all_appl
    };
    names.sort();
    names.dedup();
    if names.is_empty() {
        None
    } else {
        Some(names)
    }
}

/// Name of the frontmost GUI app (best-effort). Empty if unknown.
///
/// macOS: `lsappinfo` first (no Accessibility / Automation prompts). System
/// Events is only a last-resort fallback.
pub fn frontmost_app_name() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(name) = frontmost_app_lsappinfo() {
            return Some(name);
        }
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

        // Verify the focus actually landed; `open -a` fails silently for
        // names LaunchServices cannot resolve (lsappinfo process names etc.).
        let matches = |front: &Option<String>| {
            front
                .as_deref()
                .map(|f| {
                    let f = f.to_ascii_lowercase();
                    let a = app_name.to_ascii_lowercase();
                    f == a || f.contains(&a) || a.contains(&f)
                })
                .unwrap_or(false)
        };
        if matches(&frontmost_app_name()) {
            return Ok(());
        }
        // Retry via AppleScript activate (resolves by name differently).
        let script = format!("tell application \"{}\" to activate", app_name.replace('"', "\\\""));
        let _ = Command::new("osascript").arg("-e").arg(&script).status();
        std::thread::sleep(Duration::from_millis(400));
        if matches(&frontmost_app_name()) {
            return Ok(());
        }
        return Err(format!(
            "could not bring “{}” to the front (open -a + activate both failed)",
            app_name
        ));
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

/// Cheap, authoritative Screen Recording grant check (no prompt, no capture).
///
/// macOS: `CGPreflightScreenCaptureAccess`. Always true elsewhere.
pub fn screen_capture_allowed() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos_ffi::screen_capture_preflight()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Ensure Screen Recording access: preflight first, then ask macOS once.
///
/// The system Allow dialog appears only while the TCC state is undetermined;
/// a previous denial returns false without re-prompting (the user must flip
/// the switch in System Settings). No probe screenshot is taken, so there is
/// no size-heuristic false negative.
pub fn request_screen_recording_access() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(macos_ffi::screen_capture_request())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Force this app to the foreground.
///
/// Direct AppKit activation — no osascript (no Automation TCC prompt) and no
/// `open -b` (which can launch a second instance for unbundled runs).
/// Call from the main thread only.
pub fn activate_own_app() {
    #[cfg(target_os = "macos")]
    {
        macos_ffi::activate_application();
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
