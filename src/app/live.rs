//! Process-local live-inspection state (shared by GUI + MCP).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// Whether an MCP live-inspection stream is active in this process.
pub static LIVE_INSPECTION_RUNNING: AtomicBool = AtomicBool::new(false);

static LATEST_LIVE_GIF: OnceLock<Mutex<String>> = OnceLock::new();
static LIVE_STARTED_AT: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
static BUDGET_NOTE: OnceLock<Mutex<String>> = OnceLock::new();

pub fn get_latest_live_gif_mutex() -> &'static Mutex<String> {
    LATEST_LIVE_GIF.get_or_init(|| Mutex::new(String::new()))
}

pub fn get_live_started_mutex() -> &'static Mutex<Option<Instant>> {
    LIVE_STARTED_AT.get_or_init(|| Mutex::new(None))
}

pub fn get_budget_note_mutex() -> &'static Mutex<String> {
    BUDGET_NOTE.get_or_init(|| Mutex::new(String::new()))
}

pub fn is_live_running() -> bool {
    LIVE_INSPECTION_RUNNING.load(Ordering::SeqCst)
}

pub fn set_live_running(v: bool) {
    LIVE_INSPECTION_RUNNING.store(v, Ordering::SeqCst);
}
