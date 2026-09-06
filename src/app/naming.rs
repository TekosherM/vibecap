//! Capture filename tokens: `{app}-{date}-{seq}`.

use chrono::Local;
use std::path::Path;

pub const DEFAULT_PATTERN: &str = "{app}-{date}-{seq}";

/// Build a file stem from a pattern.
///
/// Tokens: `{app}` `{date}` `{time}` `{seq}` `{orig}`.
pub fn format_capture_stem(pattern: &str, app: Option<&str>, seq: u32) -> String {
    let now = Local::now();
    let app = sanitize_token(app.unwrap_or("desktop"));
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H-%M-%S").to_string();
    let seq_s = format!("{seq:03}");
    let mut out = if pattern.trim().is_empty() {
        DEFAULT_PATTERN.to_string()
    } else {
        pattern.to_string()
    };
    out = out.replace("{app}", &app);
    out = out.replace("{date}", &date);
    out = out.replace("{time}", &time);
    out = out.replace("{seq}", &seq_s);
    out = out.replace("{orig}", "capture");
    let stem = sanitize_token(&out);
    if stem.is_empty() {
        format!("capture-{date}-{time}")
    } else {
        stem
    }
}

fn sanitize_token(s: &str) -> String {
    let s = s.trim();
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        } else if c.is_whitespace() || c == ':' || c == '/' || c == '\\' {
            if !out.ends_with('-') {
                out.push('-');
            }
        }
    }
    out.trim_matches('-').to_string()
}

/// Next `{seq}` so `stem.jpg` does not collide in `dir`.
pub fn next_seq(dir: &Path, prefix: &str) -> u32 {
    let mut max = 0u32;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 1;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(rest) = name.strip_prefix(prefix) {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<u32>() {
                max = max.max(n);
            }
        }
    }
    max + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_sanitized() {
        let s = format_capture_stem("{app}-{date}-{seq}", Some("Google Chrome"), 7);
        assert!(s.contains("Google-Chrome") || s.contains("GoogleChrome"), "{s}");
        assert!(s.contains("-007") || s.ends_with("007") || s.contains("007"), "{s}");
        assert!(!s.contains(' '));
    }

    #[test]
    fn empty_pattern_falls_back() {
        let s = format_capture_stem("  ", None, 1);
        assert!(s.contains("desktop") || s.contains("capture"), "{s}");
    }
}
