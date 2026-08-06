//! Media library model + directory scan (no UI).

use std::path::{Path, PathBuf};

/// Max rows shown at once in the library (newest first). User can load more.
pub const LIBRARY_PAGE_SIZE: usize = 40;

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
