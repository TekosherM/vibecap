//! Opt-in GitHub Releases check (no background network).

use std::process::Command;

const RELEASES: &str = "https://api.github.com/repos/TekosherM/vibecap/releases/latest";

/// E214 — parsed release payload for the Settings card + download link.
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag: String,
    /// Release page URL (browser-openable).
    pub url: String,
    /// First lines of the release body — changelog preview.
    pub notes: String,
    /// GitHub's latest tag differs from the running build.
    pub newer: bool,
    /// E215 — direct download URL for this platform's asset, when the
    /// release carries one.
    pub asset_url: Option<String>,
}

/// E215 — the target-triple substring our release assets are named for.
/// `None` on platforms we don't ship binaries for.
fn target_asset_substr() -> Option<&'static str> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return Some("x86_64-pc-windows-msvc");
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return Some("aarch64-apple-darwin");
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return Some("x86_64-apple-darwin");
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Some("x86_64-unknown-linux-gnu");
    #[allow(unreachable_code)]
    None
}

pub fn check_latest_release() -> Result<ReleaseInfo, String> {
    parse_release_json(&fetch_latest_json()?)
}

fn parse_release_json(body: &str) -> Result<ReleaseInfo, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("could not parse GitHub JSON: {e}"))?;
    let tag = v
        .get("tag_name")
        .and_then(|t| t.as_str())
        .ok_or_else(|| "no tag_name in GitHub response".to_string())?
        .to_string();
    let url = v
        .get("html_url")
        .and_then(|u| u.as_str())
        .unwrap_or("https://github.com/TekosherM/vibecap/releases")
        .to_string();
    let notes = v
        .get("body")
        .and_then(|b| b.as_str())
        .map(|b| {
            b.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .take(8)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let current = env!("CARGO_PKG_VERSION");
    let current_tag = format!("v{current}");
    // E215 — find the asset built for this platform (name carries the
    // Rust target triple, e.g. vibecap-x86_64-pc-windows-msvc.zip).
    let asset_url = target_asset_substr().and_then(|needle| {
        v.get("assets").and_then(|a| a.as_array()).and_then(|arr| {
            arr.iter().find_map(|a| {
                let name = a.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if name.contains(needle) {
                    a.get("browser_download_url")
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
        })
    });
    Ok(ReleaseInfo {
        newer: tag != current && tag != current_tag,
        tag,
        url,
        notes,
        asset_url,
    })
}

fn fetch_latest_json() -> Result<String, String> {
    let curl = Command::new("curl")
        .args([
            "-fsSL",
            "-H",
            "User-Agent: vibecap",
            "-H",
            "Accept: application/vnd.github+json",
            RELEASES,
        ])
        .output();
    if let Ok(o) = curl {
        if o.status.success() {
            return Ok(String::from_utf8_lossy(&o.stdout).to_string());
        }
    }
    #[cfg(windows)]
    {
        let ps = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "try {{ (Invoke-WebRequest -UseBasicParsing -Uri '{RELEASES}' -Headers @{{'User-Agent'='vibecap'}}).Content }} catch {{ '' }}"
                ),
            ])
            .output()
            .map_err(|e| format!("update check failed: {e}"))?;
        if ps.status.success() {
            let s = String::from_utf8_lossy(&ps.stdout).to_string();
            if s.trim().starts_with('{') {
                return Ok(s);
            }
        }
    }
    Err("could not reach GitHub Releases (curl/network)".into())
}

// ── E215 — staged self-update: check → download → apply on restart ─────────
//
// Windows allows *renaming* a running exe (not overwriting it), so the swap
// is: `vibecap.exe` → `vibecap.old.exe`, `vibecap.new.exe` → `vibecap.exe`,
// then a delayed detached relaunch — the new process must not see this
// one's gui.lock, so it waits ~2 s before starting. The old binary is
// deleted by the new process at startup (`cleanup_old_binary`).

use std::path::{Path, PathBuf};

/// `<exe>.new[.exe]` — staged binary path. Beside the exe so the apply
/// rename is same-volume (atomic) and shares the exe dir's permissions.
pub fn staged_update_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.with_extension(if cfg!(windows) { "new.exe" } else { "new" }))
}

fn old_backup_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.with_extension(if cfg!(windows) { "old.exe" } else { "old" }))
}

/// A staged update waiting to be applied (survives restarts — the file
/// sits beside the exe until something consumes it).
pub fn staged_update_exists() -> bool {
    staged_update_path().map(|p| p.exists()).unwrap_or(false)
}

fn rejected_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.with_extension(if cfg!(windows) { "rej.exe" } else { "rej" }))
}

/// E264 — a previous binary parked by the last update is the rollback target.
pub fn rollback_available() -> bool {
    old_backup_path().map(|p| p.exists()).unwrap_or(false)
}

/// Startup housekeeping: the `.old` backup is NOT deleted — it stays beside
/// the exe as the one-click rollback target until the next update replaces
/// it (apply_staged removes it before parking a newer backup). What we do
/// sweep: a `.rej` leftover from a completed rollback, and a zero-length or
/// obviously-stale `.new` (a crashed download shouldn't block re-staging).
pub fn cleanup_old_binary() {
    if let Some(rej) = rejected_path() {
        let _ = std::fs::remove_file(rej);
    }
    if let Some(new) = staged_update_path() {
        if std::fs::metadata(&new).map(|m| m.len()).unwrap_or(0) < 500_000 {
            let _ = std::fs::remove_file(new);
        }
    }
}

/// E264 — swap back to the previous build: current exe → `<exe>.rej`,
/// `<exe>.old` → exe, then the same delayed detached relaunch as
/// `apply_staged_and_restart`. The rejected binary is swept by
/// `cleanup_old_binary` on the next launch. Caller must exit after Ok.
pub fn rollback_and_restart() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let old = old_backup_path().ok_or_else(|| "no previous binary".to_string())?;
    if !old.exists() {
        return Err("no previous binary — nothing to roll back to".into());
    }
    let rej = rejected_path().ok_or_else(|| "no rejected path".to_string())?;
    let _ = std::fs::remove_file(&rej);
    std::fs::rename(&exe, &rej).map_err(|e| format!("could not park current exe: {e}"))?;
    if let Err(e) = std::fs::rename(&old, &exe) {
        // Never leave the install without a runnable exe.
        let _ = std::fs::rename(&rej, &exe);
        return Err(format!("could not restore previous binary: {e}"));
    }
    #[cfg(windows)]
    {
        let line = format!(
            "ping -n 3 127.0.0.1 >nul & start \"\" \"{}\"",
            exe.display()
        );
        Command::new("cmd")
            .args(["/C", &line])
            .spawn()
            .map_err(|e| format!("relaunch: {e}"))?;
    }
    #[cfg(unix)]
    {
        Command::new("sh")
            .arg("-c")
            .arg(format!("sleep 2; \"{}\" >/dev/null 2>&1 &", exe.display()))
            .spawn()
            .map_err(|e| format!("relaunch: {e}"))?;
    }
    Ok(())
}

/// Download the platform asset and extract the binary to `<exe>.new`.
/// Runs on a worker — the UI thread must never see this.
pub fn download_and_stage(asset_url: &str) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| "exe has no parent dir".to_string())?;
    let tmp = exe_dir.join(".vibecap-update");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| format!("staging dir: {e}"))?;
    let pkg = tmp.join("update.pkg");

    let ok = Command::new("curl")
        .args(["-fL", "--silent", "--show-error", "-o"])
        .arg(&pkg)
        .arg(asset_url)
        .status()
        .map_err(|e| format!("curl: {e}"))?;
    if !ok.success() {
        return Err("download failed (curl)".into());
    }

    // bsdtar (`tar` on Win10+) reads zip AND tar.gz — one tool for both
    // archive shapes the release ships.
    let ok = Command::new("tar")
        .arg("-xf")
        .arg(&pkg)
        .arg("-C")
        .arg(&tmp)
        .status()
        .map_err(|e| format!("tar: {e}"))?;
    if !ok.success() {
        return Err("could not unpack the release archive".into());
    }

    // Find the binary inside the archive (root or one level of nesting).
    let bin_name = if cfg!(windows) {
        "vibecap.exe"
    } else {
        "vibecap"
    };
    let mut found = None;
    for entry in walkdir_shallow(&tmp) {
        if entry.file_name().map(|n| n == bin_name).unwrap_or(false) {
            found = Some(entry);
            break;
        }
    }
    let bin = found.ok_or_else(|| format!("archive has no {bin_name}"))?;

    // Sanity: real binary size + MZ/ELF/Mach-O magic — a corrupt or HTML
    // error page must never be swapped in.
    let head = std::fs::read(&bin).unwrap_or_default();
    let magic_ok = head.len() > 500_000
        && (head.starts_with(b"MZ")
            || head.starts_with(b"\x7fELF")
            || head.starts_with(&[0xfe, 0xed, 0xfa])
            || head.starts_with(&[0xcf, 0xfa, 0xed, 0xfe]));
    if !magic_ok {
        return Err("downloaded file doesn't look like a vibecap binary".into());
    }

    let dest = staged_update_path().ok_or_else(|| "no staged path".to_string())?;
    std::fs::copy(&bin, &dest).map_err(|e| format!("stage failed: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
    }
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(dest)
}

/// Shallow walk of a staging dir — archive members sit at root or in a
/// single named folder.
fn walkdir_shallow(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() {
                out.push(p);
            } else if p.is_dir() {
                if let Ok(inner) = std::fs::read_dir(&p) {
                    out.extend(inner.flatten().map(|i| i.path()).filter(|p| p.is_file()));
                }
            }
        }
    }
    out
}

/// Swap `<exe>.new` in for the running exe and schedule a relaunch after
/// ~2 s (the gui.lock dies with this process — a child spawned immediately
/// would see the lock and focus-quit). Caller must exit after Ok.
pub fn apply_staged_and_restart() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let new = staged_update_path().ok_or_else(|| "no staged update".to_string())?;
    if !new.exists() {
        return Err("no staged update".into());
    }
    let old = old_backup_path().ok_or_else(|| "no backup path".to_string())?;
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).map_err(|e| format!("could not park current exe: {e}"))?;
    if let Err(e) = std::fs::rename(&new, &exe) {
        // Roll back so the install is never left without a runnable exe.
        let _ = std::fs::rename(&old, &exe);
        return Err(format!("could not install update: {e}"));
    }
    #[cfg(windows)]
    {
        let line = format!(
            "ping -n 3 127.0.0.1 >nul & start \"\" \"{}\"",
            exe.display()
        );
        Command::new("cmd")
            .args(["/C", &line])
            .spawn()
            .map_err(|e| format!("relaunch: {e}"))?;
    }
    #[cfg(unix)]
    {
        Command::new("sh")
            .arg("-c")
            .arg(format!("sleep 2; \"{}\" >/dev/null 2>&1 &", exe.display()))
            .spawn()
            .map_err(|e| format!("relaunch: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn current_version_is_semverish() {
        let v = env!("CARGO_PKG_VERSION");
        assert!(v.contains('.'), "{v}");
    }

    #[test]
    fn parse_release_json_extracts_tag_url_notes() {
        let info = super::parse_release_json(
            r#"{"tag_name":"v9.9.9","html_url":"https://example.test/r","body":"line one\n\nline two"}"#,
        )
        .unwrap();
        assert_eq!(info.tag, "v9.9.9");
        assert_eq!(info.url, "https://example.test/r");
        assert!(info.notes.contains("line one"));
        assert!(info.newer); // running build is never 9.9.9
    }

    #[test]
    fn parse_release_json_same_tag_is_not_newer() {
        let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
        let info = super::parse_release_json(&format!(
            "{{\"tag_name\":\"{tag}\",\"html_url\":\"u\",\"body\":\"\"}}"
        ))
        .unwrap();
        assert!(!info.newer);
    }

    /// E215 — the platform asset URL is picked out of the assets array;
    /// a release with no matching triple yields None.
    #[test]
    fn parse_release_json_picks_platform_asset() {
        let body = r#"{"tag_name":"v9.9.9","html_url":"u","body":"","assets":[
            {"name":"vibecap-aarch64-apple-darwin.tar.gz","browser_download_url":"https://x/mac-arm"},
            {"name":"vibecap-x86_64-pc-windows-msvc.zip","browser_download_url":"https://x/win"},
            {"name":"vibecap-x86_64-unknown-linux-gnu.tar.gz","browser_download_url":"https://x/linux"}
        ]}"#;
        let info = super::parse_release_json(body).unwrap();
        match super::target_asset_substr() {
            // On a shipped triple the matching URL comes through.
            Some(needle) => {
                let u = info.asset_url.expect("asset_url should be Some");
                assert!(u.starts_with("https://x/"), "{u}");
                let _ = needle;
            }
            None => assert!(info.asset_url.is_none()),
        }
        // No assets key → None, never an error.
        let info = super::parse_release_json(r#"{"tag_name":"v9.9.9","html_url":"u"}"#).unwrap();
        assert!(info.asset_url.is_none());
    }
}
