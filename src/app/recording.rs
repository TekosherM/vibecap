//! Recording helpers: stop wait, filmstrip extract (no egui).

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
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

/// Extract up to 10 filmstrip thumbnails; returns (frames_temp dir, thumb paths that exist).
pub fn extract_filmstrip_thumbs(file: &Path) -> Result<(PathBuf, Vec<PathBuf>), String> {
    if !file.exists() {
        return Err(format!("Video file missing: {}", file.display()));
    }
    let out_dir = file
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("frames_temp");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("Could not create frames_temp: {e}"))?;

    let out = out_dir.join("thumb_%03d.jpg");
    let file_s = file
        .to_str()
        .ok_or_else(|| "Video path is not valid UTF-8".to_string())?;
    let out_s = out.to_string_lossy().to_string();

    let status = crate::platform::ffmpeg_command()?
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            file_s,
            "-vf",
            "fps=1,scale=320:180:force_original_aspect_ratio=decrease",
            "-vframes",
            "10",
            &out_s,
        ])
        .status()
        .map_err(|e| format!("Could not run ffmpeg for filmstrip: {e}"))?;

    if !status.success() {
        return Err(format!(
            "ffmpeg filmstrip failed (exit {}). File still editable below.",
            status.code().unwrap_or(-1)
        ));
    }

    let mut thumbs = Vec::new();
    for i in 1..=10 {
        let thumb_path = out_dir.join(format!("thumb_{:03}.jpg", i));
        if thumb_path.exists() {
            thumbs.push(thumb_path);
        }
    }
    if thumbs.is_empty() {
        return Err("No frames extracted — video may be corrupt or too short.".into());
    }
    Ok((out_dir, thumbs))
}

/// Crop tuple for ffmpeg: (w, h, x, y) with even dimensions for yuv420p.
pub fn even_crop(w: i32, h: i32, x: i32, y: i32) -> (i32, i32, i32, i32) {
    let w = (w.abs().max(2) / 2) * 2;
    let h = (h.abs().max(2) / 2) * 2;
    (w.max(2), h.max(2), x.max(0), y.max(0))
}
