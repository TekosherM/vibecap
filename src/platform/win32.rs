//! Windows HWND helpers: keep a taskbar button, restore from tray, hide from shots via minimize.
//!
//! `Visible(false)` drops the taskbar entry and can kill child viewports.
//! Off-screen park is clamped back onto the desktop by Windows 10/11, so the
//! studio reappears in gdigrab. Minimize keeps the taskbar ("quickbar") button
//! and removes pixels from the shot.

#![cfg(windows)]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;

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
    fn GetWindowLongPtrW(hwnd: Hwnd, n: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: Hwnd, n: i32, v: isize) -> isize;
    fn IsWindow(hwnd: Hwnd) -> i32;
    fn SetWindowDisplayAffinity(hwnd: Hwnd, affinity: u32) -> i32;
}

struct EnumState {
    pid: u32,
    prefix: String,
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
    if title.starts_with(&state.prefix) {
        state.hwnd = hwnd;
        return 0;
    }
    1
}

fn find_hwnd_by_title_prefix(prefix: &str) -> Option<Hwnd> {
    let mut state = EnumState {
        pid: std::process::id(),
        prefix: prefix.to_string(),
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

// Main studio, not the region overlay / REC bar / countdown.
fn find_studio_hwnd() -> Option<Hwnd> {
    find_hwnd_by_title_prefix("Vibecap Studio")
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
/// SetForegroundWindow alone gets silently refused when our process doesn't
/// hold foreground rights (e.g. coming back from a capture hide) — clear the
/// lock timeout and attach to the foreground thread first, same as focus_window.
pub fn restore_studio_to_taskbar() {
    let Some(hwnd) = find_studio_hwnd() else {
        return;
    };
    if unsafe { IsWindow(hwnd) } == 0 {
        return;
    }
    force_appwindow(hwnd);
    unsafe {
        let mut prev_lock = 0u32;
        SystemParametersInfoW(
            SPI_GETFOREGROUNDLOCKTIMEOUT,
            0,
            &mut prev_lock as *mut u32 as *mut c_void,
            0,
        );
        SystemParametersInfoW(SPI_SETFOREGROUNDLOCKTIMEOUT, 0, std::ptr::null_mut(), 0);

        let fg = GetForegroundWindow();
        let fg_tid = GetWindowThreadProcessId(fg, std::ptr::null_mut());
        let cur = GetCurrentThreadId();
        if fg_tid != 0 && fg_tid != cur {
            AttachThreadInput(cur, fg_tid, 1);
        }
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            ShowWindow(hwnd, SW_SHOW);
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
    }
}

const SW_HIDE: i32 = 0;

/// Hide the studio instantly via SW_HIDE — no minimize animation, so a capture
/// worker may grab after ~100 ms instead of ~450 ms. For capture hides only:
/// the taskbar button is gone while hidden (tray-hide must keep `minimize_studio`),
/// and owned child windows hide with the owner — callers must have no live
/// child viewports (region overlay / REC bar are created later or kept on
/// the minimized path).
pub fn hide_studio_window() {
    if let Some(hwnd) = find_studio_hwnd() {
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

/// True while the studio window is iconic (tray-minimized or capture-parked)
/// or SW_HIDE-hidden — either way it gets no WM_PAINT, so `update()` is asleep
/// and an external event must first restore the HWND to reach the egui loop.
pub fn studio_is_minimized() -> bool {
    find_studio_hwnd()
        .map(|hwnd| unsafe { IsIconic(hwnd) != 0 || IsWindowVisible(hwnd) == 0 })
        .unwrap_or(false)
}

const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;
const WDA_NONE: u32 = 0;

/// Toggle WDA_EXCLUDEFROMCAPTURE on the studio: the window stays on screen
/// but is invisible to BitBlt/gdigrab grabs. Region pick uses this instead of
/// a hide — the snap can fire immediately and the overlay landing is the only
/// transition (Snagit-style). Returns false when the API refuses
/// (pre-Win10-2004) or no HWND exists — callers then fall back to hiding.
pub fn set_studio_capture_excluded(excluded: bool) -> bool {
    find_studio_hwnd()
        .map(|hwnd| unsafe {
            SetWindowDisplayAffinity(
                hwnd,
                if excluded {
                    WDA_EXCLUDEFROMCAPTURE
                } else {
                    WDA_NONE
                },
            ) != 0
        })
        .unwrap_or(false)
}

/// Result of applying display-affinity to one of our windows by title prefix.
pub enum ExcludeStatus {
    /// HWND found and affinity applied.
    Applied,
    /// No window with that prefix in this process — maybe not created yet;
    /// the caller may retry next frame.
    NotFound,
    /// Window exists but SetWindowDisplayAffinity refused.
    Denied,
}

/// Toggle WDA_EXCLUDEFROMCAPTURE on one of our windows by title prefix —
/// used on the "Vibecap Region" overlay so it can sit on screen during the
/// region freeze snap without freezing itself into the backdrop.
pub fn set_title_capture_excluded(title_prefix: &str, excluded: bool) -> ExcludeStatus {
    let Some(hwnd) = find_hwnd_by_title_prefix(title_prefix) else {
        return ExcludeStatus::NotFound;
    };
    let ok = unsafe {
        SetWindowDisplayAffinity(
            hwnd,
            if excluded {
                WDA_EXCLUDEFROMCAPTURE
            } else {
                WDA_NONE
            },
        )
    };
    if ok != 0 {
        ExcludeStatus::Applied
    } else {
        ExcludeStatus::Denied
    }
}

// ── Native desktop still (GDI BitBlt) ───────────────────────────────────────
// Reads the same composited pixels ffmpeg gdigrab reads, minus the ~300–500 ms
// process spawn. Callers fall back to the ffmpeg path on any error.

const SRCCOPY: u32 = 0x00CC_0020;
const CAPTUREBLT: u32 = 0x4000_0000;
const DI_NORMAL: u32 = 0x0003;
const CURSOR_SHOWING: u32 = 0x0000_0001;
const DIB_RGB_COLORS: u32 = 0;
const BI_RGB: u32 = 0;

#[repr(C)]
struct BitmapInfoHeader {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_pels_per_meter: i32,
    bi_y_pels_per_meter: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[repr(C)]
struct BitmapInfo {
    bmi_header: BitmapInfoHeader,
    bmi_colors: [u32; 1],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct CursorInfo {
    cb_size: u32,
    flags: u32,
    h_cursor: *mut c_void,
    pt_screen_pos: Point,
}

#[link(name = "user32")]
extern "system" {
    fn GetDC(hwnd: Hwnd) -> *mut c_void;
    fn ReleaseDC(hwnd: Hwnd, hdc: *mut c_void) -> i32;
    fn GetCursorInfo(ci: *mut CursorInfo) -> i32;
    fn DrawIconEx(
        hdc: *mut c_void,
        x: i32,
        y: i32,
        hicon: *mut c_void,
        cx: i32,
        cy: i32,
        istep: u32,
        hbr: *mut c_void,
        flags: u32,
    ) -> i32;
    fn SetProcessDPIAware() -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: *mut c_void) -> *mut c_void;
    fn CreateCompatibleBitmap(hdc: *mut c_void, w: i32, h: i32) -> *mut c_void;
    fn SelectObject(hdc: *mut c_void, obj: *mut c_void) -> *mut c_void;
    fn BitBlt(
        ddc: *mut c_void,
        dx: i32,
        dy: i32,
        w: i32,
        h: i32,
        sdc: *mut c_void,
        sx: i32,
        sy: i32,
        rop: u32,
    ) -> i32;
    fn GetDIBits(
        hdc: *mut c_void,
        hbm: *mut c_void,
        start: u32,
        lines: u32,
        bits: *mut c_void,
        bmi: *mut BitmapInfo,
        usage: u32,
    ) -> i32;
    fn DeleteObject(obj: *mut c_void) -> i32;
    fn DeleteDC(hdc: *mut c_void) -> i32;
}

/// Grab the composited desktop (all monitors) or a rect of it via GDI BitBlt —
/// the same pixels ffmpeg gdigrab reads, minus the ~300–500 ms process spawn.
/// `region` uses virtual-screen coordinates and may be negative left of the
/// primary monitor. Returns Err so the caller can fall back to ffmpeg.
pub fn native_desktop_still(
    out: &std::path::Path,
    region: Option<super::capture::ScreenRect>,
    draw_mouse: bool,
) -> Result<(), String> {
    // Headless callers may reach us before winit set PMv2 awareness — upgrade
    // so GetDC returns physical pixels rather than a scaled virtual bitmap.
    unsafe { SetProcessDPIAware() };

    let (vx, vy, vw, vh) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    if vw <= 0 || vh <= 0 {
        return Err("no virtual screen metrics".into());
    }
    // Intersect the requested rect with the virtual screen.
    let (x, y, w, h) = match region {
        Some(r) => {
            let x0 = r.x.max(vx);
            let y0 = r.y.max(vy);
            let x1 = (r.x + r.w).min(vx + vw);
            let y1 = (r.y + r.h).min(vy + vh);
            (x0, y0, x1 - x0, y1 - y0)
        }
        None => (vx, vy, vw, vh),
    };
    if w <= 0 || h <= 0 {
        return Err(format!("region {w}x{h} outside virtual screen"));
    }

    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        if screen.is_null() {
            return Err("GetDC failed".into());
        }
        let mem = CreateCompatibleDC(screen);
        let bmp = if mem.is_null() {
            std::ptr::null_mut()
        } else {
            CreateCompatibleBitmap(screen, w, h)
        };
        if mem.is_null() || bmp.is_null() {
            if !bmp.is_null() {
                DeleteObject(bmp);
            }
            if !mem.is_null() {
                DeleteDC(mem);
            }
            ReleaseDC(std::ptr::null_mut(), screen);
            return Err("CreateCompatibleDC/Bitmap failed".into());
        }
        let old = SelectObject(mem, bmp);
        let blt = BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY | CAPTUREBLT);
        if blt == 0 {
            SelectObject(mem, old);
            DeleteObject(bmp);
            DeleteDC(mem);
            ReleaseDC(std::ptr::null_mut(), screen);
            return Err("BitBlt failed".into());
        }

        if draw_mouse {
            let mut ci = CursorInfo {
                cb_size: std::mem::size_of::<CursorInfo>() as u32,
                flags: 0,
                h_cursor: std::ptr::null_mut(),
                pt_screen_pos: Point { x: 0, y: 0 },
            };
            if GetCursorInfo(&mut ci) != 0 && ci.flags & CURSOR_SHOWING != 0 {
                DrawIconEx(
                    mem,
                    ci.pt_screen_pos.x - x,
                    ci.pt_screen_pos.y - y,
                    ci.h_cursor,
                    0,
                    0,
                    0,
                    std::ptr::null_mut(),
                    DI_NORMAL,
                );
            }
        }

        let mut bmi = BitmapInfo {
            bmi_header: BitmapInfoHeader {
                bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                bi_width: w,
                bi_height: -h, // negative → top-down rows
                bi_planes: 1,
                bi_bit_count: 32,
                bi_compression: BI_RGB,
                bi_size_image: 0,
                bi_x_pels_per_meter: 0,
                bi_y_pels_per_meter: 0,
                bi_clr_used: 0,
                bi_clr_important: 0,
            },
            bmi_colors: [0],
        };
        let mut buf = vec![0u8; w as usize * h as usize * 4];
        let lines = GetDIBits(
            mem,
            bmp,
            0,
            h as u32,
            buf.as_mut_ptr() as *mut c_void,
            &mut bmi,
            DIB_RGB_COLORS,
        );
        SelectObject(mem, old);
        DeleteObject(bmp);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);
        if lines == 0 {
            return Err("GetDIBits returned 0 scanlines".into());
        }
        // GDI gives BGRA; the image crate wants RGBA.
        for px in buf.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        let img = image::RgbaImage::from_raw(w as u32, h as u32, buf)
            .ok_or_else(|| "pixel buffer size mismatch".to_string())?;
        let dyn_img = image::DynamicImage::ImageRgba8(img);
        match out
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref()
        {
            // q:v 3 in the ffmpeg path ≈ quality 85 — keep parity.
            Some("jpg") | Some("jpeg") => {
                let rgb = dyn_img.to_rgb8();
                let file = std::fs::File::create(out)
                    .map_err(|e| format!("create {}: {e}", out.display()))?;
                let mut bw = std::io::BufWriter::new(file);
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bw, 85)
                    .encode_image(&rgb)
                    .map_err(|e| format!("encode {}: {e}", out.display()))
            }
            _ => dyn_img
                .save(out)
                .map_err(|e| format!("encode {}: {e}", out.display())),
        }
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
    fn GetDiskFreeSpaceExW(
        dir: *const u16,
        free_avail: *mut u64,
        total: *mut u64,
        free_total: *mut u64,
    ) -> i32;
}

/// Free bytes available on the volume containing `dir` (None on failure).
pub fn disk_free_bytes(dir: &std::path::Path) -> Option<u64> {
    let mut wide: Vec<u16> = dir
        .display()
        .to_string()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut free = 0u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_mut_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(free)
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
#[derive(Clone)]
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

/// Cursor position in virtual-screen pixels (for the window-pick overlay).
pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut ci = CursorInfo {
        cb_size: std::mem::size_of::<CursorInfo>() as u32,
        flags: 0,
        h_cursor: std::ptr::null_mut(),
        pt_screen_pos: Point { x: 0, y: 0 },
    };
    if unsafe { GetCursorInfo(&mut ci) } != 0 {
        Some((ci.pt_screen_pos.x, ci.pt_screen_pos.y))
    } else {
        None
    }
}

/// Pickable windows at a screen point in Z-order: not ours, not minimized,
/// not a shell surface (desktop / taskbar have the `explorer` process and
/// no title). Pure so the rules stay testable.
pub fn pickable_at<'a>(
    wins: &'a [EnumWindow],
    x: i32,
    y: i32,
    self_proc: &str,
) -> Vec<&'a EnumWindow> {
    wins.iter()
        .filter(|w| {
            !w.minimized
                && !w.process.eq_ignore_ascii_case(self_proc)
                && !(w.process.eq_ignore_ascii_case("explorer") && w.title.is_empty())
                && x >= w.x
                && x < w.x + w.w
                && y >= w.y
                && y < w.y + w.h
        })
        .collect()
}

/// Topmost pickable window at a screen point — the first `pickable_at` hit.
/// Live picks use `windows_at_point` (the whole stack, for scroll-cycling);
/// this stays for the pick-rules tests.
#[cfg(test)]
pub fn top_window_at<'a>(
    wins: &'a [EnumWindow],
    x: i32,
    y: i32,
    self_proc: &str,
) -> Option<&'a EnumWindow> {
    pickable_at(wins, x, y, self_proc).into_iter().next()
}

/// Every pickable window at a point, topmost first — powers scroll-cycling
/// through overlapping windows in pick mode.
pub fn windows_at_point(x: i32, y: i32) -> Vec<EnumWindow> {
    let self_proc = process_name(std::process::id());
    let wins = enum_windows();
    pickable_at(&wins, x, y, &self_proc)
        .into_iter()
        .cloned()
        .collect()
}

/// Monitor containing a virtual-screen point — the dead-space pick target:
/// hover past every window edge and the whole display is the capture.
pub fn monitor_at_point(x: i32, y: i32) -> Option<Monitor> {
    enum_monitors()
        .into_iter()
        .find(|m| x >= m.x && x < m.x + m.w && y >= m.y && y < m.y + m.h)
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

// ── Registry: Run-key autostart (direct advapi32 — no reg.exe, no console flash) ──

const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
const KEY_QUERY_VALUE: u32 = 0x0001;
const KEY_SET_VALUE: u32 = 0x0002;
const REG_SZ: u32 = 1;
const ERROR_FILE_NOT_FOUND: i32 = 2;

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(hkey: isize, sub: *const u16, opts: u32, sam: u32, out: *mut isize) -> i32;
    fn RegQueryValueExW(
        hkey: isize,
        name: *const u16,
        res: *mut u32,
        ty: *mut u32,
        data: *mut u8,
        len: *mut u32,
    ) -> i32;
    fn RegSetValueExW(
        hkey: isize,
        name: *const u16,
        res: u32,
        ty: u32,
        data: *const u8,
        len: u32,
    ) -> i32;
    fn RegDeleteValueW(hkey: isize, name: *const u16) -> i32;
    fn RegCloseKey(hkey: isize) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Vibecap";

/// True when HKCU\…\Run\Vibecap exists — instant, no child process.
pub fn run_at_login_enabled_native() -> bool {
    unsafe {
        let mut key: isize = 0;
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide(RUN_SUBKEY).as_ptr(),
            0,
            KEY_QUERY_VALUE,
            &mut key,
        ) != 0
        {
            return false;
        }
        let mut ty: u32 = 0;
        let mut len: u32 = 0;
        let rc = RegQueryValueExW(
            key,
            wide(RUN_VALUE).as_ptr(),
            std::ptr::null_mut(),
            &mut ty,
            std::ptr::null_mut(),
            &mut len,
        );
        RegCloseKey(key);
        rc == 0
    }
}

/// Write or delete the Run entry. `cmdline` is the full command, e.g. `"C:\…ibecap.exe" --hidden`.
pub fn set_run_at_login_native(enable: bool, cmdline: &str) -> Result<(), String> {
    unsafe {
        let mut key: isize = 0;
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide(RUN_SUBKEY).as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut key,
        ) != 0
        {
            return Err("could not open Run registry key".into());
        }
        let name = wide(RUN_VALUE);
        let rc = if enable {
            let data: Vec<u16> = cmdline.encode_utf16().chain(std::iter::once(0)).collect();
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr() as *const u8,
                (data.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(key, name.as_ptr())
        };
        RegCloseKey(key);
        if rc == 0 || (!enable && rc == ERROR_FILE_NOT_FOUND) {
            Ok(())
        } else {
            Err(format!("registry write failed ({rc})"))
        }
    }
}

// ── Preview audio (F126) — winmm PlaySoundW loops the extracted WAV. ──

const SND_ASYNC: u32 = 0x0001;
const SND_LOOP: u32 = 0x0008;
const SND_FILENAME: u32 = 0x0002_0000;

#[link(name = "winmm")]
extern "system" {
    fn PlaySoundW(psz_sound: *const u16, hmod: *mut c_void, fdw_sound: u32) -> i32;
}

/// Loop `path` (a WAV file) in the background. Replaces any current sound.
pub fn play_wav_loop(path: &std::path::Path) {
    let wide: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        PlaySoundW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            SND_FILENAME | SND_ASYNC | SND_LOOP,
        );
    }
}

/// Stop whatever PlaySoundW is currently playing.
pub fn stop_sound() {
    unsafe {
        PlaySoundW(std::ptr::null(), std::ptr::null_mut(), 0);
    }
}

// ---------------------------------------------------------------------------
// E163 — OLE file drag-out (CF_HDROP). Minimal hand-rolled IDataObject +
// IDropSource so Library tiles can be dragged straight into Explorer,
// Discord, browsers, etc. Blocks inside DoDragDrop's modal loop until the
// drop completes or is cancelled.
// ---------------------------------------------------------------------------

type Hresult = i32;

const S_OK: Hresult = 0;
const E_NOINTERFACE: Hresult = 0x8000_4002u32 as i32;
const E_NOTIMPL: Hresult = 0x8000_4001u32 as i32;
const E_OUTOFMEMORY: Hresult = 0x8000_700Eu32 as i32;
const DV_E_FORMATETC: Hresult = 0x8004_0064u32 as i32;
const OLE_E_ADVISENOTSUPPORTED: Hresult = 0x8004_0003u32 as i32;
const DRAGDROP_S_DROP: Hresult = 0x0004_0100;
const DRAGDROP_S_CANCEL: Hresult = 0x0004_0101;
const DRAGDROP_S_USEDEFAULTCURSORS: Hresult = 0x0004_0102;

const CF_HDROP: u16 = 15;
const TYMED_HGLOBAL: u32 = 1;
const DVASPECT_CONTENT: u32 = 1;
const DROPEFFECT_COPY: u32 = 1;
const MK_LBUTTON: u32 = 0x0001;
const GMEM_MOVEABLE_ZEROINIT: u32 = 0x0042;

#[link(name = "ole32")]
extern "system" {
    fn OleInitialize(reserved: *mut c_void) -> i32;
    fn OleUninitialize();
    fn DoDragDrop(data: *mut c_void, src: *mut c_void, ok: u32, effect: *mut u32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
    fn GlobalLock(h: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(h: *mut c_void) -> i32;
    fn GlobalFree(h: *mut c_void) -> *mut c_void;
    fn GlobalSize(h: *mut c_void) -> usize;
}

#[repr(C)]
#[derive(PartialEq, Eq)]
struct DragGuid {
    d1: u32,
    d2: u16,
    d3: u16,
    d4: [u8; 8],
}

const IID_IUNKNOWN: DragGuid = DragGuid {
    d1: 0,
    d2: 0,
    d3: 0,
    d4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};
const IID_IDATAOBJECT: DragGuid = DragGuid {
    d1: 0x0000_010E,
    d2: 0,
    d3: 0,
    d4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};
const IID_IDROPSOURCE: DragGuid = DragGuid {
    d1: 0x0000_0121,
    d2: 0,
    d3: 0,
    d4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};

#[repr(C)]
struct DragFormatEtc {
    cf_format: u16,
    ptd: *mut c_void,
    dw_aspect: u32,
    lindex: i32,
    tymed: u32,
}

#[repr(C)]
struct DragStgMedium {
    tymed: u32,
    handle: *mut c_void,
    unk_for_release: *mut c_void,
}

#[repr(C)]
struct DropSource {
    vtbl: *const DropSourceVtbl,
    refs: u32,
}

#[repr(C)]
struct DropSourceVtbl {
    query_interface:
        unsafe extern "system" fn(*mut DropSource, *const DragGuid, *mut *mut c_void) -> Hresult,
    add_ref: unsafe extern "system" fn(*mut DropSource) -> u32,
    release: unsafe extern "system" fn(*mut DropSource) -> u32,
    query_continue_drag: unsafe extern "system" fn(*mut DropSource, i32, u32) -> Hresult,
    give_feedback: unsafe extern "system" fn(*mut DropSource, u32) -> Hresult,
}

#[repr(C)]
struct DragDataObject {
    vtbl: *const DragDataVtbl,
    refs: u32,
    hdrop: *mut c_void,
}

#[repr(C)]
struct DragDataVtbl {
    query_interface: unsafe extern "system" fn(
        *mut DragDataObject,
        *const DragGuid,
        *mut *mut c_void,
    ) -> Hresult,
    add_ref: unsafe extern "system" fn(*mut DragDataObject) -> u32,
    release: unsafe extern "system" fn(*mut DragDataObject) -> u32,
    get_data: unsafe extern "system" fn(
        *mut DragDataObject,
        *const DragFormatEtc,
        *mut DragStgMedium,
    ) -> Hresult,
    get_data_here: unsafe extern "system" fn(
        *mut DragDataObject,
        *const DragFormatEtc,
        *mut DragStgMedium,
    ) -> Hresult,
    query_get_data: unsafe extern "system" fn(*mut DragDataObject, *const DragFormatEtc) -> Hresult,
    get_canonical_format_etc: unsafe extern "system" fn(
        *mut DragDataObject,
        *const DragFormatEtc,
        *mut DragFormatEtc,
    ) -> Hresult,
    set_data: unsafe extern "system" fn(
        *mut DragDataObject,
        *const DragFormatEtc,
        *mut DragStgMedium,
        i32,
    ) -> Hresult,
    enum_format_etc:
        unsafe extern "system" fn(*mut DragDataObject, u32, *mut *mut c_void) -> Hresult,
    d_advise: unsafe extern "system" fn(
        *mut DragDataObject,
        *const DragFormatEtc,
        u32,
        *mut c_void,
        *mut u32,
    ) -> Hresult,
    d_unadvise: unsafe extern "system" fn(*mut DragDataObject, u32) -> Hresult,
    enum_d_advise: unsafe extern "system" fn(*mut DragDataObject, *mut *mut c_void) -> Hresult,
}

static DROP_SRC_VTBL: DropSourceVtbl = DropSourceVtbl {
    query_interface: src_query_interface,
    add_ref: src_add_ref,
    release: src_release,
    query_continue_drag: src_query_continue_drag,
    give_feedback: src_give_feedback,
};

static DATA_VTBL: DragDataVtbl = DragDataVtbl {
    query_interface: data_query_interface,
    add_ref: data_add_ref,
    release: data_release,
    get_data: data_get_data,
    get_data_here: data_get_data_here,
    query_get_data: data_query_get_data,
    get_canonical_format_etc: data_get_canonical,
    set_data: data_set_data,
    enum_format_etc: data_enum_format_etc,
    d_advise: data_d_advise,
    d_unadvise: data_d_unadvise,
    enum_d_advise: data_enum_d_advise,
};

unsafe extern "system" fn src_query_interface(
    this: *mut DropSource,
    iid: *const DragGuid,
    out: *mut *mut c_void,
) -> Hresult {
    if out.is_null() {
        return E_NOINTERFACE;
    }
    let g = unsafe { &*iid };
    if *g == IID_IUNKNOWN || *g == IID_IDROPSOURCE {
        unsafe { *out = this as *mut c_void };
        src_add_ref(this);
        S_OK
    } else {
        unsafe { *out = std::ptr::null_mut() };
        E_NOINTERFACE
    }
}

unsafe extern "system" fn src_add_ref(this: *mut DropSource) -> u32 {
    let o = unsafe { &mut *this };
    o.refs += 1;
    o.refs
}

unsafe extern "system" fn src_release(this: *mut DropSource) -> u32 {
    let o = unsafe { &mut *this };
    o.refs -= 1;
    let n = o.refs;
    if n == 0 {
        drop(unsafe { Box::from_raw(this) });
    }
    n
}

unsafe extern "system" fn src_query_continue_drag(
    _this: *mut DropSource,
    escape_pressed: i32,
    key_state: u32,
) -> Hresult {
    if escape_pressed != 0 {
        DRAGDROP_S_CANCEL
    } else if key_state & MK_LBUTTON == 0 {
        DRAGDROP_S_DROP
    } else {
        S_OK
    }
}

unsafe extern "system" fn src_give_feedback(_this: *mut DropSource, _effect: u32) -> Hresult {
    DRAGDROP_S_USEDEFAULTCURSORS
}

fn fmt_offers_hdrop(fmt: &DragFormatEtc) -> bool {
    fmt.cf_format == CF_HDROP
        && (fmt.tymed & TYMED_HGLOBAL) != 0
        && fmt.lindex == -1
        && fmt.dw_aspect == DVASPECT_CONTENT
}

unsafe extern "system" fn data_query_interface(
    this: *mut DragDataObject,
    iid: *const DragGuid,
    out: *mut *mut c_void,
) -> Hresult {
    if out.is_null() {
        return E_NOINTERFACE;
    }
    let g = unsafe { &*iid };
    if *g == IID_IUNKNOWN || *g == IID_IDATAOBJECT {
        unsafe { *out = this as *mut c_void };
        data_add_ref(this);
        S_OK
    } else {
        unsafe { *out = std::ptr::null_mut() };
        E_NOINTERFACE
    }
}

unsafe extern "system" fn data_add_ref(this: *mut DragDataObject) -> u32 {
    let o = unsafe { &mut *this };
    o.refs += 1;
    o.refs
}

unsafe extern "system" fn data_release(this: *mut DragDataObject) -> u32 {
    let o = unsafe { &mut *this };
    o.refs -= 1;
    let n = o.refs;
    if n == 0 {
        drop(unsafe { Box::from_raw(this) });
    }
    n
}

/// Fresh HGLOBAL copy — each GetData hands over ownership, so the template
/// stays ours to free regardless of what the drop target does.
unsafe fn dup_hdrop(src: *mut c_void) -> *mut c_void {
    if src.is_null() {
        return std::ptr::null_mut();
    }
    let size = unsafe { GlobalSize(src) };
    let dst = unsafe { GlobalAlloc(GMEM_MOVEABLE_ZEROINIT, size) };
    if dst.is_null() {
        return std::ptr::null_mut();
    }
    let s = unsafe { GlobalLock(src) };
    let d = unsafe { GlobalLock(dst) };
    if !s.is_null() && !d.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(s as *const u8, d as *mut u8, size) };
    }
    if !s.is_null() {
        unsafe { GlobalUnlock(src) };
    }
    if !d.is_null() {
        unsafe { GlobalUnlock(dst) };
    }
    dst
}

unsafe extern "system" fn data_get_data(
    this: *mut DragDataObject,
    fmt: *const DragFormatEtc,
    stg: *mut DragStgMedium,
) -> Hresult {
    if fmt.is_null() || stg.is_null() {
        return DV_E_FORMATETC;
    }
    if !fmt_offers_hdrop(unsafe { &*fmt }) {
        return DV_E_FORMATETC;
    }
    let copy = unsafe { dup_hdrop((*this).hdrop) };
    if copy.is_null() {
        return E_OUTOFMEMORY;
    }
    let m = unsafe { &mut *stg };
    m.tymed = TYMED_HGLOBAL;
    m.handle = copy;
    m.unk_for_release = std::ptr::null_mut();
    S_OK
}

unsafe extern "system" fn data_get_data_here(
    _this: *mut DragDataObject,
    _fmt: *const DragFormatEtc,
    _stg: *mut DragStgMedium,
) -> Hresult {
    E_NOTIMPL
}

unsafe extern "system" fn data_query_get_data(
    _this: *mut DragDataObject,
    fmt: *const DragFormatEtc,
) -> Hresult {
    if !fmt.is_null() && fmt_offers_hdrop(unsafe { &*fmt }) {
        S_OK
    } else {
        DV_E_FORMATETC
    }
}

unsafe extern "system" fn data_get_canonical(
    _this: *mut DragDataObject,
    _in: *const DragFormatEtc,
    _out: *mut DragFormatEtc,
) -> Hresult {
    E_NOTIMPL
}

unsafe extern "system" fn data_set_data(
    _this: *mut DragDataObject,
    _fmt: *const DragFormatEtc,
    _stg: *mut DragStgMedium,
    _release: i32,
) -> Hresult {
    E_NOTIMPL
}

unsafe extern "system" fn data_enum_format_etc(
    _this: *mut DragDataObject,
    _dir: u32,
    out: *mut *mut c_void,
) -> Hresult {
    if !out.is_null() {
        unsafe { *out = std::ptr::null_mut() };
    }
    E_NOTIMPL
}

unsafe extern "system" fn data_d_advise(
    _this: *mut DragDataObject,
    _fmt: *const DragFormatEtc,
    _flags: u32,
    _sink: *mut c_void,
    _conn: *mut u32,
) -> Hresult {
    OLE_E_ADVISENOTSUPPORTED
}

unsafe extern "system" fn data_d_unadvise(_this: *mut DragDataObject, _conn: u32) -> Hresult {
    OLE_E_ADVISENOTSUPPORTED
}

unsafe extern "system" fn data_enum_d_advise(
    _this: *mut DragDataObject,
    _out: *mut *mut c_void,
) -> Hresult {
    OLE_E_ADVISENOTSUPPORTED
}

/// DROPFILES header + double-NUL-terminated UTF-16 path list in an HGLOBAL.
fn build_hdrop(paths: &[std::path::PathBuf]) -> Option<*mut c_void> {
    let mut wide: Vec<u16> = Vec::new();
    for p in paths {
        wide.extend(p.as_os_str().encode_wide());
        wide.push(0);
    }
    wide.push(0); // list terminator
    let total = 20 + wide.len() * 2;
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE_ZEROINIT, total);
        if h.is_null() {
            return None;
        }
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            GlobalFree(h);
            return None;
        }
        *(p as *mut u32) = 20; // pFiles — list starts right after the header
        *(p.add(4) as *mut i32) = 0; // pt.x
        *(p.add(8) as *mut i32) = 0; // pt.y
        *(p.add(12) as *mut i32) = 0; // fNC
        *(p.add(16) as *mut i32) = 1; // fWide
        std::ptr::copy_nonoverlapping(wide.as_ptr(), p.add(20) as *mut u16, wide.len());
        GlobalUnlock(h);
        Some(h)
    }
}

/// E163 — start a modal OLE drag of `paths` (CF_HDROP). Blocks in
/// DoDragDrop's message loop until the drop completes or is cancelled.
pub fn start_file_drag(paths: &[std::path::PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("nothing to drag".into());
    }
    let Some(hdrop) = build_hdrop(paths) else {
        return Err("could not build the file list".into());
    };
    unsafe {
        let hr = OleInitialize(std::ptr::null_mut());
        if hr < 0 {
            GlobalFree(hdrop);
            return Err(format!("OLE init failed (0x{hr:08X})"));
        }
        let data = Box::into_raw(Box::new(DragDataObject {
            vtbl: &DATA_VTBL,
            refs: 1,
            hdrop,
        }));
        let src = Box::into_raw(Box::new(DropSource {
            vtbl: &DROP_SRC_VTBL,
            refs: 1,
        }));
        let mut effect = 0u32;
        let _ = DoDragDrop(
            data as *mut c_void,
            src as *mut c_void,
            DROPEFFECT_COPY,
            &mut effect,
        );
        // Drop our refs — OLE's own addrefs were released inside the call.
        data_release(data);
        src_release(src);
        GlobalFree(hdrop); // template; targets received fresh copies
        OleUninitialize();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{pickable_at, top_window_at, EnumWindow};

    #[test]
    fn studio_title_prefix_is_stable() {
        assert!("Vibecap Studio · 123".starts_with("Vibecap Studio"));
        assert!(!"Vibecap Region".starts_with("Vibecap Studio"));
        assert!(!"Vibecap Recorder".starts_with("Vibecap Studio"));
    }

    fn win(
        title: &str,
        process: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        minimized: bool,
    ) -> EnumWindow {
        EnumWindow {
            hwnd: 0,
            process: process.into(),
            title: title.into(),
            x,
            y,
            w,
            h,
            minimized,
        }
    }

    #[test]
    fn pick_skips_self_minimized_and_shell_surfaces() {
        // Z-order: our overlay on top, then the target, then the taskbar.
        let wins = vec![
            win("Vibecap Region", "vibecap", 0, 0, 1920, 1080, false),
            win("Editor — main.rs", "code", 100, 100, 800, 600, false),
            win("", "explorer", 0, 1040, 1920, 40, false), // taskbar: no title
            win("Downloads", "explorer", 1000, 200, 600, 400, true), // minimized
        ];
        // Point inside the target → picked despite our overlay covering it.
        let hit = top_window_at(&wins, 400, 300, "vibecap").expect("a window");
        assert_eq!(hit.process, "code");
        // Point inside only the minimized window → nothing (minimized skipped).
        assert!(top_window_at(&wins, 1200, 300, "vibecap").is_none());
        // Taskbar strip → explorer shell surface is filtered out.
        assert!(top_window_at(&wins, 10, 1060, "vibecap").is_none());
    }

    #[test]
    fn pickable_at_returns_whole_stack_for_scroll_cycling() {
        // Three overlapping pickable windows under the point, plus noise.
        let wins = vec![
            win("Vibecap Region", "vibecap", 0, 0, 1920, 1080, false), // self
            win("Editor — main.rs", "code", 100, 100, 300, 300, false),
            win("Browser", "chrome", 200, 200, 700, 500, false),
            win("Chat", "slack", 150, 150, 400, 300, false),
            win("", "explorer", 0, 1040, 1920, 40, false), // taskbar
        ];
        let hits = pickable_at(&wins, 300, 300, "vibecap");
        // Z-order preserved, self + shell filtered → code, chrome, slack.
        let procs: Vec<&str> = hits.iter().map(|w| w.process.as_str()).collect();
        assert_eq!(procs, ["code", "chrome", "slack"]);
        // Point inside only chrome+slack → two hits, topmost first.
        let hits = pickable_at(&wins, 250, 400, "vibecap");
        let procs: Vec<&str> = hits.iter().map(|w| w.process.as_str()).collect();
        assert_eq!(procs, ["chrome", "slack"]);
        // Empty corner → nothing (monitor-pick handles it upstream).
        assert!(pickable_at(&wins, 1900, 20, "vibecap").is_empty());
    }
}
