use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use super::paths::media_dir;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveFormat {
    Jpg,
    Gif,
    Mp4,
}

impl LiveFormat {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "jpg" | "jpeg" => Self::Jpg,
            "mp4" => Self::Mp4,
            _ => Self::Gif,
        }
    }
}

fn path_str(p: &Path) -> Result<&str, String> {
    p.to_str()
        .ok_or_else(|| "path is not valid UTF-8".to_string())
}

fn run_status(mut cmd: Command, what: &str) -> Result<(), String> {
    let status = cmd
        .status()
        .map_err(|e| format!("could not start {}: {}", what, e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} failed (exit {:?})", what, status.code()))
    }
}

/// Full-screen screenshot to `out` (jpg/png path chosen by caller).
pub fn capture_screenshot(out: &Path) -> Result<(), String> {
    let out_s = path_str(out)?;

    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("screencapture");
        // -x silent, -t jpg. Without Screen Recording permission, macOS often
        // still exits 0 but only captures wallpaper / no app windows.
        cmd.args(["-x", "-t", "jpg", out_s]);
        run_status(cmd, "screencapture").map_err(|e| {
            format!(
                "{} — grant Screen Recording to Vibecap in System Settings → Privacy & Security → Screen Recording, then quit & reopen the app",
                e
            )
        })?;
        return validate_capture_file(out);
    }

    #[cfg(target_os = "windows")]
    {
        // gdigrab single frame
        let mut cmd = super::ffmpeg::ffmpeg_command()?;
        cmd.args([
            "-y",
            "-f",
            "gdigrab",
            "-framerate",
            "1",
            "-i",
            "desktop",
            "-frames:v",
            "1",
            "-q:v",
            "2",
            out_s,
        ]);
        return run_status(cmd, "ffmpeg gdigrab screenshot");
    }

    #[cfg(target_os = "linux")]
    {
        // Prefer grim (Wayland), then import (ImageMagick), then ffmpeg x11grab.
        if Command::new("grim")
            .arg(out_s)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Ok(());
        }
        if Command::new("import")
            .args(["-window", "root", out_s])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Ok(());
        }
        let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0.0".into());
        let mut cmd = super::ffmpeg::ffmpeg_command()?;
        cmd.args([
            "-y",
            "-f",
            "x11grab",
            "-video_size",
            &linux_screen_size(),
            "-i",
            &display,
            "-frames:v",
            "1",
            "-q:v",
            "2",
            out_s,
        ]);
        return run_status(cmd, "ffmpeg x11grab screenshot");
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = out_s;
        Err("screenshot is not supported on this platform".into())
    }
}

/// Interactive region/window capture when the OS supports it (macOS screencapture -i).
/// Falls back to full-screen capture elsewhere.
pub fn capture_screenshot_interactive(out: &Path, interactive: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let out_s = path_str(out)?;
        let mut cmd = Command::new("screencapture");
        cmd.args(["-x", "-t", "jpg"]);
        if interactive {
            cmd.arg("-i");
        }
        cmd.arg(out_s);
        run_status(cmd, "screencapture").map_err(|e| {
            format!(
                "{} — grant Screen Recording to Vibecap in System Settings → Privacy & Security → Screen Recording, then quit & reopen",
                e
            )
        })?;
        // Interactive cancel can leave no/empty file
        if !out.exists() {
            return Err("Capture cancelled or failed — no file written".into());
        }
        return validate_capture_file(out);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = interactive;
        capture_screenshot(out)
    }
}

/// Reject tiny / missing captures (common when Screen Recording is denied).
fn validate_capture_file(out: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(out).map_err(|e| format!("capture file missing: {e}"))?;
    let len = meta.len();
    // A real multi-display JPG is usually >> 50 KB; TCC-denied shots can be tiny or wallpaper-only.
    if len < 8_000 {
        let _ = std::fs::remove_file(out);
        return Err(
            "Capture looks empty ({} bytes). On macOS: System Settings → Privacy & Security → Screen Recording → enable Vibecap (and restart the app). CLI captures need Terminal/iTerm allowed too."
                .replace("{}", &len.to_string()),
        );
    }
    Ok(())
}

/// Record a fixed-duration screen clip to `out_mp4`.
pub fn record_screen_clip(out_mp4: &Path, duration_secs: u64) -> Result<(), String> {
    let out_s = path_str(out_mp4)?;
    let dur = duration_secs.max(1).to_string();

    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("screencapture");
        cmd.args(["-v", "-V", &dur, out_s]);
        return run_status(cmd, "screencapture -v");
    }

    #[cfg(target_os = "windows")]
    {
        let mut cmd = super::ffmpeg::ffmpeg_command()?;
        cmd.args([
            "-y",
            "-f",
            "gdigrab",
            "-framerate",
            "30",
            "-t",
            &dur,
            "-i",
            "desktop",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            out_s,
        ]);
        return run_status(cmd, "ffmpeg gdigrab record");
    }

    #[cfg(target_os = "linux")]
    {
        let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0.0".into());
        let mut cmd = super::ffmpeg::ffmpeg_command()?;
        cmd.args([
            "-y",
            "-f",
            "x11grab",
            "-framerate",
            "30",
            "-video_size",
            &linux_screen_size(),
            "-t",
            &dur,
            "-i",
            &display,
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            out_s,
        ]);
        return run_status(cmd, "ffmpeg x11grab record");
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (out_s, dur);
        Err("screen recording is not supported on this platform".into())
    }
}

/// Export a motion GIF from a video range (requires ffmpeg).
pub fn export_gif_clip(
    video_path: &str,
    start_time: &str,
    end_time: &str,
    gif_out: &str,
) -> Result<(), String> {
    let mut cmd = super::ffmpeg::ffmpeg_command()?;
    cmd.args([
        "-ss",
        start_time,
        "-to",
        end_time,
        "-i",
        video_path,
        "-vf",
        "fps=15,scale=800:-1:flags=lanczos",
        "-y",
        gif_out,
    ]);
    run_status(cmd, "ffmpeg gif export")
}

/// Convert a short MP4 chunk to a GIF (live inspection).
pub fn mp4_to_gif(mp4: &Path, gif: &Path) -> Result<(), String> {
    let mut cmd = super::ffmpeg::ffmpeg_command()?;
    cmd.args([
        "-i",
        path_str(mp4)?,
        "-vf",
        "fps=15,scale=800:-1:flags=lanczos",
        "-y",
        path_str(gif)?,
    ]);
    run_status(cmd, "ffmpeg mp4→gif")
}

/// Headless screenshot into the media directory; returns the output path.
pub fn capture_to_media_dir() -> Result<PathBuf, String> {
    let out = media_dir().join(format!(
        "screenshot_{}.jpg",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
    ));
    capture_screenshot(&out)?;
    Ok(out)
}

/// Capture one live-inspection frame into `dir` for the given format.
/// Returns `(latest_path, timestamped_path)`.
pub fn capture_live_frame(
    dir: &str,
    format: LiveFormat,
    interval_secs: u64,
) -> Result<(String, String), String> {
    let timestamp = chrono::Local::now()
        .format("%Y-%m-%d_%H-%M-%S")
        .to_string();
    let _ = std::fs::create_dir_all(dir);

    match format {
        LiveFormat::Jpg => {
            let latest = format!("{}/latest.jpg", dir);
            let ts = format!("{}/frame_{}.jpg", dir, timestamp);
            capture_screenshot(Path::new(&ts))?;
            let _ = std::fs::copy(&ts, &latest);
            Ok((latest, ts))
        }
        LiveFormat::Mp4 => {
            let latest = format!("{}/latest.mp4", dir);
            let ts = format!("{}/video_{}.mp4", dir, timestamp);
            record_screen_clip(Path::new(&ts), interval_secs.max(1))?;
            let _ = std::fs::copy(&ts, &latest);
            Ok((latest, ts))
        }
        LiveFormat::Gif => {
            let temp_mp4 = format!("{}/chunk_temp_{}.mp4", dir, timestamp);
            let latest = format!("{}/latest.gif", dir);
            let ts = format!("{}/live_{}.gif", dir, timestamp);
            record_screen_clip(Path::new(&temp_mp4), interval_secs.max(1))?;
            mp4_to_gif(Path::new(&temp_mp4), Path::new(&latest))?;
            let _ = std::fs::copy(&latest, &ts);
            let _ = std::fs::remove_file(&temp_mp4);
            Ok((latest, ts))
        }
    }
}

/// Spawn long-running screen recorder (GUI). Stdin piped so `q` can stop ffmpeg.
/// Returns the child process.
pub fn spawn_screen_recorder(
    out_mp4: &Path,
    fps: u32,
    with_audio: bool,
    crop: Option<(i32, i32, i32, i32)>, // w,h,x,y
) -> Result<Child, String> {
    let out_s = path_str(out_mp4)?;
    let mut cmd = super::ffmpeg::ffmpeg_command()?;
    cmd.arg("-y");

    #[cfg(target_os = "macos")]
    {
        cmd.arg("-f").arg("avfoundation");
        cmd.arg("-r").arg(fps.to_string());
        let device = if with_audio { "1:0" } else { "1:none" };
        cmd.arg("-i").arg(device);
    }

    #[cfg(target_os = "windows")]
    {
        let _ = with_audio; // system audio via dshow is machine-specific; video-only for now
        cmd.arg("-f").arg("gdigrab");
        cmd.arg("-framerate").arg(fps.to_string());
        cmd.arg("-i").arg("desktop");
    }

    #[cfg(target_os = "linux")]
    {
        let _ = with_audio;
        let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0.0".into());
        cmd.arg("-f").arg("x11grab");
        cmd.arg("-framerate").arg(fps.to_string());
        cmd.arg("-video_size").arg(linux_screen_size());
        cmd.arg("-i").arg(display);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (fps, with_audio);
        return Err("screen recording is not supported on this platform".into());
    }

    if let Some((w, h, x, y)) = crop {
        cmd.arg("-vf").arg(format!("crop={}:{}:{}:{}", w, h, x, y));
    }

    cmd.arg("-c:v").arg("libx264");
    cmd.arg("-pix_fmt").arg("yuv420p");
    cmd.arg(out_s);
    cmd.stdin(Stdio::piped());

    cmd.spawn().map_err(|e| {
        format!(
            "could not start ffmpeg recorder ({}): {}",
            super::ffmpeg::ffmpeg_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "unknown path".into()),
            e
        )
    })
}

/// Spawn voice-memo recorder writing AAC/m4a (or wav fallback on non-macOS).
pub fn spawn_voice_memo(out_audio: &Path) -> Result<Child, String> {
    let out_s = path_str(out_audio)?;
    let mut cmd = super::ffmpeg::ffmpeg_command()?;
    cmd.arg("-y");

    #[cfg(target_os = "macos")]
    {
        cmd.args(["-f", "avfoundation", "-i", ":0", "-c:a", "aac", out_s]);
    }

    #[cfg(target_os = "windows")]
    {
        // Device names vary; override with VIBECAP_AUDIO_DEVICE (DirectShow audio= name).
        let device = std::env::var("VIBECAP_AUDIO_DEVICE")
            .unwrap_or_else(|_| "virtual-audio-capturer".into());
        cmd.args([
            "-f",
            "dshow",
            "-i",
            &format!("audio={}", device),
            "-c:a",
            "aac",
            out_s,
        ]);
    }

    #[cfg(target_os = "linux")]
    {
        // Pulse default source
        cmd.args(["-f", "pulse", "-i", "default", "-c:a", "aac", out_s]);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = out_s;
        return Err("voice memo is not supported on this platform".into());
    }

    cmd.stdin(Stdio::piped());
    cmd.spawn().map_err(|e| {
        format!(
            "could not start ffmpeg audio ({}): {}",
            super::ffmpeg::ffmpeg_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "unknown path".into()),
            e
        )
    })
}

#[cfg(target_os = "linux")]
fn linux_screen_size() -> String {
    // Prefer xdpyinfo; fall back to a common default.
    if let Ok(output) = Command::new("xdpyinfo").output() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if let Some(rest) = line.trim().strip_prefix("dimensions:") {
                // e.g. " 1920x1080 pixels (...)"
                let dims = rest.split_whitespace().next().unwrap_or("1920x1080");
                if dims.contains('x') {
                    return dims.to_string();
                }
            }
        }
    }
    std::env::var("VIBECAP_SCREEN_SIZE").unwrap_or_else(|_| "1920x1080".into())
}
