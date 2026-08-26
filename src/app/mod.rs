//! Application core shared by the desktop GUI and `vibecap --mcp`.
//!
//! Keep capture OS details in `platform/`; keep paint in `ui/`.

pub mod agent_record;
pub mod annotation_baker;
pub mod budget;
pub mod cli;
pub mod feedback;
pub mod io;
pub mod library;
pub mod live;
pub mod mcp;
pub mod paths;
pub mod recording;
pub mod retro;
pub mod session;

pub use annotation_baker::{bake_annotations, AnnotationAction, AnnotationTool};

pub use budget::{
    budget_exceeded_reason, live_usage_snapshot, load_budget, save_budget, BudgetConfig,
};
pub use feedback::{
    feedback_requests_dir, feedback_responses_dir, format_feedback_answer, FeedbackRequest,
    FeedbackResponse,
};
pub use io::{
    take_pending_still, write_json_atomic, write_pending_still, write_pending_still_error,
};
pub use library::{
    filter_items, get_dir_size_bytes, scan_media_dir, LoopPosition, MediaCategory, MediaItem,
    LIBRARY_PAGE_SIZE,
};
pub use cli::{parse_args, run_headless, CliAction};
pub use mcp::run_mcp_server;
pub use paths::{
    default_live_dir, default_media_dir, mcp_live_dir,
};
pub use recording::{
    even_crop, extract_filmstrip_thumbs, finalize_recorder, kill_recorder,
};
pub use retro::RetroController;
