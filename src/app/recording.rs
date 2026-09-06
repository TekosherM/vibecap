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
pub fn extract_filmstrip_thumbs(
    file: &Path,
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
    let target = 24.0_f64;
    let fps = if duration > 0.5 {
        (target / duration).clamp(0.25, 4.0)
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
        &format!("fps={fps},scale=480:-2:flags=fast_bilinear"),
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
pub fn extract_filmstrip_rgba(
    file: &Path,
) -> Result<(Vec<(u32, u32, Vec<u8>)>, f64, f64), String> {
    let duration = crate::platform::probe_duration(file).unwrap_or(0.0);
    let (_out_dir, thumbs, fps) = extract_filmstrip_thumbs(file)?;
    let mut frames = Vec::with_capacity(thumbs.len());
    for thumb_path in &thumbs {
        match image::open(thumb_path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                if w > 0 && h > 0 {
                    frames.push((w, h, rgba.into_raw()));
                }
            }
            Err(_) => {}
        }
        let _ = std::fs::remove_file(thumb_path);
    }
    let _ = std::fs::remove_dir_all(&_out_dir);
    if frames.is_empty() {
        return Err("No frames extracted — video may be corrupt or too short.".into());
    }
    Ok((frames, fps, duration))
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
}
