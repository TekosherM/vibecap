//! Media library model + directory scan (no UI).

use std::path::{Path, PathBuf};

/// Max rows shown at once in the library (newest first). User can load more.
pub const LIBRARY_PAGE_SIZE: usize = 40;

/// Grid ordering for the Library.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum LibrarySort {
    #[default]
    Newest,
    Oldest,
    Largest,
    Smallest,
    Name,
    Type,
}

impl LibrarySort {
    pub const ALL: [LibrarySort; 6] = [
        Self::Newest,
        Self::Oldest,
        Self::Largest,
        Self::Smallest,
        Self::Name,
        Self::Type,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Newest => "Newest",
            Self::Oldest => "Oldest",
            Self::Largest => "Largest",
            Self::Smallest => "Smallest",
            Self::Name => "Name",
            Self::Type => "Type",
        }
    }

    /// Date-based sorts keep the Today/Yesterday/… group headers.
    pub fn groups_by_date(self) -> bool {
        matches!(self, Self::Newest | Self::Oldest)
    }

    /// Sort any slice of items (owned or refs) in place.
    pub fn apply<T: std::borrow::Borrow<MediaItem>>(self, items: &mut [T]) {
        match self {
            Self::Newest => {
                items.sort_by(|a, b| b.borrow().modified_secs.cmp(&a.borrow().modified_secs))
            }
            Self::Oldest => {
                items.sort_by(|a, b| a.borrow().modified_secs.cmp(&b.borrow().modified_secs))
            }
            Self::Largest => {
                items.sort_by(|a, b| b.borrow().size_bytes.cmp(&a.borrow().size_bytes))
            }
            Self::Smallest => {
                items.sort_by(|a, b| a.borrow().size_bytes.cmp(&b.borrow().size_bytes))
            }
            Self::Name => items.sort_by(|a, b| {
                a.borrow()
                    .name
                    .to_lowercase()
                    .cmp(&b.borrow().name.to_lowercase())
            }),
            Self::Type => items.sort_by(|a, b| {
                (a.borrow().category as u8)
                    .cmp(&(b.borrow().category as u8))
                    .then_with(|| b.borrow().modified_secs.cmp(&a.borrow().modified_secs))
            }),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MediaCategory {
    Screenshot,
    Video,
    Gif,
    Audio,
    Note,
}

impl MediaCategory {
    pub fn label(self) -> &'static str {
        match self {
            MediaCategory::Screenshot => "Screenshots",
            MediaCategory::Video => "Videos",
            MediaCategory::Gif => "GIFs",
            MediaCategory::Audio => "Audio",
            MediaCategory::Note => "Notes",
        }
    }

    #[allow(dead_code)]
    pub fn icon(self) -> &'static str {
        match self {
            MediaCategory::Screenshot => "📸",
            MediaCategory::Video => "🎥",
            MediaCategory::Gif => "🎞",
            MediaCategory::Audio => "🎙",
            MediaCategory::Note => "📝",
        }
    }

    pub fn from_ext(ext: &str) -> Option<Self> {
        match ext {
            "png" | "jpg" | "jpeg" | "webp" => Some(MediaCategory::Screenshot),
            "mp4" | "mov" | "webm" | "mkv" => Some(MediaCategory::Video),
            "gif" => Some(MediaCategory::Gif),
            "m4a" | "mp3" | "wav" | "aac" => Some(MediaCategory::Audio),
            "txt" | "md" => Some(MediaCategory::Note),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct MediaItem {
    pub path: PathBuf,
    pub name: String,
    pub size_str: String,
    pub size_bytes: u64,
    pub category: MediaCategory,
    /// Unix secs for sort (newest first).
    pub modified_secs: u64,
    /// E165 — true when another library item has identical content
    /// (same size + same head/tail hash). Set by `mark_duplicates`.
    pub dupe: bool,
}

/// E79 — indices of items a retention rule would sweep.
/// mode 1: mtime older than `value` days; mode 2: all but the newest `value`.
pub fn retention_pick(items: &[MediaItem], mode: u8, value: u32, now_secs: u64) -> Vec<usize> {
    match mode {
        1 => {
            let cutoff = now_secs.saturating_sub(value as u64 * 86_400);
            items
                .iter()
                .enumerate()
                .filter(|(_, i)| i.modified_secs < cutoff)
                .map(|(n, _)| n)
                .collect()
        }
        2 => {
            let mut idx: Vec<usize> = (0..items.len()).collect();
            idx.sort_by_key(|&i| std::cmp::Reverse(items[i].modified_secs));
            idx.into_iter().skip(value as usize).collect()
        }
        _ => Vec::new(),
    }
}

/// Where a media item sits in the capture → review → annotate → ask → answer loop.
/// Heuristic from filename/category (Phase 1c chrome) — not a full graph.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoopPosition {
    Capture,
    Review,
    Annotate,
    Ask,
    Answered,
}

impl LoopPosition {
    pub fn label(self) -> &'static str {
        match self {
            Self::Capture => "CAPTURE",
            Self::Review => "REVIEW",
            Self::Annotate => "ANNOTATE",
            Self::Ask => "ASK",
            Self::Answered => "ANSWERED",
        }
    }

    /// Derive loop stage from name + category (lightweight, no DB).
    pub fn for_item(item: &MediaItem) -> Self {
        let n = item.name.to_lowercase();
        if n.contains("answered") || n.contains("_reply") || n.contains("response") {
            return Self::Answered;
        }
        if n.contains("annotated") || n.contains("_markup") {
            return Self::Annotate;
        }
        if n.contains("ask") || n.contains("feedback") || n.contains("inbox") {
            return Self::Ask;
        }
        // Screenshots/GIFs default to Review (ready to inspect); video/audio stay Capture.
        match item.category {
            MediaCategory::Screenshot | MediaCategory::Gif => Self::Review,
            MediaCategory::Video | MediaCategory::Audio | MediaCategory::Note => Self::Capture,
        }
    }
}

impl MediaItem {
    pub fn loop_position(&self) -> LoopPosition {
        LoopPosition::for_item(self)
    }
}

pub fn format_size(size_bytes: u64) -> String {
    if size_bytes > 1_048_576 {
        format!("{:.1} MB", size_bytes as f64 / 1_048_576.0)
    } else {
        format!("{} KB", size_bytes / 1024)
    }
}

/// Scan a media directory into sorted library items (newest first).
pub fn scan_media_dir(save_dir: &Path) -> Vec<MediaItem> {
    let mut items = Vec::new();
    if let Ok(entries) = std::fs::read_dir(save_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            let Some(category) = MediaCategory::from_ext(&ext) else {
                continue;
            };
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            if name.starts_with('.')
                || name.starts_with("vibecap_region_snap_")
                || name.ends_with(".ffmpeg.log")
                || name.ends_with(".clean.mp4")
                // Vibecap's own metadata sidecars — never content.
                || name.ends_with(".notes.txt")
                || name.ends_with(".markers.txt")
                || name.contains("frames_temp")
                || ext == "log"
            {
                continue;
            }
            if category == MediaCategory::Note && is_sidecar_note(&path) {
                continue;
            }
            let meta = entry.metadata().ok();
            let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let size_str = format_size(size_bytes);
            let modified_secs = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            items.push(MediaItem {
                path,
                name,
                size_str,
                size_bytes,
                category,
                modified_secs,
                dupe: false,
            });
        }
    }
    mark_duplicates(&mut items);
    items.sort_by(|a, b| {
        b.modified_secs
            .cmp(&a.modified_secs)
            .then_with(|| b.name.cmp(&a.name))
    });
    items
}

/// E165 — flag items whose bytes are identical to another item's. Size
/// collisions are rare, so content is only read for size-colliding files;
/// the hash covers head + tail + len (cheap on multi-GB videos).
pub fn mark_duplicates(items: &mut [MediaItem]) {
    use std::collections::HashMap;
    // size → indices of items with that size.
    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, it) in items.iter().enumerate() {
        if it.size_bytes > 0 {
            by_size.entry(it.size_bytes).or_default().push(i);
        }
    }
    let mut seen: HashMap<u64, Vec<usize>> = HashMap::new();
    for idxs in by_size.values().filter(|v| v.len() > 1) {
        for &i in idxs {
            let h = content_fingerprint(&items[i].path, items[i].size_bytes);
            seen.entry(h).or_default().push(i);
        }
    }
    for idxs in seen.values().filter(|v| v.len() > 1) {
        for &i in idxs {
            items[i].dupe = true;
        }
    }
}

/// Head+tail+len fingerprint — not cryptographic, just enough to tell a
/// re-shot frame apart from a byte-identical copy.
fn content_fingerprint(path: &Path, len: u64) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    h.write_u64(len);
    let Ok(mut f) = std::fs::File::open(path) else {
        return h.finish();
    };
    let mut head = [0u8; 65536];
    let n = std::io::Read::read(&mut f, &mut head).unwrap_or(0);
    h.write(&head[..n]);
    if len > 65536 {
        use std::io::{Seek, SeekFrom};
        if f.seek(SeekFrom::End(-65536)).is_ok() {
            let mut tail = [0u8; 65536];
            let n = std::io::Read::read(&mut f, &mut tail).unwrap_or(0);
            h.write(&tail[..n]);
        }
    }
    h.finish()
}

pub fn get_dir_size_bytes(dir_path: &str) -> (u64, usize) {
    let mut total_size = 0u64;
    let mut count = 0usize;
    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total_size += meta.len();
                    count += 1;
                }
            }
        }
    }
    (total_size, count)
}

/// Filter items by library filter label ("All" or category label).
pub fn filter_items<'a>(items: &'a [MediaItem], filter: &str) -> Vec<&'a MediaItem> {
    items
        .iter()
        .filter(|item| filter == "All" || item.category.label() == filter)
        .collect()
}

/// Hide `.txt` notes that sit next to a capture (sidecar clutter).
fn is_sidecar_note(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    let Some(parent) = path.parent() else {
        return false;
    };
    for ext in [
        "jpg", "jpeg", "png", "gif", "webp", "mp4", "mov", "webm", "mkv", "m4a",
    ] {
        if parent.join(format!("{stem}.{ext}")).exists() {
            return true;
        }
    }
    false
}

/// Bucket a unix mtime into Today / Yesterday / This week / Earlier (rolling).
pub fn date_group_label(modified_secs: u64) -> &'static str {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(modified_secs);
    let age = now.saturating_sub(modified_secs);
    if age < 86_400 {
        "Today"
    } else if age < 172_800 {
        "Yesterday"
    } else if age < 86_400 * 7 {
        "This week"
    } else {
        "Earlier"
    }
}

pub fn category_bytes(items: &[MediaItem], cat: MediaCategory) -> u64 {
    items
        .iter()
        .filter(|i| i.category == cat)
        .map(|i| i.size_bytes)
        .sum()
}

/// E167 — recursive byte total for a directory tree (0 when missing).
pub fn dir_tree_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_file() {
                total += meta.len();
            } else if meta.is_dir() {
                total += dir_tree_size(&entry.path());
            }
        }
    }
    total
}

/// E167 — regenerable bytes under the media root: filmstrip/scrub scratch
/// dirs, live-capture sessions, and the `.vibecap` thumb cache. All of it
/// is safe to delete (thumbs regenerate on demand).
pub fn reclaimable_bytes(save_dir: &Path) -> u64 {
    ["frames_temp", "frames_scrub", "live", ".vibecap"]
        .iter()
        .map(|d| dir_tree_size(&save_dir.join(d)))
        .sum()
}

/// E167 — remove the reclaimable dirs (thumbs regenerate; frames_* and
/// live/ sessions are scratch). Returns freed bytes.
pub fn clean_reclaimable(save_dir: &Path) -> u64 {
    let freed = reclaimable_bytes(save_dir);
    for d in ["frames_temp", "frames_scrub", "live", ".vibecap"] {
        let _ = std::fs::remove_dir_all(save_dir.join(d));
    }
    freed
}

/// E250 — one watch-folder sweep: move settled media files (mtime ≥2 s so
/// in-flight copies aren't grabbed mid-write) into `media`, `_N`-suffixed
/// on collision. Pure filesystem — safe from the pump thread while parked.
/// Returns `(moved, failed)`.
pub fn watch_sweep(watch: &Path, media: &Path) -> (usize, usize) {
    let mut moved = 0usize;
    let mut failed = 0usize;
    let Ok(rd) = std::fs::read_dir(watch) else {
        return (0, 0);
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_media = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| {
                matches!(
                    e.to_ascii_lowercase().as_str(),
                    "jpg"
                        | "jpeg"
                        | "png"
                        | "gif"
                        | "webp"
                        | "mp4"
                        | "mov"
                        | "webm"
                        | "mkv"
                        | "m4a"
                        | "wav"
                        | "mp3"
                )
            })
            .unwrap_or(false);
        if !is_media {
            continue;
        }
        let settled = entry
            .metadata()
            .and_then(|m| m.modified())
            .map(|t| {
                std::time::SystemTime::now()
                    .duration_since(t)
                    .map(|d| d.as_secs() >= 2)
                    .unwrap_or(false)
            })
            .unwrap_or(true);
        if !settled {
            continue;
        }
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        let mut dest = media.join(&name);
        if dest.exists() {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "import".into());
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();
            let mut n = 2u32;
            loop {
                dest = media.join(format!("{stem}_{n}.{ext}"));
                if !dest.exists() {
                    break;
                }
                n += 1;
                if n > 999 {
                    break;
                }
            }
        }
        let ok = std::fs::rename(&path, &dest)
            .or_else(|_| std::fs::copy(&path, &dest).and_then(|_| std::fs::remove_file(&path)))
            .is_ok();
        if ok {
            moved += 1;
        } else {
            failed += 1;
        }
    }
    (moved, failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_sort_orders() {
        let mk = |name: &str, size: u64, secs: u64| MediaItem {
            path: PathBuf::from(name),
            name: name.to_string(),
            size_str: String::new(),
            size_bytes: size,
            category: MediaCategory::Screenshot,
            modified_secs: secs,
            dupe: false,
        };
        let mut v = vec![mk("b.png", 10, 100), mk("a.png", 50, 200)];
        LibrarySort::Name.apply(&mut v);
        assert_eq!(v[0].name, "a.png");
        LibrarySort::Largest.apply(&mut v);
        assert_eq!(v[0].size_bytes, 50);
        LibrarySort::Oldest.apply(&mut v);
        assert_eq!(v[0].modified_secs, 100);
        LibrarySort::Newest.apply(&mut v);
        assert_eq!(v[0].modified_secs, 200);
        assert!(LibrarySort::Newest.groups_by_date());
        assert!(!LibrarySort::Name.groups_by_date());
    }

    #[test]
    fn date_group_recent_is_today() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(date_group_label(now), "Today");
        assert_eq!(date_group_label(now.saturating_sub(100_000)), "Yesterday");
        assert_eq!(date_group_label(now.saturating_sub(400_000)), "This week");
        assert_eq!(date_group_label(now.saturating_sub(2_000_000)), "Earlier");
    }

    #[test]
    fn mark_duplicates_flags_identical_bytes_only() {
        let dir = std::env::temp_dir().join(format!("vibecap_dupe_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.png");
        let b = dir.join("b.png");
        let c = dir.join("c.png");
        let same = vec![9u8; 200_000];
        std::fs::write(&a, &same).unwrap();
        std::fs::write(&b, &same).unwrap();
        std::fs::write(&c, vec![7u8; 200_000]).unwrap();
        let mk = |p: &Path| MediaItem {
            path: p.to_path_buf(),
            name: p.file_name().unwrap().to_string_lossy().to_string(),
            size_str: String::new(),
            size_bytes: 200_000,
            category: MediaCategory::Screenshot,
            modified_secs: 0,
            dupe: false,
        };
        let mut v = vec![mk(&a), mk(&b), mk(&c)];
        mark_duplicates(&mut v);
        assert!(v[0].dupe && v[1].dupe);
        assert!(!v[2].dupe);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn retention_pick_days_and_count() {
        let mk = |secs: u64| MediaItem {
            path: PathBuf::from(format!("{secs}.png")),
            name: String::new(),
            size_str: String::new(),
            size_bytes: 0,
            category: MediaCategory::Screenshot,
            modified_secs: secs,
            dupe: false,
        };
        let now = 10_000_000u64;
        // a: 5 days old · b: 40 days old · c: 1 day old
        let v = vec![
            mk(now - 5 * 86_400),
            mk(now - 40 * 86_400),
            mk(now - 86_400),
        ];
        // Older-than-30-days → only b.
        assert_eq!(retention_pick(&v, 1, 30, now), vec![1]);
        // Keep newest 2 → only b (oldest) swept.
        assert_eq!(retention_pick(&v, 2, 2, now), vec![1]);
        // Keep newest 5 → nothing swept.
        assert!(retention_pick(&v, 2, 5, now).is_empty());
        // Off → nothing.
        assert!(retention_pick(&v, 0, 30, now).is_empty());
    }

    /// E167 — reclaimable counts the scratch/cache dirs recursively and
    /// `clean_reclaimable` frees them without touching media files.
    #[test]
    fn reclaimable_counts_and_cleans() {
        let dir = std::env::temp_dir().join(format!("vibecap_reclaim_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let live = dir.join("live").join("session-1");
        std::fs::create_dir_all(&live).unwrap();
        std::fs::write(live.join("f.raw"), vec![1u8; 1000]).unwrap();
        let thumbs = dir.join(".vibecap").join("thumbs");
        std::fs::create_dir_all(&thumbs).unwrap();
        std::fs::write(thumbs.join("t.jpg"), vec![2u8; 500]).unwrap();
        // A real media file beside them must survive.
        std::fs::write(dir.join("shot.png"), vec![3u8; 700]).unwrap();

        assert_eq!(reclaimable_bytes(&dir), 1500);
        assert_eq!(dir_tree_size(&dir), 2200);
        assert_eq!(clean_reclaimable(&dir), 1500);
        assert_eq!(reclaimable_bytes(&dir), 0);
        assert!(dir.join("shot.png").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// E211/E250 — the sweep moves settled media, suffixes collisions, and
    /// leaves non-media + fresh (in-flight) files alone.
    #[test]
    fn watch_sweep_moves_settled_media_only() {
        let root =
            std::env::temp_dir().join(format!("vibecap_watch_test_{}", std::process::id()));
        let watch = root.join("watch");
        let media = root.join("media");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&watch).unwrap();
        std::fs::create_dir_all(&media).unwrap();

        // Settled media file + a name-collision in media.
        let settled = watch.join("old.png");
        std::fs::write(&settled, b"img").unwrap();
        // Backdate mtime so it counts as settled (File::set_modified, 1.75+).
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(10);
        std::fs::File::options()
            .write(true)
            .open(&settled)
            .unwrap()
            .set_modified(old)
            .unwrap();
        std::fs::write(media.join("old.png"), b"existing").unwrap();

        std::fs::write(watch.join("notes.exe"), b"no").unwrap(); // non-media
        std::fs::write(watch.join("fresh.png"), b"new").unwrap(); // too fresh

        let (moved, failed) = watch_sweep(&watch, &media);
        assert_eq!((moved, failed), (1, 0));
        assert!(media.join("old_2.png").exists()); // collision → _2 suffix
        assert!(watch.join("notes.exe").exists()); // non-media stays
        assert!(watch.join("fresh.png").exists()); // unsettled stays
        let _ = std::fs::remove_dir_all(&root);
    }
}
