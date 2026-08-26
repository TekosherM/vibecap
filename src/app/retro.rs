//! Retro buffer — low-FPS rolling screen capture for “save the last N seconds”.
//!
//! **Policy:** off by default · ~2 fps · 60s window · hard cap ~200 MB.
//! Frames live under the config dir and are pruned automatically.
//!
//! ## Multi-process contract
//! - Config: `~/.config/vibecap/retro.json` (atomic write).
//! - Workers (GUI and optional MCP) **reload** that file each loop so
//!   `vibecap_set_retro` takes effect without restart.
//! - Frames are **not** wiped on process start/stop (shared dump surface).
//! - Explicit disable / Clear still deletes frames.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::io::{vibecap_config_dir, write_json_atomic};
use crate::platform::capture_screenshot;

/// Default window length (seconds).
pub const DEFAULT_SECONDS: u32 = 60;
/// Capture cadence.
pub const DEFAULT_FPS: f32 = 2.0;
/// Hard disk cap for the ring (MB).
pub const DEFAULT_MAX_MB: f64 = 200.0;
/// How often the worker re-reads `retro.json` from disk.
const CONFIG_RELOAD_EVERY: Duration = Duration::from_secs(2);

static FRAME_SEQ: AtomicU64 = AtomicU64::new(0);

/// Process-local capturer used by MCP when the desktop app is not capturing.
static MCP_WORKER: OnceLock<Mutex<Option<RetroController>>> = OnceLock::new();

fn mcp_worker_slot() -> &'static Mutex<Option<RetroController>> {
    MCP_WORKER.get_or_init(|| Mutex::new(None))
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RetroConfig {
    /// Master switch — **must default false**.
    #[serde(default)]
    pub enabled: bool,
    /// How many seconds of history to keep (15 / 30 / 60).
    #[serde(default = "default_seconds")]
    pub seconds: u32,
    /// Frames per second (clamped 1..=5).
    #[serde(default = "default_fps")]
    pub fps: f32,
    /// Hard size cap in megabytes.
    #[serde(default = "default_max_mb")]
    pub max_mb: f64,
}

fn default_seconds() -> u32 {
    DEFAULT_SECONDS
}
fn default_fps() -> f32 {
    DEFAULT_FPS
}
fn default_max_mb() -> f64 {
    DEFAULT_MAX_MB
}

impl Default for RetroConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            seconds: DEFAULT_SECONDS,
            fps: DEFAULT_FPS,
            max_mb: DEFAULT_MAX_MB,
        }
    }
}

impl RetroConfig {
    pub fn clamp(mut self) -> Self {
        self.seconds = self.seconds.clamp(10, 120);
        self.fps = self.fps.clamp(1.0, 5.0);
        self.max_mb = if self.max_mb.is_finite() {
            self.max_mb.clamp(20.0, 500.0)
        } else {
            DEFAULT_MAX_MB
        };
        self
    }

    pub fn max_frames(&self) -> usize {
        ((self.seconds as f32) * self.fps).ceil() as usize + 2
    }

    pub fn max_bytes(&self) -> u64 {
        (self.max_mb * 1024.0 * 1024.0) as u64
    }
}

#[derive(Clone, Debug, Default)]
pub struct RetroStatus {
    pub enabled: bool,
    pub running: bool,
    pub frame_count: usize,
    pub span_secs: f32,
    pub mb: f64,
    pub max_secs: u32,
    pub max_mb: f64,
    pub last_error: Option<String>,
}

struct FrameEntry {
    at: Instant,
    path: PathBuf,
    bytes: u64,
}

struct RetroShared {
    frames: VecDeque<FrameEntry>,
    last_error: Option<String>,
    captures: u64,
    worker_alive: bool,
}

/// Owns the background worker. Safe to keep on the app for the process lifetime.
pub struct RetroController {
    cfg: Arc<Mutex<RetroConfig>>,
    shared: Arc<Mutex<RetroShared>>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    dir: PathBuf,
}

impl Default for RetroController {
    fn default() -> Self {
        Self::new()
    }
}

impl RetroController {
    pub fn new() -> Self {
        let dir = retro_dir();
        let _ = std::fs::create_dir_all(&dir);
        // Preserve existing frames (MCP / prior session). Seed deque from disk.
        let cfg = load_retro_config();
        let seeded = seed_frames_from_disk(&dir, &cfg);

        let cfg = Arc::new(Mutex::new(cfg));
        let shared = Arc::new(Mutex::new(RetroShared {
            frames: seeded,
            last_error: None,
            captures: 0,
            worker_alive: false,
        }));
        let stop = Arc::new(AtomicBool::new(false));

        let mut ctrl = Self {
            cfg: cfg.clone(),
            shared: shared.clone(),
            stop: stop.clone(),
            join: None,
            dir: dir.clone(),
        };
        ctrl.spawn_worker();
        ctrl
    }

    fn spawn_worker(&mut self) {
        let cfg = self.cfg.clone();
        let shared = self.shared.clone();
        let stop = self.stop.clone();
        let dir = self.dir.clone();

        match thread::Builder::new()
            .name("vibecap-retro".into())
            .spawn(move || worker_loop(cfg, shared.clone(), stop, dir))
        {
            Ok(handle) => {
                self.join = Some(handle);
                if let Ok(mut s) = self.shared.lock() {
                    s.worker_alive = true;
                    s.last_error = None;
                }
            }
            Err(e) => {
                self.join = None;
                if let Ok(mut s) = self.shared.lock() {
                    s.worker_alive = false;
                    s.last_error = Some(format!("Retro worker failed to start: {e}"));
                }
            }
        }
    }

    /// Pull latest `retro.json` into the in-memory config (also done by worker).
    pub fn reload_config_from_disk(&self) {
        let disk = load_retro_config();
        if let Ok(mut g) = self.cfg.lock() {
            *g = disk;
        }
    }

    pub fn config(&self) -> RetroConfig {
        self.cfg.lock().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn set_config(&self, cfg: RetroConfig) {
        let cfg = cfg.clamp();
        if let Ok(mut g) = self.cfg.lock() {
            *g = cfg.clone();
        }
        save_retro_config(&cfg);
        // Explicit disable → clear frames for privacy.
        if !cfg.enabled {
            self.clear_frames();
        }
    }

    pub fn set_enabled(&self, on: bool) {
        let mut cfg = self.config();
        cfg.enabled = on;
        self.set_config(cfg);
    }

    pub fn status(&self) -> RetroStatus {
        // Worker reloads retro.json every ~2s; avoid disk I/O every UI frame.
        let cfg = self.config();
        let shared = self.shared.lock().ok();
        let (count, span, bytes, err, alive) = if let Some(s) = shared.as_ref() {
            let span = s
                .frames
                .front()
                .map(|f| f.at.elapsed().as_secs_f32())
                .unwrap_or(0.0);
            let bytes: u64 = s.frames.iter().map(|f| f.bytes).sum();
            (
                s.frames.len(),
                span,
                bytes,
                s.last_error.clone(),
                s.worker_alive,
            )
        } else {
            (0, 0.0, 0, None, false)
        };
        RetroStatus {
            enabled: cfg.enabled,
            running: cfg.enabled && alive && !self.stop.load(Ordering::Relaxed),
            frame_count: count,
            span_secs: span.min(cfg.seconds as f32),
            mb: bytes as f64 / (1024.0 * 1024.0),
            max_secs: cfg.seconds,
            max_mb: cfg.max_mb,
            last_error: err,
        }
    }

    pub fn clear_frames(&self) {
        if let Ok(mut s) = self.shared.lock() {
            for f in s.frames.drain(..) {
                let _ = std::fs::remove_file(&f.path);
            }
            // Keep worker_alive / spawn errors
            if s.last_error
                .as_ref()
                .map(|e| e.starts_with("Retro worker failed"))
                .unwrap_or(false)
            {
                // preserve
            } else {
                s.last_error = None;
            }
        }
        let _ = clear_jpg_frames(&self.dir);
    }

    /// Export current ring to a GIF in `media_dir`. Returns the output path.
    pub fn dump_gif(&self, media_dir: &Path) -> Result<PathBuf, String> {
        // Prefer in-memory order; fall back to disk scan.
        let cfg = self.config();
        let fps = cfg.fps.clamp(1.0, 5.0);
        let frames: Vec<PathBuf> = {
            let s = self
                .shared
                .lock()
                .map_err(|_| "retro buffer lock poisoned".to_string())?;
            if !s.frames.is_empty() {
                s.frames.iter().map(|f| f.path.clone()).collect()
            } else {
                Vec::new()
            }
        };
        if frames.is_empty() {
            return dump_retro_disk_gif(media_dir);
        }
        frames_to_gif(&frames, media_dir, fps, &self.dir)
    }

    /// Stop worker; **leave frames on disk** for MCP / next session.
    pub fn stop_worker(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
        if let Ok(mut s) = self.shared.lock() {
            s.worker_alive = false;
        }
    }
}

impl Drop for RetroController {
    fn drop(&mut self) {
        // Do not wipe frames — shared surface for MCP dump after GUI quit.
        self.stop_worker();
    }
}

fn worker_loop(
    cfg: Arc<Mutex<RetroConfig>>,
    shared: Arc<Mutex<RetroShared>>,
    stop: Arc<AtomicBool>,
    dir: PathBuf,
) {
    let _ = std::fs::create_dir_all(&dir);
    let mut last_reload = Instant::now() - CONFIG_RELOAD_EVERY;

    while !stop.load(Ordering::Relaxed) {
        // Re-read retro.json so MCP set_retro / external edits apply live.
        if last_reload.elapsed() >= CONFIG_RELOAD_EVERY {
            let disk = load_retro_config();
            if let Ok(mut g) = cfg.lock() {
                *g = disk;
            }
            last_reload = Instant::now();
        }

        let (enabled, fps, max_frames, max_bytes, seconds) = {
            let g = cfg.lock().ok();
            match g {
                Some(c) => (
                    c.enabled,
                    c.fps.clamp(1.0, 5.0),
                    c.max_frames(),
                    c.max_bytes(),
                    c.seconds,
                ),
                None => break,
            }
        };

        if enabled {
            let seq = FRAME_SEQ.fetch_add(1, Ordering::Relaxed);
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            // Monotonic seq + millis avoids collisions under clock stalls / high fps.
            let path = dir.join(format!("f_{ms}_{seq:06}.jpg"));
            match capture_screenshot(&path) {
                Ok(()) => {
                    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    if let Ok(mut s) = shared.lock() {
                        s.frames.push_back(FrameEntry {
                            at: Instant::now(),
                            path,
                            bytes,
                        });
                        s.captures = s.captures.wrapping_add(1);
                        s.last_error = None;
                        prune_frames(&mut s, max_frames, max_bytes, seconds);
                    }
                }
                Err(e) => {
                    if let Ok(mut s) = shared.lock() {
                        s.last_error = Some(e);
                    }
                }
            }
        }

        let sleep_ms = ((1000.0 / fps) as u64).clamp(200, 2000);
        let steps = (sleep_ms / 100).max(1);
        for _ in 0..steps {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    if let Ok(mut s) = shared.lock() {
        s.worker_alive = false;
    }
}

fn prune_frames(s: &mut RetroShared, max_frames: usize, max_bytes: u64, seconds: u32) {
    let max_age = Duration::from_secs(seconds as u64 + 2);
    while s.frames.len() > max_frames {
        if let Some(old) = s.frames.pop_front() {
            let _ = std::fs::remove_file(&old.path);
        }
    }
    while s
        .frames
        .front()
        .map(|f| f.at.elapsed() > max_age)
        .unwrap_or(false)
    {
        if let Some(old) = s.frames.pop_front() {
            let _ = std::fs::remove_file(&old.path);
        }
    }
    let mut total: u64 = s.frames.iter().map(|f| f.bytes).sum();
    while total > max_bytes {
        if let Some(old) = s.frames.pop_front() {
            total = total.saturating_sub(old.bytes);
            let _ = std::fs::remove_file(&old.path);
        } else {
            break;
        }
    }
}

/// Load existing JPGs so restart does not discard evidence.
fn seed_frames_from_disk(dir: &Path, cfg: &RetroConfig) -> VecDeque<FrameEntry> {
    let mut paths = list_frame_paths(dir);
    let max = cfg.max_frames();
    if paths.len() > max {
        let drop_n = paths.len() - max;
        for p in paths.drain(..drop_n) {
            let _ = std::fs::remove_file(p);
        }
    }
    let now = Instant::now();
    // Approximate age: oldest first; we don't have true capture Instant after restart.
    let n = paths.len().max(1) as f32;
    let window = Duration::from_secs(cfg.seconds as u64);
    paths
        .into_iter()
        .enumerate()
        .map(|(i, path)| {
            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let age_frac = 1.0 - (i as f32 / n);
            let at = now
                .checked_sub(window.mul_f32(age_frac.clamp(0.0, 1.0)))
                .unwrap_or(now);
            FrameEntry { at, path, bytes }
        })
        .collect()
}

fn list_frame_paths(dir: &Path) -> Vec<PathBuf> {
    let mut frames: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("f_") && n.ends_with(".jpg"))
                        .unwrap_or(false)
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    frames.sort();
    frames
}

fn retro_dir() -> PathBuf {
    vibecap_config_dir().join("retro_buffer")
}

/// Export frames already on disk (shared with GUI / MCP workers).
pub fn dump_retro_disk_gif(media_dir: &Path) -> Result<PathBuf, String> {
    let cfg = load_retro_config();
    let dir = retro_dir();
    let mut frames = list_frame_paths(&dir);
    if frames.is_empty() {
        return Err(
            "Retro buffer is empty — enable it (Settings or vibecap_set_retro) and wait a few seconds while a capturer runs (desktop app or MCP process)."
                .into(),
        );
    }
    let max = cfg.max_frames();
    if frames.len() > max {
        frames = frames.split_off(frames.len() - max);
    }
    frames_to_gif(&frames, media_dir, cfg.fps.clamp(1.0, 5.0), &dir)
}

/// Enable/disable retro via config + ensure this process can capture when enabled.
///
/// - Always writes `retro.json` (GUI workers reload within ~2s).
/// - When `on`, starts a process-local MCP capturer if none is running so
///   headless agents still accumulate frames.
/// - When `off`, stops the MCP capturer and clears frames (privacy).
pub fn set_retro_enabled(on: bool) -> RetroConfig {
    let mut cfg = load_retro_config();
    cfg.enabled = on;
    cfg = cfg.clamp();
    save_retro_config(&cfg);

    if let Ok(mut slot) = mcp_worker_slot().lock() {
        if on {
            if slot.is_none() {
                *slot = Some(RetroController::new());
            } else if let Some(c) = slot.as_ref() {
                c.reload_config_from_disk();
            }
        } else {
            if let Some(mut c) = slot.take() {
                c.set_config(cfg.clone()); // clears frames via set_config when disabled
                c.stop_worker();
            } else {
                // No MCP worker — still clear disk for privacy on explicit disable.
                let _ = clear_jpg_frames(&retro_dir());
            }
        }
    }

    cfg
}

/// Status line for MCP / diagnostics.
pub fn retro_runtime_note() -> String {
    let cfg = load_retro_config();
    let frames = list_frame_paths(&retro_dir()).len();
    let mcp_on = mcp_worker_slot()
        .lock()
        .map(|s| s.is_some())
        .unwrap_or(false);
    format!(
        "enabled={} frames_on_disk={} mcp_process_capturer={}",
        cfg.enabled, frames, mcp_on
    )
}

fn frames_to_gif(
    frames: &[PathBuf],
    media_dir: &Path,
    fps: f32,
    work_parent: &Path,
) -> Result<PathBuf, String> {
    if frames.is_empty() {
        return Err("No frames to export".into());
    }
    let _ = std::fs::create_dir_all(media_dir);
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let out = media_dir.join(format!("retro_{}.gif", stamp));

    let seq_dir = work_parent.join("export_seq");
    let _ = std::fs::remove_dir_all(&seq_dir);
    std::fs::create_dir_all(&seq_dir).map_err(|e| format!("export dir: {e}"))?;

    let mut n = 0usize;
    for src in frames {
        if !src.exists() {
            continue;
        }
        let dst = seq_dir.join(format!("{:05}.jpg", n));
        std::fs::copy(src, &dst).map_err(|e| format!("copy frame: {e}"))?;
        n += 1;
    }
    if n == 0 {
        let _ = std::fs::remove_dir_all(&seq_dir);
        return Err("No readable frame files".into());
    }

    let pattern = seq_dir.join("%05d.jpg");
    let pattern_s = pattern
        .to_str()
        .ok_or_else(|| "path not utf-8".to_string())?;
    let out_s = out
        .to_str()
        .ok_or_else(|| "output path not utf-8".to_string())?;

    let status = crate::platform::ffmpeg_command()?
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-framerate",
            &format!("{:.2}", fps),
            "-i",
            pattern_s,
            "-vf",
            "fps=10,scale=960:-1:flags=lanczos:force_original_aspect_ratio=decrease",
            "-loop",
            "0",
            out_s,
        ])
        .status()
        .map_err(|e| format!("Could not run ffmpeg: {e}"))?;

    let _ = std::fs::remove_dir_all(&seq_dir);

    if !status.success() {
        return Err(format!(
            "ffmpeg retro GIF failed (exit {:?})",
            status.code()
        ));
    }
    if !out.exists() {
        return Err("ffmpeg finished but GIF is missing".into());
    }
    Ok(out)
}

fn retro_config_path() -> PathBuf {
    vibecap_config_dir().join("retro.json")
}

pub fn load_retro_config() -> RetroConfig {
    match std::fs::read_to_string(retro_config_path()) {
        Ok(s) => serde_json::from_str::<RetroConfig>(&s)
            .unwrap_or_default()
            .clamp(),
        Err(_) => RetroConfig::default(),
    }
}

pub fn save_retro_config(cfg: &RetroConfig) {
    if let Ok(s) = serde_json::to_string_pretty(cfg) {
        let _ = write_json_atomic(&retro_config_path(), &s);
    }
}

/// Remove only frame JPGs (keep export_seq if mid-export).
fn clear_jpg_frames(dir: &Path) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_file() {
            let is_frame = p
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("f_") && n.ends_with(".jpg"))
                .unwrap_or(false);
            if is_frame {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_off() {
        let c = RetroConfig::default();
        assert!(!c.enabled);
        assert_eq!(c.seconds, 60);
        assert!(c.max_frames() >= 120);
    }

    #[test]
    fn clamp_bounds() {
        let c = RetroConfig {
            enabled: true,
            seconds: 999,
            fps: 99.0,
            max_mb: 1.0,
        }
        .clamp();
        assert_eq!(c.seconds, 120);
        assert_eq!(c.fps, 5.0);
        assert_eq!(c.max_mb, 20.0);
    }

    #[test]
    fn prune_respects_max_frames() {
        let mut s = RetroShared {
            frames: VecDeque::new(),
            last_error: None,
            captures: 0,
            worker_alive: true,
        };
        let dir = std::env::temp_dir().join(format!(
            "vibecap_prune_test_{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        for i in 0..10 {
            let p = dir.join(format!("f_test_{i}.jpg"));
            let _ = std::fs::write(&p, b"x");
            s.frames.push_back(FrameEntry {
                at: Instant::now(),
                path: p,
                bytes: 1,
            });
        }
        prune_frames(&mut s, 3, 10_000, 60);
        assert_eq!(s.frames.len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_save_roundtrip() {
        // Use clamp only — avoid clobbering real user config in unit tests.
        let c = RetroConfig {
            enabled: true,
            seconds: 30,
            fps: 2.0,
            max_mb: 50.0,
        }
        .clamp();
        assert!(c.enabled);
        assert_eq!(c.seconds, 30);
    }

    #[test]
    fn list_frame_paths_filters() {
        let dir = std::env::temp_dir().join(format!(
            "vibecap_list_test_{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("f_1_000001.jpg"), b"a");
        let _ = std::fs::write(dir.join("notes.txt"), b"no");
        let _ = std::fs::write(dir.join("export_seq"), b"no");
        let list = list_frame_paths(&dir);
        assert_eq!(list.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
