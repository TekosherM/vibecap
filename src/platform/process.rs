use std::process::Command;

/// Resume a paused child process (Unix SIGCONT). No-op on Windows.
pub fn cont_process(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-CONT", &pid.to_string()])
            .status();
    }
    #[cfg(windows)]
    {
        let _ = pid;
        // ffmpeg pause via SIGSTOP is not available on Windows.
    }
}

/// Pause a child process (Unix SIGSTOP). No-op on Windows.
pub fn stop_process(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-STOP", &pid.to_string()])
            .status();
    }
    #[cfg(windows)]
    {
        let _ = pid;
    }
}
