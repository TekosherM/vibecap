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

/// Window target in physical desktop pixels `(hwnd, x, y, w, h)` for the first
/// visible, non-minimized top-level window whose process name or title
/// contains `name` (case-insensitive). `None` when no such window exists.
///
/// The HWND lets ffmpeg gdigrab capture the window itself (`-i hwnd=…`) even
/// when it is occluded — no focus stealing required.
///
/// PowerShell is used so this works without extra crates. The lookup process
/// opts into per-monitor DPI awareness so the pixels line up with what
/// ffmpeg gdigrab captures.
#[cfg(target_os = "windows")]
pub fn window_rect_on_screen(name: &str) -> Option<(u64, i32, i32, i32, i32)> {
    let needle = name.trim();
    if needle.is_empty() {
        return None;
    }
    let esc = ps_like_escape(needle);
    let script = format!(
        r###"
try {{
  Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; [StructLayout(LayoutKind.Sequential)] public struct VbRect {{ public int Left; public int Top; public int Right; public int Bottom; }} public static class VbWin {{ [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out VbRect r); [DllImport("user32.dll")] public static extern int GetSystemMetrics(int n); [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h); [DllImport("shcore.dll")] public static extern int SetProcessDpiAwareness(int v); }}' | Out-Null
  try {{ [void][VbWin]::SetProcessDpiAwareness(2) }} catch {{}}
  # Virtual screen (all monitors): origin + size, so maximized windows whose
  # invisible borders spill past the edges get clamped to grabbable pixels.
  $vx = [VbWin]::GetSystemMetrics(76)
  $vy = [VbWin]::GetSystemMetrics(77)
  $vw = [VbWin]::GetSystemMetrics(78)
  $vh = [VbWin]::GetSystemMetrics(79)
  foreach ($p in Get-Process -ErrorAction SilentlyContinue) {{
    try {{
      $t = $p.MainWindowTitle
      $n = $p.ProcessName
      if (($t -like '*{esc}*') -or ($n -like '*{esc}*')) {{
        $h = $p.MainWindowHandle
        if ($h -eq [IntPtr]::Zero) {{ continue }}
        if ([VbWin]::IsIconic($h)) {{ continue }}
        # NOTE: PowerShell variables are case-insensitive — keep these names
        # distinct from $t/$n/$h/$p above ($R would clobber the $rc struct).
        $rc = New-Object VbRect
        if ([VbWin]::GetWindowRect($h, [ref]$rc)) {{
          if ($rc.Left -le -10000 -or $rc.Top -le -10000) {{ continue }}
          $cL = [Math]::Max($rc.Left, $vx)
          $cT = [Math]::Max($rc.Top, $vy)
          $cR = [Math]::Min($rc.Right, $vx + $vw)
          $cB = [Math]::Min($rc.Bottom, $vy + $vh)
          $cW = $cR - $cL
          $cH = $cB - $cT
          if ($cW -gt 0 -and $cH -gt 0) {{ Write-Output "$([Int64]$h) $cL $cT $cW $cH"; exit 0 }}
        }}
      }}
    }} catch {{}}
  }}
  exit 1
}} catch {{ exit 1 }}
"###,
    );
    let out = windows_powershell(&script).output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_window_target_line(&String::from_utf8_lossy(&out.stdout))
}

/// Parse the first `hwnd x y w h` line from [`window_rect_on_screen`] output.
#[cfg(any(target_os = "windows", test))]
pub fn parse_window_target_line(s: &str) -> Option<(u64, i32, i32, i32, i32)> {
    for line in s.lines() {
        let nums: Vec<i64> = line
            .split_whitespace()
            .filter_map(|p| p.parse().ok())
            .collect();
        if nums.len() >= 5
            && nums[0] > 0
            && nums[3] > 0
            && nums[4] > 0
            && u64::try_from(nums[0]).is_ok()
        {
            return Some((
                nums[0] as u64,
                nums[1] as i32,
                nums[2] as i32,
                nums[3] as i32,
                nums[4] as i32,
            ));
        }
    }
    None
}

/// Escape a literal for a PowerShell single-quoted string.
#[cfg(any(target_os = "windows", test))]
pub fn ps_escape(s: &str) -> String {
    s.replace('\'', "''")
}

/// Escape a literal for a PowerShell `-like '*…*'` pattern fragment.
#[cfg(any(target_os = "windows", test))]
pub fn ps_like_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '*' | '?' | '[' | ']') {
            out.push('`');
        }
        out.push(c);
    }
    out.replace('\'', "''")
}

#[cfg(target_os = "windows")]
fn windows_enum_windows() -> Vec<WindowInfo> {
    let script = r###"
try {
  Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class VbList {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L; public int T; public int R; public int B; }
  public static string Dump() {
    var sb = new StringBuilder();
    EnumWindows((h, l) => {
      if (!IsWindowVisible(h)) return true;
      var t = new StringBuilder(512);
      GetWindowText(h, t, 512);
      var title = t.ToString().Replace("\t", " ");
      uint pid = 0;
      GetWindowThreadProcessId(h, out pid);
      RECT rc;
      if (!GetWindowRect(h, out rc)) return true;
      int w = rc.R - rc.L, ht = rc.B - rc.T;
      if (w < 8 || ht < 8) return true;
      int min = IsIconic(h) ? 1 : 0;
      sb.Append((long)h).Append('\t').Append(pid).Append('\t').Append('\t').Append(title)
        .Append('\t').Append(rc.L).Append('\t').Append(rc.T).Append('\t').Append(w).Append('\t').Append(ht)
        .Append('\t').Append(min).Append('\n');
      return true;
    }, IntPtr.Zero);
    return sb.ToString();
  }
}
'@
  $dump = [VbList]::Dump()
  foreach ($line in $dump -split "`n") {
    if ($line.Trim() -eq '') { continue }
    $p = $line -split "`t", 9
    if ($p.Length -lt 9) { continue }
    $pid = 0; [void][int]::TryParse($p[1], [ref]$pid)
    $pn = ''
    if ($pid -gt 0) { try { $pn = (Get-Process -Id $pid -ErrorAction Stop).ProcessName } catch {} }
    Write-Output ($p[0] + "`t" + $p[1] + "`t" + $pn + "`t" + $p[3] + "`t" + $p[4] + "`t" + $p[5] + "`t" + $p[6] + "`t" + $p[7] + "`t" + $p[8])
  }
} catch { }
"###;
    let out = windows_powershell(script).output().ok();
    let Some(o) = out else {
        return Vec::new();
    };
    parse_window_info_lines(&String::from_utf8_lossy(&o.stdout))
}

#[cfg(any(target_os = "windows", test))]
pub fn parse_window_info_lines(s: &str) -> Vec<WindowInfo> {
    let mut out = Vec::new();
    for line in s.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        if p.len() < 9 {
            continue;
        }
        let w: i32 = p[6].trim().parse().unwrap_or(0);
        let h: i32 = p[7].trim().parse().unwrap_or(0);
        if w < 8 || h < 8 {
            continue;
        }
        let title = p[3].trim().to_string();
        let process = p[2].trim().to_string();
        if title.is_empty() && process.is_empty() {
            continue;
        }
        out.push(WindowInfo {
            id: p[0].trim().to_string(),
            title,
            process,
            x: p[4].trim().parse().unwrap_or(0),
            y: p[5].trim().parse().unwrap_or(0),
            w,
            h,
            minimized: p[8].trim() == "1",
        });
    }
    out
}

#[cfg(target_os = "windows")]
fn windows_list_monitors() -> Vec<MonitorInfo> {
    let script = r###"
try {
  Add-Type -AssemblyName System.Windows.Forms | Out-Null
  $i = 0
  foreach ($s in [System.Windows.Forms.Screen]::AllScreens) {
    $b = $s.Bounds
    $pr = if ($s.Primary) { 1 } else { 0 }
    Write-Output ("$i $($b.X) $($b.Y) $($b.Width) $($b.Height) $pr")
    $i++
  }
} catch {}
"###;
    let out = windows_powershell(script).output().ok();
    let Some(o) = out else {
        return Vec::new();
    };
    parse_monitor_lines(&String::from_utf8_lossy(&o.stdout))
}

#[cfg(any(target_os = "windows", test))]
pub fn parse_monitor_lines(s: &str) -> Vec<MonitorInfo> {
    let mut out = Vec::new();
    for line in s.lines() {
        let n: Vec<i64> = line
            .split_whitespace()
            .filter_map(|p| p.parse().ok())
            .collect();
        if n.len() >= 6 {
            out.push(MonitorInfo {
                index: n[0] as u32,
                x: n[1] as i32,
                y: n[2] as i32,
                w: n[3] as i32,
                h: n[4] as i32,
                primary: n[5] != 0,
            });
        }
    }
    out
}

#[cfg(target_os = "windows")]
fn windows_powershell(script: &str) -> Command {
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    super::ffmpeg::silence_console(&mut cmd);
    cmd
}

/// Foreground process name (e.g. `chrome`), else the window title.
#[cfg(target_os = "windows")]
fn windows_foreground_process_name() -> Option<String> {
    let script = r###"
try {
  Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; using System.Text; public static class VbFg { [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow(); [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid); [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n); }' | Out-Null
  $hw = [VbFg]::GetForegroundWindow()
  if ($hw -eq [IntPtr]::Zero) { exit 1 }
  $pid = 0; [void][VbFg]::GetWindowThreadProcessId($hw, [ref]$pid)
  try { $pn = (Get-Process -Id $pid -ErrorAction Stop).ProcessName } catch { $pn = '' }
  if ($pn -ne '') { Write-Output $pn; exit 0 }
  $sb = New-Object System.Text.StringBuilder 512
  [void][VbFg]::GetWindowText($hw, $sb, 512)
  $t = $sb.ToString()
  if ($t -ne '') { Write-Output $t; exit 0 }
  exit 1
} catch { exit 1 }
"###;
    let out = windows_powershell(script).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
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
#[cfg(target_os = "windows")]
fn windows_focus_app(app_name: &str) -> Result<(), String> {
    let needle = app_name.trim();
    if needle.is_empty() {
        return Ok(());
    }
    let esc = ps_like_escape(needle);
    let lit = ps_escape(needle);
    let script = format!(
        r###"
try {{
  Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class VbFocus {{ [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow(); [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n); [DllImport("user32.dll")] public static extern bool SystemParametersInfo(uint a, uint b, ref uint c, uint d); [DllImport("user32.dll")] public static extern bool SystemParametersInfo(uint a, uint b, System.UIntPtr c, uint d); }}' | Out-Null
  # Let a background caller take the foreground (restored below).
  $prevLock = [uint32]0
  try {{ [void][VbFocus]::SystemParametersInfo(0x2000, 0, [ref]$prevLock, 0) }} catch {{}}
  try {{ [void][VbFocus]::SystemParametersInfo(0x2001, 0, [System.UIntPtr]::Zero, 0) }} catch {{}}
  $focused = $false
  foreach ($pass in 1, 2) {{
    foreach ($p in Get-Process -ErrorAction SilentlyContinue) {{
      try {{
        $t = $p.MainWindowTitle
        $n = $p.ProcessName
        $exact = ($n -eq '{lit}') -or ($t -eq '{lit}')
        $fuzzy = (($t -like '*{esc}*') -or ($n -like '*{esc}*'))
        if (($pass -eq 1 -and $exact) -or ($pass -eq 2 -and -not $exact -and $fuzzy)) {{
          $h = $p.MainWindowHandle
          if ($h -eq [IntPtr]::Zero) {{ continue }}
          [void][VbFocus]::ShowWindow($h, 9)
          [void][VbFocus]::SetForegroundWindow($h)
          Start-Sleep -Milliseconds 200
          if ([VbFocus]::GetForegroundWindow() -eq $h) {{ $focused = $true; break }}
        }}
      }} catch {{}}
    }}
    if ($focused) {{ break }}
  }}
  try {{ [void][VbFocus]::SystemParametersInfo(0x2001, 0, [System.UIntPtr]$prevLock, 0) }} catch {{}}
  if (-not $focused) {{
    $ws = New-Object -ComObject WScript.Shell
    if ($ws.AppActivate('{lit}')) {{ Start-Sleep -Milliseconds 400; exit 0 }}
    exit 1
  }}
  exit 0
}} catch {{ exit 1 }}
"###,
    );
    let status = windows_powershell(&script)
        .status()
        .map_err(|e| format!("focus helper failed to start: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "could not bring “{needle}” to the front (no visible window matched)"
        ))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_window_target_reads_hwnd_xywh() {
        assert_eq!(
            parse_window_target_line("67920 60 60 2880 1484\r\n"),
            Some((67920, 60, 60, 2880, 1484))
        );
        assert_eq!(parse_window_target_line("no numbers here"), None);
        assert_eq!(parse_window_target_line("0 0 800 600"), None);
        assert_eq!(parse_window_target_line("1 2 0 4"), None);
        assert_eq!(parse_window_target_line(""), None);
    }

    #[test]
    fn ps_escape_doubles_single_quotes() {
        assert_eq!(ps_escape("it's"), "it''s");
        assert_eq!(ps_escape("plain"), "plain");
    }

    #[test]
    fn ps_like_escape_neutralizes_wildcards() {
        assert_eq!(ps_like_escape("a*b?c[d]"), "a`*b`?c`[d`]");
        assert_eq!(ps_like_escape("chrome"), "chrome");
    }

    #[test]
    fn parse_window_info_tab_rows() {
        let rows = parse_window_info_lines(
            "1234\t99\tchrome\tX / Home\t10\t20\t800\t600\t0\n5\t1\texplorer\t\t0\t0\t4\t4\t0\n",
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].process, "chrome");
        assert_eq!(rows[0].title, "X / Home");
        assert!(!rows[0].is_self());
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
    fn parse_monitor_lines_reads_primary() {
        let m = parse_monitor_lines("0 0 0 1920 1080 1\n1 -1920 0 1920 1080 0\n");
        assert_eq!(m.len(), 2);
        assert!(m[0].primary);
        assert_eq!(m[1].x, -1920);
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
