//! Application core shared by the desktop GUI and `vibecap --mcp`.
//!
//! Keep capture OS details in `platform/`; keep paint in `ui/`.

pub mod agent_record;
pub mod annotation_baker;
pub mod budget;
pub mod capture_flow;
pub mod cli;
pub mod doctor;
pub mod feedback;
pub mod instance;
pub mod io;
pub mod library;
pub mod live;
pub mod mcp;
pub mod naming;
pub mod paths;
pub mod recording;
pub mod retro;
pub mod session;
pub mod thumbs;
pub mod update;
pub mod zip;

pub use annotation_baker::{
    bake_annotations, renumber_step_badges, snap_annotation_point, straighten_if_near_line,
    AnnotationAction, AnnotationTool,
};

pub use budget::{
    budget_exceeded_reason, live_usage_snapshot, load_budget, save_budget, BudgetConfig,
};
pub use cli::{parse_args, run_headless, CliAction};
pub use doctor::doctor_text;
pub use feedback::{
    feedback_last_poll_secs, feedback_requests_dir, feedback_responses_dir, format_feedback_answer,
    touch_feedback_poll, FeedbackRequest, FeedbackResponse,
};
pub use io::{
    data_uri, file_uri, take_pending_still, write_json_atomic, write_pending_still,
    write_pending_still_error,
};
pub use library::{
    category_bytes, date_group_label, filter_items, get_dir_size_bytes, retention_pick,
    scan_media_dir, LibrarySort, LoopPosition, MediaCategory, MediaItem, LIBRARY_PAGE_SIZE,
};
pub use mcp::{mcp_tool_count, run_mcp_server, MCP_TOOL_NAMES};
pub use naming::{format_capture_stem, DEFAULT_PATTERN};
pub use paths::{default_live_dir, default_media_dir, mcp_live_dir};
pub use recording::{extract_filmstrip_rgba, finalize_recorder, kill_recorder};
pub use retro::RetroController;
pub use zip::write_zip;
