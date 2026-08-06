//! Media / live path helpers used by GUI and CLI.

use std::path::PathBuf;

use crate::platform::{capture_to_media_dir, live_dir as platform_live_dir, live_session_dir, media_dir};

pub fn default_media_dir() -> PathBuf {
    media_dir()
}

pub fn default_live_dir() -> PathBuf {
    platform_live_dir()
}

/// MCP / agent live stream dir — unique per process so multiple agents can run at once.
pub fn mcp_live_dir() -> PathBuf {
    live_session_dir()
}

pub fn capture_screenshot_to_media_dir() -> Result<PathBuf, String> {
    capture_to_media_dir()
}
