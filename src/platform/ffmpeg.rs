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
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

/// `None` = not probed yet · `Some(inner)` = discovery ran (inner may be None).
/// A Mutex (not OnceLock) so the Settings/Capture "Re-check" button can re-run
/// discovery after the user installs ffmpeg mid-session.
static FFMPEG: Mutex<Option<Option<PathBuf>>> = Mutex::new(None);

/// Absolute path to a runnable `ffmpeg`, if found.
pub fn ffmpeg_path() -> Option<PathBuf> {
    let mut g = FFMPEG.lock().unwrap();
    g.get_or_insert_with(discover).clone()
}

/// Whether ffmpeg can be started (for status strip / diagnostics).
/// Cheap hot path: no PathBuf clone, just the cached probe result.
pub fn ffmpeg_available() -> bool {
    let mut g = FFMPEG.lock().unwrap();
    g.get_or_insert_with(discover).is_some()
}

/// Re-run discovery (e.g. after the user installs ffmpeg while Vibecap is open).
/// Returns the new availability.
pub fn ffmpeg_recheck() -> bool {
    let mut g = FFMPEG.lock().unwrap();
    *g = Some(discover());
    g.as_ref().unwrap().is_some()
}

/// Hide the extra console window ffmpeg opens on Windows GUI launches.
pub(crate) fn silence_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}

/// Build a `Command` for the resolved binary.
pub fn ffmpeg_command() -> Result<Command, String> {
    match ffmpeg_path() {
        Some(p) => {
            let mut cmd = Command::new(p);
            silence_console(&mut cmd);
            Ok(cmd)
        }
        None => Err(ffmpeg_missing_message()),
    }
}

/// Run ffmpeg without inheriting the GUI's null stdio (release Windows subsystem).
pub fn run_ffmpeg(mut cmd: Command, what: &str) -> Result<(), String> {
    silence_console(&mut cmd);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::piped());
    let output = cmd
        .output()
        .map_err(|e| format!("could not start {what}: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        let err = err.trim();
        if err.is_empty() {
            Err(format!("{what} failed (exit {:?})", output.status.code()))
        } else {
            Err(format!(
                "{what} failed (exit {:?}): {err}",
                output.status.code()
            ))
        }
    }
}

/// Last bytes of a sibling `.ffmpeg.log` (agent remux / record failures).
pub fn ffmpeg_log_tail(log_path: &Path, max_bytes: usize) -> Option<String> {
    let data = std::fs::read(log_path).ok()?;
    if data.is_empty() {
        return None;
    }
    let start = data.len().saturating_sub(max_bytes);
    let slice = &data[start..];
    let s = String::from_utf8_lossy(slice).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// DirectShow audio capture device names (Windows). Empty elsewhere.
pub fn list_audio_input_devices() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let Ok(mut cmd) = ffmpeg_command() else {
            return Vec::new();
        };
        silence_console(&mut cmd);
        cmd.args(["-hide_banner", "-list_devices", "true", "-f", "dshow", "-i", "dummy"]);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        let Ok(out) = cmd.output() else {
            return Vec::new();
        };
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        parse_dshow_audio_devices(&text)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

#[cfg(any(target_os = "windows", test))]
pub fn parse_dshow_audio_devices(ffmpeg_list: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_audio = false;
    for line in ffmpeg_list.lines() {
        let l = line.trim();
        if l.contains("DirectShow audio devices") {
            in_audio = true;
            continue;
        }
        if l.contains("DirectShow video devices") {
            in_audio = false;
            continue;
        }
        if in_audio {
            if let Some(start) = l.find('"') {
                if let Some(end) = l[start + 1..].find('"') {
                    let name = &l[start + 1..start + 1 + end];
                    if !name.is_empty() {
                        names.push(name.to_string());
                    }
                }
            }
        }
    }
    names
}

pub fn ffmpeg_missing_message() -> String {
    #[cfg(target_os = "windows")]
    {
        "ffmpeg not found. Windows capture uses ffmpeg gdigrab — install with \
         `winget install Gyan.FFmpeg` (or scoop/choco). Set VIBECAP_FFMPEG to the full \
         binary path if it lives somewhere unusual."
            .into()
    }
    #[cfg(not(target_os = "windows"))]
    {
        "ffmpeg not found. Linux agent capture uses ffmpeg x11grab — install with \
         `sudo apt install ffmpeg` (or `brew install ffmpeg` on macOS). Finder/Dock launches \
         do not see Homebrew on PATH; set VIBECAP_FFMPEG to the full binary path if needed."
            .into()
    }
}

static FFPROBE: OnceLock<Option<PathBuf>> = OnceLock::new();

fn ffprobe_path() -> Option<&'static Path> {
    FFPROBE
        .get_or_init(|| {
            // Homebrew installs ffprobe next to ffmpeg (ffmpeg.exe on Windows).
            if let Some(fp) = ffmpeg_path() {
                if let Some(parent) = fp.parent() {
                    let sibling = parent.join("ffprobe");
                    if sibling.is_file() {
                        return Some(sibling);
                    }
                    #[cfg(target_os = "windows")]
                    {
                        let sibling_exe = parent.join("ffprobe.exe");
                        if sibling_exe.is_file() {
                            return Some(sibling_exe);
                        }
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
        let mut probe = Command::new(fp);
        silence_console(&mut probe);
        if let Ok(out) = probe
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
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            // winget symlinks installed CLIs here (e.g. Gyan.FFmpeg).
            out.push(PathBuf::from(&local).join(r"Microsoft\WinGet\Links\ffmpeg.exe"));
        }
        if let Ok(profile) = std::env::var("USERPROFILE") {
            out.push(PathBuf::from(&profile).join(r"scoop\shims\ffmpeg.exe"));
        }
    }
    out
}

fn is_runnable_ffmpeg(p: &Path) -> bool {
    if !p.is_file() {
        return false;
    }
    // Quick probe — avoids picking a stale symlink.
    let mut cmd = Command::new(p);
    silence_console(&mut cmd);
    cmd.arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn which_on_path(name: &str) -> Option<PathBuf> {
    // Prefer the platform resolver when available; also walk PATH manually.
    #[cfg(target_os = "windows")]
    {
        let mut where_cmd = Command::new("where");
        silence_console(&mut where_cmd);
        if let Ok(out) = where_cmd.arg(name).output() {
            if out.status.success() {
                let first = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if !first.is_empty() {
                    return Some(PathBuf::from(first));
                }
            }
        }
        return walk_path_env(name, None);
    }
    #[cfg(not(target_os = "windows"))]
    {
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
}

fn which_with_extra_path(name: &str) -> Option<PathBuf> {
    let mut extras: Vec<PathBuf> = vec![
        "/opt/homebrew/bin".into(),
        "/usr/local/bin".into(),
        "/usr/local/sbin".into(),
    ];
    if let Ok(home) = std::env::var("HOME") {
        extras.push(format!("{home}/homebrew/bin").into());
        extras.push(format!("{home}/.linuxbrew/bin").into());
        extras.push(format!("{home}/bin").into());
        extras.push(format!("{home}/.cargo/bin").into());
    }
    let base = std::env::var("PATH").unwrap_or_default();
    let joined = {
        let mut parts = extras;
        if !base.is_empty() {
            parts.extend(std::env::split_paths(&base));
        }
        std::env::join_paths(parts).ok()?
    };
    walk_path_env(name, Some(joined.to_string_lossy().as_ref()))
}

fn walk_path_env(name: &str, path_override: Option<&str>) -> Option<PathBuf> {
    let path = path_override
        .map(|s| s.to_string())
        .or_else(|| std::env::var("PATH").ok())?;
    // split_paths handles the OS separator (';' on Windows, ':' elsewhere).
    for dir in std::env::split_paths(&path) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(target_os = "windows")]
        {
            let exe = dir.join(format!("{name}.exe"));
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
        assert!(
            m.contains("x11grab")
                || m.contains("brew")
                || m.contains("apt")
                || m.contains("gdigrab")
                || m.contains("winget")
        );
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

    #[test]
    fn parse_dshow_audio_picks_quoted_names() {
        let sample = r#"
[dshow @ 0] DirectShow video devices
[dshow @ 0]  "Integrated Camera"
[dshow @ 0] DirectShow audio devices
[dshow @ 0]  "Microphone (Realtek)"
[dshow @ 0]  "Stereo Mix"
"#;
        let names = parse_dshow_audio_devices(sample);
        assert_eq!(names, vec!["Microphone (Realtek)", "Stereo Mix"]);
    }
}
