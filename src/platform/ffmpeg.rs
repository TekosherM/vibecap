//! Locate the `ffmpeg` binary for GUI + CLI processes.
//!
//! macOS `.app` launches (Finder / Spotlight / Dock) get a stripped `PATH`
//! (`/usr/bin:/bin:/usr/sbin:/sbin`) that **does not** include Homebrew
//! (`/usr/local/bin` or `/opt/homebrew/bin`). Bare `Command::new("ffmpeg")`
//! then fails with "No such file or directory" even when `brew install ffmpeg`
//! succeeded.
//!
//! Override: set env `VIBECAP_FFMPEG` to an absolute path.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static FFMPEG: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Absolute path to a runnable `ffmpeg`, if found.
pub fn ffmpeg_path() -> Option<&'static Path> {
    FFMPEG.get_or_init(discover).as_deref()
}

/// Whether ffmpeg can be started (for status strip / diagnostics).
pub fn ffmpeg_available() -> bool {
    ffmpeg_path().is_some()
}

/// Build a `Command` for the resolved binary.
pub fn ffmpeg_command() -> Result<Command, String> {
    match ffmpeg_path() {
        Some(p) => Ok(Command::new(p)),
        None => Err(ffmpeg_missing_message()),
    }
}

pub fn ffmpeg_missing_message() -> String {
    "ffmpeg not found. Linux agent capture uses ffmpeg x11grab — install with \
     `sudo apt install ffmpeg` (or `brew install ffmpeg` on macOS). Finder/Dock launches \
     do not see Homebrew on PATH; set VIBECAP_FFMPEG to the full binary path if needed."
        .into()
}

static FFPROBE: OnceLock<Option<PathBuf>> = OnceLock::new();

fn ffprobe_path() -> Option<&'static Path> {
    FFPROBE
        .get_or_init(|| {
            // Homebrew installs ffprobe next to ffmpeg.
            if let Some(fp) = ffmpeg_path() {
                if let Some(parent) = fp.parent() {
                    let sibling = parent.join("ffprobe");
                    if sibling.is_file() {
                        return Some(sibling);
                    }
                }
            }
            for c in [
                "/opt/homebrew/bin/ffprobe",
                "/usr/local/bin/ffprobe",
                "/usr/bin/ffprobe",
            ] {
                let p = PathBuf::from(c);
                if p.is_file() {
                    return Some(p);
                }
            }
            which_on_path("ffprobe")
        })
        .as_deref()
}

/// Media duration in seconds (ffprobe first, `ffmpeg -i` Duration parse as fallback).
pub fn probe_duration(file: &Path) -> Option<f64> {
    if let Some(fp) = ffprobe_path() {
        if let Ok(out) = Command::new(fp)
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
            ])
            .arg(file)
            .output()
        {
            if out.status.success() {
                if let Ok(s) = std::str::from_utf8(&out.stdout) {
                    if let Ok(d) = s.trim().parse::<f64>() {
                        if d.is_finite() && d > 0.0 {
                            return Some(d);
                        }
                    }
                }
            }
        }
    }
    if let Ok(mut cmd) = ffmpeg_command() {
        cmd.arg("-i").arg(file);
        if let Ok(out) = cmd.output() {
            let text = String::from_utf8_lossy(&out.stderr);
            if let Some(idx) = text.find("Duration: ") {
                let rest = &text[idx + "Duration: ".len()..];
                let t = rest.split([',', '\n', ' ']).next().unwrap_or_default();
                return parse_timecode(t.trim());
            }
        }
    }
    None
}

/// Parse `HH:MM:SS(.ff)` / `MM:SS` into seconds.
pub fn parse_timecode(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    let mut secs = 0.0_f64;
    for part in s.split(':') {
        secs = secs * 60.0 + part.trim().parse::<f64>().ok()?;
    }
    if secs.is_finite() && secs >= 0.0 {
        Some(secs)
    } else {
        None
    }
}

/// Format seconds as `HH:MM:SS`.
pub fn format_timecode(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

fn discover() -> Option<PathBuf> {
    // 1) Explicit override
    if let Ok(raw) = std::env::var("VIBECAP_FFMPEG") {
        let p = PathBuf::from(raw.trim());
        if is_runnable_ffmpeg(&p) {
            return Some(p);
        }
    }

    // 2) Current process PATH (`which` / path search)
    if let Some(p) = which_on_path("ffmpeg") {
        if is_runnable_ffmpeg(&p) {
            return Some(p);
        }
    }

    // 3) Well-known install locations (GUI-safe)
    for candidate in known_locations() {
        if is_runnable_ffmpeg(&candidate) {
            return Some(candidate);
        }
    }

    // 4) PATH with Homebrew prefixes injected (covers odd layouts)
    if let Some(p) = which_with_extra_path("ffmpeg") {
        if is_runnable_ffmpeg(&p) {
            return Some(p);
        }
    }

    None
}

fn known_locations() -> Vec<PathBuf> {
    let mut out = vec![
        PathBuf::from("/opt/homebrew/bin/ffmpeg"), // Apple Silicon Homebrew
        PathBuf::from("/usr/local/bin/ffmpeg"),    // Intel Homebrew / manual
        PathBuf::from("/usr/bin/ffmpeg"),
        PathBuf::from("/bin/ffmpeg"),
    ];
    // User-local Homebrew or custom prefixes
    if let Ok(home) = std::env::var("HOME") {
        out.push(PathBuf::from(format!("{home}/homebrew/bin/ffmpeg")));
        out.push(PathBuf::from(format!("{home}/.linuxbrew/bin/ffmpeg")));
        out.push(PathBuf::from(format!("{home}/bin/ffmpeg")));
    }
    // Windows-ish (if ever launched with unix-style helpers)
    #[cfg(target_os = "windows")]
    {
        out.push(PathBuf::from(r"C:\ffmpeg\bin\ffmpeg.exe"));
        out.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin\ffmpeg.exe"));
    }
    out
}

fn is_runnable_ffmpeg(p: &Path) -> bool {
    if !p.is_file() {
        return false;
    }
    // Quick probe — avoids picking a stale symlink.
    Command::new(p)
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn which_on_path(name: &str) -> Option<PathBuf> {
    // Prefer `which` when available; also walk PATH manually.
    if let Ok(out) = Command::new("which").arg(name).output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    walk_path_env(name, None)
}

fn which_with_extra_path(name: &str) -> Option<PathBuf> {
    let mut extras = vec![
        "/opt/homebrew/bin".into(),
        "/usr/local/bin".into(),
        "/usr/local/sbin".into(),
    ];
    if let Ok(home) = std::env::var("HOME") {
        extras.push(format!("{home}/homebrew/bin"));
        extras.push(format!("{home}/.linuxbrew/bin"));
        extras.push(format!("{home}/bin"));
        extras.push(format!("{home}/.cargo/bin"));
    }
    let base = std::env::var("PATH").unwrap_or_default();
    let joined = {
        let mut parts = extras;
        if !base.is_empty() {
            parts.push(base);
        }
        parts.join(":")
    };
    walk_path_env(name, Some(&joined))
}

fn walk_path_env(name: &str, path_override: Option<&str>) -> Option<PathBuf> {
    let path = path_override
        .map(|s| s.to_string())
        .or_else(|| std::env::var("PATH").ok())?;
    for dir in path.split(':') {
        if dir.is_empty() {
            continue;
        }
        let candidate = PathBuf::from(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(target_os = "windows")]
        {
            let exe = PathBuf::from(dir).join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_message_mentions_install_and_env() {
        let m = ffmpeg_missing_message();
        assert!(m.contains("ffmpeg"));
        assert!(m.contains("VIBECAP_FFMPEG"));
        assert!(m.contains("x11grab") || m.contains("brew") || m.contains("apt"));
    }

    #[test]
    fn timecode_parse_and_format() {
        assert_eq!(parse_timecode("00:00:05"), Some(5.0));
        assert_eq!(parse_timecode("00:01:30"), Some(90.0));
        assert_eq!(parse_timecode("01:00:00.5"), Some(3600.5));
        assert_eq!(parse_timecode(""), None);
        assert_eq!(parse_timecode("xx"), None);
        assert_eq!(format_timecode(90.0), "00:01:30");
        assert_eq!(format_timecode(0.4), "00:00:00");
    }
}
