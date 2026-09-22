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
    export_gif_clip, remux_to_clean_mp4, resolve_output_dir, spawn_screen_recorder_opts,
    CaptureOpts,
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
    /// Fragmented MP4 (kill-safe); remuxed to a regular MP4 on stop.
    /// `default` keeps pre-frag state files readable.
    #[serde(default)]
    pub frag: bool,
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

/// Public pid liveness check (doctor --fix probes stale state with it).
pub fn record_pid_alive(pid: u32) -> bool {
    pid_alive(pid)
}

/// E222 — a dead recorder left its state file plus a frag-MP4 on disk;
/// returns the mp4 so the GUI can remux it clean on launch. The frag file
/// is already playable — remux restores a normal fast-start container.
pub fn orphaned_frag_mp4() -> Option<PathBuf> {
    let s = load_record_state()?;
    if pid_alive(s.pid) {
        return None;
    }
    let mp4 = s.mp4_path();
    let big_enough = std::fs::metadata(&mp4)
        .map(|m| m.len() > 512)
        .unwrap_or(false);
    if s.frag && big_enough {
        Some(mp4)
    } else {
        None
    }
}

/// E222 — drop a dead recorder's state file + breadcrumb after recovery.
pub fn discard_orphaned_state() {
    let s = load_record_state();
    clear_state(s.as_ref());
}

// ── E265 instance handshake ──────────────────────────────────────────────
// GUI recordings never touched the agent state file, so `vibecap record
// start` couldn't see them (and vice-versa). A small heartbeat file bridges
// the two: the GUI writes it at arm and clears it at every terminal stop;
// the agent path refuses to start while a live GUI recording owns it.

fn gui_record_state_path() -> PathBuf {
    vibecap_config_dir().join("gui-recording.json")
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct GuiRecordState {
    /// PID of the *GUI* process (its ffmpeg child dies with it).
    pub pid: u32,
    pub mp4: String,
    pub started_unix: u64,
}

/// GUI → file: announce that a studio recording is arming.
pub fn write_gui_record_state(mp4: &Path) {
    let state = GuiRecordState {
        pid: std::process::id(),
        mp4: mp4.display().to_string(),
        started_unix: now_unix(),
    };
    if let Ok(json) = serde_json::to_string(&state) {
        let _ = write_json_atomic(&gui_record_state_path(), &json);
    }
}

/// GUI → file: recording reached a terminal state (stop/cancel/fail).
pub fn clear_gui_record_state() {
    let _ = std::fs::remove_file(gui_record_state_path());
}

/// Agent/CLI → GUI: is a studio recording live? Stale entries (crashed GUI)
/// are swept so a leftover file can never wedge the CLI.
pub fn live_gui_record() -> Option<GuiRecordState> {
    let s: GuiRecordState =
        serde_json::from_str(&std::fs::read_to_string(gui_record_state_path()).ok()?).ok()?;
    if pid_alive(s.pid) {
        Some(s)
    } else {
        clear_gui_record_state();
        None
    }
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
    #[cfg(target_os = "windows")]
    {
        windows_pid_alive(pid)
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        // Best-effort: if we cannot probe, assume alive so stop still tries.
        true
    }
}

/// E238 — native `OpenProcess` probe (`kill -0` does not exist on Windows;
/// `tasklist` cost a ~50–100 ms spawn per status/lock/doctor check).
#[cfg(target_os = "windows")]
fn windows_pid_alive(pid: u32) -> bool {
    crate::platform::pid_alive(pid)
}

/// Cross-platform graceful terminate: SIGINT on unix (ffmpeg finalizes the
/// MP4), escalating to SIGTERM/SIGKILL. On Windows a detached ffmpeg has no
/// console for a graceful Ctrl+C and the MP4 is fragmented — kill-safe by
/// design — so `TerminateProcess` (E238, no `taskkill` spawn) is correct.
fn terminate_pid(pid: u32) {
    #[cfg(unix)]
    {
        // SIGINT: ffmpeg finalizes the MP4. Then SIGTERM if it hangs.
        let _ = CommandKill::signal(pid, 2);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = crate::platform::terminate_process(pid);
    }
}

fn force_kill_pid(pid: u32) {
    #[cfg(unix)]
    {
        let _ = CommandKill::signal(pid, 15);
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    #[cfg(target_os = "windows")]
    {
        let _ = crate::platform::terminate_process(pid);
    }
}

fn kill_pid(pid: u32) {
    #[cfg(unix)]
    {
        let _ = CommandKill::signal(pid, 9);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = crate::platform::terminate_process(pid);
    }
}

/// Tiny kill helper so we do not take a libc crate.
#[cfg(unix)]
struct CommandKill;
#[cfg(unix)]
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
    // E265 — a studio recording owns the capture lock too.
    if let Some(gui) = live_gui_record() {
        return Err(format!(
            "the Vibecap app is recording → {} (stop it in the app first)",
            gui.mp4
        ));
    }

    let dir = resolve_output_dir(output_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create output dir: {e}"))?;
    // Persist absolute paths: record stop/status may run from any cwd, and a
    // relative "--output-dir ." would otherwise point elsewhere later.
    let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
    let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let pid_hint = std::process::id();
    let mp4 = dir.join(format!("video_{}_{}.mp4", stamp, pid_hint));

    let child = spawn_screen_recorder_opts(&mp4, 30, false, None, opts, true)?;
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
        frag: true,
    };
    persist_state(&state)?;
    Ok(state)
}

pub fn record_status_line() -> String {
    // E265 — report a live studio recording before falling through to the
    // agent state file so `record status` never lies about the lock owner.
    if let Some(gui) = live_gui_record() {
        let elapsed = now_unix().saturating_sub(gui.started_unix);
        return format!(
            "recording=true owner=gui pid={} elapsed_secs={} mp4={}",
            gui.pid, elapsed, gui.mp4
        );
    }
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

/// Companion GIF from `record stop --gif`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GifOutcome {
    None,
    Ready(PathBuf),
    /// Encoding in the background (long clips). MP4 is already playable.
    Pending(PathBuf),
}

pub fn stop_agent_record(want_gif: bool) -> Result<(AgentRecordState, GifOutcome), String> {
    let state = load_record_state().ok_or_else(|| {
        // E265 — don't pretend "nothing to stop" when the studio owns a live
        // recording; the CLI must not kill a process it didn't spawn.
        if let Some(gui) = live_gui_record() {
            format!(
                "the Vibecap app is recording → {} — stop it in the app, not here",
                gui.mp4
            )
        } else {
            "not recording — nothing to stop".to_string()
        }
    })?;
    let was_alive = pid_alive(state.pid);
    if was_alive {
        terminate_pid(state.pid);
        let start = std::time::Instant::now();
        while pid_alive(state.pid) && start.elapsed() < std::time::Duration::from_secs(8) {
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
        if pid_alive(state.pid) {
            force_kill_pid(state.pid);
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        if pid_alive(state.pid) {
            kill_pid(state.pid);
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

    // Killed recorders never write a `moov` atom; fragmented recordings remux
    // into a regular playable MP4 (stream copy, no re-encode). A cleanly
    // stopped recorder remuxes losslessly too. Keep the original on failure.
    if state.frag && state.mp4_path().exists() {
        let clean = state.mp4_path().with_extension("clean.mp4");
        match remux_to_clean_mp4(&state.mp4_path(), &clean) {
            Ok(()) => {
                let _ = std::fs::remove_file(state.mp4_path());
                if std::fs::rename(&clean, state.mp4_path()).is_err() {
                    let _ = std::fs::remove_file(&clean);
                } else {
                    let log = state.mp4_path().with_extension("ffmpeg.log");
                    let _ = std::fs::remove_file(log);
                }
            }
            Err(e) => {
                let _ = std::fs::remove_file(&clean);
                let log = state.mp4_path().with_extension("ffmpeg.log");
                if let Some(tail) = crate::platform::ffmpeg_log_tail(&log, 800) {
                    eprintln!("warning: remux failed: {e}\n--- ffmpeg.log ---\n{tail}");
                }
            }
        }
    }

    let make_gif = want_gif || state.gif;
    let gif = if make_gif && state.mp4_path().exists() {
        let gif_path = state.mp4_path().with_extension("gif");
        let gif_s = gif_path.display().to_string();
        let bytes = std::fs::metadata(state.mp4_path())
            .map(|m| m.len())
            .unwrap_or(0);
        let dur = crate::platform::probe_duration(&state.mp4_path()).unwrap_or(0.0);
        if bytes > 8_000_000 || dur > 12.0 {
            let mp4 = state.mp4.clone();
            let dest = gif_s.clone();
            std::thread::spawn(move || {
                let _ = export_gif_clip(&mp4, "00:00:00", "99:00:00", &dest);
            });
            GifOutcome::Pending(gif_path)
        } else {
            match export_gif_clip(&state.mp4, "00:00:00", "99:00:00", &gif_s) {
                Ok(()) => GifOutcome::Ready(gif_path),
                Err(_) => GifOutcome::None,
            }
        }
    } else {
        GifOutcome::None
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
            frag: true,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: AgentRecordState = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
        assert_eq!(back.mp4_path(), PathBuf::from("/tmp/video.mp4"));
    }

    #[test]
    fn old_state_without_frag_still_parses() {
        let back: AgentRecordState = serde_json::from_str(
            r#"{"pid":7,"mp4":"/tmp/v.mp4","output_dir":"/tmp","display":null,"window":null,"gif":false,"started_at":"x","started_unix":1}"#,
        )
        .unwrap();
        assert!(!back.frag);
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

    /// E262 — killed mid-record: a dead pid + frag state + a real partial
    /// file surfaces as an orphan for the launch-time remux, and discard
    /// clears the marker so recovery doesn't loop.
    #[test]
    fn orphaned_frag_surfaces_and_discards() {
        // Preserve any real in-flight state around the test.
        let state_path = agent_record_state_path();
        let prior = std::fs::read_to_string(&state_path).ok();

        let dir = std::env::temp_dir().join(format!("vibecap_orph_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mp4 = dir.join("partial.mp4");
        std::fs::write(&mp4, vec![0u8; 1024]).unwrap();

        let s = AgentRecordState {
            pid: 0, // dead
            mp4: mp4.display().to_string(),
            output_dir: dir.display().to_string(),
            display: None,
            window: None,
            gif: false,
            started_at: "t".into(),
            started_unix: 1,
            frag: true,
        };
        let json = serde_json::to_string(&s).unwrap();
        let _ = std::fs::create_dir_all(state_path.parent().unwrap());
        std::fs::write(&state_path, &json).unwrap();

        assert_eq!(orphaned_frag_mp4(), Some(mp4.clone()));

        discard_orphaned_state();
        assert!(load_record_state().is_none());
        assert!(orphaned_frag_mp4().is_none());

        // Restore + clean up.
        if let Some(p) = prior {
            let _ = std::fs::write(&state_path, p);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
