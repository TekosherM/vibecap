//! MCP stdio JSON-RPC server (`vibecap --mcp`).

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use chrono::Local;

use crate::app::budget::{
    budget_exceeded_reason, budget_status_line, live_usage_snapshot, load_budget, save_budget,
};
use crate::app::feedback::{
    feedback_requests_dir, feedback_responses_dir, format_feedback_answer, FeedbackRequest,
    FeedbackResponse,
};
use crate::app::io::write_json_atomic;
use crate::app::live::{
    get_budget_note_mutex, get_latest_live_gif_mutex, get_live_started_mutex, LIVE_INSPECTION_RUNNING,
};
use crate::app::retro::{dump_retro_disk_gif, retro_runtime_note, set_retro_enabled};
use crate::app::agent_record::{
    record_status_line, start_agent_record, stop_agent_record,
};
use crate::platform::{
    capture_live_frame, capture_to_dir, export_gif_clip, focus_app, list_running_apps,
    live_session_dir, media_dir, record_screen_clip_opts, resolve_output_dir, CaptureOpts,
    LiveFormat,
};

fn mcp_live_dir() -> PathBuf {
    live_session_dir()
}

fn default_media_dir() -> PathBuf {
    media_dir()
}

fn json_str<'a>(args: Option<&'a serde_json::Value>, key: &str) -> Option<&'a str> {
    args.and_then(|a| a.get(key)).and_then(|s| s.as_str()).map(str::trim).filter(|s| !s.is_empty())
}

fn capture_opts_from(args: Option<&serde_json::Value>) -> CaptureOpts {
    let display = json_str(args, "display").map(|s| s.to_string());
    let window = json_str(args, "window")
        .or_else(|| json_str(args, "app_name"))
        .map(|s| s.to_string());
    CaptureOpts::from_parts(display, window)
}

fn output_dir_from(args: Option<&serde_json::Value>) -> PathBuf {
    resolve_output_dir(json_str(args, "output_dir").map(PathBuf::from).as_deref())
}

fn get_dir_size_bytes(dir_path: &str) -> (u64, usize) {
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

pub fn run_mcp_server() {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() { continue; }

        let parsed: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = parsed.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = parsed.get("id").cloned();

        match method {
            "initialize" => {
                let version = env!("CARGO_PKG_VERSION");
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "vibecap",
                            "version": version
                        }
                    }
                });
                let _ = writeln!(handle, "{}", response.to_string());
                let _ = handle.flush();
            }
            "notifications/initialized" => {}
            "ping" => {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {}
                });
                let _ = writeln!(handle, "{}", response.to_string());
                let _ = handle.flush();
            }
            "tools/list" => {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": [
                            {
                                "name": "vibecap_capture",
                                "description": "Still of the target display or focused window (Linux: ffmpeg x11grab — the real screen, not the web studio shutter). Writes JPEG to output_dir or the default media dir. Prints the path.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "app_name": {
                                            "type": "string",
                                            "description": "Focus this app first (alias of window)"
                                        },
                                        "window": {
                                            "type": "string",
                                            "description": "Window title to focus and, on Linux, crop to"
                                        },
                                        "display": {
                                            "type": "string",
                                            "description": "X11 DISPLAY to grab (e.g. :0 or :1). Defaults to $DISPLAY"
                                        },
                                        "output_dir": {
                                            "type": "string",
                                            "description": "Directory for the JPEG. Default: platform Videos/Vibecap (see vibecap --paths)"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "vibecap_record_start",
                                "description": "Start unbounded MP4 recording of the target display/window. Drive the flow, then call vibecap_record_stop. Linux backend is ffmpeg x11grab.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "app_name": { "type": "string", "description": "Focus this app first" },
                                        "window": { "type": "string", "description": "Window title to crop to (Linux)" },
                                        "display": { "type": "string", "description": "X11 DISPLAY (e.g. :0)" },
                                        "output_dir": { "type": "string", "description": "Directory for the MP4" },
                                        "gif": { "type": "boolean", "description": "Also write a companion GIF on stop" }
                                    }
                                }
                            },
                            {
                                "name": "vibecap_record_stop",
                                "description": "Stop the unbounded recording started by vibecap_record_start (or record_video with no duration). Writes MP4 to the caller output_dir. Optional GIF.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "gif": { "type": "boolean", "description": "Export a companion GIF of the whole clip" }
                                    }
                                }
                            },
                            {
                                "name": "vibecap_record_status",
                                "description": "Whether an unbounded agent recording is live, plus pid and MP4 path.",
                                "inputSchema": { "type": "object", "properties": {} }
                            },
                            {
                                "name": "vibecap_record_video",
                                "description": "Short clip when duration_secs is set (max 600, companion GIF). Omit duration_secs to start unbounded recording — then vibecap_record_stop. Prefer start/stop for long signed-in flows.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "app_name": {
                                            "type": "string",
                                            "description": "Optional application name to focus before recording"
                                        },
                                        "window": { "type": "string", "description": "Window title (Linux crop)" },
                                        "display": { "type": "string", "description": "X11 DISPLAY" },
                                        "output_dir": { "type": "string", "description": "Directory for MP4/GIF" },
                                        "duration_secs": {
                                            "type": "number",
                                            "description": "If set, record this many seconds (max 600) then stop. Omit to start unbounded."
                                        },
                                        "gif": { "type": "boolean", "description": "Companion GIF (default true for timed clips)" }
                                    }
                                }
                            },
                            {
                                "name": "vibecap_export_gif",
                                "description": "Extracts high-FPS GIF around start/end timeline timestamps",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "video_path": {
                                            "type": "string",
                                            "description": "Path to input video file"
                                        },
                                        "start_time": {
                                            "type": "string",
                                            "description": "Start timestamp (HH:MM:SS)"
                                        },
                                        "end_time": {
                                            "type": "string",
                                            "description": "End timestamp (HH:MM:SS)"
                                        }
                                    },
                                    "required": ["video_path", "start_time", "end_time"]
                                }
                            },
                            {
                                "name": "vibecap_start_live_inspection",
                                "description": "Starts continuous background live inspection recording emitting rolling frames (gif, jpg, or mp4) every N seconds into a repo temp directory so AI agent can inspect user actions live while keeping user aware of disk storage usage.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "app_name": {
                                            "type": "string",
                                            "description": "Optional application name to focus before starting live stream (e.g. Google Chrome, iTerm, Simulator)"
                                        },
                                        "format": {
                                            "type": "string",
                                            "description": "Media format to emit: 'gif' (animated clip, default), 'jpg' (fast screenshot), or 'mp4' (video chunk)"
                                        },
                                        "interval_secs": {
                                            "type": "number",
                                            "description": "Frequency/interval in seconds between live frame emissions (default: 3)"
                                        },
                                        "output_dir": {
                                            "type": "string",
                                            "description": "Target output directory (default: platform media folder /live, e.g. Videos/Vibecap/live)"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "vibecap_get_live_frame",
                                "description": "Fetches the file path of the latest live emitted frame along with current session disk storage usage",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {}
                                }
                            },
                            {
                                "name": "vibecap_stop_live_inspection",
                                "description": "Stops the active continuous background live inspection stream and reports final disk storage summary",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {}
                                }
                            },
                            {
                                "name": "vibecap_set_budget",
                                "description": "Sets agent spending controls for frame/media analysis: caps on frames captured, storage MB, and session minutes, plus an analysis tier (eco/standard/intensive). Intensive frame analysis can be expensive — use eco when exploring. Live inspection auto-stops and new streams are refused once a cap is reached. Shared with the Vibecap app (Settings → Agent Session & Budget).",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "max_frames": {
                                            "type": "number",
                                            "description": "Maximum frames the live session may capture before auto-stop (0 = unlimited)"
                                        },
                                        "max_mb": {
                                            "type": "number",
                                            "description": "Maximum live-session storage in megabytes before auto-stop (0 = unlimited)"
                                        },
                                        "max_minutes": {
                                            "type": "number",
                                            "description": "Maximum live-session minutes before auto-stop (0 = unlimited)"
                                        },
                                        "analysis_tier": {
                                            "type": "string",
                                            "enum": ["eco", "standard", "intensive"],
                                            "description": "eco = jpg @ >=5s intervals (cheapest), standard = gif @ ~3s (balanced), intensive = gif/mp4 @ 1s (richest, most expensive frame analysis)"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "vibecap_get_spending",
                                "description": "Reports current session spending: frames captured, storage MB, elapsed minutes, the active caps, analysis tier, and whether the budget is exhausted. Call before and during intensive analysis to control costs.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {}
                                }
                            },
                            {
                                "name": "vibecap_request_feedback",
                                "description": "Queue a human-in-the-loop question in the Vibecap Agent Inbox. Human answers with text, choice chips, voice, and/or image mark-up. This does NOT push into chat — you MUST poll vibecap_get_feedback until status is not pending. media_path optional for decision-only questions.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "media_path": {
                                            "type": "string",
                                            "description": "Absolute path to image/GIF/video to review. Omit or empty for text-only decisions (permission, prefer A/B without attachment)."
                                        },
                                        "question": {
                                            "type": "string",
                                            "description": "Specific question (e.g. 'Does this animation look right?' or 'OK to delete these files?')"
                                        },
                                        "priority": {
                                            "type": "string",
                                            "enum": ["low", "normal", "high"],
                                            "description": "Inbox sort weight. high surfaces first (default normal)."
                                        },
                                        "agent_label": {
                                            "type": "string",
                                            "description": "Who is asking — shown in inbox (e.g. codex, claude, cursor, grok)."
                                        },
                                        "options": {
                                            "type": "array",
                                            "items": { "type": "string" },
                                            "description": "Optional quick-choice chips (max 8), e.g. [\"approve\",\"reject\"] or [\"A\",\"B\",\"neither\"]."
                                        },
                                        "preferred_reply": {
                                            "type": "string",
                                            "enum": ["any", "text", "annotate", "voice", "choice"],
                                            "description": "Hint for how the human should answer (default any)."
                                        },
                                        "context": {
                                            "type": "string",
                                            "description": "Extra agent notes: what you tried, constraints, before/after paths, etc."
                                        }
                                    },
                                    "required": ["question"]
                                }
                            },
                            {
                                "name": "vibecap_get_feedback",
                                "description": "Poll human answer for a request_id. Returns pending until answered/cancelled/dismissed. On answer: text, choice, annotated_media path, and/or voice_note. Annotate-only replies may have empty text — open annotated_media with vision.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "request_id": {
                                            "type": "string",
                                            "description": "The request_id returned by vibecap_request_feedback"
                                        }
                                    },
                                    "required": ["request_id"]
                                }
                            },
                            {
                                "name": "vibecap_list_feedback",
                                "description": "List feedback requests (pending/closed). Use after session restart to recover request_ids, or to see inbox state.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "status": {
                                            "type": "string",
                                            "enum": ["all", "pending", "closed"],
                                            "description": "Filter: all (default), pending only, or closed (answered/cancelled/dismissed)."
                                        }
                                    }
                                }
                            },
                            {
                                "name": "vibecap_cancel_feedback",
                                "description": "Cancel a pending feedback request you no longer need. Human inbox updates; get_feedback returns cancelled.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "request_id": {
                                            "type": "string",
                                            "description": "Pending request_id to cancel"
                                        }
                                    },
                                    "required": ["request_id"]
                                }
                            },
                            {
                                "name": "vibecap_list_apps",
                                "description": "List running application names for window-focus targets (use with vibecap_capture app_name or the desktop Window picker).",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {}
                                }
                            },
                            {
                                "name": "vibecap_set_retro",
                                "description": "Enable or disable the shared retro buffer (rolling low-FPS capture). Off by default. Writes retro.json (desktop app reloads within ~2s) and, when enabled, starts a capturer in this MCP process so frames accumulate even without the GUI. Disable clears frames. Cap ~200MB, ~2fps.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "enabled": {
                                            "type": "boolean",
                                            "description": "true to enable rolling capture, false to disable and clear frames"
                                        }
                                    },
                                    "required": ["enabled"]
                                }
                            },
                            {
                                "name": "vibecap_save_retro",
                                "description": "Export the shared retro ring buffer as a GIF into the media folder. Requires frames from a capturer (desktop Settings retro, or vibecap_set_retro enabled in this or another vibecap process).",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {}
                                }
                            },
                            {
                                "name": "vibecap_bug_report",
                                "description": "One-shot bug pack: full-screen screenshot plus retro GIF when frames exist. Optional app_name focuses first. Prefer vibecap_set_retro enabled=true before the user reproduces the bug.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "app_name": {
                                            "type": "string",
                                            "description": "Optional application to focus before the still"
                                        }
                                    }
                                }
                            }
                        ]
                    }
                });
                let _ = writeln!(handle, "{}", response.to_string());
                let _ = handle.flush();
            }
            "tools/call" => {
                let tool_name = parsed.get("params")
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");

                let (content_text, is_error) = match tool_name {
                    "vibecap_capture" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let opts = capture_opts_from(args);
                        let dir = output_dir_from(args);
                        if let Some(app) = opts.window.as_deref() {
                            let _ = focus_app(app);
                        }
                        match capture_to_dir(&dir, &opts) {
                            Ok(out) => (format!("Captured screenshot successfully to {}", out.display()), false),
                            Err(e) => (e, true),
                        }
                    }
                    "vibecap_record_start" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let opts = capture_opts_from(args);
                        let dir = output_dir_from(args);
                        let gif = args.and_then(|a| a.get("gif")).and_then(|v| v.as_bool()).unwrap_or(false);
                        let home_live = mcp_live_dir().display().to_string();
                        if let Some(reason) = budget_exceeded_reason(&home_live) {
                            (format!("⚠️ BUDGET EXHAUSTED — recording refused: {reason}"), true)
                        } else {
                            match start_agent_record(Some(&dir), &opts, gif) {
                                Ok(s) => (
                                    format!(
                                        "Recording started (unbounded). pid={} mp4={}\nCall vibecap_record_stop when the flow ends. Display={} window={}",
                                        s.pid,
                                        s.mp4,
                                        s.display.as_deref().unwrap_or("-"),
                                        s.window.as_deref().unwrap_or("-")
                                    ),
                                    false,
                                ),
                                Err(e) => (e, true),
                            }
                        }
                    }
                    "vibecap_record_stop" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let gif = args.and_then(|a| a.get("gif")).and_then(|v| v.as_bool()).unwrap_or(false);
                        match stop_agent_record(gif) {
                            Ok((s, gif_path)) => {
                                let bytes = std::fs::metadata(s.mp4_path()).map(|m| m.len()).unwrap_or(0);
                                let gif_note = gif_path
                                    .map(|p| format!(" gif={}", p.display()))
                                    .unwrap_or_default();
                                (
                                    format!(
                                        "Recording stopped. mp4={} bytes={}{}",
                                        s.mp4, bytes, gif_note
                                    ),
                                    false,
                                )
                            }
                            Err(e) => (e, true),
                        }
                    }
                    "vibecap_record_status" => (record_status_line(), false),
                    "vibecap_record_video" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let opts = capture_opts_from(args);
                        let dir = output_dir_from(args);
                        let duration_given = args.and_then(|a| a.get("duration_secs")).and_then(|v| v.as_u64());
                        let home_live = mcp_live_dir().display().to_string();
                        if let Some(reason) = budget_exceeded_reason(&home_live) {
                            (format!("⚠️ BUDGET EXHAUSTED — recording refused: {}. Raise caps with vibecap_set_budget or ask the human to adjust them in the Vibecap app.", reason), true)
                        } else if duration_given.is_none() || duration_given == Some(0) {
                            let gif = args.and_then(|a| a.get("gif")).and_then(|v| v.as_bool()).unwrap_or(false);
                            match start_agent_record(Some(&dir), &opts, gif) {
                                Ok(s) => (
                                    format!(
                                        "No duration_secs — started unbounded recording pid={} mp4={}. Call vibecap_record_stop when done.",
                                        s.pid, s.mp4
                                    ),
                                    false,
                                ),
                                Err(e) => (e, true),
                            }
                        } else {
                        let raw_duration = duration_given.unwrap_or(5);
                        let duration_secs = raw_duration.min(600);
                        let clamp_note = if raw_duration > 600 { " (clamped from your request — 600s max per clip)" } else { "" };
                        if let Some(app) = opts.window.as_deref() {
                            let _ = focus_app(app);
                        }

                        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                        let pid = std::process::id();
                        let out_mp4 = dir.join(format!("video_{}_{}.mp4", timestamp, pid));
                        let out_gif = dir.join(format!("video_{}_{}_clip.gif", timestamp, pid));
                        let want_gif = args.and_then(|a| a.get("gif")).and_then(|v| v.as_bool()).unwrap_or(true);

                        match record_screen_clip_opts(&out_mp4, duration_secs, &opts) {
                            Ok(()) => {
                                let gif_s = out_gif.display().to_string();
                                let mp4_s = out_mp4.display().to_string();
                                if want_gif {
                                    let _ = export_gif_clip(&mp4_s, "00:00:00", "99:00:00", &gif_s);
                                }
                                let gif_note = if want_gif {
                                    format!(" and exported GIF to {}", gif_s)
                                } else {
                                    String::new()
                                };
                                (format!("Successfully recorded {}s video to {}{}{}", duration_secs, mp4_s, gif_note, clamp_note), false)
                            }
                            Err(e) => (format!("Failed to record video: {}", e), true),
                        }
                        }
                    }
                    "vibecap_export_gif" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let video_path = args.and_then(|a| a.get("video_path")).and_then(|s| s.as_str()).unwrap_or("");
                        let start_time = args.and_then(|a| a.get("start_time")).and_then(|s| s.as_str()).unwrap_or("00:00:00");
                        let end_time = args.and_then(|a| a.get("end_time")).and_then(|s| s.as_str()).unwrap_or("00:00:05");

                        let gif_out = format!("{}_clip.gif", video_path.trim_end_matches(".mp4"));
                        match export_gif_clip(video_path, start_time, end_time, &gif_out) {
                            Ok(()) => (format!("Exported timeline GIF to {}", gif_out), false),
                            Err(e) => (format!("Failed to export GIF snippet: {}", e), true),
                        }
                    }
                    "vibecap_start_live_inspection" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let app_name = args.and_then(|a| a.get("app_name")).and_then(|s| s.as_str()).map(|s| s.to_string());
                        // When format/interval are omitted, the analysis tier drives the defaults —
                        // this makes eco/standard/intensive mechanically real, not just advisory.
                        let budget_now = load_budget();
                        let format_choice = args.and_then(|a| a.get("format")).and_then(|s| s.as_str()).map(|s| s.to_lowercase())
                            .unwrap_or_else(|| if budget_now.analysis_tier == "eco" { "jpg".to_string() } else { "gif".to_string() });
                        let interval_secs = args.and_then(|a| a.get("interval_secs")).and_then(|v| v.as_u64())
                            .unwrap_or_else(|| match budget_now.analysis_tier.as_str() { "eco" => 5, "intensive" => 1, _ => 3 });
                        
                        // Per-process session dir so several MCP servers can stream at once.
                        let default_dir = mcp_live_dir().display().to_string();
                        let live_dir = args.and_then(|a| a.get("output_dir")).and_then(|s| s.as_str()).unwrap_or(&default_dir).to_string();

                        if LIVE_INSPECTION_RUNNING.load(Ordering::SeqCst) {
                            ("Live inspection is already running in this MCP process! Call vibecap_get_live_frame, or vibecap_stop_live_inspection. Other agent instances may run their own streams in parallel.".to_string(), false)
                        } else if let Some(reason) = budget_exceeded_reason(&live_dir) {
                            (format!("⚠️ BUDGET EXHAUSTED — live inspection refused: {}. Raise the caps with vibecap_set_budget, ask the human to adjust them in the Vibecap app (Settings → Agent Session & Budget), or clean up {}.", reason, live_dir), true)
                        } else {
                            LIVE_INSPECTION_RUNNING.store(true, Ordering::SeqCst);
                            if let Ok(mut l) = get_live_started_mutex().lock() { *l = Some(Instant::now()); }
                            if let Ok(mut n) = get_budget_note_mutex().lock() { n.clear(); }
                            let _ = std::fs::create_dir_all(&live_dir);

                            if let Some(app) = &app_name {
                                let _ = focus_app(app);
                            }

                            let dir_clone = live_dir.clone();
                            let fmt_clone = format_choice.clone();
                            std::thread::spawn(move || {
                                let live_fmt = LiveFormat::from_str_loose(&fmt_clone);
                                while LIVE_INSPECTION_RUNNING.load(Ordering::SeqCst) {
                                    // Budget enforcement: auto-stop the stream when any cap is reached.
                                    if let Some(reason) = budget_exceeded_reason(&dir_clone) {
                                        LIVE_INSPECTION_RUNNING.store(false, Ordering::SeqCst);
                                        if let Ok(mut l) = get_live_started_mutex().lock() { *l = None; }
                                        if let Ok(mut n) = get_budget_note_mutex().lock() {
                                            *n = format!("BUDGET_EXHAUSTED — auto-stopped: {}", reason);
                                        }
                                        break;
                                    }

                                    let (latest_frame, timestamped_frame) =
                                        match capture_live_frame(&dir_clone, live_fmt, interval_secs) {
                                            Ok(pair) => pair,
                                            Err(_) => (String::new(), String::new()),
                                        };

                                    if !timestamped_frame.is_empty() {
                                        if let Ok(mut lock) = get_latest_live_gif_mutex().lock() {
                                            *lock = format!("{}|{}|{}", fmt_clone, latest_frame, timestamped_frame);
                                        }
                                    }

                                    if live_fmt == LiveFormat::Jpg {
                                        std::thread::sleep(Duration::from_secs(interval_secs));
                                    }
                                }
                            });

                            (format!("Started live inspection (format: {}, frequency: {}s, output_dir: {}).\n⚠️ STORAGE AWARENESS: Live frames are being stored in {}. Remember to inform the user about storage usage and call vibecap_stop_live_inspection when done.\n{}", format_choice, interval_secs, live_dir, live_dir, budget_status_line(&live_dir)), false)
                        }
                    }
                    "vibecap_get_live_frame" => {
                        let is_running = LIVE_INSPECTION_RUNNING.load(Ordering::SeqCst);
                        let state = get_latest_live_gif_mutex().lock().map(|l| l.clone()).unwrap_or_default();
                        
                        let parts: Vec<&str> = state.split('|').collect();
                        let (fmt, latest_frame, ts_frame) = if parts.len() == 3 {
                            (parts[0], parts[1], parts[2])
                        } else {
                            ("unknown", "", "")
                        };

                        let default_dir = mcp_live_dir().display().to_string();
                        let target_dir = if !ts_frame.is_empty() {
                            std::path::Path::new(ts_frame).parent().and_then(|p| p.to_str()).unwrap_or(&default_dir)
                        } else {
                            &default_dir
                        };

                        let (bytes, count) = get_dir_size_bytes(target_dir);
                        let mb = (bytes as f64) / (1024.0 * 1024.0);

                        if !is_running && ts_frame.is_empty() {
                            ("Live inspection is not running in this MCP process. Call vibecap_start_live_inspection first.".to_string(), true)
                        } else {
                            (format!("Status: live_running={}, format={}, latest_frame={}, timestamped_frame={}\n📊 STORAGE AWARENESS: Total session storage used: {:.2} MB across {} frame files in {}\n{}", is_running, fmt, latest_frame, ts_frame, mb, count, target_dir, budget_status_line(target_dir)), false)
                        }
                    }
                    "vibecap_stop_live_inspection" => {
                        LIVE_INSPECTION_RUNNING.store(false, Ordering::SeqCst);
                        if let Ok(mut l) = get_live_started_mutex().lock() { *l = None; }
                        let state = get_latest_live_gif_mutex().lock().map(|l| l.clone()).unwrap_or_default();
                        let parts: Vec<&str> = state.split('|').collect();
                        let ts_frame = if parts.len() == 3 { parts[2] } else { "" };
                        
                        let default_dir = mcp_live_dir().display().to_string();
                        let target_dir = if !ts_frame.is_empty() {
                            std::path::Path::new(ts_frame).parent().and_then(|p| p.to_str()).unwrap_or(&default_dir)
                        } else {
                            &default_dir
                        };

                        let (bytes, count) = get_dir_size_bytes(target_dir);
                        let mb = (bytes as f64) / (1024.0 * 1024.0);

                        (format!("Stopped live inspection stream.\n📊 FINAL STORAGE SUMMARY: Captured {} frames occupying {:.2} MB in {}. Inform the user so they can review or clean up temporary storage if desired.\n{}", count, mb, target_dir, budget_status_line(target_dir)), false)
                    }
                    "vibecap_set_budget" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let mut cfg = load_budget();
                        let mut notes: Vec<String> = Vec::new();
                        let mut invalid = false;
                        if let Some(a) = args {
                            if let Some(v) = a.get("max_frames") {
                                match v.as_u64() {
                                    Some(n) => cfg.max_frames = u32::try_from(n).unwrap_or_else(|_| { notes.push("max_frames clamped to u32::MAX".to_string()); u32::MAX }),
                                    None => invalid = true,
                                }
                            }
                            if let Some(v) = a.get("max_mb") {
                                match v.as_f64() {
                                    Some(n) if n.is_finite() && n >= 0.0 => cfg.max_mb = n,
                                    _ => invalid = true,
                                }
                            }
                            if let Some(v) = a.get("max_minutes") {
                                match v.as_u64() {
                                    Some(n) => cfg.max_minutes = u32::try_from(n).unwrap_or_else(|_| { notes.push("max_minutes clamped to u32::MAX".to_string()); u32::MAX }),
                                    None => invalid = true,
                                }
                            }
                            if let Some(v) = a.get("analysis_tier") {
                                match v.as_str() {
                                    Some(t) => {
                                        let t = t.to_lowercase();
                                        if t == "eco" || t == "standard" || t == "intensive" { cfg.analysis_tier = t; } else { invalid = true; }
                                    }
                                    None => invalid = true,
                                }
                            }
                        }
                        if invalid {
                            ("Invalid budget arguments: analysis_tier must be eco|standard|intensive and caps must be non-negative numbers. Nothing was saved.".to_string(), true)
                        } else if let Err(e) = save_budget(&cfg) {
                            (format!("Failed to save budget: {}", e), true)
                        } else {
                            let tier_guidance = match cfg.analysis_tier.as_str() {
                                "eco" => "eco: defaults to format='jpg' at 5s intervals — fewest frames, cheapest analysis.",
                                "intensive" => "intensive: defaults to 1s gif/mp4 — richest motion detail, but frame analysis is EXPENSIVE. Poll vibecap_get_spending and downshift when exploring.",
                                _ => "standard: defaults to gif at ~3s intervals — balanced detail vs cost.",
                            };
                            let notes_txt = if notes.is_empty() { String::new() } else { format!("\n⚠️ {}", notes.join("; ")) };
                            (format!("Budget updated: max_frames={} (0=unlimited), max_mb={:.1} (0=unlimited), max_minutes={} (0=unlimited), analysis_tier={}.\n💡 TIER GUIDANCE: {}\nCaps are enforced live: the stream auto-stops and new streams are refused once a cap is hit. The same budget is visible to the human in the Vibecap app (Settings → Agent Session & Budget).{}", cfg.max_frames, cfg.max_mb, cfg.max_minutes, cfg.analysis_tier, tier_guidance, notes_txt), false)
                        }
                    }
                    "vibecap_get_spending" => {
                        let state = get_latest_live_gif_mutex().lock().map(|l| l.clone()).unwrap_or_default();
                        let parts: Vec<&str> = state.split('|').collect();
                        let ts_frame = if parts.len() == 3 { parts[2] } else { "" };
                        let default_dir = mcp_live_dir().display().to_string();
                        let target_dir = if !ts_frame.is_empty() {
                            std::path::Path::new(ts_frame).parent().and_then(|p| p.to_str()).unwrap_or(&default_dir)
                        } else {
                            &default_dir
                        };
                        let (frames, mb, minutes) = live_usage_snapshot(target_dir);
                        let cfg = load_budget();
                        let frames_cap = if cfg.max_frames == 0 { "unlimited".to_string() } else { cfg.max_frames.to_string() };
                        let mb_cap = if cfg.max_mb <= 0.0 { "unlimited".to_string() } else { format!("{:.1}", cfg.max_mb) };
                        let min_cap = if cfg.max_minutes == 0 { "unlimited".to_string() } else { cfg.max_minutes.to_string() };
                        let status = match budget_exceeded_reason(target_dir) {
                            Some(r) => format!("⚠️ BUDGET EXHAUSTED: {}", r),
                            None => "within budget".to_string(),
                        };
                        let tier_note = if cfg.analysis_tier == "intensive" { " — frame analysis at this tier is expensive; downshift to eco when just exploring" } else { "" };
                        (format!("📊 SESSION SPENDING\nFrames captured: {} (cap: {})\nStorage: {:.2} MB (cap: {})\nElapsed: {:.1} min (cap: {})\nAnalysis tier: {}{}\nStatus: {}", frames, frames_cap, mb, mb_cap, minutes, min_cap, cfg.analysis_tier, tier_note, status), false)
                    }
                    "vibecap_request_feedback" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let media_path = args
                            .and_then(|a| a.get("media_path"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("")
                            .trim();
                        let question = args
                            .and_then(|a| a.get("question"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("")
                            .trim();
                        let priority = args
                            .and_then(|a| a.get("priority"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("normal")
                            .trim()
                            .to_lowercase();
                        let priority = match priority.as_str() {
                            "low" | "high" | "normal" => priority,
                            _ => "normal".to_string(),
                        };
                        let agent_label = args
                            .and_then(|a| a.get("agent_label"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("")
                            .trim()
                            .chars()
                            .take(64)
                            .collect::<String>();
                        let preferred_reply = args
                            .and_then(|a| a.get("preferred_reply"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("any")
                            .trim()
                            .to_lowercase();
                        let preferred_reply = match preferred_reply.as_str() {
                            "text" | "annotate" | "voice" | "choice" | "any" => preferred_reply,
                            _ => "any".to_string(),
                        };
                        let context = args
                            .and_then(|a| a.get("context"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("")
                            .trim()
                            .chars()
                            .take(2000)
                            .collect::<String>();
                        let mut options: Vec<String> = args
                            .and_then(|a| a.get("options"))
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                                    .filter(|s| !s.is_empty())
                                    .take(8)
                                    .map(|s| s.chars().take(80).collect())
                                    .collect()
                            })
                            .unwrap_or_default();
                        // Also accept comma-separated string for simpler clients.
                        if options.is_empty() {
                            if let Some(s) = args
                                .and_then(|a| a.get("options"))
                                .and_then(|v| v.as_str())
                            {
                                options = s
                                    .split(',')
                                    .map(|p| p.trim().to_string())
                                    .filter(|p| !p.is_empty())
                                    .take(8)
                                    .map(|s| s.chars().take(80).collect())
                                    .collect();
                            }
                        }
                        if question.is_empty() {
                            ("Missing required argument: question".to_string(), true)
                        } else if question.len() > 2000 {
                            ("question too long (max 2000 chars)".to_string(), true)
                        } else if !media_path.is_empty() && !std::path::Path::new(media_path).exists()
                        {
                            (format!("media_path does not exist: {}", media_path), true)
                        } else {
                            let id = format!("fb_{}", Local::now().format("%Y%m%d_%H%M%S%3f"));
                            let req = FeedbackRequest {
                                id: id.clone(),
                                media_path: media_path.to_string(),
                                question: question.to_string(),
                                created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                                status: "pending".to_string(),
                                priority,
                                agent_label,
                                options: options.clone(),
                                preferred_reply,
                                context,
                            };
                            let req_path = feedback_requests_dir().join(format!("{}.json", id));
                            match serde_json::to_string_pretty(&req)
                                .map_err(|e| e.to_string())
                                .and_then(|s| write_json_atomic(&req_path, &s))
                            {
                                Ok(_) => {
                                    let media_note = if media_path.is_empty() {
                                        "text-only (no media)".to_string()
                                    } else {
                                        media_path.to_string()
                                    };
                                    let opts_note = if options.is_empty() {
                                        String::new()
                                    } else {
                                        format!("\nChoices offered: {}", options.join(" | "))
                                    };
                                    (format!(
                                        "Feedback request '{}' submitted for {}.\n\
                                         🧑 HUMAN-IN-THE-LOOP (poll required — answers are NOT pushed into chat):\n\
                                         1. Human answers in Vibecap → 🤖 Agent Inbox (text / choice / voice / mark-up)\n\
                                         2. Poll vibecap_get_feedback(request_id='{}') every few seconds until status≠pending\n\
                                         3. If text is empty but annotated_media is set, open that PNG with vision\n\
                                         4. Use vibecap_list_feedback if you lose the request_id; vibecap_cancel_feedback to abandon{}",
                                        id, media_note, id, opts_note
                                    ), false)
                                }
                                Err(e) => (format!("Failed to persist feedback request: {}", e), true),
                            }
                        }
                    }
                    "vibecap_get_feedback" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let request_id = args
                            .and_then(|a| a.get("request_id"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("");
                        if request_id.is_empty()
                            || !request_id
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                        {
                            (
                                "Invalid request_id (allowed: letters, digits, _ and -)".to_string(),
                                true,
                            )
                        } else {
                            let req_path =
                                feedback_requests_dir().join(format!("{}.json", request_id));
                            let req_status = std::fs::read_to_string(&req_path)
                                .ok()
                                .and_then(|s| serde_json::from_str::<FeedbackRequest>(&s).ok())
                                .map(|r| r.status);
                            let resp_path =
                                feedback_responses_dir().join(format!("{}.json", request_id));
                            if let Ok(s) = std::fs::read_to_string(&resp_path) {
                                match serde_json::from_str::<FeedbackResponse>(&s) {
                                    Ok(resp) => {
                                        let status = req_status
                                            .unwrap_or_else(|| "answered".to_string());
                                        if status == "cancelled" || resp.selected_option == "cancelled"
                                        {
                                            (
                                                format!(
                                                    "🚫 status=cancelled request_id={}\n(Agent withdrew this request.)",
                                                    request_id
                                                ),
                                                false,
                                            )
                                        } else if status == "dismissed"
                                            || resp.selected_option == "dismissed"
                                        {
                                            (
                                                format!(
                                                    "⏭ status=dismissed request_id={}\n(Human closed without a substantive answer.)",
                                                    request_id
                                                ),
                                                false,
                                            )
                                        } else {
                                            (format_feedback_answer(request_id, &resp), false)
                                        }
                                    }
                                    Err(_) => ("Corrupt feedback response file".to_string(), true),
                                }
                            } else if req_path.exists() {
                                (
                                    format!(
                                        "⏳ status=pending request_id={}\n\
                                         Human has not answered yet. Keep polling (e.g. every 2–5s).\n\
                                         They answer in Vibecap → 🤖 Agent Inbox — not via this chat.",
                                        request_id
                                    ),
                                    false,
                                )
                            } else {
                                (format!("Unknown request_id: {}", request_id), true)
                            }
                        }
                    }
                    "vibecap_list_feedback" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let filter = args
                            .and_then(|a| a.get("status"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("all")
                            .to_lowercase();
                        let mut rows: Vec<FeedbackRequest> = Vec::new();
                        if let Ok(entries) = std::fs::read_dir(feedback_requests_dir()) {
                            for entry in entries.flatten() {
                                if let Ok(s) = std::fs::read_to_string(entry.path()) {
                                    if let Ok(req) = serde_json::from_str::<FeedbackRequest>(&s) {
                                        let keep = match filter.as_str() {
                                            "pending" => req.status == "pending",
                                            "closed" => req.status != "pending",
                                            _ => true,
                                        };
                                        if keep {
                                            rows.push(req);
                                        }
                                    }
                                }
                            }
                        }
                        rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                        if rows.is_empty() {
                            (format!("No feedback requests (filter={}).", filter), false)
                        } else {
                            let mut out = format!(
                                "Feedback inbox ({} item(s), filter={}):\n",
                                rows.len(),
                                filter
                            );
                            for r in rows.iter().take(50) {
                                let media = if r.media_path.is_empty() {
                                    "—".to_string()
                                } else {
                                    std::path::Path::new(&r.media_path)
                                        .file_name()
                                        .map(|f| f.to_string_lossy().to_string())
                                        .unwrap_or_else(|| r.media_path.clone())
                                };
                                let q: String = r.question.chars().take(80).collect();
                                out.push_str(&format!(
                                    "· {} | {} | pri={} | {} | {}\n  {}\n",
                                    r.id,
                                    r.status,
                                    r.priority,
                                    if r.agent_label.is_empty() {
                                        "-"
                                    } else {
                                        &r.agent_label
                                    },
                                    media,
                                    q
                                ));
                            }
                            if rows.len() > 50 {
                                out.push_str(&format!("… and {} more\n", rows.len() - 50));
                            }
                            (out, false)
                        }
                    }
                    "vibecap_cancel_feedback" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let request_id = args
                            .and_then(|a| a.get("request_id"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("");
                        if request_id.is_empty()
                            || !request_id
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                        {
                            (
                                "Invalid request_id (allowed: letters, digits, _ and -)".to_string(),
                                true,
                            )
                        } else {
                            let req_path =
                                feedback_requests_dir().join(format!("{}.json", request_id));
                            match std::fs::read_to_string(&req_path)
                                .ok()
                                .and_then(|s| serde_json::from_str::<FeedbackRequest>(&s).ok())
                            {
                                None => (format!("Unknown request_id: {}", request_id), true),
                                Some(mut req) => {
                                    if req.status != "pending" {
                                        (
                                            format!(
                                                "Request {} is already {} — not cancelled.",
                                                request_id, req.status
                                            ),
                                            true,
                                        )
                                    } else {
                                        req.status = "cancelled".to_string();
                                        let resp = FeedbackResponse {
                                            id: request_id.to_string(),
                                            feedback_text: String::new(),
                                            voice_note_path: String::new(),
                                            annotated_media_path: String::new(),
                                            answered_at: Local::now()
                                                .format("%Y-%m-%d %H:%M:%S")
                                                .to_string(),
                                            selected_option: "cancelled".to_string(),
                                        };
                                        let resp_path = feedback_responses_dir()
                                            .join(format!("{}.json", request_id));
                                        let ok_req = serde_json::to_string_pretty(&req)
                                            .map_err(|e| e.to_string())
                                            .and_then(|s| write_json_atomic(&req_path, &s));
                                        let ok_resp = serde_json::to_string_pretty(&resp)
                                            .map_err(|e| e.to_string())
                                            .and_then(|s| write_json_atomic(&resp_path, &s));
                                        match (ok_req, ok_resp) {
                                            (Ok(_), Ok(_)) => (
                                                format!(
                                                    "🚫 status=cancelled request_id={}\nHuman will see it as closed.",
                                                    request_id
                                                ),
                                                false,
                                            ),
                                            (Err(e), _) | (_, Err(e)) => {
                                                (format!("Failed to cancel: {}", e), true)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "vibecap_list_apps" => {
                        let apps = list_running_apps();
                        if apps.is_empty() {
                            (
                                "No running apps detected (platform list empty). Pass app_name manually to vibecap_capture."
                                    .into(),
                                false,
                            )
                        } else {
                            (
                                format!(
                                    "Running apps ({}):\n{}",
                                    apps.len(),
                                    apps.iter()
                                        .map(|a| format!("· {}", a))
                                        .collect::<Vec<_>>()
                                        .join("\n")
                                ),
                                false,
                            )
                        }
                    }
                    "vibecap_set_retro" => {
                        let enabled = parsed
                            .get("params")
                            .and_then(|p| p.get("arguments"))
                            .and_then(|a| a.get("enabled"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let cfg = set_retro_enabled(enabled);
                        (
                            format!(
                                "Retro buffer {} — window {}s · ~{:.0} fps · cap {:.0} MB.\n{}\n{}",
                                if cfg.enabled { "ENABLED" } else { "DISABLED" },
                                cfg.seconds,
                                cfg.fps,
                                cfg.max_mb,
                                retro_runtime_note(),
                                if cfg.enabled {
                                    "Capturing in this MCP process; desktop app (if open) reloads config within ~2s and may also capture. Wait a few seconds, then vibecap_save_retro."
                                } else {
                                    "Frames cleared. Re-enable before the next repro."
                                }
                            ),
                            false,
                        )
                    }
                    "vibecap_save_retro" => match dump_retro_disk_gif(&default_media_dir()) {
                        Ok(path) => (
                            format!(
                                "🎞 Retro GIF saved to {}\n{}\nOpen with vision if you need to inspect motion.",
                                path.display(),
                                retro_runtime_note()
                            ),
                            false,
                        ),
                        Err(e) => (format!("{e}\n{}", retro_runtime_note()), true),
                    },
                    "vibecap_bug_report" => {
                        let args = parsed.get("params").and_then(|p| p.get("arguments"));
                        let opts = capture_opts_from(args);
                        let dir = output_dir_from(args);
                        if let Some(app) = opts.window.as_deref() {
                            let _ = focus_app(app);
                        }
                        match capture_to_dir(&dir, &opts) {
                            Err(e) => (format!("Bug report still failed: {e}"), true),
                            Ok(shot) => {
                                let mut parts =
                                    vec![format!("still={}", shot.display())];
                                match dump_retro_disk_gif(&dir) {
                                    Ok(gif) => {
                                        parts.push(format!("retro_gif={}", gif.display()))
                                    }
                                    Err(e) => parts.push(format!(
                                        "retro=skipped ({e})"
                                    )),
                                }
                                (
                                    format!(
                                        "🐛 Bug pack saved\n{}\nUse vision on still and/or GIF paths.",
                                        parts.join("\n")
                                    ),
                                    false,
                                )
                            }
                        }
                    }
                    _ => (format!("Unknown tool: {}", tool_name), true),
                };

                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": content_text
                            }
                        ],
                        "isError": is_error
                    }
                });
                let _ = writeln!(handle, "{}", response.to_string());
                let _ = handle.flush();
            }
            _ => {
                if let Some(id_val) = id {
                    let response = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "error": {
                            "code": -32601,
                            "message": "Method not found"
                        }
                    });
                    let _ = writeln!(handle, "{}", response.to_string());
                    let _ = handle.flush();
                }
            }
        }
    }
}
