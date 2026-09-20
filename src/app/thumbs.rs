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

/// Max bytes kept in `.vibecap/thumbs/` — oldest-modified files evicted first.
const THUMB_CACHE_CAP_BYTES: u64 = 300 * 1024 * 1024;

/// Sweep the thumbs dir on a worker: delete orphans (media file gone) and
/// evict oldest-modified thumbs while over `THUMB_CACHE_CAP_BYTES`.
/// `media_names` = full file names of live library items (thumb `x.mp4.jpg`
/// is orphaned when `x.mp4` no longer exists).
pub fn sweep_thumbs(save_dir: PathBuf, media_names: std::collections::HashSet<String>) {
    std::thread::spawn(move || sweep_thumbs_dir(&save_dir, &media_names));
}

fn sweep_thumbs_dir(save_dir: &Path, media_names: &std::collections::HashSet<String>) {
    let dir = thumbs_dir(save_dir);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut kept: Vec<(PathBuf, std::time::SystemTime, u64)> = Vec::new();
    let mut total = 0u64;
    for e in entries.flatten() {
        let path = e.path();
        if !path.is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        let media = name.strip_suffix(".jpg").unwrap_or(&name).to_string();
        if !media_names.contains(&media) {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        let Ok(meta) = e.metadata() else {
            continue;
        };
        let len = meta.len();
        total += len;
        kept.push((
            path,
            meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            len,
        ));
    }
    if total <= THUMB_CACHE_CAP_BYTES {
        return;
    }
    kept.sort_by_key(|(_, m, _)| *m);
    for (path, _, len) in kept {
        if total <= THUMB_CACHE_CAP_BYTES {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

/// E170 — thumbnail repair: delete zero-byte/corrupt thumbs, then regenerate
/// anything missing for the given media files. Runs on its own worker.
pub fn repair_thumbs(save_dir: PathBuf, media: Vec<PathBuf>) {
    std::thread::spawn(move || {
        let dir = thumbs_dir(&save_dir);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                let broken = e.metadata().map(|m| m.len() == 0).unwrap_or(false);
                if broken {
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
        for p in media {
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

    #[test]
    fn sweep_removes_orphans_keeps_live() {
        let dir = std::env::temp_dir().join(format!("vibecap_sweep_{}", std::process::id()));
        let thumbs = thumbs_dir(&dir);
        std::fs::create_dir_all(&thumbs).unwrap();
        let live = thumbs.join("keep.mp4.jpg");
        let dead = thumbs.join("gone.mp4.jpg");
        std::fs::write(&live, b"x").unwrap();
        std::fs::write(&dead, b"x").unwrap();
        let names = std::collections::HashSet::from(["keep.mp4".to_string()]);
        sweep_thumbs_dir(&dir, &names);
        assert!(live.exists(), "live thumb must survive");
        assert!(!dead.exists(), "orphaned thumb must be removed");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
