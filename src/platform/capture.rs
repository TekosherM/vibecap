use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use super::source::{resolve_grab, CaptureOpts};
use super::shell::focus_app;
#[cfg(target_os = "macos")]
use super::shell::list_capture_windows;
#[cfg(target_os = "windows")]
use super::window_rect_on_screen;

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

/// Pixel crop on the virtual desktop (`w,h,x,y` evened for yuv420p).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl ScreenRect {
    pub fn even(x: i32, y: i32, w: i32, h: i32) -> Self {
        even_screen_rect(x, y, w, h)
    }

    pub fn as_whxy(self) -> (i32, i32, i32, i32) {
        (self.w, self.h, self.x, self.y)
    }
}

/// Even rectangle for ffmpeg (yuv420p needs even w/h).
///
/// `x`/`y` are left as-is: gdigrab offsets are virtual-desktop coordinates and
/// may be negative on a monitor left of the primary.
pub fn even_screen_rect(x: i32, y: i32, w: i32, h: i32) -> ScreenRect {
    let w = (w.abs().max(2) / 2) * 2;
    let h = (h.abs().max(2) / 2) * 2;
    ScreenRect {
        x,
        y,
        w: w.max(2),
        h: h.max(2),
    }
}

fn path_str(p: &Path) -> Result<&str, String> {
    p.to_str()
        .ok_or_else(|| "path is not valid UTF-8".to_string())
}

#[cfg(target_os = "macos")]
fn macos_window_id_for(name: &str) -> Option<String> {
    let n = name.trim().to_ascii_lowercase();
    if n.is_empty() {
        return None;
    }
    let wins = list_capture_windows();
    wins.iter()
        .find(|w| w.process.eq_ignore_ascii_case(name) || w.title.eq_ignore_ascii_case(name))
        .or_else(|| {
            wins.iter().find(|w| {
                w.title.to_ascii_lowercase().contains(&n)
                    || w.process.to_ascii_lowercase().contains(&n)
            })
        })
        .filter(|w| !w.is_self())
        .map(|w| w.id.clone())
}

fn run_status(cmd: Command, what: &str) -> Result<(), String> {
    super::ffmpeg::run_ffmpeg(cmd, what)
}

/// Full-screen screenshot to `out` (jpg/png path chosen by caller).
pub fn capture_screenshot(out: &Path) -> Result<(), String> {
    capture_screenshot_opts(out, &CaptureOpts::default())
}

/// Screenshot of the named display / window (Linux x11grab) or full screen.
///
/// On Linux the supported agent backend is **ffmpeg x11grab**. grim / import
/// are last-resort fallbacks only when x11grab cannot start and the caller
/// did not name a display.
pub fn capture_screenshot_opts(out: &Path, opts: &CaptureOpts) -> Result<(), String> {
    // Windows resolves focus itself per-branch (focused → desktop crop,
    // unfocused → HWND grab), so it skips the shared pre-focus.
    #[cfg(target_os = "windows")]
    let focus_ok = windows_focus_ok(opts);
    #[cfg(not(target_os = "windows"))]
    if let Some(app) = opts.window.as_deref() {
        let _ = focus_app(app);
    }
    let spec = resolve_grab(opts);
    let out_s = path_str(out)?;

    #[cfg(target_os = "macos")]
    {
        let _ = spec;
        let mut cmd = Command::new("screencapture");
        // -x silent, -t jpg. Without Screen Recording permission, macOS often
        // still exits 0 but only captures wallpaper / no app windows.
        cmd.args(["-x", "-t", "jpg"]);
        if let Some(name) = opts.window.as_deref() {
            if name.to_ascii_lowercase().contains("vibecap") {
                return Err(
                    "refusing to capture Vibecap itself — pick another window or Full".into(),
                );
            }
            match macos_window_id_for(name) {
                Some(id) => {
                    cmd.arg("-l").arg(id);
                }
                None => {
                    return Err(format!(
                        "could not find a window matching “{name}” — is it open and not minimized?"
                    ));
                }
            }
        }
        cmd.arg(out_s);
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
        let _ = spec;
        if let Some(name) = opts.window.as_deref() {
            if is_self_capture_name(name) {
                return Err(
                    "refusing to capture Vibecap itself — pick another window or Full".into(),
                );
            }
            // Named window. Focused (the common case) → crop the desktop to
            // the window rect: pixel-correct even for GPU-composited apps
            // (Chrome/Electron) that render black via HWND capture.
            // Unfocused (focus refused) → HWND grab: correct for GDI apps even
            // when occluded, though GPU apps may render black. Either way we
            // never silently return a fullscreen shot of the wrong window.
            match window_rect_on_screen(name) {
                Some((hwnd, x, y, w, h)) => {
                    if focus_ok {
                        // Focus verification returns once the target is
                        // foreground; give it a couple of frames to raise +
                        // repaint before the desktop crop reads pixels. (The
                        // old path got this settle for free from PowerShell
                        // helper + ffmpeg spawn latency.)
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        windows_gdigrab_still_cropped(
                            out,
                            ScreenRect::even(x, y, w, h),
                            opts.draw_mouse,
                        )?;
                    } else {
                        windows_gdigrab_window(out_s, hwnd)?;
                    }
                    return validate_capture_file(out);
                }
                None => {
                    return Err(format!(
                        "could not find a window matching “{name}” — is it open and not minimized?"
                    ));
                }
            }
        }
        let region = monitor_rect(opts);
        windows_gdigrab_still(out_s, region, opts.draw_mouse)?;
        return validate_capture_file(out);
    }

    #[cfg(target_os = "linux")]
    {
        match linux_x11grab_still(&spec, out_s) {
            Ok(()) => return validate_capture_file(out),
            Err(e) => {
                // Named display/window: do not silently snap a different output.
                if !opts.is_default() {
                    return Err(e);
                }
                if Command::new("grim")
                    .arg(out_s)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
                {
                    return validate_capture_file(out);
                }
                if Command::new("import")
                    .args(["-window", "root", out_s])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
                {
                    return validate_capture_file(out);
                }
                return Err(e);
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (out_s, spec);
        Err("screenshot is not supported on this platform".into())
    }
}

#[cfg(target_os = "linux")]
fn linux_x11grab_still(spec: &GrabSpec, out_s: &str) -> Result<(), String> {
    let mut cmd = super::ffmpeg::ffmpeg_command()?;
    cmd.args([
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "x11grab",
        "-video_size",
        spec.video_size.as_deref().unwrap_or("1920x1080"),
        "-i",
        &spec.input,
        "-frames:v",
        "1",
        "-q:v",
        "2",
        "-update",
        "1",
        out_s,
    ]);
    run_status(cmd, "ffmpeg x11grab screenshot")
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
pub fn validate_capture_file(out: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(out).map_err(|e| format!("capture file missing: {e}"))?;
    let len = meta.len();
    // A real fullscreen JPG is usually >> 50 KB; tiny files mean the grabber
    // wrote an empty frame.
    if len < 8_000 {
        let _ = std::fs::remove_file(out);
        #[cfg(target_os = "macos")]
        return Err(format!(
            "Capture looks empty ({len} bytes). macOS Screen Recording is not granted to this Vibecap.\n\
             Fix: System Settings → Privacy & Security → Screen Recording → enable only **Vibecap** (the app).\n\
             Remove extra entries (old cargo/terminal copies). Then fully quit Vibecap (tray Quit) and reopen from /Applications."
        ));
        #[cfg(target_os = "windows")]
        return Err(format!(
            "Capture looks empty ({len} bytes). ffmpeg gdigrab wrote an empty frame — \
             retry the capture; if it persists, check that the target window is open, \
             not minimized, and on a connected display."
        ));
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        return Err(format!(
            "Capture looks empty ({len} bytes). The grabber wrote an empty frame — retry the capture."
        ));
    }
    Ok(())
}

fn is_self_capture_name(name: &str) -> bool {
    name.to_ascii_lowercase().contains("vibecap")
}

#[cfg(target_os = "windows")]
fn monitor_rect(opts: &CaptureOpts) -> Option<ScreenRect> {
    let idx = opts.monitor?;
    super::shell::list_monitors()
        .into_iter()
        .find(|m| m.index == idx)
        .map(|m| ScreenRect::even(m.x, m.y, m.w, m.h))
}

#[cfg(target_os = "windows")]
fn windows_gdigrab_still(
    out_s: &str,
    region: Option<ScreenRect>,
    draw_mouse: bool,
) -> Result<(), String> {
    // GDI BitBlt reads the same composited desktop as gdigrab without the
    // ~300–500 ms process spawn; fall back to ffmpeg on any failure.
    if super::win32::native_desktop_still(Path::new(out_s), region, draw_mouse).is_ok() {
        return Ok(());
    }
    let mut cmd = super::ffmpeg::ffmpeg_command()?;
    cmd.args([
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "gdigrab",
        "-draw_mouse",
        if draw_mouse { "1" } else { "0" },
        // High grab rate so the first frame arrives immediately (fps=1 waits ~1s).
        "-framerate",
        "30",
        "-probesize",
        "32",
        "-analyzeduration",
        "0",
    ]);
    if let Some(r) = region {
        cmd.arg("-offset_x").arg(r.x.to_string());
        cmd.arg("-offset_y").arg(r.y.to_string());
        cmd.arg("-video_size").arg(format!("{}x{}", r.w, r.h));
    }
    cmd.args([
        "-i",
        "desktop",
        "-frames:v",
        "1",
        "-q:v",
        "3",
        "-f",
        "image2",
        "-update",
        "1",
        out_s,
    ]);
    run_status(cmd, "ffmpeg gdigrab screenshot")
}

/// Window still by HWND: captures the window itself even when occluded, so no
/// focus race. `hwnd` is the decimal window handle from `window_rect_on_screen`.
///
/// NOTE: GPU-composited windows (Chrome, Edge, Electron apps) render BLACK
/// through this path — they must be frontmost and cropped from the desktop
/// instead (see `capture_screenshot_opts`). This stays as the fallback for
/// windows that refuse focus, where it is correct for GDI apps.
#[cfg(target_os = "windows")]
fn windows_gdigrab_window(out_s: &str, hwnd: u64) -> Result<(), String> {
    let mut cmd = super::ffmpeg::ffmpeg_command()?;
    cmd.args([
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "gdigrab",
        "-draw_mouse",
        "0",
        "-framerate",
        "30",
        "-probesize",
        "32",
        "-analyzeduration",
        "0",
        "-i",
        &format!("hwnd=0x{hwnd:X}"),
        "-frames:v",
        "1",
        "-q:v",
        "3",
        "-f",
        "image2",
        "-update",
        "1",
        out_s,
    ]);
    run_status(cmd, "ffmpeg gdigrab window screenshot")
}

/// Best-effort focus for a window capture. `true` when there is no window to
/// focus or the window verified frontmost; the grab strategy branches on this
/// (focused → desktop crop, unfocused → HWND grab).
#[cfg(target_os = "windows")]
fn windows_focus_ok(opts: &CaptureOpts) -> bool {
    match opts.window.as_deref() {
        Some(app) => focus_app(app).is_ok(),
        None => true,
    }
}

/// Focused-window still: gdigrab desktop offsets (GPU-safe). Falls back to a
/// full desktop grab + CPU crop if the offset grab writes an empty frame.
#[cfg(target_os = "windows")]
fn windows_gdigrab_still_cropped(
    out: &Path,
    rect: ScreenRect,
    draw_mouse: bool,
) -> Result<(), String> {
    let out_s = path_str(out)?;
    if windows_gdigrab_still(out_s, Some(rect), draw_mouse).is_ok()
        && validate_capture_file(out).is_ok()
    {
        return Ok(());
    }
    let tmp = out.with_extension("full.tmp.jpg");
    let tmp_s = tmp
        .to_str()
        .ok_or_else(|| "path is not valid UTF-8".to_string())?
        .to_string();
    windows_gdigrab_still(&tmp_s, None, draw_mouse)?;
    let result = crop_image_file(&tmp, out, rect);
    let _ = std::fs::remove_file(&tmp);
    result?;
    validate_capture_file(out)
}

/// Still of a pixel rectangle on the virtual desktop (Windows gdigrab offsets;
/// elsewhere: full still + CPU crop).
///
/// Public capture API for headless/embedding call sites (the GUI crops from
/// its hidden-window snap instead, so its own window is never in the shot).
#[allow(dead_code)]
pub fn capture_screenshot_region(out: &Path, region: ScreenRect) -> Result<(), String> {
    let region = ScreenRect::even(region.x, region.y, region.w, region.h);

    #[cfg(target_os = "windows")]
    {
        windows_gdigrab_still(path_str(out)?, Some(region), false)?;
        return validate_capture_file(out);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let tmp = out.with_extension("full.tmp.jpg");
        capture_screenshot(&tmp)?;
        let result = crop_image_file(&tmp, out, region);
        let _ = std::fs::remove_file(&tmp);
        result?;
        validate_capture_file(out)
    }
}

/// Crop `src` to `region` (clamped to image bounds) and write `dest`.
pub fn crop_image_file(src: &Path, dest: &Path, region: ScreenRect) -> Result<(), String> {
    let img = image::open(src).map_err(|e| format!("could not open capture: {e}"))?;
    let (iw, ih) = (img.width(), img.height());
    if iw == 0 || ih == 0 {
        return Err("capture image is empty".into());
    }
    let x = (region.x.max(0) as u32).min(iw.saturating_sub(1));
    let y = (region.y.max(0) as u32).min(ih.saturating_sub(1));
    let w = (region.w.max(2) as u32).min(iw.saturating_sub(x)).max(2);
    let h = (region.h.max(2) as u32).min(ih.saturating_sub(y)).max(2);
    let w = w.min(iw.saturating_sub(x)).max(1);
    let h = h.min(ih.saturating_sub(y)).max(1);
    img.crop_imm(x, y, w, h)
        .save(dest)
        .map_err(|e| format!("could not save cropped capture: {e}"))?;
    Ok(())
}

/// Record a fixed-duration screen clip to `out_mp4`.
pub fn record_screen_clip(out_mp4: &Path, duration_secs: u64) -> Result<(), String> {
    record_screen_clip_opts(out_mp4, duration_secs, &CaptureOpts::default())
}

/// Fixed-duration record of the named display / window.
pub fn record_screen_clip_opts(
    out_mp4: &Path,
    duration_secs: u64,
    opts: &CaptureOpts,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let focus_ok = windows_focus_ok(opts);
    #[cfg(not(target_os = "windows"))]
    if let Some(app) = opts.window.as_deref() {
        let _ = focus_app(app);
    }
    let spec = resolve_grab(opts);
    let out_s = path_str(out_mp4)?;
    let dur = duration_secs.max(1).to_string();

    #[cfg(target_os = "macos")]
    {
        let _ = spec;
        let mut cmd = Command::new("screencapture");
        cmd.args(["-v", "-V", &dur, out_s]);
        return run_status(cmd, "screencapture -v");
    }

    #[cfg(target_os = "windows")]
    {
        let _ = spec;
        // Named window: focused → desktop region (GPU-safe); unfocused →
        // HWND input (occlusion-proof; GPU apps may render black). Unnamed →
        // desktop; a named-but-missing window is an error, never fullscreen.
        let (offsets, input) = match opts.window.as_deref() {
            Some(name) => match window_rect_on_screen(name) {
                Some((hwnd, x, y, w, h)) => {
                    if focus_ok {
                        let r = ScreenRect::even(x, y, w, h);
                        (Some(r), "desktop".to_string())
                    } else {
                        (None, format!("hwnd=0x{hwnd:X}"))
                    }
                }
                None => {
                    return Err(format!(
                        "could not find a window matching “{name}” — is it open and not minimized?"
                    ));
                }
            },
            None => (None, "desktop".to_string()),
        };
        let mut cmd = super::ffmpeg::ffmpeg_command()?;
        cmd.args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "gdigrab",
            "-draw_mouse",
            "1",
            "-framerate",
            "30",
        ]);
        if let Some(r) = offsets {
            cmd.arg("-offset_x").arg(r.x.to_string());
            cmd.arg("-offset_y").arg(r.y.to_string());
            cmd.arg("-video_size").arg(format!("{}x{}", r.w, r.h));
        }
        cmd.args([
            "-t", &dur, "-i", &input, "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt",
            "yuv420p", out_s,
        ]);
        return run_status(cmd, "ffmpeg gdigrab record");
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = super::ffmpeg::ffmpeg_command()?;
        cmd.args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "x11grab",
            "-framerate",
            "30",
            "-video_size",
            spec.video_size.as_deref().unwrap_or("1920x1080"),
            "-t",
            &dur,
            "-i",
            &spec.input,
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-pix_fmt",
            "yuv420p",
            out_s,
        ]);
        return run_status(cmd, "ffmpeg x11grab record");
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (out_s, dur, spec);
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
    export_gif_clip_ex(video_path, start_time, end_time, gif_out, 15, 800)
}

pub fn export_gif_clip_ex(
    video_path: &str,
    start_time: &str,
    end_time: &str,
    gif_out: &str,
    fps: u32,
    width: u32,
) -> Result<(), String> {
    let fps = fps.clamp(4, 30);
    let width = width.clamp(160, 1920);
    let vf = format!("fps={fps},scale={width}:-1:flags=lanczos");
    let mut cmd = super::ffmpeg::ffmpeg_command()?;
    cmd.args([
        "-ss",
        start_time,
        "-to",
        end_time,
        "-i",
        video_path,
        "-vf",
        &vf,
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

/// Remux a (possibly killed) fragmented MP4 into a regular MP4 with a `moov`
/// atom, stream-copied (no re-encode). Used after the agent recorder is
/// killed from another process. On success the destination replaces the
/// source atomically; the source is kept untouched on failure.
pub fn remux_to_clean_mp4(src: &Path, dest: &Path) -> Result<(), String> {
    let tmp = dest.with_extension("remux.tmp.mp4");
    let mut cmd = super::ffmpeg::ffmpeg_command()?;
    cmd.args([
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-i",
        path_str(src)?,
        "-c",
        "copy",
        "-movflags",
        "faststart",
    ]);
    cmd.arg(path_str(&tmp)?);
    let status = run_status(cmd, "ffmpeg remux");
    if status.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return status;
    }
    // Refuse to replace a real recording with an empty remux.
    let len = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    if len < 512 {
        let _ = std::fs::remove_file(&tmp);
        return Err("remux produced an empty file".into());
    }
    std::fs::rename(&tmp, dest).map_err(|e| format!("could not replace recording: {e}"))?;
    let log = src.with_extension("ffmpeg.log");
    let _ = std::fs::remove_file(log);
    Ok(())
}

/// Headless still into `dir`. Creates the directory. Returns the JPEG path.
pub fn capture_to_dir(dir: &Path, opts: &CaptureOpts) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("could not create output dir: {e}"))?;
    let out = dir.join(format!(
        "screenshot_{}.jpg",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
    ));
    capture_screenshot_opts(&out, opts)?;
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
///
/// Simple wrapper without source overrides; prefer [`spawn_screen_recorder_opts`].
#[allow(dead_code)]
pub fn spawn_screen_recorder(
    out_mp4: &Path,
    fps: u32,
    with_audio: bool,
    crop: Option<(i32, i32, i32, i32)>, // w,h,x,y
) -> Result<Child, String> {
    spawn_screen_recorder_opts(out_mp4, fps, with_audio, crop, &CaptureOpts::default(), false)
}

/// Unbounded recorder with optional display / window (Linux x11grab).
///
/// `frag_mp4` writes a fragmented MP4 (`frag_keyframe+empty_moov`) that stays
/// readable when the recorder is *killed* instead of stopped cleanly — the
/// agent record path is stopped from a different process (`taskkill` /
/// SIGKILL), which can never send ffmpeg's graceful `q`. Pair with
/// `remux_to_clean_mp4` on stop to restore a regular fast-start file.
pub fn spawn_screen_recorder_opts(
    out_mp4: &Path,
    fps: u32,
    with_audio: bool,
    crop: Option<(i32, i32, i32, i32)>,
    opts: &CaptureOpts,
    frag_mp4: bool,
) -> Result<Child, String> {
    #[cfg(target_os = "windows")]
    let focus_ok = windows_focus_ok(opts);
    #[cfg(not(target_os = "windows"))]
    if let Some(app) = opts.window.as_deref() {
        let _ = focus_app(app);
    }
    let spec = resolve_grab(opts);
    let out_s = path_str(out_mp4)?;
    let mut cmd = super::ffmpeg::ffmpeg_command()?;
    cmd.arg("-y");
    cmd.arg("-hide_banner");
    cmd.arg("-loglevel").arg("error");

    #[cfg(target_os = "macos")]
    {
        let _ = &spec;
        cmd.arg("-f").arg("avfoundation");
        cmd.arg("-r").arg(fps.to_string());
        let device = if with_audio { "1:0" } else { "1:none" };
        cmd.arg("-i").arg(device);
    }

    #[cfg(target_os = "windows")]
    {
        let _ = with_audio; // system audio via dshow is machine-specific; video-only for now
        let _ = &spec;
        cmd.arg("-f").arg("gdigrab");
        cmd.arg("-draw_mouse").arg("1");
        cmd.arg("-framerate").arg(fps.to_string());
        // Explicit pixel crop wins (region recordings via desktop offsets).
        // Otherwise a named window records focused → desktop region
        // (GPU-safe) or unfocused → HWND (occlusion-proof; GPU apps may
        // render black). A named-but-missing window is an error, never a
        // silent fullscreen recording (the "wrong window" bug).
        // NOTE: gdigrab offsets must precede `-i desktop`; the hwnd form takes
        // no offsets — out-of-bounds crop= on Windows crashes ffmpeg, so the
        // branches below are mutually exclusive by construction.
        if let Some(name) = opts.window.as_deref() {
            if is_self_capture_name(name) {
                return Err(
                    "refusing to capture Vibecap itself — pick another window or Full".into(),
                );
            }
        }
        let (offsets, input) = match crop.map(|(w, h, x, y)| ScreenRect::even(x, y, w, h)) {
            Some(r) => (Some(r), "desktop".to_string()),
            None => match opts.window.as_deref() {
                Some(name) => match window_rect_on_screen(name) {
                    Some((hwnd, x, y, w, h)) => {
                        if focus_ok {
                            (Some(ScreenRect::even(x, y, w, h)), "desktop".to_string())
                        } else {
                            (None, format!("hwnd=0x{hwnd:X}"))
                        }
                    }
                    None => {
                        return Err(format!(
                            "could not find a window matching “{name}” — is it open and not minimized?"
                        ));
                    }
                },
                None => (monitor_rect(opts), "desktop".to_string()),
            },
        };
        if let Some(r) = offsets {
            cmd.arg("-offset_x").arg(r.x.to_string());
            cmd.arg("-offset_y").arg(r.y.to_string());
            cmd.arg("-video_size").arg(format!("{}x{}", r.w, r.h));
        }
        cmd.arg("-i").arg(&input);
    }

    #[cfg(target_os = "linux")]
    {
        let _ = with_audio;
        cmd.arg("-f").arg("x11grab");
        cmd.arg("-framerate").arg(fps.to_string());
        cmd.arg("-video_size")
            .arg(spec.video_size.as_deref().unwrap_or("1920x1080"));
        cmd.arg("-i").arg(&spec.input);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (fps, with_audio, spec);
        return Err("screen recording is not supported on this platform".into());
    }

    // Prefer explicit region crop; otherwise a Linux window grab already sized the input.
    // Windows already applied offset_x/y/video_size above — a second crop= would
    // either no-op or fail when the rect is outside the (now smaller) frame.
    let crop = if cfg!(target_os = "windows") {
        None
    } else {
        crop.or(if spec.crop.is_some() && cfg!(not(target_os = "linux")) {
            spec.crop
        } else {
            None
        })
    };
    if let Some((w, h, x, y)) = crop {
        cmd.arg("-vf").arg(format!("crop={}:{}:{}:{}", w, h, x, y));
    }

    cmd.arg("-c:v").arg("libx264");
    // Real-time screen capture: the default "medium" preset saturates a core on
    // laptops and drops gdigrab frames — veryfast keeps up at 30-60fps.
    cmd.arg("-preset").arg("veryfast");
    cmd.arg("-pix_fmt").arg("yuv420p");
    if frag_mp4 {
        // Kill-safe fragments; remuxed to a regular MP4 on stop.
        cmd.arg("-movflags").arg("frag_keyframe+empty_moov");
    }
    cmd.arg(out_s);
    cmd.stdin(Stdio::piped());

    // Detach from the caller's terminal:
    // - stdout/stderr → sibling .ffmpeg.log so an agent's piped shell does not
    //   block on the recorder and ffmpeg chatter stays out of agent output.
    // - own process group (unix) so a harness killing the shell's process
    //   group cannot take the recorder down with it.
    let log_path = out_mp4.with_extension("ffmpeg.log");
    match std::fs::File::create(&log_path) {
        Ok(log) => {
            let log_err = log
                .try_clone()
                .map_err(|e| format!("could not clone recorder log handle: {e}"))?;
            cmd.stdout(Stdio::from(log));
            cmd.stderr(Stdio::from(log_err));
        }
        Err(_) => {
            cmd.stdout(Stdio::null());
            cmd.stderr(Stdio::null());
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

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
        let device = std::env::var("VIBECAP_AUDIO_DEVICE").ok().filter(|s| !s.trim().is_empty())
            .or_else(|| {
                super::ffmpeg::list_audio_input_devices().into_iter().find(|n| {
                    let l = n.to_ascii_lowercase();
                    l.contains("microphone") || l.contains("mic")
                })
            })
            .or_else(|| super::ffmpeg::list_audio_input_devices().into_iter().next())
            .unwrap_or_else(|| "virtual-audio-capturer".into());
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
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_screen_rect_forces_even_dimensions() {
        let r = even_screen_rect(-4, 3, 801, 601);
        assert_eq!((r.x, r.y, r.w, r.h), (-4, 3, 800, 600));
        assert_eq!(r.as_whxy(), (800, 600, -4, 3));
        let tiny = even_screen_rect(0, 0, 1, 1);
        assert_eq!((tiny.w, tiny.h), (2, 2));
    }

    #[test]
    fn crop_image_file_crops_and_clamps() {
        let dir = std::env::temp_dir().join(format!("vibecap_crop_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("src.png");
        let dest = dir.join("dest.png");
        // 100x80 red image.
        let img = image::RgbImage::from_pixel(100, 80, image::Rgb([200, 10, 10]));
        image::DynamicImage::ImageRgb8(img).save(&src).unwrap();
        crop_image_file(&src, &dest, ScreenRect { x: 10, y: 10, w: 50, h: 40 }).unwrap();
        assert_eq!(image::image_dimensions(&dest).unwrap(), (50, 40));
        // Out-of-bounds region clamps instead of failing.
        crop_image_file(
            &src,
            &dest,
            ScreenRect { x: 90, y: 70, w: 500, h: 500 },
        )
        .unwrap();
        let (w, h) = image::image_dimensions(&dest).unwrap();
        assert!(w <= 100 && h <= 80 && w >= 1 && h >= 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

