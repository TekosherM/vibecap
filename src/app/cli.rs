//! Headless agent CLI — no MCP client and no pre-wired mcp.json required.
//!
//!   vibecap --screenshot --output-dir DIR [--display :0] [--window NAME]
//!   vibecap --record-start --output-dir DIR [--display :0] [--window NAME] [--gif]
//!   vibecap --record-stop [--gif]
//!   vibecap --record-status
//!   vibecap --paths
//!   vibecap --mcp

use std::path::PathBuf;

use crate::app::agent_record::{
    load_record_state, record_status_line, start_agent_record, stop_agent_record,
};
use crate::platform::{
    capture_backend_label, capture_to_dir, default_output_dir_display, ffmpeg_available,
    media_dir_display, resolve_output_dir, CaptureOpts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliAction {
    Help,
    Version,
    Paths,
    Screenshot,
    RecordStart,
    RecordStop,
    RecordStatus,
    Mcp,
    Gui { hidden: bool, no_tray: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    pub action: CliAction,
    pub output_dir: Option<PathBuf>,
    pub display: Option<String>,
    pub window: Option<String>,
    pub gif: bool,
}

impl CliArgs {
    pub fn opts(&self) -> CaptureOpts {
        CaptureOpts::from_parts(self.display.clone(), self.window.clone())
    }
}

/// Parse argv (without argv[0]). Unknown flags are ignored for GUI compat
/// except we still surface them on help.
pub fn parse_args(args: &[String]) -> CliArgs {
    let mut output_dir = None;
    let mut display = None;
    let mut window = None;
    let mut gif = false;
    let mut hidden = false;
    let mut no_tray = false;

    let mut i = 0;
    let mut tokens: Vec<String> = Vec::new();
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "--output-dir" | "--out-dir" | "-o" => {
                if let Some(v) = args.get(i + 1) {
                    output_dir = Some(PathBuf::from(v));
                    i += 2;
                    continue;
                }
            }
            "--display" | "-d" => {
                if let Some(v) = args.get(i + 1) {
                    display = Some(v.clone());
                    i += 2;
                    continue;
                }
            }
            "--window" | "--app" | "--app-name" => {
                if let Some(v) = args.get(i + 1) {
                    window = Some(v.clone());
                    i += 2;
                    continue;
                }
            }
            "--gif" => {
                gif = true;
                i += 1;
                continue;
            }
            "--hidden" => {
                hidden = true;
                i += 1;
                continue;
            }
            "--no-tray" => {
                no_tray = true;
                i += 1;
                continue;
            }
            _ => {
                tokens.push(a.to_string());
                i += 1;
            }
        }
    }

    let has = |n: &str| tokens.iter().any(|t| t == n);
    let action = if has("--help") || has("-h") || has("help") {
        CliAction::Help
    } else if has("--version") || has("-v") || has("version") {
        CliAction::Version
    } else if has("--paths") || has("paths") {
        CliAction::Paths
    } else if has("--mcp") || has("mcp") {
        CliAction::Mcp
    } else if has("--screenshot") || has("screenshot") {
        CliAction::Screenshot
    } else if has("--record-start") {
        CliAction::RecordStart
    } else if has("--record-stop") {
        CliAction::RecordStop
    } else if has("--record-status") {
        CliAction::RecordStatus
    } else if tokens.first().map(|s| s.as_str()) == Some("record") {
        match tokens.get(1).map(|s| s.as_str()) {
            Some("start") => CliAction::RecordStart,
            Some("stop") => CliAction::RecordStop,
            Some("status") => CliAction::RecordStatus,
            _ => CliAction::Help,
        }
    } else {
        CliAction::Gui { hidden, no_tray }
    };

    CliArgs {
        action,
        output_dir,
        display,
        window,
        gif,
    }
}

pub fn help_text() -> String {
    let default_dir = default_output_dir_display();
    format!(
        "\
Vibecap Studio {version}
Native screen capture, annotation studio, and MCP sidecar for AI agents.

Usage:
  vibecap [FLAGS]
  vibecap screenshot [--output-dir DIR] [--display :0] [--window NAME]
  vibecap record start [--output-dir DIR] [--display :0] [--window NAME] [--gif]
  vibecap record stop [--gif]
  vibecap record status
  vibecap --mcp
  vibecap --paths

Capture-only agent (no MCP, no mcp.json):
  1. vibecap record start --output-dir ./frames --display \"$DISPLAY\"
  2. drive the signed-in flow
  3. vibecap --screenshot --output-dir ./frames
  4. vibecap record stop
  Files land in --output-dir (default {default_dir}).

Flags:
  (none)              Launch the desktop UI (system tray enabled)
  --mcp               Stdio MCP server (vibecap_capture, record_start/stop, …)
  --screenshot        Headless still of the target display/window → --output-dir
  --record-start      Start unbounded MP4 (same as `record start`)
  --record-stop       Stop and finalize MP4 (optional --gif)
  --record-status     Print whether a recording is live
  --output-dir, -o    Caller directory for stills / MP4 (also VIBECAP_OUTPUT_DIR)
  --display, -d       X11 DISPLAY to grab (also VIBECAP_DISPLAY / $DISPLAY)
  --window, --app     Focus and, on Linux, crop to this window title
  --gif               Also write a companion GIF on record stop
  --paths             Print default media dir, config dir, backend
  --no-tray           Disable system tray (window close quits the app)
  --hidden            Start hidden in the tray (implies tray)
  --version, -v       Print version
  --help, -h          Print this help

Default output: {default_dir}
  Resolved as dirs::video_dir()/Vibecap, else ~/Movies/Vibecap (macOS),
  else ~/Vibecap. Always override with --output-dir for agent jobs.

If MCP never attaches (Cursor / Grok Bot dynamic-tool harness often does not
surface vibecap --mcp): use this CLI. Do not use the web studio shutter for a
real Chrome window — that still is the demo subject unless you pass --display.

Web studio (Lumen Cart evidence, or native stills via the same capturer):
  cd web && npm run dev
  POST /api/agent/call {{\"tool\":\"vibecap_capture\",\"args\":{{\"display\":\":0\",\"output_dir\":\"./frames\"}}}}

Capture backend: {backend}
Docs: README.md  ·  docs/AGENTS.md  ·  docs/USAGE.md  ·  docs/MCP.md  ·  docs/WEB.md
",
        version = env!("CARGO_PKG_VERSION"),
        default_dir = default_dir,
        backend = capture_backend_label(),
    )
}

pub fn paths_text() -> String {
    format!(
        "media_dir={}\nconfig_dir={}\noutput_dir_default={}\nbackend={}\nffmpeg={}\nDISPLAY={}\nVIBECAP_OUTPUT_DIR={}\n",
        media_dir_display(),
        crate::platform::config_dir().display(),
        resolve_output_dir(None).display(),
        capture_backend_label(),
        if ffmpeg_available() { "yes" } else { "no" },
        std::env::var("DISPLAY").unwrap_or_else(|_| "(unset)".into()),
        std::env::var("VIBECAP_OUTPUT_DIR").unwrap_or_else(|_| "(unset)".into()),
    )
}

/// Run a headless CLI action. Returns `Some(exit_code)` when the process should
/// stop before launching the GUI (`0` = success).
pub fn run_headless(cli: &CliArgs) -> Option<i32> {
    match &cli.action {
        CliAction::Help => {
            print!("{}", help_text());
            Some(0)
        }
        CliAction::Version => {
            println!("vibecap {}", env!("CARGO_PKG_VERSION"));
            Some(0)
        }
        CliAction::Paths => {
            print!("{}", paths_text());
            Some(0)
        }
        CliAction::Screenshot => {
            let dir = resolve_output_dir(cli.output_dir.as_deref());
            match capture_to_dir(&dir, &cli.opts()) {
                Ok(path) => {
                    println!("{}", path.display());
                    Some(0)
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    Some(1)
                }
            }
        }
        CliAction::RecordStart => match start_agent_record(cli.output_dir.as_deref(), &cli.opts(), cli.gif)
        {
            Ok(s) => {
                println!(
                    "recording started pid={} mp4={} output_dir={} display={} window={}",
                    s.pid,
                    s.mp4,
                    s.output_dir,
                    s.display.as_deref().unwrap_or("-"),
                    s.window.as_deref().unwrap_or("-")
                );
                Some(0)
            }
            Err(e) => {
                eprintln!("error: {e}");
                Some(1)
            }
        },
        CliAction::RecordStop => match stop_agent_record(cli.gif) {
            Ok((s, gif)) => {
                let bytes = std::fs::metadata(s.mp4_path())
                    .map(|m| m.len())
                    .unwrap_or(0);
                print!("recording stopped mp4={} bytes={}", s.mp4, bytes);
                if let Some(g) = gif {
                    print!(" gif={}", g.display());
                }
                println!();
                Some(0)
            }
            Err(e) => {
                eprintln!("error: {e}");
                Some(1)
            }
        },
        CliAction::RecordStatus => {
            if let Some(s) = load_record_state() {
                println!("{}\nmp4={}", record_status_line(), s.mp4);
            } else {
                println!("{}", record_status_line());
            }
            Some(0)
        }
        CliAction::Mcp | CliAction::Gui { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(s: &str) -> Vec<String> {
        s.split_whitespace().map(|x| x.to_string()).collect()
    }

    #[test]
    fn parse_screenshot_with_output_dir_and_display() {
        let c = parse_args(&argv(
            "--screenshot --output-dir /workspace/run4/frames --display :1 --window Chrome",
        ));
        assert_eq!(c.action, CliAction::Screenshot);
        assert_eq!(
            c.output_dir,
            Some(PathBuf::from("/workspace/run4/frames"))
        );
        assert_eq!(c.display.as_deref(), Some(":1"));
        assert_eq!(c.window.as_deref(), Some("Chrome"));
    }

    #[test]
    fn parse_record_subcommand() {
        let c = parse_args(&argv("record start -o /tmp/out --gif"));
        assert_eq!(c.action, CliAction::RecordStart);
        assert_eq!(c.output_dir, Some(PathBuf::from("/tmp/out")));
        assert!(c.gif);
        let c = parse_args(&argv("record stop"));
        assert_eq!(c.action, CliAction::RecordStop);
        let c = parse_args(&argv("--record-status"));
        assert_eq!(c.action, CliAction::RecordStatus);
    }

    #[test]
    fn parse_help_and_mcp() {
        assert_eq!(parse_args(&argv("--help")).action, CliAction::Help);
        assert_eq!(parse_args(&argv("--mcp")).action, CliAction::Mcp);
        assert_eq!(parse_args(&argv("--paths")).action, CliAction::Paths);
    }

    #[test]
    fn help_mentions_output_dir_and_x11_and_start_stop() {
        let h = help_text();
        assert!(h.contains("--output-dir"), "{h}");
        assert!(h.contains("--record-start"), "{h}");
        assert!(h.contains("--record-stop"), "{h}");
        assert!(h.contains("x11grab") || h.contains("DISPLAY"), "{h}");
        assert!(h.contains(&default_output_dir_display()), "{h}");
        assert!(!h.contains("~/Movies/Vibecap") || h.contains("~/Movies/Vibecap (macOS)"));
    }

    #[test]
    fn paths_text_has_backend() {
        let p = paths_text();
        assert!(p.contains("media_dir="));
        assert!(p.contains("backend="));
    }
}
