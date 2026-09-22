//! `vibecap doctor` — one-shot diagnostics for agents and humans.
//! `doctor --json` prints the same report as JSON; `doctor --fix` applies
//! the safe auto-remediations (missing dirs, stale record state).

use crate::platform::{
    capture_backend_label, config_dir, ffmpeg_available, ffmpeg_path, list_monitors,
    media_dir_display, resolve_output_dir, window_tools_hint,
};

#[derive(serde::Serialize)]
pub struct DoctorReport {
    pub version: String,
    pub backend: String,
    pub ffmpeg: Option<String>,
    pub ffmpeg_run: bool,
    pub media_dir: String,
    pub output_dir_default: String,
    pub config_dir: String,
    pub env_output_dir: Option<String>,
    pub env_ffmpeg: Option<String>,
    pub env_display: Option<String>,
    pub env_audio_device: Option<String>,
    pub gui_stdio: String,
    pub monitors: Vec<String>,
    pub window_crop: Option<String>,
    pub mcp_tools: usize,
    pub mcp_tool_names: Vec<String>,
    /// True when a stale record state points at a dead pid.
    pub stale_record_state: bool,
    /// E245 — last recorder's stderr tail (in-memory ring; None until a
    /// recording has finished this session).
    pub ffmpeg_log_tail: Option<String>,
    /// E236 — ms from process start to first painted frame (GUI only).
    pub startup_ms: Option<u64>,
    /// E237 — working-set memory of this process in MiB.
    pub memory_mb: Option<u64>,
}

fn env_or_none(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|s| !s.is_empty())
}

pub fn doctor_report() -> DoctorReport {
    let stale = crate::app::agent_record::load_record_state()
        .map(|s| s.pid > 0 && !crate::app::agent_record::record_pid_alive(s.pid))
        .unwrap_or(false);
    DoctorReport {
        version: env!("CARGO_PKG_VERSION").into(),
        backend: capture_backend_label().into(),
        ffmpeg: ffmpeg_path().map(|p| p.display().to_string()),
        ffmpeg_run: ffmpeg_available(),
        media_dir: media_dir_display(),
        output_dir_default: resolve_output_dir(None).display().to_string(),
        config_dir: config_dir().display().to_string(),
        env_output_dir: env_or_none("VIBECAP_OUTPUT_DIR"),
        env_ffmpeg: env_or_none("VIBECAP_FFMPEG"),
        env_display: env_or_none("DISPLAY"),
        env_audio_device: env_or_none("VIBECAP_AUDIO_DEVICE"),
        gui_stdio: if cfg!(windows) {
            "detached (windows_subsystem + ffmpeg null stdio)".into()
        } else {
            "console-ok".into()
        },
        monitors: list_monitors()
            .iter()
            .map(|m| {
                format!(
                    "{}x{}+{}+{}{}",
                    m.w,
                    m.h,
                    m.x,
                    m.y,
                    if m.primary { " primary" } else { "" }
                )
            })
            .collect(),
        window_crop: window_tools_hint(),
        mcp_tools: crate::app::mcp_tool_count(),
        mcp_tool_names: crate::app::MCP_TOOL_NAMES
            .iter()
            .map(|s| s.to_string())
            .collect(),
        stale_record_state: stale,
        ffmpeg_log_tail: crate::platform::ffmpeg_log_ring(),
        startup_ms: crate::app::startup_elapsed_ms(),
        memory_mb: crate::platform::process_memory_mb(),
    }
}

/// E251 — apply the safe remediations; returns a line per action taken.
/// Deliberately conservative: creates missing dirs and clears a stale
/// record-state file. Never edits PATH/env (that's the user's call).
pub fn doctor_fix() -> Vec<String> {
    let mut done = Vec::new();
    for (label, dir) in [
        ("output_dir", resolve_output_dir(None)),
        ("media_dir", crate::platform::media_dir()),
    ] {
        if !dir.exists() && std::fs::create_dir_all(&dir).is_ok() {
            done.push(format!("created {label}={}", dir.display()));
        }
    }
    if let Some(s) = crate::app::agent_record::load_record_state() {
        if s.pid == 0 || !crate::app::agent_record::record_pid_alive(s.pid) {
            let _ = std::fs::remove_file(crate::app::agent_record::agent_record_state_path());
            let _ = std::fs::remove_file(crate::app::agent_record::breadcrumb_path(
                std::path::Path::new(&s.output_dir),
            ));
            done.push(format!(
                "cleared stale record state (pid {} dead) — mp4={}",
                s.pid, s.mp4
            ));
        }
    }
    if done.is_empty() {
        done.push("nothing to fix".into());
    }
    done
}

pub fn doctor_json() -> String {
    serde_json::to_string_pretty(&doctor_report()).unwrap_or_else(|_| "{}".into())
}

pub fn doctor_text() -> String {
    let r = doctor_report();
    let mut lines = Vec::new();
    lines.push(format!("vibecap {}", r.version));
    lines.push(format!("backend={}", r.backend));
    lines.push(format!(
        "ffmpeg={}",
        r.ffmpeg.as_deref().unwrap_or("missing")
    ));
    lines.push(format!(
        "ffmpeg_run={}",
        if r.ffmpeg_run { "yes" } else { "no" }
    ));
    lines.push(format!("media_dir={}", r.media_dir));
    lines.push(format!("output_dir_default={}", r.output_dir_default));
    lines.push(format!("config_dir={}", r.config_dir));
    lines.push(format!(
        "VIBECAP_OUTPUT_DIR={}",
        r.env_output_dir.as_deref().unwrap_or("(unset)")
    ));
    lines.push(format!(
        "VIBECAP_FFMPEG={}",
        r.env_ffmpeg.as_deref().unwrap_or("(unset)")
    ));
    lines.push(format!(
        "DISPLAY={}",
        r.env_display.as_deref().unwrap_or("(unset)")
    ));
    lines.push(format!("gui_stdio={}", r.gui_stdio));
    if r.monitors.is_empty() {
        lines.push("monitors=(unknown)".into());
    } else {
        for (i, m) in r.monitors.iter().enumerate() {
            lines.push(format!("monitor{i}={m}"));
        }
    }
    lines.push(format!(
        "VIBECAP_AUDIO_DEVICE={}",
        r.env_audio_device.as_deref().unwrap_or("(unset)")
    ));
    if let Some(hint) = &r.window_crop {
        lines.push(format!("window_crop={hint}"));
    }
    lines.push(format!("mcp_tools={}", r.mcp_tools));
    if let Some(ms) = r.startup_ms {
        lines.push(format!("startup_ms={ms}"));
    }
    if let Some(mb) = r.memory_mb {
        lines.push(format!("memory_mb={mb}"));
    }
    if let Some(tail) = &r.ffmpeg_log_tail {
        lines.push("ffmpeg_log_tail:".into());
        for l in tail
            .lines()
            .rev()
            .take(12)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            lines.push(format!("  {l}"));
        }
    }
    if r.stale_record_state {
        lines.push("stale_record_state=yes (doctor --fix clears it)".into());
    }
    lines.push("cli=vibecap --screenshot --output-dir DIR [--window NAME]".into());
    lines.push("record=vibecap record start|stop|status".into());
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_mentions_backend_and_ffmpeg() {
        let t = doctor_text();
        assert!(t.contains("backend="), "{t}");
        assert!(t.contains("ffmpeg="), "{t}");
        assert!(t.contains("vibecap "), "{t}");
    }

    #[test]
    fn doctor_json_is_an_object() {
        let j = doctor_json();
        assert!(j.contains("\"backend\""), "{j}");
        assert!(j.contains("\"mcp_tools\""), "{j}");
    }
}
