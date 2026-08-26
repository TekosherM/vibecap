//! Capture source selection shared by CLI, MCP, GUI, and (via the same flags)
//! the web studio native capturer.
//!
//! Linux agent backend is **ffmpeg x11grab**. Agents name a `DISPLAY` and/or a
//! focused window; they do not get the web studio demo shutter.

use std::path::{Path, PathBuf};

use super::paths::{media_dir, media_dir_display};

/// Optional overrides for a still or recording.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptureOpts {
    /// X11 display (`:0`, `:1`) or override for `DISPLAY`.
    pub display: Option<String>,
    /// Window title / app name to focus and, on Linux, crop to.
    pub window: Option<String>,
}

impl CaptureOpts {
    pub fn from_parts(display: Option<String>, window: Option<String>) -> Self {
        Self {
            display: empty_to_none(display),
            window: empty_to_none(window),
        }
    }

    pub fn is_default(&self) -> bool {
        self.display.is_none() && self.window.is_none()
    }
}

fn empty_to_none(v: Option<String>) -> Option<String> {
    v.and_then(|s| {
        let t = s.trim().to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    })
}

/// Resolved ffmpeg x11grab / gdigrab / avfoundation input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrabSpec {
    /// Value for ffmpeg `-i` (e.g. `:1.0` or `:1.0+120,80`).
    pub input: String,
    /// `WIDTHxHEIGHT` when the backend needs an explicit size (Linux x11grab).
    pub video_size: Option<String>,
    /// Optional crop (w,h,x,y) already evened for yuv420p.
    pub crop: Option<(i32, i32, i32, i32)>,
    pub display: String,
    pub window: Option<String>,
}

/// Normalize a display string so ffmpeg/xdpyinfo accept it.
///
/// `1` → `:1`, `:0` stays `:0`. Empty falls back to `$DISPLAY` then `:0`.
pub fn normalize_display(raw: Option<&str>) -> String {
    let from_arg = raw
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let from_env = std::env::var("VIBECAP_DISPLAY")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::env::var("DISPLAY")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    let mut d = from_arg.or(from_env).unwrap_or_else(|| ":0".into());
    if !d.starts_with(':') && !d.contains('/') {
        // Bare screen number from agents (`0`, `1`).
        if d.chars().all(|c| c.is_ascii_digit() || c == '.') {
            d = format!(":{d}");
        }
    }
    d
}

/// Default media folder, or a caller-specified directory.
///
/// Resolution order: `override_dir` → `VIBECAP_OUTPUT_DIR` → platform media dir
/// (`{Videos}/Vibecap`, else `~/Movies/Vibecap` on macOS, else `~/Vibecap`).
pub fn resolve_output_dir(override_dir: Option<&Path>) -> PathBuf {
    if let Some(p) = override_dir {
        let t = p.as_os_str().to_string_lossy();
        if !t.trim().is_empty() {
            let dir = p.to_path_buf();
            let _ = std::fs::create_dir_all(&dir);
            return dir;
        }
    }
    if let Ok(env) = std::env::var("VIBECAP_OUTPUT_DIR") {
        let t = env.trim();
        if !t.is_empty() {
            let dir = PathBuf::from(t);
            let _ = std::fs::create_dir_all(&dir);
            return dir;
        }
    }
    media_dir()
}

/// One-line default used in `--help` and docs. Always the resolved media dir,
/// never a hardcoded `~/Movies` vs `~/Vibecap` guess.
pub fn default_output_dir_display() -> String {
    media_dir_display()
}

/// Even dimensions for yuv420p (shared with region crop).
pub fn even_dim(v: i32) -> i32 {
    let v = v.abs().max(2);
    (v / 2) * 2
}

/// Build a grab spec from opts. On Linux this is the x11grab input; elsewhere
/// the display string is informational and window is a focus hint.
pub fn resolve_grab(opts: &CaptureOpts) -> GrabSpec {
    let display = normalize_display(opts.display.as_deref());
    let window = opts
        .window
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    #[cfg(target_os = "linux")]
    {
        if let Some(ref title) = window {
            if let Some(geom) = linux_window_geometry(title) {
                let w = even_dim(geom.w);
                let h = even_dim(geom.h);
                return GrabSpec {
                    input: format!("{}+{},{}", display, geom.x.max(0), geom.y.max(0)),
                    video_size: Some(format!("{w}x{h}")),
                    crop: Some((w, h, geom.x.max(0), geom.y.max(0))),
                    display,
                    window,
                };
            }
        }
        return GrabSpec {
            input: display.clone(),
            video_size: Some(linux_screen_size_for(&display)),
            crop: None,
            display,
            window,
        };
    }

    #[cfg(not(target_os = "linux"))]
    {
        GrabSpec {
            input: display.clone(),
            video_size: None,
            crop: None,
            display,
            window,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowGeom {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Best-effort window geometry (Linux X11). None when tools are missing.
pub fn linux_window_geometry(title: &str) -> Option<WindowGeom> {
    #[cfg(target_os = "linux")]
    {
        if let Some(g) = geometry_via_xdotool(title) {
            return Some(g);
        }
        if let Some(g) = geometry_via_wmctrl(title) {
            return Some(g);
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = title;
        None
    }
}

#[cfg(target_os = "linux")]
fn geometry_via_xdotool(title: &str) -> Option<WindowGeom> {
    let search = Command::new("xdotool")
        .args(["search", "--name", title])
        .output()
        .ok()?;
    if !search.status.success() {
        return None;
    }
    let id = String::from_utf8_lossy(&search.stdout)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())?
        .to_string();
    let geo = Command::new("xdotool")
        .args(["getwindowgeometry", "--shell", &id])
        .output()
        .ok()?;
    if !geo.status.success() {
        return None;
    }
    parse_xdotool_shell(&String::from_utf8_lossy(&geo.stdout))
}

#[cfg(target_os = "linux")]
fn geometry_via_wmctrl(title: &str) -> Option<WindowGeom> {
    let out = Command::new("wmctrl").args(["-lG"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let needle = title.to_ascii_lowercase();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        // 0x04400007  0 10 20 800 600 host Title words…
        let mut parts = line.split_whitespace();
        let _id = parts.next()?;
        let _desk = parts.next()?;
        let x: i32 = parts.next()?.parse().ok()?;
        let y: i32 = parts.next()?.parse().ok()?;
        let w: i32 = parts.next()?.parse().ok()?;
        let h: i32 = parts.next()?.parse().ok()?;
        let _host = parts.next()?;
        let rest = parts.collect::<Vec<_>>().join(" ");
        if rest.to_ascii_lowercase().contains(&needle) {
            return Some(WindowGeom { x, y, w, h });
        }
    }
    None
}

/// Parse `xdotool getwindowgeometry --shell` (`X=`, `Y=`, `WIDTH=`, `HEIGHT=`).
pub fn parse_xdotool_shell(text: &str) -> Option<WindowGeom> {
    let mut x = None;
    let mut y = None;
    let mut w = None;
    let mut h = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("X=") {
            x = v.parse().ok();
        } else if let Some(v) = line.strip_prefix("Y=") {
            y = v.parse().ok();
        } else if let Some(v) = line.strip_prefix("WIDTH=") {
            w = v.parse().ok();
        } else if let Some(v) = line.strip_prefix("HEIGHT=") {
            h = v.parse().ok();
        }
    }
    Some(WindowGeom {
        x: x?,
        y: y?,
        w: w?,
        h: h?,
    })
}

#[cfg(target_os = "linux")]
fn linux_screen_size_for(display: &str) -> String {
    if let Ok(output) = Command::new("xdpyinfo").args(["-display", display]).output() {
        if output.status.success() {
            if let Some(dims) = parse_xdpyinfo_dimensions(&String::from_utf8_lossy(&output.stdout)) {
                return dims;
            }
        }
    }
    if let Ok(output) = Command::new("xdpyinfo").output() {
        if let Some(dims) = parse_xdpyinfo_dimensions(&String::from_utf8_lossy(&output.stdout)) {
            return dims;
        }
    }
    std::env::var("VIBECAP_SCREEN_SIZE").unwrap_or_else(|_| "1920x1080".into())
}

/// Parse `dimensions: 1920x1080 pixels` from xdpyinfo.
pub fn parse_xdpyinfo_dimensions(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("dimensions:") {
            let dims = rest.split_whitespace().next().unwrap_or("");
            if dims.contains('x') && dims.chars().any(|c| c.is_ascii_digit()) {
                return Some(dims.to_string());
            }
        }
    }
    None
}

/// ffmpeg x11grab argv fragment (no binary). Used by tests + web studio docs.
pub fn x11grab_still_args(spec: &GrabSpec, out: &str) -> Vec<String> {
    let mut args = vec![
        "-y".into(),
        "-f".into(),
        "x11grab".into(),
        "-video_size".into(),
        spec.video_size
            .clone()
            .unwrap_or_else(|| "1920x1080".into()),
        "-i".into(),
        spec.input.clone(),
        "-frames:v".into(),
        "1".into(),
        "-q:v".into(),
        "2".into(),
        out.into(),
    ];
    let _ = &mut args;
    args
}

pub fn x11grab_record_args(spec: &GrabSpec, fps: u32, out: &str) -> Vec<String> {
    vec![
        "-y".into(),
        "-f".into(),
        "x11grab".into(),
        "-framerate".into(),
        fps.max(1).to_string(),
        "-video_size".into(),
        spec.video_size
            .clone()
            .unwrap_or_else(|| "1920x1080".into()),
        "-i".into(),
        spec.input.clone(),
        "-c:v".into(),
        "libx264".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        out.into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_display_prefixes_bare_number() {
        assert_eq!(normalize_display(Some("1")), ":1");
        assert_eq!(normalize_display(Some(":0")), ":0");
        assert_eq!(normalize_display(Some("  :1.0  ")), ":1.0");
    }

    #[test]
    fn normalize_display_falls_back_to_env_or_colon_zero() {
        // When DISPLAY is set in this environment we honor it; otherwise :0.
        let got = normalize_display(None);
        assert!(got.starts_with(':') || got.contains('/'), "got {got}");
    }

    #[test]
    fn parse_xdotool_shell_reads_xywh() {
        let g = parse_xdotool_shell("WINDOW=42\nX=12\nY=34\nWIDTH=800\nHEIGHT=600\nSCREEN=0\n")
            .unwrap();
        assert_eq!(
            g,
            WindowGeom {
                x: 12,
                y: 34,
                w: 800,
                h: 600
            }
        );
    }

    #[test]
    fn parse_xdpyinfo_dimensions_line() {
        let text = "screen #0:\n  dimensions:    2560x1440 pixels (677x381 millimeters)\n";
        assert_eq!(
            parse_xdpyinfo_dimensions(text).as_deref(),
            Some("2560x1440")
        );
    }

    #[test]
    fn even_dim_rounds_down_to_even() {
        assert_eq!(even_dim(801), 800);
        assert_eq!(even_dim(1), 2);
        assert_eq!(even_dim(0), 2);
    }

    #[test]
    fn resolve_output_dir_honors_override() {
        let tmp = std::env::temp_dir().join(format!(
            "vibecap_outdir_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        let got = resolve_output_dir(Some(&tmp));
        assert_eq!(got, tmp);
        assert!(tmp.is_dir());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_output_dir_empty_override_uses_default() {
        let got = resolve_output_dir(Some(Path::new("")));
        assert!(got.ends_with("Vibecap") || got == PathBuf::from("Vibecap"));
    }

    #[test]
    fn x11grab_still_args_name_the_display() {
        let spec = GrabSpec {
            input: ":1.0+10,20".into(),
            video_size: Some("1280x720".into()),
            crop: None,
            display: ":1".into(),
            window: Some("Chrome".into()),
        };
        let args = x11grab_still_args(&spec, "/tmp/shot.jpg");
        assert!(args.contains(&"x11grab".into()));
        assert!(args.contains(&":1.0+10,20".into()));
        assert!(args.contains(&"1280x720".into()));
        assert_eq!(args.last().unwrap(), "/tmp/shot.jpg");
        let rec = x11grab_record_args(&spec, 30, "/tmp/v.mp4");
        assert!(rec.contains(&"libx264".into()));
        assert!(rec.contains(&":1.0+10,20".into()));
    }

    #[test]
    fn default_output_dir_display_is_not_empty() {
        let s = default_output_dir_display();
        assert!(s.contains("Vibecap"), "{s}");
    }

    #[test]
    fn capture_opts_trims_empty() {
        let o = CaptureOpts::from_parts(Some("  ".into()), Some("Chrome".into()));
        assert_eq!(o.display, None);
        assert_eq!(o.window.as_deref(), Some("Chrome"));
    }
}
