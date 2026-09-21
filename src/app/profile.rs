//! E213 — portable profile export/import.
//!
//! A profile is a `.vcap-profile` zip holding the durable `SessionState`
//! (settings, hotkeys, library sets, snippets, retention rules) plus a
//! manifest. Runtime state is deliberately excluded — agent_record.json,
//! pending_still, and review drafts describe a *process*, not a profile.
//!
//! Import validates the zip and relies on `SessionState`'s
//! `#[serde(default)]` fields: a profile from an older/newer build fills
//! missing keys with defaults instead of failing.

use std::path::Path;

use super::session::SessionState;
use super::zip::{read_zip_stored, write_zip_entries};

const MANIFEST_ENTRY: &str = "manifest.json";
const SESSION_ENTRY: &str = "session.json";
const PROFILE_KIND: &str = "vibecap-profile";

pub fn export_profile(dest: &Path, session: &SessionState) -> Result<(), String> {
    let manifest = serde_json::json!({
        "kind": PROFILE_KIND,
        "version": 1,
        "app": "vibecap",
        "app_version": env!("CARGO_PKG_VERSION"),
        "exported_at": chrono::Local::now().to_rfc3339(),
    });
    let session_json = serde_json::to_vec_pretty(session).map_err(|e| e.to_string())?;
    write_zip_entries(
        dest,
        &[
            (MANIFEST_ENTRY.into(), manifest.to_string().into_bytes()),
            (SESSION_ENTRY.into(), session_json),
        ],
    )
}

/// Read + validate a `.vcap-profile`; returns the session to apply.
/// Missing/unknown fields fall back to serde defaults.
pub fn import_profile(src: &Path) -> Result<SessionState, String> {
    let entries = read_zip_stored(src)?;
    let session = entries
        .iter()
        .find(|(n, _)| n == SESSION_ENTRY)
        .map(|(_, b)| b)
        .ok_or("not a vibecap profile — no session.json inside")?;
    serde_json::from_slice(session).map_err(|e| format!("profile session.json: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_import_roundtrip_preserves_prefs() {
        let dir = std::env::temp_dir().join(format!("vibecap_prof_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("out.vcap-profile");
        let mut s = SessionState::default();
        s.theme = "mono".into();
        s.hotkey_shot_digit = 7;
        s.library_tags.insert("a.png".into(), vec!["work".into()]);
        export_profile(&dest, &s).unwrap();
        let back = import_profile(&dest).unwrap();
        assert_eq!(back.theme, "mono");
        assert_eq!(back.hotkey_shot_digit, 7);
        assert_eq!(back.library_tags["a.png"], vec!["work".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_fills_missing_fields_with_defaults() {
        let dir = std::env::temp_dir().join(format!("vibecap_profdef_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("sparse.vcap-profile");
        // A profile from an older build — only one key present.
        write_zip_entries(&dest, &[("session.json".into(), br#"{"fps":60}"#.to_vec())]).unwrap();
        let back = import_profile(&dest).unwrap();
        assert_eq!(back.fps, 60);
        // Fields absent from the file land on their serde defaults.
        assert_eq!(back.name_pattern, SessionState::default().name_pattern);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_rejects_non_profile_zip() {
        let dir = std::env::temp_dir().join(format!("vibecap_profbad_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("media.zip");
        write_zip_entries(&dest, &[("photo.png".into(), b"px".to_vec())]).unwrap();
        assert!(import_profile(&dest).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
