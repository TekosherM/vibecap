//! Human-in-the-loop feedback request/response files (shared by GUI + MCP).

use std::path::PathBuf;

use super::io::vibecap_config_dir;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FeedbackRequest {
    pub id: String,
    /// Absolute path to image/gif/video to review. Empty = text-only / decision request.
    #[serde(default)]
    pub media_path: String,
    pub question: String,
    pub created_at: String,
    /// "pending" | "answered" | "cancelled" | "dismissed"
    pub status: String,
    /// "low" | "normal" | "high"
    #[serde(default = "default_feedback_priority")]
    pub priority: String,
    /// Who asked (e.g. "codex", "claude", "cursor") — shown in Agent Inbox.
    #[serde(default)]
    pub agent_label: String,
    /// Optional multiple-choice chips (approve/reject, A/B, pick one).
    #[serde(default)]
    pub options: Vec<String>,
    /// Hint for the human: "any" | "text" | "annotate" | "voice" | "choice"
    #[serde(default = "default_preferred_reply")]
    pub preferred_reply: String,
    /// Extra agent context (what they already tried, constraints, etc.).
    #[serde(default)]
    pub context: String,
}

fn default_feedback_priority() -> String {
    "normal".to_string()
}
fn default_preferred_reply() -> String {
    "any".to_string()
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FeedbackResponse {
    pub id: String,
    pub feedback_text: String,
    #[serde(default)]
    pub voice_note_path: String,
    #[serde(default)]
    pub annotated_media_path: String,
    pub answered_at: String,
    /// When the human picked a quick option (or agent/human cancel/dismiss).
    #[serde(default)]
    pub selected_option: String,
}

/// Format a durable response for agents (handles annotate-only / choice-only / empty text).
pub fn format_feedback_answer(request_id: &str, resp: &FeedbackResponse) -> String {
    let mut lines = vec![format!("✅ status=answered request_id={}", request_id)];
    if !resp.selected_option.is_empty() {
        lines.push(format!("choice: {}", resp.selected_option));
    }
    let text = resp.feedback_text.trim();
    if !text.is_empty() {
        lines.push(format!("text:\n\"{}\"", text));
    } else if !resp.annotated_media_path.is_empty()
        || !resp.voice_note_path.is_empty()
        || !resp.selected_option.is_empty()
    {
        lines.push(
            "text: (none — human answered via choice / annotation / voice; see below)".to_string(),
        );
    } else {
        lines.push("text: (empty)".to_string());
    }
    if !resp.annotated_media_path.is_empty() {
        lines.push(format!("🎨 annotated_media: {}", resp.annotated_media_path));
        lines.push(
            "→ Open this path with vision; drawings are the primary answer when text is empty."
                .to_string(),
        );
    }
    if !resp.voice_note_path.is_empty() {
        lines.push(format!("🎙 voice_note: {}", resp.voice_note_path));
    }
    lines.push(format!("answered_at: {}", resp.answered_at));
    lines.join("\n")
}

pub fn feedback_requests_dir() -> PathBuf {
    let dir = vibecap_config_dir().join("feedback").join("requests");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn feedback_responses_dir() -> PathBuf {
    let dir = vibecap_config_dir().join("feedback").join("responses");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

