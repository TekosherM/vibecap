//! `vibecap doctor` — one-shot diagnostics for agents and humans.

use crate::platform::{
    capture_backend_label, config_dir, ffmpeg_available, ffmpeg_path, list_monitors,
    media_dir_display, resolve_output_dir, window_tools_hint,
};

pub fn doctor_text() -> String {
    let mut lines = Vec::new();
    lines.push(format!("vibecap {}", env!("CARGO_PKG_VERSION")));
    lines.push(format!("backend={}", capture_backend_label()));
    lines.push(format!(
        "ffmpeg={}",
        ffmpeg_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "missing".into())
    ));
    lines.push(format!("ffmpeg_run={}", if ffmpeg_available() { "yes" } else { "no" }));
    lines.push(format!("media_dir={}", media_dir_display()));
    lines.push(format!(
        "output_dir_default={}",
        resolve_output_dir(None).display()
    ));
    lines.push(format!("config_dir={}", config_dir().display()));
    lines.push(format!(
        "VIBECAP_OUTPUT_DIR={}",
        std::env::var("VIBECAP_OUTPUT_DIR").unwrap_or_else(|_| "(unset)".into())
    ));
    lines.push(format!(
        "VIBECAP_FFMPEG={}",
        std::env::var("VIBECAP_FFMPEG").unwrap_or_else(|_| "(unset)".into())
    ));
    lines.push(format!(
        "DISPLAY={}",
        std::env::var("DISPLAY").unwrap_or_else(|_| "(unset)".into())
    ));
    #[cfg(windows)]
    lines.push("gui_stdio=detached (windows_subsystem + ffmpeg null stdio)".into());
    #[cfg(not(windows))]
    lines.push("gui_stdio=console-ok".into());
    let mons = list_monitors();
    if mons.is_empty() {
        lines.push("monitors=(unknown)".into());
    } else {
        for (i, m) in mons.iter().enumerate() {
            lines.push(format!(
                "monitor{i}={}x{}+{}+{}{}",
                m.w,
                m.h,
                m.x,
                m.y,
                if m.primary { " primary" } else { "" }
            ));
        }
    }
    lines.push(format!(
        "VIBECAP_AUDIO_DEVICE={}",
        std::env::var("VIBECAP_AUDIO_DEVICE").unwrap_or_else(|_| "(unset)".into())
    ));
    if let Some(hint) = window_tools_hint() {
        lines.push(format!("window_crop={hint}"));
    }
    lines.push("cli=vibecap --screenshot --output-dir DIR [--window NAME]".into());
    lines.push("record=vibecap record start|stop|status".into());
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_mentions_backend_and_ffmpeg() {
        let t = doctor_text();
        assert!(t.contains("backend="), "{t}");
        assert!(t.contains("ffmpeg="), "{t}");
        assert!(t.contains("vibecap "), "{t}");
    }
}
