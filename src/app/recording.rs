//! Recording helpers: stop wait, filmstrip extract (no egui).

use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};

/// Wait for ffmpeg to exit after stdin `q`, with a hard timeout.
pub fn finalize_recorder(mut child: Child) -> std::io::Result<std::process::ExitStatus> {
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(b"q\n");
        let _ = stdin.flush();
    }
    let wait_start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if wait_start.elapsed() < Duration::from_secs(8) => {
                std::thread::sleep(Duration::from_millis(40));
            }
            Ok(None) => {
                let _ = child.kill();
                return child.wait();
            }
            Err(e) => return Err(e),
        }
    }
}

/// Kill a recorder process (resume first if paused via SIGSTOP).
pub fn kill_recorder(mut child: Child, was_paused: bool) {
    if was_paused {
        crate::platform::cont_process(child.id());
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Extract preview frames sampled across the whole clip; returns
/// (frames_temp dir, thumb paths that exist, extraction fps).
///
/// The fps is chosen so ~`TARGET` frames span the full duration, which lets the
/// in-app player flipbook at a known rate and the timeline align to real time.
/// `max_w` = filmstrip frame width (480 full, 240 for the low-res mode).
fn extract_filmstrip_thumbs_scaled(
    file: &Path,
    max_w: u32,
) -> Result<(PathBuf, Vec<PathBuf>, f64), String> {
    if !file.exists() {
        return Err(format!("Video file missing: {}", file.display()));
    }
    let out_dir = file
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("frames_temp");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("Could not create frames_temp: {e}"))?;

    let duration = crate::platform::probe_duration(file).unwrap_or(0.0);
    // 64 frames: a ~40 s clip flips every ~0.6 s — visibly alive in the
    // flipbook. Fewer frames made long clips look frozen ("won't start").
    let target = 64.0_f64;
    let fps = if duration > 0.5 {
        (target / duration).clamp(0.25, 6.0)
    } else {
        1.0
    };
    let vframes = if duration > 0.5 {
        (duration * fps).ceil() as i64
    } else {
        10
    }
    .clamp(1, 96);

    let out = out_dir.join("thumb_%03d.jpg");
    let file_s = file
        .to_str()
        .ok_or_else(|| "Video path is not valid UTF-8".to_string())?;
    let out_s = out.to_string_lossy().to_string();

    let mut cmd = crate::platform::ffmpeg_command()?;
    cmd.args([
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-i",
        file_s,
        "-vf",
        &format!("fps={fps},scale={max_w}:-2:flags=fast_bilinear"),
        "-vframes",
        &vframes.to_string(),
        &out_s,
    ]);
    crate::platform::run_ffmpeg(cmd, "ffmpeg filmstrip")
        .map_err(|e| format!("{e}. File still editable below."))?;

    let mut thumbs = Vec::new();
    for i in 1..=96 {
        let thumb_path = out_dir.join(format!("thumb_{:03}.jpg", i));
        if thumb_path.exists() {
            thumbs.push(thumb_path);
        }
    }
    if thumbs.is_empty() {
        return Err("No frames extracted — video may be corrupt or too short.".into());
    }
    Ok((out_dir, thumbs, fps))
}

/// Decode filmstrip JPEGs to RGBA on a worker thread (keeps the UI loop alive).
///
/// Returns `(frames, fps, duration_secs)` where each frame is `(w, h, rgba)`.
/// `progress` (when Some) receives `(decoded, total)` after each thumb so the
/// UI can show a determinate "i/n" label instead of a spinner.
pub fn extract_filmstrip_rgba(
    file: &Path,
    progress: Option<&(dyn Fn(usize, usize) + Sync)>,
    low_res: bool,
) -> Result<(Vec<(u32, u32, Vec<u8>)>, f64, f64), String> {
    let duration = crate::platform::probe_duration(file).unwrap_or(0.0);
    let (_out_dir, thumbs, fps) =
        extract_filmstrip_thumbs_scaled(file, if low_res { 240 } else { 480 })?;
    let total = thumbs.len();

    // Decode JPEGs in parallel — up to 96 image opens + RGBA converts is the
    // slow part of preview load; scoped threads keep it on the worker anyway.
    let workers = std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(4)
        .min(total.max(1));
    let decoded = std::sync::atomic::AtomicUsize::new(0);
    let mut slots: Vec<Option<(u32, u32, Vec<u8>)>> = Vec::with_capacity(total);
    slots.resize_with(total, || None);
    let mut chunks: Vec<Vec<Option<(u32, u32, Vec<u8>)>>> =
        (0..workers).map(|_| Vec::new()).collect();
    for (i, slot) in slots.into_iter().enumerate() {
        chunks[i % workers].push(slot);
    }

    let decoded_ref = &decoded;
    std::thread::scope(|s| {
        for (w, chunk) in chunks.iter_mut().enumerate() {
            let thumbs_w: Vec<&PathBuf> = thumbs
                .iter()
                .enumerate()
                .filter(|(i, _)| i % workers == w)
                .map(|(_, p)| p)
                .collect();
            let progress = &progress;
            s.spawn(move || {
                for (slot, path) in chunk.iter_mut().zip(thumbs_w) {
                    if let Ok(img) = image::open(path) {
                        let rgba = img.to_rgba8();
                        let (w, h) = rgba.dimensions();
                        if w > 0 && h > 0 {
                            *slot = Some((w, h, rgba.into_raw()));
                        }
                    }
                    let done = decoded_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if let Some(p) = progress {
                        p(done, total);
                    }
                }
            });
        }
    });

    let mut frames = Vec::with_capacity(total);
    // Reassemble in order — chunks are round-robin, slot j of worker w is
    // thumb index w + j*workers.
    for idx in 0..total {
        let slot = chunks[idx % workers][idx / workers].take();
        if let Some(f) = slot {
            frames.push(f);
        }
    }
    for thumb_path in &thumbs {
        let _ = std::fs::remove_file(thumb_path);
    }
    let _ = std::fs::remove_dir_all(&_out_dir);
    if frames.is_empty() {
        return Err("No frames extracted — video may be corrupt or too short.".into());
    }
    Ok((frames, fps, duration))
}

/// Mean absolute difference between two same-sized RGBA frames on a
/// subsampled grid (every 64th pixel) — 0.0 is identical, 255.0 opposite.
/// Cheap enough to run over a 64-frame filmstrip without a worker.
fn frame_diff(a: &[u8], b: &[u8]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 255.0;
    }
    let step_px = 64usize;
    let step = step_px * 4;
    let mut sum = 0u64;
    let mut count = 0u64;
    let mut i = 0usize;
    while i + 3 < n {
        let d0 = (a[i] as i32 - b[i] as i32).abs() as u64;
        let d1 = (a[i + 1] as i32 - b[i + 1] as i32).abs() as u64;
        let d2 = (a[i + 2] as i32 - b[i + 2] as i32).abs() as u64;
        sum += d0 + d1 + d2;
        count += 3;
        i += step;
    }
    if count == 0 {
        return 255.0;
    }
    sum as f64 / count as f64
}

/// Detect "dead air" — a frozen head and/or tail — over filmstrip frames.
///
/// A run of consecutive frames that barely differ from the previous one is
/// treated as dead. Returns `Some((content_start_s, content_end_s))` when at
/// least ~0.8 s of dead air exists at either end and trimming still leaves
/// ~1 s of content. None when the clip is alive edge-to-edge (or analysis
/// can't run — too few frames / zero fps).
pub fn dead_air_bounds(frames: &[(u32, u32, Vec<u8>)], fps: f64) -> Option<(f64, f64)> {
    const DEAD_DIFF: f64 = 2.0; // mean |Δ| per channel byte
    let n = frames.len();
    if n < 4 || fps <= 0.0 {
        return None;
    }
    let dead = |i: usize, j: usize| frame_diff(&frames[i].2, &frames[j].2) < DEAD_DIFF;

    // Head: leading run of frames ≈ the first frame.
    let mut head_dead = 0usize;
    while head_dead + 1 < n && dead(head_dead, head_dead + 1) {
        head_dead += 1;
    }
    // Tail: trailing run of frames ≈ the last frame.
    let mut tail_dead = 0usize;
    while tail_dead + 1 < n && dead(n - 1 - tail_dead, n - 2 - tail_dead) {
        tail_dead += 1;
    }

    let head_s = head_dead as f64 / fps;
    let tail_s = tail_dead as f64 / fps;
    let content_start = head_s;
    let content_end = (n - tail_dead) as f64 / fps;

    // Require meaningful dead air at at least one end and ≥1 s of content.
    let dead_total = head_s + tail_s;
    if dead_total < 0.8 || content_end - content_start < 1.0 {
        return None;
    }
    Some((content_start, content_end))
}

/// Crop tuple for ffmpeg: (w, h, x, y) with even dimensions for yuv420p.
///
/// Shared helper for recorder call sites.
#[allow(dead_code)]
pub fn even_crop(w: i32, h: i32, x: i32, y: i32) -> (i32, i32, i32, i32) {
    crate::platform::even_screen_rect(x, y, w, h).as_whxy()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_crop_forces_even_dimensions() {
        // Negative origin is valid (virtual desktop left of primary).
        assert_eq!(even_crop(801, 601, -4, 3), (800, 600, -4, 3));
        assert_eq!(even_crop(2, 2, 0, 0), (2, 2, 0, 0));
    }

    #[test]
    fn dead_air_detects_frozen_ends() {
        let frame = |v: u8| (64u32, 64u32, vec![v; 64 * 64 * 4]);
        // 10 frozen head · 20 moving · 10 frozen tail @ 10 fps.
        let mut frames = vec![frame(10); 10];
        for i in 0..20u8 {
            frames.push(frame(30 + i * 8));
        }
        frames.extend(vec![frame(220); 10]);
        let b = dead_air_bounds(&frames, 10.0).expect("dead air expected");
        assert!((b.0 - 0.9).abs() < 0.15, "content start {}", b.0);
        assert!((b.1 - 3.1).abs() < 0.15, "content end {}", b.1);
    }

    #[test]
    fn dead_air_none_when_alive() {
        let mut frames = Vec::new();
        for i in 0..40u8 {
            frames.push((64u32, 64u32, vec![i.wrapping_mul(6); 64 * 64 * 4]));
        }
        assert!(dead_air_bounds(&frames, 10.0).is_none());
    }
}
