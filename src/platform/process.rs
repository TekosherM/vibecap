#[cfg(unix)]
use std::process::Command;

/// Suspend/resume exists on Unix (SIGSTOP/SIGCONT) and Windows
/// (ntdll NtSuspendProcess/NtResumeProcess on our own ffmpeg child).
pub fn pause_supported() -> bool {
    cfg!(unix) || cfg!(windows)
}

/// Resume a paused child process (Unix SIGCONT, Windows NtResumeProcess).
pub fn cont_process(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-CONT", &pid.to_string()])
            .status();
    }
    #[cfg(windows)]
    {
        let _ = super::win32::resume_process(pid);
    }
}

/// Pause a child process (Unix SIGSTOP, Windows NtSuspendProcess).
pub fn stop_process(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-STOP", &pid.to_string()])
            .status();
    }
    #[cfg(windows)]
    {
        let _ = super::win32::suspend_process(pid);
    }
}
