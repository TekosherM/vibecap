//! Agent budget config + live-dir usage accounting (shared by GUI + MCP).

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use super::io::{vibecap_config_dir, write_json_atomic};
use super::live::{get_budget_note_mutex, get_live_started_mutex, LIVE_INSPECTION_RUNNING};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct BudgetConfig {
    /// 0 = unlimited
    pub max_frames: u32,
    /// 0.0 = unlimited
    pub max_mb: f64,
    /// 0 = unlimited
    pub max_minutes: u32,
    /// "eco" | "standard" | "intensive"
    pub analysis_tier: String,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_frames: 0,
            max_mb: 0.0,
            max_minutes: 0,
            analysis_tier: "standard".to_string(),
        }
    }
}

pub fn budget_file_path() -> PathBuf {
    vibecap_config_dir().join("budget.json")
}

/// Ok(None) = no budget set; Ok(Some) = budget; Err = corrupt/unreadable (enforcement fails closed).
pub fn budget_file_state() -> Result<Option<BudgetConfig>, String> {
    match std::fs::read_to_string(budget_file_path()) {
        Ok(s) => serde_json::from_str(&s)
            .map(Some)
            .map_err(|e| format!("corrupt budget.json: {}", e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("unreadable budget.json: {}", e)),
    }
}

pub fn load_budget() -> BudgetConfig {
    budget_file_state().ok().flatten().unwrap_or_default()
}

pub fn save_budget(cfg: &BudgetConfig) -> Result<(), String> {
    let s = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    write_json_atomic(&budget_file_path(), &s)
}

/// Budget usage snapshot for the live dir: (frames, mb, elapsed_minutes).
/// Frames = timestamped capture files only (frame_*/live_*/video_*); MB = all bytes on disk.
/// Elapsed is 0 when no stream is running (prevents stale timers from haunting later sessions).
pub fn live_usage_snapshot(live_dir: &str) -> (usize, f64, f64) {
    let mut frames = 0usize;
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(live_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                    if name.starts_with("frame_")
                        || name.starts_with("live_")
                        || name.starts_with("video_")
                    {
                        frames += 1;
                    }
                }
            }
        }
    }
    let mb = (total as f64) / (1024.0 * 1024.0);
    let elapsed = if LIVE_INSPECTION_RUNNING.load(Ordering::SeqCst) {
        match get_live_started_mutex().lock() {
            Ok(l) => (*l)
                .map(|t| t.elapsed().as_secs_f64() / 60.0)
                .unwrap_or(0.0),
            Err(_) => 0.0,
        }
    } else {
        0.0
    };
    (frames, mb, elapsed)
}

/// Returns Some(reason) if any budget cap is exceeded.
pub fn budget_exceeded_reason(live_dir: &str) -> Option<String> {
    if let Err(e) = budget_file_state() {
        return Some(format!(
            "⚠️ {} — enforcement is fail-closed; fix or delete {}",
            e,
            budget_file_path().display()
        ));
    }
    let cfg = load_budget();
    let (frames, mb, minutes) = live_usage_snapshot(live_dir);
    if cfg.max_frames > 0 && frames >= cfg.max_frames as usize {
        return Some(format!(
            "frame cap reached ({}/{} frames)",
            frames, cfg.max_frames
        ));
    }
    if cfg.max_mb > 0.0 && mb >= cfg.max_mb {
        return Some(format!(
            "storage cap reached ({:.1}/{:.1} MB)",
            mb, cfg.max_mb
        ));
    }
    if cfg.max_minutes > 0 && minutes >= cfg.max_minutes as f64 {
        return Some(format!(
            "time cap reached ({:.1}/{} min)",
            minutes, cfg.max_minutes
        ));
    }
    None
}

/// One-line budget status for MCP tool responses.
pub fn budget_status_line(live_dir: &str) -> String {
    let cfg = load_budget();
    let (frames, mb, minutes) = live_usage_snapshot(live_dir);
    let note = get_budget_note_mutex()
        .lock()
        .map(|n| n.clone())
        .unwrap_or_default();
    let frames_cap = if cfg.max_frames == 0 {
        "unlimited".to_string()
    } else {
        cfg.max_frames.to_string()
    };
    let mb_cap = if cfg.max_mb <= 0.0 {
        "unlimited".to_string()
    } else {
        format!("{:.1}", cfg.max_mb)
    };
    let min_cap = if cfg.max_minutes == 0 {
        "unlimited".to_string()
    } else {
        cfg.max_minutes.to_string()
    };
    let mut s = format!(
        "💰 BUDGET: frames {}/{}, {:.1}/{} MB, {:.1}/{} min (tier: {})",
        frames, frames_cap, mb, mb_cap, minutes, min_cap, cfg.analysis_tier
    );
    if !note.is_empty() {
        s.push_str(&format!(" — ⚠️ {}", note));
    }
    s
}
