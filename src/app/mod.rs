//! Application core shared by the desktop GUI and `vibecap --mcp`.
//!
//! Keep capture OS details in `platform/`; keep paint in `ui/`.

pub mod budget;
pub mod feedback;
pub mod io;
pub mod library;
pub mod live;
pub mod mcp;
pub mod paths;
pub mod recording;
pub mod retro;
pub mod session;

pub use budget::{
    budget_exceeded_reason, budget_file_path, budget_status_line, live_usage_snapshot, load_budget,
    save_budget, BudgetConfig,
};
pub use feedback::{
    feedback_requests_dir, feedback_responses_dir, format_feedback_answer, FeedbackRequest,
    FeedbackResponse,
};
pub use io::write_json_atomic;
pub use library::{
    filter_items, get_dir_size_bytes, scan_media_dir, LoopPosition, MediaCategory, MediaItem,
    LIBRARY_PAGE_SIZE,
};
pub use live::{
    get_budget_note_mutex, get_latest_live_gif_mutex, get_live_started_mutex,
    LIVE_INSPECTION_RUNNING,
};
pub use mcp::run_mcp_server;
pub use paths::{
    capture_screenshot_to_media_dir, default_live_dir, default_media_dir, mcp_live_dir,
};
pub use recording::{
    even_crop, extract_filmstrip_thumbs, finalize_recorder, kill_recorder,
};
pub use retro::{
    dump_retro_disk_gif, set_retro_enabled, RetroConfig, RetroController,
};
