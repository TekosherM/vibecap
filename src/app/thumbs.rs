//! Disk thumbnail cache beside the media folder (`.vibecap/thumbs/`).

use std::path::{Path, PathBuf};

use crate::app::library::MediaCategory;

pub fn thumbs_dir(save_dir: &Path) -> PathBuf {
    save_dir.join(".vibecap").join("thumbs")
}

pub fn thumb_file(media: &Path) -> PathBuf {
    let parent = media.parent().unwrap_or_else(|| Path::new("."));
    let name = media
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "media".into());
    thumbs_dir(parent).join(format!("{name}.jpg"))
}

fn thumb_fresh(thumb: &Path, media: &Path) -> bool {
    let Ok(t) = thumb.metadata() else {
        return false;
    };
    let Ok(m) = media.metadata() else {
        return false;
    };
    match (t.modified(), m.modified()) {
        (Ok(tt), Ok(mt)) => tt >= mt && t.len() > 32,
        _ => t.len() > 32,
    }
}

/// Build or reuse a small JPEG next to the file. Safe to call off the UI thread.
pub fn ensure_thumb(media: &Path) -> Option<PathBuf> {
    if !media.is_file() {
        return None;
    }
    let dest = thumb_file(media);
    if thumb_fresh(&dest, media) {
        return Some(dest);
    }
    let _ = std::fs::create_dir_all(dest.parent()?);
    let ext = media
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match MediaCategory::from_ext(&ext) {
        Some(MediaCategory::Screenshot | MediaCategory::Gif) => {
            let img = image::open(media).ok()?;
            let t = img.thumbnail(192, 108);
            t.save(&dest).ok()?;
            Some(dest)
        }
        Some(MediaCategory::Video) => {
            let mut cmd = crate::platform::ffmpeg_command().ok()?;
            let src = media.to_str()?;
            let dst = dest.to_str()?;
            cmd.args([
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-ss",
                "0.4",
                "-i",
                src,
                "-vframes",
                "1",
                "-vf",
                "scale=192:-2:flags=fast_bilinear",
                dst,
            ]);
            crate::platform::run_ffmpeg(cmd, "thumb").ok()?;
            dest.exists().then_some(dest)
        }
        _ => None,
    }
}

pub fn warmup_thumbs(paths: Vec<PathBuf>) {
    std::thread::spawn(move || {
        for p in paths {
            let _ = ensure_thumb(&p);
        }
    });
}

/// Remove leftover `frames_temp/` dirs under the media folder (crash leftovers).
pub fn cleanup_frames_temp(save_dir: &Path) {
    let p = save_dir.join("frames_temp");
    if p.is_dir() {
        let _ = std::fs::remove_dir_all(&p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_file_sits_in_dot_vibecap() {
        let p = PathBuf::from("C:/media/shot.jpg");
        let t = thumb_file(&p);
        let s = t.to_string_lossy();
        assert!(s.contains(".vibecap"), "{s}");
        assert!(s.contains("thumbs"), "{s}");
    }
}
