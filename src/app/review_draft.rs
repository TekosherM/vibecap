//! E222 — crash recovery for unsaved Review annotations.
//!
//! While the Still editor has strokes on the canvas we keep a debounced
//! draft on disk (`review_draft.json` in the config dir). A crash or kill
//! leaves it behind; on launch we restore the still + strokes into Review
//! (no tab switch) and clear the draft once the user exports, clears, or
//! loads a different still.
//!
//! `AnnotationAction` holds egui types (`Pos2`, `Color32`) and `Sticker`
//! holds a live `TextureHandle`, so the draft stores a plain-data mirror:
//! tool name, RGBA color, stroke, canvas-space points, text, badge number,
//! and stickers as base64 PNGs (pixels re-upload to a texture on restore).

use std::io::Cursor;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Pos2};
use serde::{Deserialize, Serialize};

use super::annotation_baker::{AnnotationAction, AnnotationTool, Sticker};
use super::io::{base64_encode, vibecap_config_dir, write_json_atomic};

#[derive(Serialize, Deserialize)]
pub struct ReviewDraft {
    pub still_path: String,
    pub saved_unix: i64,
    /// Canvas rect the points were drawn against — lets
    /// `sync_annotation_canvas` remap into the new layout's rect.
    #[serde(default)]
    pub canvas_rect: Option<[f32; 4]>,
    pub actions: Vec<DraftAction>,
}

#[derive(Serialize, Deserialize)]
pub struct DraftAction {
    pub tool: String,
    pub color: [u8; 4],
    pub stroke_width: f32,
    pub points: Vec<[f32; 2]>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub badge: usize,
    #[serde(default)]
    pub sticker_png: Option<String>,
}

pub fn review_draft_path() -> PathBuf {
    vibecap_config_dir().join("review_draft.json")
}

fn tool_name(t: &AnnotationTool) -> &'static str {
    match t {
        AnnotationTool::Pen => "pen",
        AnnotationTool::Arrow => "arrow",
        AnnotationTool::Rectangle => "rectangle",
        AnnotationTool::Ellipse => "ellipse",
        AnnotationTool::Highlight => "highlight",
        AnnotationTool::Text => "text",
        AnnotationTool::Blur => "blur",
        AnnotationTool::StepBadge => "step_badge",
        AnnotationTool::Spotlight => "spotlight",
        AnnotationTool::Measure => "measure",
        AnnotationTool::Sticker => "sticker",
    }
}

fn tool_from_name(s: &str) -> Option<AnnotationTool> {
    Some(match s {
        "pen" => AnnotationTool::Pen,
        "arrow" => AnnotationTool::Arrow,
        "rectangle" => AnnotationTool::Rectangle,
        "ellipse" => AnnotationTool::Ellipse,
        "highlight" => AnnotationTool::Highlight,
        "text" => AnnotationTool::Text,
        "blur" => AnnotationTool::Blur,
        "step_badge" => AnnotationTool::StepBadge,
        "spotlight" => AnnotationTool::Spotlight,
        "measure" => AnnotationTool::Measure,
        "sticker" => AnnotationTool::Sticker,
        _ => return None,
    })
}

fn sticker_png(sticker: &Sticker) -> Option<String> {
    let img = image::DynamicImage::ImageRgba8((*sticker.rgba).clone());
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .ok()?;
    Some(base64_encode(&buf))
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut nibbles = Vec::with_capacity(s.len());
    for b in s.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => continue,
            _ => return None,
        };
        nibbles.push(v);
    }
    let mut out = Vec::with_capacity(nibbles.len() * 6 / 8);
    for chunk in nibbles.chunks(4) {
        let n: u32 = chunk.iter().fold(0u32, |acc, &v| (acc << 6) | v as u32);
        match chunk.len() {
            4 => out.extend_from_slice(&[(n >> 16) as u8, (n >> 8) as u8, n as u8]),
            3 => out.extend_from_slice(&[(n >> 10) as u8, (n >> 2) as u8]),
            2 => out.push((n >> 4) as u8),
            _ => {}
        }
    }
    Some(out)
}

fn action_to_draft(a: &AnnotationAction) -> DraftAction {
    DraftAction {
        tool: tool_name(&a.tool).to_string(),
        color: a.color.to_array(),
        stroke_width: a.stroke_width,
        points: a.points.iter().map(|p| [p.x, p.y]).collect(),
        text: a.text_content.clone(),
        badge: a.badge_number,
        sticker_png: a.sticker.as_ref().and_then(sticker_png),
    }
}

fn action_from_draft(ctx: &egui::Context, d: &DraftAction) -> Option<AnnotationAction> {
    let tool = tool_from_name(&d.tool)?;
    let sticker = d.sticker_png.as_ref().and_then(|b64| {
        let bytes = base64_decode(b64)?;
        let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let ci = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
        let tex = ctx.load_texture("sticker_draft", ci, egui::TextureOptions::LINEAR);
        Some(Sticker {
            rgba: std::sync::Arc::new(img),
            tex,
        })
    });
    Some(AnnotationAction {
        tool,
        color: Color32::from_rgba_unmultiplied(d.color[0], d.color[1], d.color[2], d.color[3]),
        stroke_width: d.stroke_width,
        points: d.points.iter().map(|[x, y]| Pos2::new(*x, *y)).collect(),
        text_content: d.text.clone(),
        badge_number: d.badge,
        sticker,
    })
}

/// Cheap fingerprint of the live action list — the debounce key. Covers
/// tool, color, stroke, point coords, text and badge so undo/redo,
/// reorder, drag-mid-stroke and deletes all register as a change.
pub fn actions_fingerprint(actions: &[AnnotationAction]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    actions.len().hash(&mut h);
    for a in actions {
        tool_name(&a.tool).hash(&mut h);
        a.color.to_array().hash(&mut h);
        a.stroke_width.to_bits().hash(&mut h);
        a.badge_number.hash(&mut h);
        a.text_content.hash(&mut h);
        for p in &a.points {
            p.x.to_bits().hash(&mut h);
            p.y.to_bits().hash(&mut h);
        }
        a.sticker.is_some().hash(&mut h);
    }
    h.finish()
}

/// Persist the draft (empty action list clears the file instead).
pub fn write_review_draft(
    still_path: &Path,
    canvas_rect: Option<egui::Rect>,
    actions: &[AnnotationAction],
) {
    if actions.is_empty() {
        clear_review_draft();
        return;
    }
    let draft = ReviewDraft {
        still_path: still_path.to_string_lossy().to_string(),
        saved_unix: chrono::Utc::now().timestamp(),
        canvas_rect: canvas_rect.map(|r| [r.min.x, r.min.y, r.max.x, r.max.y]),
        actions: actions.iter().map(action_to_draft).collect(),
    };
    if let Ok(json) = serde_json::to_string(&draft) {
        let _ = write_json_atomic(&review_draft_path(), &json);
    }
}

pub fn clear_review_draft() {
    let _ = std::fs::remove_file(review_draft_path());
}

/// Draft present on disk? Read but do not consume — the next debounced
/// write or an explicit clear decides its fate.
pub fn read_review_draft() -> Option<ReviewDraft> {
    let raw = std::fs::read_to_string(review_draft_path()).ok()?;
    let draft: ReviewDraft = serde_json::from_str(&raw).ok()?;
    if draft.actions.is_empty() {
        return None;
    }
    Some(draft)
}

/// Hydrate draft actions into live `AnnotationAction`s (uploads sticker
/// textures). Unknown tools drop out rather than failing the restore.
pub fn actions_from_draft(ctx: &egui::Context, draft: &ReviewDraft) -> Vec<AnnotationAction> {
    draft
        .actions
        .iter()
        .filter_map(|d| action_from_draft(ctx, d))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip() {
        for raw in [&b""[..], b"f", b"fo", b"foo", b"hello world"] {
            let enc = base64_encode(raw);
            assert_eq!(base64_decode(&enc).as_deref(), Some(raw));
        }
    }

    #[test]
    fn fingerprint_tracks_mutations() {
        let mk = |pts: &[(f32, f32)]| AnnotationAction {
            tool: AnnotationTool::Arrow,
            color: Color32::RED,
            stroke_width: 2.0,
            points: pts.iter().map(|&(x, y)| Pos2::new(x, y)).collect(),
            text_content: String::new(),
            badge_number: 0,
            sticker: None,
        };
        let a = vec![mk(&[(0.0, 0.0), (10.0, 10.0)])];
        let b = vec![mk(&[(0.0, 0.0), (10.0, 11.0)])];
        assert_ne!(actions_fingerprint(&a), actions_fingerprint(&b));
        assert_eq!(actions_fingerprint(&a), actions_fingerprint(&a));
    }
}
