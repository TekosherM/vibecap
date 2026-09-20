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
            });
        }
    }
    items.sort_by(|a, b| {
        b.modified_secs
            .cmp(&a.modified_secs)
            .then_with(|| b.name.cmp(&a.name))
    });
    items
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
}
