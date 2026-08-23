# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Native desktop work on `master` not yet cut as a binary. Desktop assets remain [v0.1.0](https://github.com/TekosherM/vibecap/releases/tag/v0.1.0).

### Added
- Feedback loop coverage for 30 agent/human scenarios (`docs/FEEDBACK_USE_CASES.md`)
- MCP: `vibecap_list_feedback`, `vibecap_cancel_feedback` (12 tools total)
- `request_feedback`: optional `media_path`, `options`, `priority`, `agent_label`, `preferred_reply`, `context`
- Response `selected_option` + clear annotate-only / choice-only messaging on `get_feedback`
- Agent Inbox: priority badges, choice chips, dismiss, agent label / context, prefer-reply hints
- **Phase 1 UI (Safelight / Loop):** `src/ui/` design system — Graphite dark theme, accent reserved for live states, left Loop rail, Shutter capture dock, severity toast cards, empty states for Media/Inbox
- Marketing plan (`docs/MARKETING_PLAN.md`) and design revamp proposal (`docs/DESIGN_REVAMP_PROPOSAL.md`)

### Changed
- Skill + MCP docs emphasize **poll required** (answers are not pushed into chat)
- Shell navigation: top emoji tabs → left Loop rail (Shutter · Media · Clip · Still · Inbox · Settings)
- Recording start no longer waits on minimized-window `update` ticks (worker-spawn + arming HUD)
- Stop recording always shows main window and loads Clip filmstrip more reliably
- **Phase 1a extraction:** budget, feedback, live-state, MCP, library scan, recording helpers, and tab UIs moved out of `main.rs` (`~4145 → ~1757` lines; zero MCP behavior change). Layout: `src/app/*` + `src/ui/*_tab.rs`
- **Phase 1b tokens:** all UI colors go through `src/ui/theme.rs` (G4); `Density` + `apply_light_theme` stub; semantic/annotation/overlay tokens
- **Phase 1c chrome:** loop-position badges on Media library cards; bottom status strip (storage / budget / ffmpeg / inbox / REC); translucent Shutter dock
- **Phase 1d polish:** ⌘K/Ctrl+K command palette; session restore (`session.json`); density Comfortable/Compact; soft-delete with **Z** undo (~12s)
- **Phase 2:** post-capture action toast (Annotate / Copy / Reveal); dual-pane Agent Inbox (thread list + conversation detail, identity dots, priority chips, media preview, reply composer)
- **Phase 3:** first-run wizard (welcome → save dir → budget tier → shortcuts; skippable; Settings → Replay; `wizard_done` in `session.json`)
- **Capture HUD:** region selector with **thirds grid**, corner/edge handles, live **W×H** plate (`src/ui/capture_hud.rs`)
- **Light theme:** runtime dual-palette tokens; Settings Dark/Light + ⌘K “Toggle theme”; persisted in `session.json`
- **Retro buffer (spike):** optional rolling low-FPS capture (off by default, ~2 fps, 15/30/60s window, 200 MB hard cap); Save last as GIF from Settings / Capture / ⌘K; config in `retro.json`
- **Capture HUD loupe:** cursor loupe ring (2× chrome, coords), arrow-key nudge (⇧=10px), Enter to confirm, last-region ghost + session memory
- **Record countdown:** Off / 3s / 5s bubble before arm (Esc cancels); session-persisted
- **Bug report pack:** one-shot still + retro GIF (⌘K / Settings / Capture)
- **Window picker:** Capture → Window lists running apps (macOS System Events / best-effort elsewhere); focus then capture/record
- **MCP tools (16):** `vibecap_list_apps`, `vibecap_set_retro`, `vibecap_save_retro`, `vibecap_bug_report`
- **Retro contract fix (code review):** workers reload `retro.json` every ~2s; frames preserved across process start/stop; MCP `set_retro` starts process-local capturer; spawn failures surfaced; monotonic frame IDs; prune/list unit tests; `docs/BUG_PATTERNS.md` + `ACTIVE_BUGS.md`
- **Fullscreen screenshot fix:** track last non-Vibecap frontmost app and restore it before Full/Window capture (avoids bare desktop); Capture tab tip; `scripts/smoke_capture.sh` pre-commit functional gate
- **Studio → Clip + Still:** Loop rail splits video and image editors into dedicated stages; preview-first Safelight bodies (no accent on non-live titles); Media opens video/GIF in Clip and screenshots in Still; session keys `clip` / `still` (legacy `edit`/`studio` → Clip)
- **Tray icon:** brand aperture shutter (Safelight mark) replaces crude orange “V”; idle uses macOS template tinting; recording shows red REC disc
- **Tray menu + progress:** Loop stages (Shutter/Media/Clip/Still/Inbox/Settings), bug-report pack, disabled live status row; menu bar title `REC mm:ss` while recording, `…` while arming; Inbox badge count; Cancel Start during arm
- **ffmpeg path resolve:** GUI/Finder launches no longer miss Homebrew `ffmpeg` (search `/usr/local/bin`, `/opt/homebrew/bin`, `VIBECAP_FFMPEG`); clearer install error if still missing
- **Dock / app icon:** embed Safelight aperture PNG via eframe `with_icon` (replaces default egui “e”); regenerate full `AppIcon.icns` + `CFBundleIconName`; install script refreshes Launch Services
- **Agent HITL notify:** new feedback requests fire OS notification + sound, Dock bounce, tray title `Inbox`/`Inbox N`, toast with agent+question, and auto-open Inbox (per-request-id tracking, not just count)

## [0.2.0] — 2026-08-23

Web studio + agent-loop docs. Native crate stays 0.1.0; desktop binaries are still the [v0.1.0](https://github.com/TekosherM/vibecap/releases/tag/v0.1.0) assets.

### Added
- **Web HTTP studio** (`web/`): unbounded record, snap-while-REC, JPEG/WebM downloads, Neon/PGLite evidence pack
- Live **hook plan** (`GET /api/agent/hooks`, `vibecap_hooks`): when to collect DOM, console, HTTP, shell, database, stills, video
- Docs: `docs/WEB.md`, `docs/HOOKS.md`, `web/README.md`

### Changed
- Agent skill shortened; capture-only first; inbox/poll loops optional
- README, USAGE, CONTRIBUTING, ARCHITECTURE, TESTING document two connectors
- Web stills live in the pack and `GET /api/agent/still/{id}.jpg` — not `~/Movies/Vibecap`

## [0.1.0] — 2026-08-05

First public open-source release.

### Added
- Native desktop app (eframe/egui): capture, library, annotation studio, video editor, feedback inbox, settings
- Annotation tools: pen, arrow, rectangle, highlight, text, blur, step badges, clipboard copy
- Voice notes and text notes beside screenshots
- Timeline GIF export and wardrobe video/image tools (via ffmpeg)
- MCP stdio server with 10 tools (capture, record, GIF, live inspection, budget, feedback)
- Agent budget controls (frames / MB / minutes + eco/standard/intensive tiers)
- Human feedback loop shared between app and MCP (`~/.config/vibecap`)
- Cross-platform capture layer (`src/platform`): macOS screencapture, Windows gdigrab, Linux x11grab/grim
- Portable paths via `dirs` (Videos/Vibecap media, config dir)
- CLI: `--mcp`, `--screenshot`, `--help`, `--version`
- Docs: USAGE, MCP, ARCHITECTURE, PLATFORMS, TESTING, CONTRIBUTING
- CI matrix (macOS smoke + Windows/Linux build)
- Dual license: MIT OR Apache-2.0

### Notes
- macOS remains the primary day-to-day capture quality target
- Windows system audio and Wayland motion capture are best-effort
- Requires ffmpeg for GIF/editor; Screen Recording permission on macOS

[0.2.0]: https://github.com/TekosherM/vibecap/releases/tag/v0.2.0
[0.1.0]: https://github.com/TekosherM/vibecap/releases/tag/v0.1.0
