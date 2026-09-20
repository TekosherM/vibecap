//! Opt-in GitHub Releases check (no background network).

use std::process::Command;

const RELEASES: &str = "https://api.github.com/repos/TekosherM/vibecap/releases/latest";

pub fn check_latest_release() -> Result<String, String> {
    let body = fetch_latest_json()?;
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("could not parse GitHub JSON: {e}"))?;
    let tag = v
        .get("tag_name")
        .and_then(|t| t.as_str())
        .ok_or_else(|| "no tag_name in GitHub response".to_string())?;
    let current = env!("CARGO_PKG_VERSION");
    let current_tag = format!("v{current}");
    if tag == current || tag == current_tag {
        Ok(format!("up to date ({current_tag})"))
    } else {
        Ok(format!("latest {tag} · running {current_tag}"))
    }
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

#[cfg(test)]
mod tests {
    #[test]
    fn current_version_is_semverish() {
        let v = env!("CARGO_PKG_VERSION");
        assert!(v.contains('.'), "{v}");
    }
}
