//! Shared atomic JSON write for budget + feedback files.

use std::path::{Path, PathBuf};

use crate::platform::config_dir as platform_config_dir;

pub fn vibecap_config_dir() -> PathBuf {
    platform_config_dir()
}

// ── Startup timing (E236) ────────────────────────────────────────────

static APP_T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
static STARTUP_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Call at process start (main()) — t0 for the cold-launch budget.
pub fn mark_app_start() {
    let _ = APP_T0.set(std::time::Instant::now());
}

/// Call once at the end of the first painted update() — records
/// launch→interactive latency. Later calls are no-ops.
pub fn note_first_frame() {
    if let Some(t0) = APP_T0.get() {
        let ms = t0.elapsed().as_millis() as u64;
        let _ = STARTUP_MS.compare_exchange(
            0,
            ms.max(1),
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        );
    }
}

/// ms from process start to first painted frame; None for CLI/headless.
pub fn startup_elapsed_ms() -> Option<u64> {
    match STARTUP_MS.load(std::sync::atomic::Ordering::SeqCst) {
        0 => None,
        v => Some(v),
    }
}

/// Write-then-rename so a concurrent reader never sees a partial file.
pub fn write_json_atomic(path: &PathBuf, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, contents).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Marker written by the screenshot worker so the UI can restore even if
/// the window was orderOut and the channel was missed.
fn pending_still_path() -> PathBuf {
    vibecap_config_dir().join("pending_still.path")
}

pub fn write_pending_still(path: &Path) {
    let _ = std::fs::create_dir_all(vibecap_config_dir());
    let _ = std::fs::write(pending_still_path(), path.to_string_lossy().as_bytes());
}

pub fn write_pending_still_error(msg: &str) {
    let _ = std::fs::create_dir_all(vibecap_config_dir());
    let _ = std::fs::write(pending_still_path(), format!("ERROR\n{msg}").as_bytes());
}

/// Returns Ok(path) or Err(message). Clears the marker.
pub fn take_pending_still() -> Option<Result<PathBuf, String>> {
    let p = pending_still_path();
    let raw = std::fs::read_to_string(&p).ok()?;
    let _ = std::fs::remove_file(&p);
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(msg) = raw.strip_prefix("ERROR\n") {
        return Some(Err(msg.to_string()));
    }
    if let Some(msg) = raw.strip_prefix("ERROR:") {
        return Some(Err(msg.trim().to_string()));
    }
    let path = PathBuf::from(raw);
    if path.exists() {
        Some(Ok(path))
    } else {
        Some(Err(format!("Capture file missing: {}", path.display())))
    }
}

// ── E210 CLI poke — command handoff to a running (or next-launched) GUI ──────

fn pending_cmd_path() -> PathBuf {
    vibecap_config_dir().join("pending_cmd.txt")
}

/// E250 — marker-existence probe for the pump thread (it cannot call
/// `take_pending_cmd` — that would consume the payload).
pub fn pending_cmd_waiting() -> bool {
    pending_cmd_path().exists()
}

/// `vibecap poke <cmd>` writes one word: show|hide|screenshot|record|stop.
pub fn write_pending_cmd(cmd: &str) {
    let _ = std::fs::create_dir_all(vibecap_config_dir());
    let _ = std::fs::write(pending_cmd_path(), cmd.as_bytes());
}

/// Consume a queued poke, if any.
pub fn take_pending_cmd() -> Option<String> {
    let p = pending_cmd_path();
    let raw = std::fs::read_to_string(&p).ok()?;
    let _ = std::fs::remove_file(&p);
    let cmd = raw.trim().to_string();
    (!cmd.is_empty()).then_some(cmd)
}

// ── E187 deep links — vibecap://… handoff to the running/next GUI ──────────

fn pending_deep_path() -> PathBuf {
    vibecap_config_dir().join("pending_deep.txt")
}

/// Marker-existence probe for the pump thread (must not consume).
pub fn pending_deep_waiting() -> bool {
    pending_deep_path().exists()
}

/// A `vibecap://…` invocation writes the URL for the GUI to consume.
pub fn write_pending_deep(url: &str) {
    let _ = std::fs::create_dir_all(vibecap_config_dir());
    let _ = std::fs::write(pending_deep_path(), url.as_bytes());
}

/// Consume a queued deep link, if any.
pub fn take_pending_deep() -> Option<String> {
    let p = pending_deep_path();
    let raw = std::fs::read_to_string(&p).ok()?;
    let _ = std::fs::remove_file(&p);
    let url = raw.trim().to_string();
    (!url.is_empty()).then_some(url)
}

/// D68 — `file:///…` URI for a capture path (percent-encoded, forward slashes).
pub fn file_uri(path: &Path) -> String {
    let mut s = path.to_string_lossy().replace('\\', "/");
    if !s.starts_with('/') {
        s.insert(0, '/');
    }
    let mut out = String::with_capacity(s.len() + 8);
    out.push_str("file://");
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '/' | ':' | '.' | '-' | '_' | '~' => out.push(c),
            _ => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).as_bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
        }
    }
    out
}

/// D68 — minimal base64 (no dep) for data-URI copies.
pub fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// D68 — `data:<mime>;base64,…` for a capture. Capped so a 200 MB clip never
/// lands on the clipboard.
pub fn data_uri(path: &Path, max_bytes: u64) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "too large for a data URI ({:.1} MB — cap is {} MB)",
            bytes.len() as f64 / 1e6,
            max_bytes / 1_000_000
        ));
    }
    let mime = match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        _ => "application/octet-stream",
    };
    Ok(format!("data:{mime};base64,{}", base64_encode(&bytes)))
}

/// E279 — stable machine-readable failure codes shared by the CLI
/// (`error[E_X]: …` / `{"code":…}`) and MCP (`result.errorCode`). Agents
/// branch on the code; the human text stays free-form.
pub fn error_code(msg: &str) -> &'static str {
    let m = msg.to_lowercase();
    if m.starts_with("usage:") {
        "E_USAGE"
    } else if m.contains("already recording") {
        "E_ALREADY_RECORDING"
    } else if m.contains("not recording") || m.contains("nothing to stop") {
        "E_NOT_RECORDING"
    } else if m.contains("could not find a window") || m.contains("no matching window") {
        "E_NO_WINDOW"
    } else if m.contains("refusing to capture") {
        "E_SELF_CAPTURE"
    } else if m.contains("budget") {
        "E_BUDGET"
    } else if m.contains("ffmpeg") {
        "E_FFMPEG"
    } else if m.contains("permission") || m.contains("denied") || m.contains("access") {
        "E_PERMISSION"
    } else if m.contains("not found") || m.contains("no such") || m.contains("missing") {
        "E_NOT_FOUND"
    } else {
        "E_FAILED"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_is_stable() {
        assert_eq!(
            error_code("not recording — nothing to stop"),
            "E_NOT_RECORDING"
        );
        assert_eq!(error_code("already recording pid 4"), "E_ALREADY_RECORDING");
        assert_eq!(
            error_code("could not find a window matching “x”"),
            "E_NO_WINDOW"
        );
        assert_eq!(error_code("ffmpeg failed: boom"), "E_FFMPEG");
        assert_eq!(error_code("BUDGET EXHAUSTED"), "E_BUDGET");
        assert_eq!(error_code("mystery"), "E_FAILED");
    }

    #[test]
    fn file_uri_encodes_spaces_and_backslashes() {
        let p = PathBuf::from("C:\\Dev\\vibecap shots\\a b.png");
        assert_eq!(file_uri(&p), "file:///C:/Dev/vibecap%20shots/a%20b.png");
    }

    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }
}
