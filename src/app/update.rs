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
    Ok(ReleaseInfo {
        newer: tag != current && tag != current_tag,
        tag,
        url,
        notes,
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
}
