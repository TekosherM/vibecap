//! Unbounded agent recording: start, status, stop.
//!
//! State lives in `{config}/agent-record.json` so a later CLI/MCP process can
//! stop a recorder started by another invocation. A breadcrumb copy is also
//! written to `{output_dir}/.vibecap-record.json`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::app::io::{vibecap_config_dir, write_json_atomic};
use crate::platform::{
    export_gif_clip, resolve_output_dir, spawn_screen_recorder_opts, CaptureOpts,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRecordState {
    pub pid: u32,
    pub mp4: String,
    pub output_dir: String,
    pub display: Option<String>,
    pub window: Option<String>,
    pub gif: bool,
    pub started_at: String,
    pub started_unix: u64,
}

impl AgentRecordState {
    pub fn mp4_path(&self) -> PathBuf {
        PathBuf::from(&self.mp4)
    }
}

pub fn agent_record_state_path() -> PathBuf {
    vibecap_config_dir().join("agent-record.json")
}

pub fn breadcrumb_path(output_dir: &Path) -> PathBuf {
    output_dir.join(".vibecap-record.json")
}

pub fn load_record_state() -> Option<AgentRecordState> {
    read_state(&agent_record_state_path()).or_else(|| {
        // Last-ditch: look next to default output if config was wiped.
        read_state(&breadcrumb_path(&resolve_output_dir(None)))
    })
}

fn read_state(path: &Path) -> Option<AgentRecordState> {
    let s = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

fn persist_state(state: &AgentRecordState) -> Result<(), String> {
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    write_json_atomic(&agent_record_state_path(), &json)?;
    let crumb = breadcrumb_path(Path::new(&state.output_dir));
    let _ = write_json_atomic(&crumb, &json);
    Ok(())
}

fn clear_state(state: Option<&AgentRecordState>) {
    let _ = std::fs::remove_file(agent_record_state_path());
    if let Some(s) = state {
        let _ = std::fs::remove_file(breadcrumb_path(Path::new(&s.output_dir)));
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        // signal 0 = existence check
        CommandKill::signal(pid, 0).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        // Best-effort: if we cannot probe, assume alive so stop still tries.
        true
    }
}

/// Tiny kill helper so we do not take a libc crate.
struct CommandKill;
impl CommandKill {
    fn signal(pid: u32, sig: i32) -> Result<bool, String> {
        let status = std::process::Command::new("kill")
            .args([format!("-{sig}"), pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        Ok(status.success())
    }
}

pub fn start_agent_record(
    output_dir: Option<&Path>,
    opts: &CaptureOpts,
    want_gif: bool,
) -> Result<AgentRecordState, String> {
    if let Some(existing) = load_record_state() {
        if pid_alive(existing.pid) {
            return Err(format!(
                "already recording pid {} → {} (call record stop first)",
                existing.pid, existing.mp4
            ));
        }
        clear_state(Some(&existing));
    }

    let dir = resolve_output_dir(output_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create output dir: {e}"))?;
    // Persist absolute paths: record stop/status may run from any cwd, and a
    // relative "--output-dir ." would otherwise point elsewhere later.
    let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
    let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let pid_hint = std::process::id();
    let mp4 = dir.join(format!("video_{}_{}.mp4", stamp, pid_hint));

    let child = spawn_screen_recorder_opts(&mp4, 30, false, None, opts)?;
    let rec_pid = child.id();
    // Detach: leak the Child so dropping this process does not SIGKILL ffmpeg.
    std::mem::forget(child);

    let state = AgentRecordState {
        pid: rec_pid,
        mp4: mp4.display().to_string(),
        output_dir: dir.display().to_string(),
        display: opts.display.clone(),
        window: opts.window.clone(),
        gif: want_gif,
        started_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        started_unix: now_unix(),
    };
    persist_state(&state)?;
    Ok(state)
}

pub fn record_status_line() -> String {
    match load_record_state() {
        None => "not recording".into(),
        Some(s) => {
            let alive = pid_alive(s.pid);
            let elapsed = now_unix().saturating_sub(s.started_unix);
            let note = if alive {
                String::new()
            } else {
                // Recorder died without a stop (crash or harness kill) — the
                // mp4 is almost certainly unfinalized (missing moov atom).
                " (recorder exited without stop — mp4 may be unfinalized; run record stop to clear state)"
                    .to_string()
            };
            format!(
                "recording={} pid={}{} elapsed_secs={} mp4={} output_dir={} display={} window={}",
                alive,
                s.pid,
                note,
                elapsed,
                s.mp4,
                s.output_dir,
                s.display.as_deref().unwrap_or("-"),
                s.window.as_deref().unwrap_or("-")
            )
        }
    }
}

pub fn stop_agent_record(want_gif: bool) -> Result<(AgentRecordState, Option<PathBuf>), String> {
    let state = load_record_state().ok_or_else(|| "not recording — nothing to stop".to_string())?;
    let was_alive = pid_alive(state.pid);
    if was_alive {
        // SIGINT: ffmpeg finalizes the MP4. Then SIGTERM if it hangs.
        let _ = CommandKill::signal(state.pid, 2);
        let start = std::time::Instant::now();
        while pid_alive(state.pid) && start.elapsed() < std::time::Duration::from_secs(8) {
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
        if pid_alive(state.pid) {
            let _ = CommandKill::signal(state.pid, 15);
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        if pid_alive(state.pid) {
            let _ = CommandKill::signal(state.pid, 9);
        }
    }

    // Brief settle so the container is a complete file.
    if !state.mp4_path().exists()
        || std::fs::metadata(state.mp4_path())
            .map(|m| m.len())
            .unwrap_or(0)
            < 512
    {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let make_gif = want_gif || state.gif;
    let gif = if make_gif && state.mp4_path().exists() {
        let gif_path = state.mp4_path().with_extension("gif");
        let gif_s = gif_path.display().to_string();
        match export_gif_clip(&state.mp4, "00:00:00", "99:00:00", &gif_s) {
            Ok(()) => Some(gif_path),
            Err(_) => None,
        }
    } else {
        None
    };

    if !was_alive {
        eprintln!(
            "warning: recorder pid {} had already exited — mp4 {} was never finalized and may not play",
            state.pid, state.mp4
        );
    }

    clear_state(Some(&state));
    Ok((state, gif))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrip_json() {
        let s = AgentRecordState {
            pid: 42,
            mp4: "/tmp/video.mp4".into(),
            output_dir: "/tmp".into(),
            display: Some(":1".into()),
            window: Some("Chrome".into()),
            gif: true,
            started_at: "now".into(),
            started_unix: 100,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: AgentRecordState = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
        assert_eq!(back.mp4_path(), PathBuf::from("/tmp/video.mp4"));
    }

    #[test]
    fn breadcrumb_sits_in_output_dir() {
        let p = breadcrumb_path(Path::new("/workspace/run4"));
        assert_eq!(p, PathBuf::from("/workspace/run4/.vibecap-record.json"));
    }

    #[test]
    fn pid_zero_is_dead() {
        assert!(!pid_alive(0));
    }
}
