use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A visible top-level window the picker / capturer can target.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub process: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub minimized: bool,
}

impl WindowInfo {
    pub fn label(&self) -> String {
        match (self.title.is_empty(), self.process.is_empty()) {
            (true, true) => self.id.clone(),
            (true, false) => self.process.clone(),
            (false, true) => self.title.clone(),
            (false, false) => format!("{} — {}", self.title, self.process),
        }
    }

    pub fn is_self(&self) -> bool {
        self.process.to_ascii_lowercase().contains("vibecap")
            || self.title.to_ascii_lowercase().contains("vibecap")
    }
}

/// Physical display in virtual-desktop coordinates.
#[derive(Debug, Clone, Copy)]
pub struct MonitorInfo {
    pub index: u32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub primary: bool,
}

struct Timed<T> {
    at: Instant,
    val: T,
}

static FRONT_CACHE: Mutex<Option<Timed<Option<String>>>> = Mutex::new(None);
static WIN_CACHE: Mutex<Option<Timed<Vec<WindowInfo>>>> = Mutex::new(None);

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
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
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
        let wins = list_capture_windows();
        if !wins.is_empty() {
            return wins
                .into_iter()
                .filter(|w| !w.is_self())
                .map(|w| w.label())
                .collect();
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

/// Visible windows with title/process/bounds (cached ~750 ms).
pub fn list_capture_windows() -> Vec<WindowInfo> {
    if let Ok(cache) = WIN_CACHE.lock() {
        if let Some(c) = cache.as_ref() {
            if c.at.elapsed() < Duration::from_millis(750) {
                return c.val.clone();
            }
        }
    }
    let val = list_capture_windows_uncached();
    if let Ok(mut cache) = WIN_CACHE.lock() {
        *cache = Some(Timed {
            at: Instant::now(),
            val: val.clone(),
        });
    }
    val
}

/// Non-blocking read for per-frame UI — returns the warm cache (any age) or
/// empty. The enum shells out to PowerShell on Windows, so widgets must never
/// call [`list_capture_windows`] directly; a refresh worker keeps this warm.
pub fn list_capture_windows_cached() -> Vec<WindowInfo> {
    if let Ok(cache) = WIN_CACHE.lock() {
        if let Some(c) = cache.as_ref() {
            return c.val.clone();
        }
    }
    Vec::new()
}

fn list_capture_windows_uncached() -> Vec<WindowInfo> {
    #[cfg(target_os = "windows")]
    {
        return windows_enum_windows();
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(o) = Command::new("wmctrl").args(["-lG"]).output() {
            if o.status.success() {
                let mut out = Vec::new();
                for line in String::from_utf8_lossy(&o.stdout).lines() {
                    let parts: Vec<_> = line.split_whitespace().collect();
                    // id desk x y w h host title…
                    if parts.len() >= 8 {
                        let title = parts[7..].join(" ");
                        if title.is_empty() {
                            continue;
                        }
                        out.push(WindowInfo {
                            id: parts[0].to_string(),
                            title,
                            process: String::new(),
                            x: parts[2].parse().unwrap_or(0),
                            y: parts[3].parse().unwrap_or(0),
                            w: parts[4].parse().unwrap_or(0),
                            h: parts[5].parse().unwrap_or(0),
                            minimized: false,
                        });
                    }
                }
                return out;
            }
        }
        Vec::new()
    }
    #[cfg(target_os = "macos")]
    {
        macos_cg_windows()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Vec::new()
    }
}

/// Connected displays. Empty if the OS helper is unavailable.
pub fn list_monitors() -> Vec<MonitorInfo> {
    #[cfg(target_os = "windows")]
    {
        return windows_list_monitors();
    }
    #[cfg(target_os = "linux")]
    {
        Vec::new()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
fn macos_cg_windows() -> Vec<WindowInfo> {
    let script = r#"
ObjC.import('CoreGraphics');
var opts = $.kCGWindowListOptionOnScreenOnly | $.kCGWindowListExcludeDesktopElements;
var list = $.CGWindowListCopyWindowInfo(opts, $.kCGNullWindowID);
var arr = ObjC.deepUnwrap(list) || [];
for (var i = 0; i < arr.length; i++) {
  var w = arr[i];
  if ((w.kCGWindowLayer || 0) !== 0) continue;
  var name = w.kCGWindowName || '';
  var owner = w.kCGWindowOwnerName || '';
  if (!name && !owner) continue;
  var b = w.kCGWindowBounds || {};
  console.log([w.kCGWindowNumber, owner, name, b.X||0, b.Y||0, b.Width||0, b.Height||0].join('\t'));
}
"#;
    let out = Command::new("osascript")
        .args(["-l", "JavaScript", "-e", script])
        .output()
        .ok();
    let Some(o) = out else {
        return Vec::new();
    };
    parse_macos_window_lines(&String::from_utf8_lossy(&o.stdout))
}

#[cfg(any(target_os = "macos", test))]
pub fn parse_macos_window_lines(s: &str) -> Vec<WindowInfo> {
    let mut out = Vec::new();
    for line in s.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        if p.len() < 7 {
            continue;
        }
        let w: i32 = p[5].trim().parse().unwrap_or(0);
        let h: i32 = p[6].trim().parse().unwrap_or(0);
        if w < 8 || h < 8 {
            continue;
        }
        let process = p[1].trim().to_string();
        let title = p[2].trim().to_string();
        if title.is_empty() && process.is_empty() {
            continue;
        }
        out.push(WindowInfo {
            id: p[0].trim().to_string(),
            title,
            process,
            x: p[3].trim().parse().unwrap_or(0),
            y: p[4].trim().parse().unwrap_or(0),
            w,
            h,
            minimized: false,
        });
    }
    out
}

/// Why window crop may be fullscreen (Linux tools missing).
pub fn window_tools_hint() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        return Some(
            "window stills: screencapture -l <CGWindowID>; recordings focus then capture display"
                .into(),
        );
    }
    #[cfg(target_os = "linux")]
    {
        let has_wmctrl = Command::new("wmctrl")
            .arg("-m")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        let has_xdotool = Command::new("xdotool")
            .arg("version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if has_wmctrl || has_xdotool {
            None
        } else {
            Some("wmctrl/xdotool missing — window crop falls back to full display".into())
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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
    let front = Command::new("lsappinfo").arg("front").output().ok()?;
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
    if let Ok(cache) = FRONT_CACHE.lock() {
        if let Some(c) = cache.as_ref() {
            if c.at.elapsed() < Duration::from_millis(500) {
                return c.val.clone();
            }
        }
    }
    let val = frontmost_app_name_uncached();
    if let Ok(mut cache) = FRONT_CACHE.lock() {
        *cache = Some(Timed {
            at: Instant::now(),
            val: val.clone(),
        });
    }
    val
}

fn frontmost_app_name_uncached() -> Option<String> {
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
        let out = Command::new("osascript")
            .args(["-e", script])
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

    #[cfg(target_os = "windows")]
    {
        windows_foreground_process_name()
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
        let script = format!(
            "tell application \"{}\" to activate",
            app_name.replace('"', "\\\"")
        );
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
        return windows_focus_app(app_name);
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
        if Command::new("gtk-launch").arg(app_name).spawn().is_ok() {
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

/// E82 — "Open with…" chooser. Windows shows the system OpenAs dialog;
/// macOS has no shell equivalent so it reveals in Finder; Linux opens the
/// default handler (no standard picker).
pub fn open_with(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("rundll32.exe")
            .args(["shell32.dll,OpenAs_RunDLL", &path.to_string_lossy()])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("open-with failed: {e}"))
    }
    #[cfg(target_os = "macos")]
    {
        reveal_in_file_manager(path)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        open_path(path)
    }
}

/// E163 — start an OS file drag of `paths` out of the app window. Windows
/// uses OLE CF_HDROP (modal DoDragDrop loop); other platforms unsupported.
pub fn start_file_drag(paths: &[std::path::PathBuf]) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        super::win32::start_file_drag(paths)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = paths;
        Err("drag-out is not supported on this platform".into())
    }
}

/// Window target in physical desktop pixels `(hwnd, x, y, w, h)` for the first
/// visible, non-minimized top-level window whose process name or title
/// contains `name` (case-insensitive). `None` when no such window exists.
///
/// The HWND lets ffmpeg gdigrab capture the window itself (`-i hwnd=…`) even
/// when it is occluded — no focus stealing required.
///
/// Resolved natively via EnumWindows/GetWindowRect — no process spawn. Exact
/// title/process match wins over fuzzy, so the rect agrees with focus_app.
#[cfg(target_os = "windows")]
pub fn window_rect_on_screen(name: &str) -> Option<(u64, i32, i32, i32, i32)> {
    super::win32::find_window_rect(name)
}

#[cfg(target_os = "windows")]
fn windows_enum_windows() -> Vec<WindowInfo> {
    super::win32::enum_windows()
        .into_iter()
        .map(|w| WindowInfo {
            id: w.hwnd.to_string(),
            title: w.title,
            process: w.process,
            x: w.x,
            y: w.y,
            w: w.w,
            h: w.h,
            minimized: w.minimized,
        })
        .collect()
}

#[cfg(target_os = "windows")]
fn windows_list_monitors() -> Vec<MonitorInfo> {
    super::win32::enum_monitors()
        .into_iter()
        .enumerate()
        .map(|(i, m)| MonitorInfo {
            index: i as u32,
            x: m.x,
            y: m.y,
            w: m.w,
            h: m.h,
            primary: m.primary,
        })
        .collect()
}

/// Foreground process name (e.g. `chrome`), else the window title.
/// Native GetForegroundWindow — no PowerShell spawn.
#[cfg(target_os = "windows")]
fn windows_foreground_process_name() -> Option<String> {
    super::win32::foreground_process_name()
}

/// Bring an existing window to the front by process-name / title fragment.
///
/// This focuses; it never launches a new instance. `Err` when nothing
/// matched or the window did not come forward, so callers capture a wrong
/// window instead of failing loudly.
///
/// Background processes (headless CLI/MCP) are normally refused foreground
/// rights by Windows; the lookup briefly clears the foreground-lock timeout
/// (the documented `SystemParametersInfo` mechanism, restored afterwards) so
/// `SetForegroundWindow` succeeds from a console too.
///
/// E238 — pure native path: `focus_window` already clears the foreground
/// lock, AttachThreadInput-glues to the foreground thread, restores a
/// minimized window, and verifies `GetForegroundWindow` — strictly stronger
/// than the old WScript.Shell.AppActivate fallback (which did none of the
/// thread-input work). No process spawn anywhere on the focus path.
#[cfg(target_os = "windows")]
fn windows_focus_app(app_name: &str) -> Result<(), String> {
    let needle = app_name.trim();
    if needle.is_empty() {
        return Ok(());
    }
    super::win32::focus_window(needle)
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
            if Command::new("open")
                .arg(u)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
            {
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
    #[cfg(windows)]
    {
        super::win32::restore_studio_to_taskbar();
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
        let posix = path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let script_reveal = format!(
            "tell application \"Finder\" to reveal POSIX file \"{}\"",
            posix
        );
        let status = Command::new("osascript")
            .args([
                "-e",
                &script_reveal,
                "-e",
                "tell application \"Finder\" to activate",
            ])
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
            let _ = Command::new("open")
                .arg("-a")
                .arg("Finder")
                .arg(parent)
                .status();
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

/// Register / unregister "run at login" so Vibecap sits in the tray after
/// sign-in. Windows writes `HKCU\...\Run\Vibecap = "<exe>" --hidden` via
/// `reg.exe` (one spawn, user-initiated — no new dep for a one-shot call).
#[cfg(target_os = "windows")]
pub fn set_run_at_login(enable: bool) -> Result<(), String> {
    // Direct advapi32 — a `reg` child would flash a console and stall the UI.
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let cmdline = format!("\"{}\" --hidden", exe.display());
    crate::platform::set_run_at_login_native(enable, &cmdline)
}

/// True when the current user has Vibecap registered to run at login.
#[cfg(target_os = "windows")]
pub fn run_at_login_enabled() -> bool {
    crate::platform::run_at_login_enabled_native()
}

#[cfg(not(target_os = "windows"))]
pub fn set_run_at_login(_enable: bool) -> Result<(), String> {
    Err("run at login is not supported on this platform".into())
}

#[cfg(not(target_os = "windows"))]
pub fn run_at_login_enabled() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_info_is_self_matches_process_name() {
        assert!(!WindowInfo {
            id: "1234".into(),
            title: "X / Home".into(),
            process: "chrome".into(),
            x: 10,
            y: 20,
            w: 800,
            h: 600,
            minimized: false,
        }
        .is_self());
        assert!(WindowInfo {
            id: "1".into(),
            title: "Vibecap".into(),
            process: "vibecap".into(),
            x: 0,
            y: 0,
            w: 100,
            h: 100,
            minimized: false,
        }
        .is_self());
    }

    #[test]
    fn parse_macos_window_rows() {
        let rows = parse_macos_window_lines(
            "42\tGoogle Chrome\tInbox\t10\t20\t1280\t800\n7\tFinder\tDesktop\t0\t0\t2\t2\n",
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "42");
        assert_eq!(rows[0].process, "Google Chrome");
        assert_eq!(rows[0].w, 1280);
    }
}
