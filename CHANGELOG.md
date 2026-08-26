# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Agent capture path (Linux field report)

Agents on Cursor / Grok Bot often never see MCP tools and got demo-shutter stills plus a 5s `record_video` cap. This cut makes the **CLI the attachable path** and aligns docs.

- **CLI:** `record start` / `--screenshot` / `record stop` / `--paths`; `--output-dir`, `--display`, `--window`, `--gif`
- **MCP:** `vibecap_record_start` / `_stop` / `_status`; `output_dir` + `display` + `window` on capture; omit `duration_secs` on `record_video` to start unbounded
- **Linux:** ffmpeg **x11grab** is the supported agent backend (not a grim/import fallback). Name a `DISPLAY` or window title.
- **Output:** one default (`vibecap --paths`); caller `--output-dir` always wins. Help / skill / MCP.md no longer disagree on `~/Movies` vs `~/Vibecap`.
- **Web:** `POST /api/agent/call` with `display` / `window` / `output_dir` shells out to the same CLI (real screen). Without those args, stills stay the shutter (demo Lumen Cart if source is Demo).
- **Hook:** `.cursor/mcp.json` + `scripts/vibecap-mcp.sh` (no pre-wired machine path). Skill leads with the capture-only recipe.
- Docs: `docs/AGENTS.md`

Native desktop changelog below this cut is already on `master`. Web evidence job shipped in 0.3.0.

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
- **Agent recorder detach (2026-08-26):** `record start` no longer blocks the caller's shell or dies with it — ffmpeg runs in its own process group with stdout/stderr → sibling `.ffmpeg.log`; `record status` reports a dead pid honestly (`recorder exited without stop — mp4 may be unfinalized`) instead of stale elapsed; `record stop` warns when the recorder already died; output dirs are canonicalized to absolute paths so stop/status work from any cwd (fixes `bytes=0` + missed breadcrumb cleanup for relative `--output-dir .`)
- **GUI screenshot focus hardening (2026-08-26):** refuse to shoot bare desktop when no focus target is known (fresh session, Vibecap always frontmost) — actionable toast instead of wallpaper JPG; `focus_app` verifies the app actually came to front (`frontmost_app_name` check), retries via AppleScript `activate`, and aborts capture with the reason on failure; recording warns instead of silently capturing the wrong screen
- **UI polish pass (2026-08-26):** Library heading de-accented (accent reserved for live states); Capture tab gains an always-visible live-stats row (frames · MB · caps, red dot + warning when budget exceeded) with Agent-session section open by default; Inbox auto-select respects explicit user picks (`feedback_user_picked`) and re-arms one silent pick per new request; **⌘I / Ctrl+I** jumps to the Agent Inbox from anywhere; all unused-import warnings pruned (`cargo fix` + manual cleanup)
- **macOS capture/TCC:** frontmost app + window list via `lsappinfo` (avoids repeated System Events prompts); install script symlinks CLI → app binary so GUI+MCP share one Screen Recording identity; longer pre-capture hide/focus settle; clearer dual-entry permission help
- **Permission prompt loop fixed:** Screen Recording state now read via `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess` (authoritative, no probe screenshot, no size-heuristic false negatives). System Allow dialog appears only while state is undetermined; denied users get a toast/modal instead of auto-opened System Settings on every launch
- **Window restore after capture fixed:** capture parks the window off-screen instead of `orderOut` (orderOut broke restore on macOS + eframe 0.28); finished screenshots hand off via a `pending_still.path` marker so the main thread restores geometry and opens Still even if the channel is missed; app re-activation uses direct AppKit (`NSApp activateWithOptions:`) instead of `osascript`/`open -b` (no Automation TCC prompt, no second GUI instance)
- **Still persistence:** `session.json` `edit_file` now stores the Still image when the Still tab is active, so relaunch restores the last screenshot into Still
- **UI consistency (Loop-rail language everywhere):** shared components (`section_card`, primary/secondary/small buttons, segmented controls, switch toggles, kbd chips) in `src/ui/components.rs`; Settings rebuilt as token cards (no egui group boxes, no emoji buttons, no accent headings); Still/Clip/Capture use the same buttons/segmented/switch; control rows wrap on narrow windows and Settings scrolls
- **Loop rail always visible** on every stage (Still, Clip, Settings, annotation) so navigation never disappears
- **Bigger, remembered window:** opens at 1160×800 by default (was 760×640); last window size is persisted in `session.json` and restored with a 1024×700 floor
- **Clip studio timeline:** NLE-style timeline under the filmstrip — thumbnails as background, vertical in/out split lines with drag grips (drag to trim), dimmed outside the range, in/out/span readout; play button opens the clip in the system player; duration probed via ffprobe (`ffmpeg -i` fallback) with new `parse_timecode`/`format_timecode` helpers + tests
- **In-app big-screen player (Clip):** 16:9 player canvas with flipbook playback (click canvas or transport to play/pause), scrubber slider, timecode readout, centered play badge; frames now sample the whole clip at a probed fps so playback and timeline align to real time; "Open" remains for full-fidelity system playback (preview is silent)
- **Tab organization pass:** Clip rebuilt as toolbar (file info left / actions right) + PLAYER card + timeline + two-column EXPORT / TOOLS cards with grouped clusters (AUDIO / TRANSFORM / ENCODE); Still rebuilt the same way (PREVIEW + ADJUST with TRANSFORM / COLOR / SIZE & CROP groups); Capture options grouped into a TARGET / WINDOW / AUDIO card; shared `group` component
- **Status strip pinned to window bottom** (TopBottomPanel) instead of floating mid-page; section cards and Still/Clip panels now stretch to full window width

## [0.3.0] — 2026-08-23

Web evidence job. Native crate **0.3.0**. `--help` points at the HTTP studio if MCP never attaches.

### Added
- `vibecap_job` — one call: record, walk (coupon 422, tax 500, pay 402, 3 stills), ingest FE/BE/DB/logs, stop, pack
- Walk / coupon / ZIP tax / Pay on the live Lumen Cart checkout
- WebM clips persist (`clip_url`) and `GET /api/agent/clip/{id}.webm` (survives reload)
- Pack downloads JSON + stills + clip
- Screen/camera banner: JSON still taps Lumen Cart

### Changed
- Native `--help` / version: if MCP never attaches, use `cd web && npm run dev` then `vibecap_job`
- Agent / README / WEB / skill lead with `vibecap_job`
- Job is re-runnable (resets checkout) and holds each failure on camera (~4s)

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
- CI: Linux installs `libxdo-dev` (fixes red Ubuntu badge); web `npm run typecheck` job

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
