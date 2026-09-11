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

#[cfg(test)]
mod tests {
    #[test]
    fn studio_title_prefix_is_stable() {
        assert!("Vibecap Studio · 123".starts_with("Vibecap Studio"));
        assert!(!"Vibecap Region".starts_with("Vibecap Studio"));
        assert!(!"Vibecap Recorder".starts_with("Vibecap Studio"));
    }
}
