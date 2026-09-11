//! Windows HWND helpers: keep a taskbar button, restore from tray, hide from shots via minimize.
//!
//! `Visible(false)` drops the taskbar entry and can kill child viewports.
//! Off-screen park is clamped back onto the desktop by Windows 10/11, so the
//! studio reappears in gdigrab. Minimize keeps the taskbar ("quickbar") button
//! and removes pixels from the shot.

#![cfg(windows)]

use std::ffi::c_void;

type Hwnd = *mut c_void;

const SW_RESTORE: i32 = 9;
const SW_SHOW: i32 = 5;
const SW_MINIMIZE: i32 = 6;
const GWL_EXSTYLE: i32 = -20;
const WS_EX_APPWINDOW: isize = 0x0004_0000;
const WS_EX_TOOLWINDOW: isize = 0x0000_0080;

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(cb: unsafe extern "system" fn(Hwnd, isize) -> i32, lparam: isize) -> i32;
    fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
    fn GetWindowTextW(hwnd: Hwnd, lp: *mut u16, n: i32) -> i32;
    fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
    fn SetForegroundWindow(hwnd: Hwnd) -> i32;
    fn AllowSetForegroundWindow(pid: u32) -> i32;
    fn GetWindowLongPtrW(hwnd: Hwnd, n: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: Hwnd, n: i32, v: isize) -> isize;
    fn IsWindow(hwnd: Hwnd) -> i32;
}

struct EnumState {
    pid: u32,
    hwnd: Hwnd,
}

unsafe extern "system" fn enum_cb(hwnd: Hwnd, lparam: isize) -> i32 {
    let state = unsafe { &mut *(lparam as *mut EnumState) };
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid != state.pid {
        return 1;
    }
    let mut buf = [0u16; 256];
    let n = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), 256) };
    if n <= 0 {
        return 1;
    }
    let title = String::from_utf16_lossy(&buf[..n as usize]);
    // Main studio, not the region overlay / REC bar / countdown.
    if title.starts_with("Vibecap Studio") {
        state.hwnd = hwnd;
        return 0;
    }
    1
}

fn find_studio_hwnd() -> Option<Hwnd> {
    let mut state = EnumState {
        pid: std::process::id(),
        hwnd: std::ptr::null_mut(),
    };
    unsafe {
        EnumWindows(enum_cb, &mut state as *mut EnumState as isize);
    }
    if state.hwnd.is_null() {
        None
    } else {
        Some(state.hwnd)
    }
}

fn force_appwindow(hwnd: Hwnd) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let ex = (ex | WS_EX_APPWINDOW) & !WS_EX_TOOLWINDOW;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);
    }
}

/// Minimize the studio. Stays on the taskbar; not in gdigrab.
pub fn minimize_studio() {
    if let Some(hwnd) = find_studio_hwnd() {
        force_appwindow(hwnd);
        unsafe {
            ShowWindow(hwnd, SW_MINIMIZE);
        }
    }
}

/// Unminimize, keep AppWindow (taskbar), and foreground.
pub fn restore_studio_to_taskbar() {
    let Some(hwnd) = find_studio_hwnd() else {
        return;
    };
    if unsafe { IsWindow(hwnd) } == 0 {
        return;
    }
    force_appwindow(hwnd);
    unsafe {
        ShowWindow(hwnd, SW_RESTORE);
        ShowWindow(hwnd, SW_SHOW);
        let _ = AllowSetForegroundWindow(std::process::id());
        SetForegroundWindow(hwnd);
    }
}

// ── Window / monitor enumeration + focus ────────────────────────────────────
// These replace the old PowerShell helpers. Each PS call spawned a process and
// compiled an Add-Type shim (~300-800 ms); the direct calls are sub-ms. The
// capture path used to chain three of them per windowed shot.

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct RawMonitorInfo {
    cb_size: u32,
    rc_monitor: RawRect,
    rc_work: RawRect,
    flags: u32,
}

type Hmonitor = *mut c_void;

const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const SPI_GETFOREGROUNDLOCKTIMEOUT: u32 = 0x2000;
const SPI_SETFOREGROUNDLOCKTIMEOUT: u32 = 0x2001;
const MONITORINFOF_PRIMARY: u32 = 0x1;
const SM_XVIRTUALSCREEN: i32 = 76;
const SM_YVIRTUALSCREEN: i32 = 77;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;

#[link(name = "user32")]
extern "system" {
    fn IsWindowVisible(hwnd: Hwnd) -> i32;
    fn IsIconic(hwnd: Hwnd) -> i32;
    fn GetWindowRect(hwnd: Hwnd, rc: *mut RawRect) -> i32;
    fn GetForegroundWindow() -> Hwnd;
    fn BringWindowToTop(hwnd: Hwnd) -> i32;
    fn AttachThreadInput(from: u32, to: u32, attach: i32) -> i32;
    fn SystemParametersInfoW(action: u32, param: u32, pv: *mut c_void, winini: u32) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn EnumDisplayMonitors(
        hdc: *mut c_void,
        clip: *const RawRect,
        cb: unsafe extern "system" fn(Hmonitor, *mut c_void, *mut RawRect, isize) -> i32,
        data: isize,
    ) -> i32;
    fn GetMonitorInfoW(mon: Hmonitor, info: *mut RawMonitorInfo) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn QueryFullProcessImageNameW(
        handle: *mut c_void,
        flags: u32,
        buf: *mut u16,
        size: *mut u32,
    ) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn GetCurrentThreadId() -> u32;
}

/// Exe stem for a pid (e.g. `chrome`) — same shape as Get-Process ProcessName.
/// Empty when the process refuses a query handle (elevated / protected).
fn process_name(pid: u32) -> String {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return String::new();
        }
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 || len == 0 || len as usize > buf.len() {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        std::path::Path::new(&path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }
}

/// A visible top-level window with geometry, for the picker and matching.
pub struct EnumWindow {
    pub hwnd: u64,
    pub process: String,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub minimized: bool,
}

struct EnumAllState {
    out: Vec<EnumWindow>,
    names: std::collections::HashMap<u32, String>,
}

unsafe extern "system" fn enum_all_cb(hwnd: Hwnd, lparam: isize) -> i32 {
    let state = unsafe { &mut *(lparam as *mut EnumAllState) };
    unsafe {
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let mut rc = RawRect::default();
        if GetWindowRect(hwnd, &mut rc) == 0 {
            return 1;
        }
        let w = rc.right - rc.left;
        let h = rc.bottom - rc.top;
        if w < 8 || h < 8 {
            return 1;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), 512);
        let title = if n > 0 {
            String::from_utf16_lossy(&buf[..n as usize]).replace('\t', " ")
        } else {
            String::new()
        };
        let process = state
            .names
            .entry(pid)
            .or_insert_with(|| process_name(pid))
            .clone();
        if title.is_empty() && process.is_empty() {
            return 1;
        }
        state.out.push(EnumWindow {
            hwnd: hwnd as u64,
            process,
            title,
            x: rc.left,
            y: rc.top,
            w,
            h,
            minimized: IsIconic(hwnd) != 0,
        });
    }
    1
}

/// All visible top-level windows in Z-order (topmost first).
pub fn enum_windows() -> Vec<EnumWindow> {
    let mut state = EnumAllState {
        out: Vec::new(),
        names: std::collections::HashMap::new(),
    };
    unsafe {
        EnumWindows(enum_all_cb, &mut state as *mut EnumAllState as isize);
    }
    state.out
}

/// Case-insensitive `needle` vs window title / process stem. Exact first,
/// then contains — fixes "code" hitting the wrong Code window.
fn match_score(w: &EnumWindow, needle_lower: &str) -> Option<u8> {
    if w.title.is_empty() && w.process.is_empty() {
        return None;
    }
    let t = w.title.to_lowercase();
    let p = w.process.to_lowercase();
    if t == needle_lower || p == needle_lower {
        Some(0)
    } else if t.contains(needle_lower) || p.contains(needle_lower) {
        Some(1)
    } else {
        None
    }
}

fn find_window(needle: &str) -> Option<EnumWindow> {
    let nl = needle.trim().to_lowercase();
    if nl.is_empty() {
        return None;
    }
    let wins = enum_windows();
    // Prefer non-minimized, then exact over fuzzy.
    wins.into_iter()
        .filter(|w| match_score(w, &nl).is_some())
        .min_by_key(|w| (w.minimized as u8, match_score(w, &nl).unwrap()))
}

/// `(hwnd, x, y, w, h)` for the first window matching `needle`, clamped to the
/// virtual screen so maximized windows' invisible borders stay grabbable.
/// `None` when nothing visible matches — callers must never fall back to
/// fullscreen silently.
pub fn find_window_rect(needle: &str) -> Option<(u64, i32, i32, i32, i32)> {
    let w = find_window(needle)?;
    if w.minimized || w.x <= -10000 || w.y <= -10000 {
        return None;
    }
    let (vx, vy, vw, vh) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    let cl = w.x.max(vx);
    let ct = w.y.max(vy);
    let cr = (w.x + w.w).min(vx + vw);
    let cb = (w.y + w.h).min(vy + vh);
    let (cw, ch) = (cr - cl, cb - ct);
    if cw > 0 && ch > 0 {
        Some((w.hwnd, cl, ct, cw, ch))
    } else {
        None
    }
}

/// Foreground process name (exe stem), falling back to the window title.
pub fn foreground_process_name() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let pn = process_name(pid);
        if !pn.is_empty() {
            return Some(pn);
        }
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), 512);
        if n > 0 {
            Some(String::from_utf16_lossy(&buf[..n as usize]))
        } else {
            None
        }
    }
}

/// Physical monitor rects in virtual-desktop coordinates.
pub struct Monitor {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub primary: bool,
}

unsafe extern "system" fn mon_cb(
    mon: Hmonitor,
    _hdc: *mut c_void,
    _rc: *mut RawRect,
    data: isize,
) -> i32 {
    let out = unsafe { &mut *(data as *mut Vec<Monitor>) };
    let mut info = RawMonitorInfo {
        cb_size: std::mem::size_of::<RawMonitorInfo>() as u32,
        rc_monitor: RawRect::default(),
        rc_work: RawRect::default(),
        flags: 0,
    };
    unsafe {
        if GetMonitorInfoW(mon, &mut info) != 0 {
            let r = info.rc_monitor;
            out.push(Monitor {
                x: r.left,
                y: r.top,
                w: r.right - r.left,
                h: r.bottom - r.top,
                primary: info.flags & MONITORINFOF_PRIMARY != 0,
            });
        }
    }
    1
}

pub fn enum_monitors() -> Vec<Monitor> {
    let mut out = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            mon_cb,
            &mut out as *mut Vec<Monitor> as isize,
        );
    }
    out
}

/// Bring a window to the foreground by title/process fragment.
///
/// Clears the foreground-lock timeout (documented SPI mechanism, restored
/// afterwards) and attaches to the foreground thread so SetForegroundWindow
/// succeeds from a background process. `Err` when nothing matched or the
/// window did not come forward — callers fall back to HWND capture or the
/// legacy AppActivate path.
pub fn focus_window(needle: &str) -> Result<(), String> {
    let needle = needle.trim();
    if needle.is_empty() {
        return Ok(());
    }
    let Some(w) = find_window(needle) else {
        return Err(format!("no window matched “{needle}”"));
    };
    let hwnd = w.hwnd as Hwnd;
    unsafe {
        let mut prev_lock: u32 = 0;
        SystemParametersInfoW(
            SPI_GETFOREGROUNDLOCKTIMEOUT,
            0,
            &mut prev_lock as *mut u32 as *mut c_void,
            0,
        );
        SystemParametersInfoW(SPI_SETFOREGROUNDLOCKTIMEOUT, 0, std::ptr::null_mut(), 0);

        if w.minimized {
            ShowWindow(hwnd, SW_RESTORE);
        }
        let fg = GetForegroundWindow();
        let fg_tid = GetWindowThreadProcessId(fg, std::ptr::null_mut());
        let cur = GetCurrentThreadId();
        if fg_tid != 0 && fg_tid != cur {
            AttachThreadInput(cur, fg_tid, 1);
        }
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        if fg_tid != 0 && fg_tid != cur {
            AttachThreadInput(cur, fg_tid, 0);
        }
        SystemParametersInfoW(
            SPI_SETFOREGROUNDLOCKTIMEOUT,
            0,
            prev_lock as usize as *mut c_void,
            0,
        );
        std::thread::sleep(std::time::Duration::from_millis(120));
        if GetForegroundWindow() == hwnd {
            Ok(())
        } else {
            Err(format!("“{needle}” did not come forward"))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn studio_title_prefix_is_stable() {
        assert!("Vibecap Studio · 123".starts_with("Vibecap Studio"));
        assert!(!"Vibecap Region".starts_with("Vibecap Studio"));
        assert!(!"Vibecap Recorder".starts_with("Vibecap Studio"));
    }
}
