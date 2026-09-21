//! Headless agent CLI — no MCP client and no pre-wired mcp.json required.
//!
//!   vibecap --screenshot --output-dir DIR [--display :0] [--window NAME] [--json]
//!   vibecap record start --output-dir DIR [--display :0] [--window NAME] [--gif] [--dry-run]
//!   vibecap record stop [--gif]
//!   vibecap record status [--watch] [--json]
//!   vibecap list [--limit N] [--json]
//!   vibecap open <file-or-name>
//!   vibecap annotate <file-or-name>
//!   vibecap doctor [--json] [--fix]
//!   vibecap --paths
//!   vibecap --mcp

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::agent_record::{
    load_record_state, record_pid_alive, record_status_line, start_agent_record, stop_agent_record,
    AgentRecordState,
};
use crate::app::io::error_code;
use crate::platform::{
    capture_backend_label, capture_to_dir, default_output_dir_display, ffmpeg_available, media_dir,
    media_dir_display, open_path, record_dry_run_line, resolve_output_dir, CaptureOpts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliAction {
    Help,
    Version,
    Paths,
    Doctor,
    Screenshot,
    RecordStart,
    RecordStop,
    RecordStatus,
    /// E282 — newest-first listing of the media (or --output-dir) directory.
    List,
    /// E281 — open a library file by name/path in the OS default app.
    Open,
    /// E283 — load a still into the Studio annotate surface (via the
    /// pending-still marker the GUI already polls).
    Annotate,
    /// E210 — `vibecap poke <show|hide|screenshot|record|stop>` — forwards a
    /// command to the running instance (or the next one to launch).
    Poke,
    Mcp,
    Gui {
        hidden: bool,
        no_tray: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CliArgs {
    pub action: CliAction,
    pub output_dir: Option<PathBuf>,
    pub display: Option<String>,
    pub window: Option<String>,
    pub gif: bool,
    /// E276 — machine-readable output on every verb.
    pub json: bool,
    /// E251 — doctor applies safe remediations before reporting.
    pub fix: bool,
    /// E280 — `record start --dry-run` prints the exact ffmpeg argv.
    pub dry_run: bool,
    /// E277 — `record status --watch` ticks until the recorder exits.
    pub watch: bool,
    /// E282 — `list --limit N` (default 50).
    pub limit: usize,
    /// E282 — `list --type video|screenshot|gif|audio|note`.
    pub kind_filter: Option<String>,
    /// Positional for `open` / `annotate`.
    pub target: Option<String>,
    /// E283 — repeatable `--arrow/--rect/--ellipse/--blur/--text/--badge/--spotlight`
    /// ops ("kind:value"); presence makes annotate headless.
    pub annot_ops: Vec<String>,
    /// E283 — `annotate --out FILE` (default `<stem>_annotated.png`).
    pub out: Option<PathBuf>,
    /// E283 — `--color red|#rrggbb`, `--stroke PX` for headless annotate.
    pub color: Option<String>,
    pub stroke: Option<f32>,
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
    let mut json = false;
    let mut fix = false;
    let mut dry_run = false;
    let mut watch = false;
    let mut limit = 50usize;
    let mut kind_filter = None;
    let mut annot_ops: Vec<String> = Vec::new();
    let mut out = None;
    let mut color = None;
    let mut stroke = None;

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
            "--limit" | "-n" => {
                if let Some(v) = args.get(i + 1) {
                    limit = v.parse().unwrap_or(50);
                    i += 2;
                    continue;
                }
            }
            "--type" | "--kind" => {
                if let Some(v) = args.get(i + 1) {
                    kind_filter = Some(v.to_lowercase());
                    i += 2;
                    continue;
                }
            }
            "--arrow" | "--rect" | "--ellipse" | "--blur" | "--text" | "--badge"
            | "--spotlight" | "--measure" | "--hl" => {
                if let Some(v) = args.get(i + 1) {
                    annot_ops.push(format!("{}:{}", a.trim_start_matches('-'), v));
                    i += 2;
                    continue;
                }
            }
            "--out" => {
                if let Some(v) = args.get(i + 1) {
                    out = Some(PathBuf::from(v));
                    i += 2;
                    continue;
                }
            }
            "--color" | "--colour" => {
                if let Some(v) = args.get(i + 1) {
                    color = Some(v.clone());
                    i += 2;
                    continue;
                }
            }
            "--stroke" => {
                if let Some(v) = args.get(i + 1) {
                    stroke = v.parse().ok();
                    i += 2;
                    continue;
                }
            }
            "--gif" => {
                gif = true;
                i += 1;
                continue;
            }
            "--json" => {
                json = true;
                i += 1;
                continue;
            }
            "--fix" => {
                fix = true;
                i += 1;
                continue;
            }
            "--dry-run" => {
                dry_run = true;
                i += 1;
                continue;
            }
            "--watch" | "-w" => {
                watch = true;
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
    let first = tokens.first().map(|s| s.as_str());
    let action = if has("--help") || has("-h") || has("help") {
        CliAction::Help
    } else if has("--version") || has("-v") || has("version") {
        CliAction::Version
    } else if has("--paths") || has("paths") {
        CliAction::Paths
    } else if has("--doctor") || has("doctor") {
        CliAction::Doctor
    } else if has("--mcp") || has("mcp") {
        CliAction::Mcp
    } else if first == Some("poke") {
        // E210 — before the bare-word `screenshot` check so
        // `poke screenshot` doesn't misfire into a headless capture.
        CliAction::Poke
    } else if has("--screenshot") || has("screenshot") {
        CliAction::Screenshot
    } else if has("--record-start") {
        CliAction::RecordStart
    } else if has("--record-stop") {
        CliAction::RecordStop
    } else if has("--record-status") {
        CliAction::RecordStatus
    } else if first == Some("list") || has("--list") {
        CliAction::List
    } else if first == Some("open") {
        CliAction::Open
    } else if first == Some("annotate") {
        CliAction::Annotate
    } else if first == Some("record") {
        match tokens.get(1).map(|s| s.as_str()) {
            Some("start") => CliAction::RecordStart,
            Some("stop") => CliAction::RecordStop,
            Some("status") => CliAction::RecordStatus,
            _ => CliAction::Help,
        }
    } else {
        CliAction::Gui { hidden, no_tray }
    };
    let target = match action {
        CliAction::Open | CliAction::Annotate | CliAction::Poke => tokens.get(1).cloned(),
        _ => None,
    };

    CliArgs {
        action,
        output_dir,
        display,
        window,
        gif,
        json,
        fix,
        dry_run,
        watch,
        limit,
        kind_filter,
        target,
        annot_ops,
        out,
        color,
        stroke,
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
  vibecap record start [--output-dir DIR] [--display :0] [--window NAME] [--gif] [--dry-run]
  vibecap record stop [--gif]
  vibecap record status [--watch]
  vibecap list [--limit N] [--type video]
  vibecap open <file-or-name>
  vibecap annotate <file-or-name> [--arrow x1,y1,x2,y2] [--rect …] [--blur …]
                   [--ellipse …] [--hl …] [--spotlight …] [--measure …]
                   [--badge x,y] [--text x,y,label] [--color red|#rrggbb]
                   [--stroke PX] [--out FILE]
  vibecap poke <show|hide|screenshot|record|stop>
  vibecap doctor [--json] [--fix]
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
  --json              Machine-readable output on every verb (E276)
  --fix               doctor only: create missing dirs, clear stale record state
  --dry-run           record start only: print the exact ffmpeg argv, no spawn
  --watch, -w         record status only: tick each second until the recorder exits
  --limit, -n         list only: max rows (default 50)
  --type, --kind      list only: filter by screenshot|video|gif|audio|note
  list                Newest-first listing of the media dir (or --output-dir)
  open <name>         Stills open in Review; other media open in the OS app
  annotate <name>     No ops → open in the Studio annotate surface.
                      With --arrow/--rect/--blur/--text/… → headless bake to
                      <stem>_annotated.png (or --out). Coords are 0..1.
  --arrow/--rect/…    Headless annotate ops (repeatable); see Usage
  --out               annotate output file (default <stem>_annotated.png)
  --color, --stroke   annotate pen color (name|#rrggbb) and width
  poke <cmd>          Forward show|hide|screenshot|record|stop to the GUI —
                      running instance or the next launch (E210)
  --paths             Print default media dir, config dir, backend
  doctor, --doctor    Diagnostics: ffmpeg, monitors, audio, stdio, window crop
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
    let ffmpeg = crate::platform::ffmpeg_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| {
            if ffmpeg_available() {
                "yes".into()
            } else {
                "no".into()
            }
        });
    let extra = crate::platform::window_tools_hint()
        .map(|h| format!("window_crop={h}\n"))
        .unwrap_or_default();
    format!(
        "media_dir={}\nconfig_dir={}\noutput_dir_default={}\nbackend={}\nffmpeg={}\nDISPLAY={}\nVIBECAP_OUTPUT_DIR={}\n{extra}",
        media_dir_display(),
        crate::platform::config_dir().display(),
        resolve_output_dir(None).display(),
        capture_backend_label(),
        ffmpeg,
        std::env::var("DISPLAY").unwrap_or_else(|_| "(unset)".into()),
        std::env::var("VIBECAP_OUTPUT_DIR").unwrap_or_else(|_| "(unset)".into()),
        extra = extra,
    )
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// E279 — one failure path for every verb: text gets `error[E_X]: …`,
/// `--json` gets `{"ok":false,"code":"E_X","error":…}` on stderr.
fn cli_fail(cli: &CliArgs, msg: &str) -> i32 {
    let code = error_code(msg);
    if cli.json {
        eprintln!(
            "{}",
            serde_json::json!({"ok": false, "code": code, "error": msg})
        );
    } else {
        eprintln!("error[{code}]: {msg}");
    }
    1
}

/// Usage errors exit 2 so scripts can tell them apart from runtime failures.
fn usage_fail(cli: &CliArgs, usage: &str) -> i32 {
    cli_fail(cli, usage);
    2
}

/// Resolve a `open`/`annotate` argument: an existing path wins, else an exact
/// file-name match inside the media dir.
fn resolve_media_target(id: &str, dir: &Path) -> Result<PathBuf, String> {
    let p = PathBuf::from(id);
    if p.is_file() {
        return Ok(p);
    }
    let in_dir = dir.join(id);
    if in_dir.is_file() {
        return Ok(in_dir);
    }
    if let Some(m) = crate::app::library::scan_media_dir(dir)
        .into_iter()
        .find(|m| m.name == id)
    {
        return Ok(m.path);
    }
    Err(format!(
        "no media file matching \"{id}\" — not a path and not in {}",
        dir.display()
    ))
}

fn kind_str(c: crate::app::library::MediaCategory) -> &'static str {
    use crate::app::library::MediaCategory::*;
    match c {
        Screenshot => "screenshot",
        Video => "video",
        Gif => "gif",
        Audio => "audio",
        Note => "note",
    }
}

/// E283 — parse "x1,y1[,x2,y2]" normalized canvas coords (0..1).
fn parse_norm_points(s: &str) -> Result<Vec<eframe::egui::Pos2>, String> {
    let vals: Vec<f32> = s
        .split(',')
        .map(|t| t.trim().parse::<f32>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("bad coordinates \"{s}\" — expected x1,y1[,x2,y2] in 0..1"))?;
    if vals.len() < 2 || vals.len() % 2 != 0 {
        return Err(format!(
            "bad coordinates \"{s}\" — need an even count ≥2 (x1,y1[,x2,y2])"
        ));
    }
    Ok(vals
        .chunks(2)
        .map(|c| eframe::egui::Pos2::new(c[0], c[1]))
        .collect())
}

/// E283 — `--color`: name or #rrggbb.
fn parse_color(s: Option<&str>) -> eframe::egui::Color32 {
    use eframe::egui::Color32;
    match s.unwrap_or("red").to_lowercase().as_str() {
        "red" => Color32::from_rgb(235, 64, 52),
        "amber" | "yellow" => Color32::from_rgb(240, 180, 40),
        "green" => Color32::from_rgb(70, 180, 90),
        "blue" => Color32::from_rgb(70, 130, 240),
        "white" => Color32::WHITE,
        "black" => Color32::BLACK,
        hex => {
            let h = hex.trim_start_matches('#');
            if h.len() == 6 {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&h[0..2], 16),
                    u8::from_str_radix(&h[2..4], 16),
                    u8::from_str_radix(&h[4..6], 16),
                ) {
                    return Color32::from_rgb(r, g, b);
                }
            }
            Color32::from_rgb(235, 64, 52)
        }
    }
}

/// E283 — bake `--arrow/--rect/--ellipse/--blur/--text/--badge/--spotlight/
/// --measure/--hl` ops onto a still with the same rasterizer the GUI uses.
/// Coords are normalized 0..1 so scripts stay resolution-independent.
fn headless_annotate(src: &Path, cli: &CliArgs) -> Result<PathBuf, String> {
    use crate::app::annotation_baker::{bake_annotations, AnnotationAction, AnnotationTool};
    let mut img = image::open(src).map_err(|e| format!("could not open {}: {e}", src.display()))?;
    let color = parse_color(cli.color.as_deref());
    let stroke = cli.stroke.unwrap_or(3.0);
    let mut badge_n = 0usize;
    let mut actions = Vec::new();
    for op in &cli.annot_ops {
        let (kind, arg) = op
            .split_once(':')
            .ok_or_else(|| format!("bad annotation op \"{op}\""))?;
        let (tool, points, text) = match kind {
            "arrow" | "measure" | "spotlight" => {
                let t = match kind {
                    "arrow" => AnnotationTool::Arrow,
                    "measure" => AnnotationTool::Measure,
                    _ => AnnotationTool::Spotlight,
                };
                (t, parse_norm_points(arg)?, String::new())
            }
            "rect" => (
                AnnotationTool::Rectangle,
                parse_norm_points(arg)?,
                String::new(),
            ),
            "ellipse" => (
                AnnotationTool::Ellipse,
                parse_norm_points(arg)?,
                String::new(),
            ),
            "blur" => (AnnotationTool::Blur, parse_norm_points(arg)?, String::new()),
            "hl" => (
                AnnotationTool::Highlight,
                parse_norm_points(arg)?,
                String::new(),
            ),
            "badge" => {
                badge_n += 1;
                (
                    AnnotationTool::StepBadge,
                    parse_norm_points(arg)?,
                    String::new(),
                )
            }
            "text" => {
                // "x,y,the label text" — label may itself contain commas.
                let mut parts = arg.splitn(3, ',');
                let coord = format!(
                    "{},{}",
                    parts.next().unwrap_or(""),
                    parts.next().unwrap_or("")
                );
                let label = parts.next().unwrap_or("").to_string();
                if label.is_empty() {
                    return Err("--text needs x,y,label".into());
                }
                (AnnotationTool::Text, parse_norm_points(&coord)?, label)
            }
            other => return Err(format!("unknown annotation op \"{other}\"")),
        };
        actions.push(AnnotationAction {
            tool,
            color,
            stroke_width: stroke,
            points,
            text_content: text,
            badge_number: badge_n,
            sticker: None,
        });
    }
    bake_annotations(&mut img, &actions, None);
    let out = cli.out.clone().unwrap_or_else(|| {
        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("still");
        src.with_file_name(format!("{stem}_annotated.png"))
    });
    img.save(&out)
        .map_err(|e| format!("could not write {}: {e}", out.display()))?;
    Ok(out)
}

fn print_record_status(cli: &CliArgs, s: Option<&AgentRecordState>) {
    if cli.json {
        let obj = match s {
            Some(s) => serde_json::json!({
                "recording": record_pid_alive(s.pid),
                "pid": s.pid,
                "mp4": s.mp4,
                "output_dir": s.output_dir,
                "elapsed_secs": now_unix().saturating_sub(s.started_unix),
                "display": s.display,
                "window": s.window,
                "gif": s.gif,
            }),
            None => serde_json::json!({"recording": false}),
        };
        println!("{obj}");
    } else if let Some(s) = s {
        println!("{}\nmp4={}", record_status_line(), s.mp4);
    } else {
        println!("{}", record_status_line());
    }
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
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "media_dir": media_dir_display(),
                        "config_dir": crate::platform::config_dir().display().to_string(),
                        "output_dir_default": resolve_output_dir(None).display().to_string(),
                        "backend": capture_backend_label(),
                        "ffmpeg": crate::platform::ffmpeg_path()
                            .map(|p| p.display().to_string()),
                        "env": {
                            "DISPLAY": std::env::var("DISPLAY").ok(),
                            "VIBECAP_OUTPUT_DIR": std::env::var("VIBECAP_OUTPUT_DIR").ok(),
                        },
                    })
                );
            } else {
                print!("{}", paths_text());
            }
            Some(0)
        }
        CliAction::Doctor => {
            if cli.fix {
                for line in crate::app::doctor::doctor_fix() {
                    println!("fix: {line}");
                }
            }
            if cli.json {
                println!("{}", crate::app::doctor::doctor_json());
            } else {
                print!("{}", crate::app::doctor_text());
            }
            Some(0)
        }
        CliAction::Screenshot => {
            let dir = resolve_output_dir(cli.output_dir.as_deref());
            match capture_to_dir(&dir, &cli.opts()) {
                Ok(path) => {
                    if cli.json {
                        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                        println!(
                            "{}",
                            serde_json::json!({
                                "ok": true,
                                "path": path.display().to_string(),
                                "bytes": bytes,
                            })
                        );
                    } else {
                        println!("{}", path.display());
                    }
                    Some(0)
                }
                Err(e) => Some(cli_fail(cli, &e)),
            }
        }
        CliAction::RecordStart => {
            if cli.dry_run {
                // E280 — same argv builder the real spawn uses; `dry` skips
                // window focus but still resolves rects, so the printed line
                // is what would actually run.
                let dir = resolve_output_dir(cli.output_dir.as_deref());
                let probe = dir.join("video_dry-run.mp4");
                return match record_dry_run_line(&probe, &cli.opts()) {
                    Ok(line) => {
                        if cli.json {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "ok": true,
                                    "dry_run": true,
                                    "command": line,
                                    "output_dir": dir.display().to_string(),
                                })
                            );
                        } else {
                            println!("dry-run — would spawn:\n{line}");
                        }
                        Some(0)
                    }
                    Err(e) => Some(cli_fail(cli, &e)),
                };
            }
            match start_agent_record(cli.output_dir.as_deref(), &cli.opts(), cli.gif) {
                Ok(s) => {
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "ok": true,
                                "pid": s.pid,
                                "mp4": s.mp4,
                                "output_dir": s.output_dir,
                                "display": s.display,
                                "window": s.window,
                            })
                        );
                    } else {
                        println!(
                            "recording started pid={} mp4={} output_dir={} display={} window={}",
                            s.pid,
                            s.mp4,
                            s.output_dir,
                            s.display.as_deref().unwrap_or("-"),
                            s.window.as_deref().unwrap_or("-")
                        );
                    }
                    Some(0)
                }
                Err(e) => Some(cli_fail(cli, &e)),
            }
        }
        CliAction::RecordStop => match stop_agent_record(cli.gif) {
            Ok((s, gif)) => {
                let bytes = std::fs::metadata(s.mp4_path())
                    .map(|m| m.len())
                    .unwrap_or(0);
                if cli.json {
                    let (gif_path, gif_pending) = match &gif {
                        crate::app::agent_record::GifOutcome::Ready(g) => {
                            (Some(g.display().to_string()), None)
                        }
                        crate::app::agent_record::GifOutcome::Pending(g) => {
                            (None, Some(g.display().to_string()))
                        }
                        crate::app::agent_record::GifOutcome::None => (None, None),
                    };
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": true,
                            "mp4": s.mp4,
                            "bytes": bytes,
                            "gif": gif_path,
                            "gif_pending": gif_pending,
                        })
                    );
                } else {
                    print!("recording stopped mp4={} bytes={}", s.mp4, bytes);
                    match gif {
                        crate::app::agent_record::GifOutcome::Ready(g) => {
                            print!(" gif={}", g.display());
                        }
                        crate::app::agent_record::GifOutcome::Pending(g) => {
                            print!(" gif_pending={}", g.display());
                        }
                        crate::app::agent_record::GifOutcome::None => {}
                    }
                    println!();
                }
                Some(0)
            }
            Err(e) => Some(cli_fail(cli, &e)),
        },
        CliAction::RecordStatus => {
            if cli.watch {
                // E277 — one line per second (NDJSON under --json) until the
                // recorder exits or state clears. Kill with Ctrl-C.
                loop {
                    let state = load_record_state();
                    let live = state
                        .as_ref()
                        .map(|s| record_pid_alive(s.pid))
                        .unwrap_or(false);
                    print_record_status(cli, state.as_ref());
                    if !live {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            } else {
                let state = load_record_state();
                print_record_status(cli, state.as_ref());
            }
            Some(0)
        }
        CliAction::List => {
            let dir = cli.output_dir.clone().unwrap_or_else(media_dir);
            let mut items = crate::app::library::scan_media_dir(&dir);
            items.sort_by(|a, b| b.modified_secs.cmp(&a.modified_secs));
            if let Some(k) = cli.kind_filter.as_deref() {
                items.retain(|m| kind_str(m.category) == k);
            }
            items.truncate(cli.limit);
            if cli.json {
                let arr: Vec<serde_json::Value> = items
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "name": m.name,
                            "path": m.path.display().to_string(),
                            "bytes": m.size_bytes,
                            "kind": kind_str(m.category),
                            "modified_unix": m.modified_secs,
                            "dupe": m.dupe,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".into())
                );
            } else {
                for m in &items {
                    println!("{}\t{}\t{}", m.name, m.size_str, kind_str(m.category));
                }
                println!("{} item(s) in {}", items.len(), dir.display());
            }
            Some(0)
        }
        CliAction::Open => {
            let Some(id) = cli.target.as_deref() else {
                return Some(usage_fail(cli, "usage: vibecap open <file-or-name>"));
            };
            let dir = cli.output_dir.clone().unwrap_or_else(media_dir);
            match resolve_media_target(id, &dir) {
                Ok(p) => {
                    // E281 — stills open straight into Review via the pending
                    // marker the GUI polls; clips/audio hand off to the OS.
                    let ext = p
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp") {
                        crate::app::io::write_pending_still(&p);
                        println!("opening {} in Review", p.display());
                        None
                    } else {
                        match open_path(&p) {
                            Ok(()) => {
                                println!("opened {}", p.display());
                                Some(0)
                            }
                            Err(e) => Some(cli_fail(cli, &e)),
                        }
                    }
                }
                Err(e) => Some(cli_fail(cli, &e)),
            }
        }
        CliAction::Annotate => {
            let Some(id) = cli.target.as_deref() else {
                return Some(usage_fail(cli, "usage: vibecap annotate <file-or-name>"));
            };
            let dir = cli.output_dir.clone().unwrap_or_else(media_dir);
            match resolve_media_target(id, &dir) {
                Ok(p) => {
                    if cli.annot_ops.is_empty() {
                        // Interactive: durable handoff — the GUI polls
                        // pending_still every frame. If a GUI already runs the
                        // lock-fail path focuses it and it picks the marker up;
                        // otherwise we launch and consume it ourselves.
                        crate::app::io::write_pending_still(&p);
                        println!("opening {} in the Studio", p.display());
                        None
                    } else {
                        // E283 — headless bake: same rasterizer the GUI uses.
                        match headless_annotate(&p, cli) {
                            Ok(out) => {
                                if cli.json {
                                    println!(
                                        "{}",
                                        serde_json::json!({
                                            "ok": true,
                                            "src": p.display().to_string(),
                                            "out": out.display().to_string(),
                                            "ops": cli.annot_ops.len(),
                                        })
                                    );
                                } else {
                                    println!("{}", out.display());
                                }
                                Some(0)
                            }
                            Err(e) => Some(cli_fail(cli, &e)),
                        }
                    }
                }
                Err(e) => Some(cli_fail(cli, &e)),
            }
        }
        CliAction::Poke => {
            // E210 — forward a command to the running GUI (or the next one to
            // launch) via the pending_cmd marker the GUI polls each frame.
            let cmd = cli.target.as_deref().unwrap_or("show");
            let valid = ["show", "hide", "screenshot", "record", "stop"];
            if !valid.contains(&cmd) {
                return Some(usage_fail(
                    cli,
                    "usage: vibecap poke <show|hide|screenshot|record|stop>",
                ));
            }
            crate::app::io::write_pending_cmd(cmd);
            if cli.json {
                println!("{}", serde_json::json!({"ok": true, "poke": cmd}));
            } else {
                println!("poked {cmd}");
            }
            // Fall through to the GUI path: a running instance consumes the
            // marker after the lock-fail focus; a cold launch picks it up in
            // poll_pending_cmd on its first frames.
            None
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
        assert_eq!(c.output_dir, Some(PathBuf::from("/workspace/run4/frames")));
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
        assert_eq!(parse_args(&argv("doctor")).action, CliAction::Doctor);
        assert_eq!(parse_args(&argv("--doctor")).action, CliAction::Doctor);
    }

    #[test]
    fn parse_doctor_json_and_fix() {
        let c = parse_args(&argv("doctor --json --fix"));
        assert_eq!(c.action, CliAction::Doctor);
        assert!(c.json);
        assert!(c.fix);
    }

    #[test]
    fn parse_record_dry_run_and_watch() {
        let c = parse_args(&argv("record start --dry-run --json"));
        assert_eq!(c.action, CliAction::RecordStart);
        assert!(c.dry_run);
        assert!(c.json);
        let c = parse_args(&argv("record status --watch"));
        assert_eq!(c.action, CliAction::RecordStatus);
        assert!(c.watch);
    }

    #[test]
    fn parse_list_open_annotate() {
        let c = parse_args(&argv("list --limit 5 --json"));
        assert_eq!(c.action, CliAction::List);
        assert_eq!(c.limit, 5);
        assert!(c.json);
        let c = parse_args(&argv("open shot.png"));
        assert_eq!(c.action, CliAction::Open);
        assert_eq!(c.target.as_deref(), Some("shot.png"));
        let c = parse_args(&argv("annotate still.png"));
        assert_eq!(c.action, CliAction::Annotate);
        assert_eq!(c.target.as_deref(), Some("still.png"));
        let c = parse_args(&argv("poke screenshot"));
        assert_eq!(c.action, CliAction::Poke);
        assert_eq!(c.target.as_deref(), Some("screenshot"));
    }

    #[test]
    fn resolve_media_target_finds_named_file() {
        let dir = std::env::temp_dir().join(format!("vibecap-cli-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("shot_1.png");
        std::fs::write(&f, b"png").unwrap();
        assert_eq!(resolve_media_target("shot_1.png", &dir).unwrap(), f);
        assert!(resolve_media_target("nope.png", &dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn help_mentions_output_dir_and_x11_and_start_stop() {
        let h = help_text();
        assert!(h.contains("doctor"), "{h}");
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

    #[test]
    fn parse_annotate_ops_and_flags() {
        let c = parse_args(&argv(
            "annotate s.png --arrow 0.1,0.1,0.9,0.9 --blur 0.2,0.2,0.4,0.4 --text 0.5,0.5,hi there --color #00ff00 --stroke 4 --out o.png",
        ));
        assert_eq!(c.action, CliAction::Annotate);
        assert_eq!(c.annot_ops.len(), 3);
        assert_eq!(c.annot_ops[0], "arrow:0.1,0.1,0.9,0.9");
        assert_eq!(c.color.as_deref(), Some("#00ff00"));
        assert_eq!(c.stroke, Some(4.0));
        assert_eq!(c.out, Some(PathBuf::from("o.png")));
    }

    #[test]
    fn headless_annotate_bakes_real_pixels() {
        let dir = std::env::temp_dir().join(format!("vibecap-annot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("still.png");
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            64,
            64,
            image::Rgba([0, 0, 0, 255]),
        ))
        .save(&src)
        .unwrap();
        let cli = parse_args(&argv(
            "annotate still.png --arrow 0.1,0.1,0.9,0.9 --badge 0.5,0.2 --text 0.1,0.8,hi",
        ));
        let out = headless_annotate(&src, &cli).unwrap();
        assert!(out.exists());
        assert!(out
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("_annotated"));
        let baked = image::open(&out).unwrap().to_rgba8();
        // default red pen actually drew
        assert!(
            baked.pixels().any(|p| p[0] > 200 && p[1] < 120),
            "no red pixels baked"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_new_verbs_and_flags() {
        let c = parse_args(&argv("doctor --json --fix"));
        assert_eq!(c.action, CliAction::Doctor);
        assert!(c.json && c.fix);

        let c = parse_args(&argv("record start --dry-run --json"));
        assert_eq!(c.action, CliAction::RecordStart);
        assert!(c.dry_run && c.json);

        let c = parse_args(&argv("record status --watch"));
        assert_eq!(c.action, CliAction::RecordStatus);
        assert!(c.watch);

        let c = parse_args(&argv("list --limit 5 --json"));
        assert_eq!(c.action, CliAction::List);
        assert_eq!(c.limit, 5);
        assert!(c.json);

        let c = parse_args(&argv("open shot_1.png"));
        assert_eq!(c.action, CliAction::Open);
        assert_eq!(c.target.as_deref(), Some("shot_1.png"));

        let c = parse_args(&argv("annotate shot_1.png"));
        assert_eq!(c.action, CliAction::Annotate);
        assert_eq!(c.target.as_deref(), Some("shot_1.png"));
    }

    #[test]
    fn dry_run_line_is_real_argv() {
        // E280 — the dry-run line must be the same builder output as spawn.
        let line = record_dry_run_line(Path::new("out/video_dry-run.mp4"), &CaptureOpts::default())
            .expect("dry-run should resolve on a supported platform");
        assert!(line.contains("-movflags"), "{line}");
        assert!(line.contains("video_dry-run.mp4"), "{line}");
        assert!(line.contains("libx264"), "{line}");
    }
}
