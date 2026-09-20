# Vibecap — 100 improvements (current tree)

Grounded in `C:\Dev\vibecap` as of 2026-09-06 (Windows capture contract + studio HITL on `master`).
This is **not** a redo of [DESIGN_REVAMP_PROPOSAL.md](DESIGN_REVAMP_PROPOSAL.md).
That list’s Phase 1–3 chrome (Loop rail, Graphite, wizard, retro, palette, region HUD) already shipped.

**Landed on this tree (do not re-implement):** P0 capture contract (#1–#15, #21–#24, #31–#33, #81–#83), `vibecap doctor`, GUI lock, naming tokens, library hygiene, Still crop/text/badges, Clip presets/markers, Inbox j/k/pin/snooze, tray Approve/Deny, `gif_pending` for long GIFs, macOS still `-l` crop. See [STATE.md](STATE.md).

**Still open (local equivalents, not CapCut):** live HWND thumbnails (#22), library hover-scrub of many video frames (#43), OS drag-out to Explorer (#47), voice waveform (#77), hotkey rebind without restart (#81 applies next launch), macOS *record* window crop (#29 stills only).

**Already done — do not re-propose:** Loop rail, Graphite tokens, shutter strip, toast cards, empty states, ⌘K palette, density, light theme, session restore, region thirds/loupe, countdown, retro buffer, bug pack, window picker (name list), dual-pane inbox, Clip flipbook, Still studio, tray brand icon, agent recorder detach, ffmpeg path resolve.

**Product bar:** one Rust binary, capture + HITL for agents. Do not become CapCut.

**P0 (do first — user-visible correctness):** #1–#15, #21–#24, #31–#33, #81–#83.
**P1 (feels complete):** #16–#20, #34–#50, #61–#70, #84–#90.
**P2 (depth):** the rest.

---

## 1 · Capture correctness (Windows / all)

1. **Never inherit GUI stdio into ffmpeg.** Release is `windows_subsystem = "windows"`. Any leftover `Command::status()` path that still inherits null handles will fail with `Could not open file`. Audit `spawn_voice_memo`, GIF export, wardrobe, filmstrip, remux.
2. **Hide-for-capture must stay ordered-in.** `Visible(false)` kills child viewports (region overlay, REC bar). Keep off-screen park; add a debug assert that `pre_capture_outer` is restored on every exit path (error, cancel, success).
3. **Region overlay is a dedicated viewport**, never the main window’s `CentralPanel`. Regression test: selecting Region does not maximize/restore the studio chrome.
4. **REC bar always visible on Windows** while arming/recording. Tray-only stop is how recordings became unstoppable.
5. **Window stills use gdigrab offsets** (GPU-safe) with HWND grab only as fallback. Chrome/Electron HWND is black — never silently return that.
6. **`--window` never falls back to fullscreen.** Missing/minimized window = loud error, no file.
7. **Virtual-desktop origin may be negative.** `even_screen_rect` must not clamp `x,y` to 0 (left-of-primary monitors).
8. **HiDPI region record.** Overlay points ≠ gdigrab pixels. Keep snapshot-map on Windows; `pixels_per_point` on macOS live overlay. Persist `selected_screen_rect`, not just egui `Rect`.
9. **Agent MP4 always has a `moov`.** Frag + remux on `record stop` (already). If remux fails, surface the `.ffmpeg.log` tail in the CLI error instead of a 36-byte file.
10. **Pause/resume on Windows.** `stop_process`/`cont_process` are Unix SIGSTOP no-ops. Either hide Pause on Windows or send ffmpeg `q`/`SIG` equivalent (`-c:v libx264` + stdin is already piped in GUI).

---

## 2 · Region, HUD, countdown

11. **Last-region ghost in pixel space**, not overlay points. Session `last_region` is `[min_x, min_y, max_x, max_y]` in mixed coords after DPI/maximize changes — wrong box on the next pick.
12. **Re-record last region without re-picking** (partially done). Add a Shutter chip “Last 800×600” with Clear.
13. **Display picker** for multi-monitor. Today gdigrab `desktop` is the whole virtual screen; no “this monitor” control.
14. **Region nudge after confirm** before capture (Enter = capture, arrows still move). Accidental drag-stop currently fires immediately.
15. **Click-through vs capture.** Overlay must eat clicks (so you can drag). Document that; add a “click-through preview” mode only if we freeze a snapshot first (Windows already does).
16. **Countdown while hidden.** Bubble is painted on the main ctx; if the studio is parked, the user may never see 3/5. Paint countdown on the same always-on-top viewport family as the REC bar.
17. **Shutter flash** (one-frame invert/white) on still capture so you know it fired when the window is hidden.
18. **Cancel region with right-click** in addition to Esc (Windows muscle memory from Snipping Tool).
19. **Aspect-ratio lock** on region (Shift = square, Alt = 16:9) for README/demo clips.
20. **Magnifier that samples pixels** (loupe is chrome-only today — `capture_hud.rs` says so). Optional; keep off by default (cost).

---

## 3 · Window & app targeting

21. **Window list = real windows, not process names.** `list_running_apps` / combo is titles+process; multiple Chrome windows collapse to one “chrome”. Need HWND/title rows.
22. **Live thumbnails in the window picker** (or at least the focused window’s title + bounds). Combo of strings is easy to pick wrong.
23. **Refresh list automatically** when opening Window target (not only first scan + ↻).
24. **Focus verify before shot.** `focus_app` can `AppActivate` the wrong substring (`code` vs `Code.exe`). Prefer exact process-name match, then title contains.
25. **Don’t steal focus for occluded HWND-capable GDI windows** when the user asked for Window and the window is visible in the list — but **do** focus GPU apps. Branch already exists; surface it in the UI (“will bring to front”).
26. **Skip minimized windows** in the picker (already skipped in `window_rect_on_screen`); show them greyed with “restore to capture”.
27. **UWP / ApplicationFrameHost.** Many Store apps have empty `MainWindowTitle`. Enumerate via `EnumWindows` not `Get-Process`.
28. **PowerShell spawn cost.** `frontmost_app_name` / `window_rect_on_screen` shell out (~100–300 ms). Cache 500 ms; or a tiny native `windows` crate helper to drop the PS round-trip.
29. **macOS window crop.** Docs admit `--window` focuses but does not crop on macOS. Crop via `screencapture -l <windowid>` or `CGWindowList`.
30. **Linux window crop** already uses wmctrl/xdotool best-effort. If those binaries are missing, say so in `--paths` instead of silent fullscreen.

---

## 4 · Shutter UX & post-capture

31. **Post-capture toast must not steal the still tab** if the user is mid-annotate. Today success always `open_still_from_path`.
32. **Copy path / Copy image / Reveal / Annotate / Discard** on the toast (Discard = undo trash). Copy image exists on Still (⌘C); toast should offer it.
33. **Naming tokens** `{app}-{date}-{seq}` with a live preview in Settings. Default `screenshot_YYYY-MM-DD_HH-MM-SS.jpg` is unreadable in a folder of 200.
34. **Save-to last folder vs default media dir.** Agents pass `--output-dir`; GUI always uses `save_dir`. Add “set as agent default” so GUI and CLI agree (`VIBECAP_OUTPUT_DIR`).
35. **GIF as a first-class shutter action** (still / record / GIF). Today GIF is an export from Clip or `--gif` on stop.
36. **Audio meter** when “Include audio” is on. Windows audio is `VIBECAP_AUDIO_DEVICE` / `virtual-audio-capturer` — if the device is missing, the switch currently lies.
37. **Disable or warn the audio switch** on Windows until a device is detected (`ffmpeg -list_devices`).
38. **FPS 24/30/60 + custom.** Segmented 30/60 only (`settings_tab.rs`). 24 is enough for bug clips and half the disk.
39. **Cursor draw toggle** for stills (`-draw_mouse 0` hardcoded). Demos want the pointer; bug stills often don’t.
40. **Self-capture guard.** If the only “window” match is Vibecap, refuse Window target (Fullscreen already tries `last_front_app`).

---

## 5 · Library / Media

41. **Kill remaining emoji** in the library toolbar (`🔄 Refresh`, `🗑 Delete`, `📂 Open in Finder`) — `library_tab.rs` still uses them; rail icons do not.
42. **Grid of thumbnails**, not a checkbox list of names. Decode off-thread; cache beside the file (`.vibecap/thumbs/`).
43. **Hover-scrub for videos** (reuse filmstrip extract at 1 fps).
44. **Date groups:** Today / Yesterday / This week / Earlier. Flat newest-first page of 40 is a dump.
45. **Search** filename + sidecar `.txt` notes.
46. **Shift-click range select** in addition to checkbox + Select all shown.
47. **Drag out to Explorer/Finder**; drag in to import (images/mp4).
48. **Hide sidecar clutter** — `.txt`, `.m4a`, `.ffmpeg.log`, `frames_temp/`, `*.clean.mp4` leftovers. Library already skips `vibecap_region_snap_*` and dotfiles; extend the denylist.
49. **Storage bar per category** (screenshots vs video) with one “free 80% by deleting live frames” action. Live-stats row exists on Capture; Library should show the same numbers.
50. **Open in Clip vs Still is heuristic on extension.** GIFs should offer both “trim as clip” and “still frame”.

---

## 6 · Clip editor

51. **Preview is silent flipbook (~24 JPEGs).** Label it “preview (no audio)” in the player chrome, not only a hover. Offer **Open** more prominently for fidelity.
52. **Don’t block the UI on extract** (async already). Show a determinate bar (`frame i/n`) instead of “Preparing preview frames…”.
53. **Keep `frames_temp/` out of the library** and delete on Clip close / app exit (today thumbs are removed in `extract_filmstrip_rgba`, but a crash leaves the dir).
54. **In/out handles must match export.** Verify GIF/trim ffmpeg `-ss/-to` uses the same seconds as the ruler (probed duration vs filmstrip fps drift).
55. **Frame step ←/→** and `J/K` while the player is focused.
56. **Loop region** between in/out.
57. **Export presets:** “Discord 8 MB”, “README 480p 3s”, “full lossless”. One ffmpeg line each.
58. **GIF dialog:** fps / width / estimated size before encode. Current export is a fixed `fps=15,scale=800`.
59. **Audio extract** (m4a) from the TOOLS card — wardrobe has transforms; no “strip audio / extract audio”.
60. **Chapter markers** during record (hotkey drops a timestamp sidecar) → Clip marker ticks. Pause is the wrong tool for “note this moment”.

---

## 7 · Still & annotation

61. **Crop by dragging on the preview**, not four text fields (`img_crop_x/y/w/h` parse in `main.rs`). Numeric fields stay as precision.
62. **Annotation undo/redo.** `annotation_actions` is a vec with no history stack; Esc exits the whole studio.
63. **Esc from annotate returns to Still/Inbox**, not a blank capture tab.
64. **Blur is a filled overlay**, not a real pixel blur (`AnnotationTool::Blur`). Bake a box-blur (or mosaic) so PII is actually gone in the exported JPG.
65. **Text tool: in-place editor** at the click, not a separate `pending_text` field you type first.
66. **Step badges renumber** when you delete one.
67. **Zoom/pan** on the still canvas (scroll = zoom, space+drag = pan, 0 = fit). 4K stills in a 1160×800 window are unusable to annotate.
68. **Save as copy vs overwrite.** Baking currently writes next to the original; make the two actions explicit.
69. **Copy image vs copy path** as two shortcuts (⌘C image, ⇧⌘C path) — ⌘C is image-only today.
70. **Voice note on Windows.** `spawn_voice_memo` uses dshow `virtual-audio-capturer` by default — a virtual *playback* capture driver, not a mic. Use the default WASAPI/dshow *audio input* device.

---

## 8 · Inbox / HITL

71. **j/k thread list, a = first chip, Esc = back to list.** Mouse-only inbox is slow when an agent is blocked.
72. **Snooze / pin.** Age timer exists as copy; no snooze, no pin, no restore-from-closed other than “Clear closed”.
73. **Tray quick-reply** for choice-chip requests (approve/deny) without showing the window.
74. **“Agent last polled Ns ago”** on the thread. Agents poll files; humans think the agent gave up.
75. **Search history** of answered requests (question + answer + media name).
76. **Saved snippets** (“looks good”, “blur the token”, “re-record 16:9”).
77. **Voice reply waveform + re-record.** One-shot recorder with no preview is easy to ship a mute file.
78. **Don’t auto-jump selection** when a new request arrives if the user is composing (`feedback_user_picked` helps; composing should also lock).
79. **Markdown-lite in the composer** (backticks, one link) — agents read the JSON string as-is.
80. **Deep link** `vibecap://feedback/<id>` so chat clients can open the exact thread.

---

## 9 · Tray, hotkeys, settings, wizard

81. **Hotkeys configurable.** Hardcoded Ctrl+Shift+2/3 (`main.rs`). Conflict with browser/OS; no UI to change; no detection.
82. **S/R in-app vs global.** Document on the Capture card is right; wizard shortcuts step should show **Windows** keys (Ctrl+Shift), not macOS glyphs only.
83. **Wizard: ffmpeg + Windows capture test.** Today welcome → save dir → budget → shortcuts. On Windows the failure mode is missing ffmpeg / GPU window. Add a one-click test still.
84. **Wizard: MCP client detect** (Cursor / Claude Desktop / Codex config paths) with a copyable snippet. Highest activation ROI; still missing.
85. **Settings ffmpeg hint is Homebrew-only** (`brew install ffmpeg`). Windows should say `winget install Gyan.FFmpeg` (the error in `ffmpeg.rs` already does — Settings UI does not).
86. **Windows permissions card.** macOS has Screen Recording; Windows needs “can gdigrab?” + “mic/loopback device” + “tray allowed”. Empty Settings on Windows looks unfinished.
87. **Close-to-tray copy is macOS** (“menu bar icon”). On Windows say “notification area / system tray”.
88. **Tray “Hide to Menu Bar”** label (`tray_ui.rs`) — Windows users do not have a menu bar. “Hide to tray”.
89. **Single-instance optional.** Docs celebrate multi-process (GUI + MCP). GUI+GUI is confusing (two trays, two hotkeys). Second GUI should focus the first unless `--mcp` / `--screenshot`.
90. **Update checker** against GitHub Releases (opt-in). `0.3.0` tag vs months of unreleased master is how users run stale capture code.

---

## 10 · Agent CLI / MCP / reliability

91. **`--paths` should print the resolved ffmpeg path and gdigrab/x11grab/screencapture**, which it does — also print `windows_subsystem` / “GUI stdio detached” so agents can diagnose `Could not open file`.
92. **CLI `--screenshot --window` on Windows** must use the same offset/HWND path as the GUI (it does via `capture_screenshot_opts`). Add a smoke script `scripts/smoke_capture.ps1` parallel to `smoke_capture.sh`.
93. **`record stop --gif` is a synchronous full-clip transcode** (known). For long agent clips, return the MP4 immediately and GIF as a follow-up tool / background job.
94. **Leave `.ffmpeg.log` next to every MP4.** Useful for debug; pollutes Library. Default: delete on clean stop, keep on error.
95. **MCP tools often never appear** in Cursor/Grok dynamic harnesses. Keep CLI as the supported path; add `vibecap doctor` (ffmpeg, display, tray, last error, session path).
96. **`vibecap_capture` vs GUI still** should share one function (they mostly do). Guarantee identical filenames/sidecar policy so Inbox media_path always exists.
97. **Budget auto-stop is MCP-visible, not always GUI-visible.** Status strip has live frames; fire a toast + tray title when a cap hits.
98. **main.rs is still the orchestrator for capture hide/restore/region.** Extract `src/app/capture_flow.rs` so Windows park/overlay/REC-bar cannot regress in a 3k-line `update()`.
99. **CI capture smoke on Windows** (gdigrab 1 frame to temp, assert ≥8000 bytes). Linux has x11; Windows CI currently compiles and hopes.
100. **Docs/STATE.md lag.** STATE still says 2026-08-26 and “macOS primary”. After any capture behavior change, update STATE + PLATFORMS in the same PR or agents will re-break Windows hide/stdio.

---

## Suggested first cut (if building)

1. Lock the capture contract: stdio detach, ordered-in hide, dedicated overlay, visible REC bar, window ≠ fullscreen (#1–#6).
2. Windows targeting: real window list, display picker, audio-switch honesty (#13, #21–#24, #36–#37).
3. Library/Inbox hygiene: no emoji, thumbs, j/k, sidecar denylist (#41–#48, #71–#74).
4. Agent: `vibecap doctor`, smoke.ps1, GIF async, log cleanup (#92–#95).

Do **not** start a non-destructive NLE, OCR, or i18n until the Windows capture loop is boring.

---

# Round 2 — beat Snagit / Snipping Tool (2026-09)

Second hundred. Assumes the round-1 contract + the Snagit-class latency work
(SW_HIDE, WDA_EXCLUDEFROMCAPTURE, instant overlay, window click-pick, wake
pump) all hold. Numbered 1–100 for this round; `✓` = shipped in this pass.

## A · Capture interaction (feel & power)

1. **Capture delay timer** (0/3/5/10 s) — Snipping Tool parity; lets you open a menu or hover a tooltip before the shot. ✓
2. **Repeat last capture** — `R` in the overlay, palette verb, tray item: re-fires the exact same region/window/fullscreen without re-picking. ✓
3. **Window-pick for recording** — click a window, record its rect (crop record, no focus juggling). ✓
4. **Monitor pick** — in pick mode, hover dead space → highlight the whole monitor; click = capture that display. ✓
5. **Aspect-lock toolbar chips** (Free/1:1/16:9/9:16) in the region HUD, keeping Shift/Alt modifiers. ✓
6. **Move selection** — Space+drag or middle-drag inside the box repositions it.
7. **Post-drag edit handles** — resize from corners until Enter/click-outside; release no longer hard-commits.
8. **WASD nudge** alongside arrows (Shift = 10 px). ✓
9. **Double-click the last-region ghost** to instantly re-capture it. ✓
10. **Global Esc during pick** — a focus-loss can't orphan the overlay (listen on the pump).
11. **Clipboard-only stills** — copy and discard the file; never touches the library. ✓
12. **Clipboard format pref** — PNG vs JPEG for copy (PNG preserves sharp text edges).
13. **Shutter sound** — subtle click on capture; off by default.
14. **Pre-warm backdrop** — reuse the previous snap as the overlay's backdrop instantly, stamped "refreshing…" until the new snap lands. ✓
15. **PrtScn capture** — optional single-key still via a dedicated hotkey slot.
16. **Z-cycle in window-pick** — scroll wheel steps through overlapping windows under the cursor. ✓
17. **Pick card shows process + monitor** under the window title.
18. **Countdown on always-on-top viewport** — bubble must be visible while the studio is hidden (uses its own viewport, verify in live test).
19. **Menu-capture helper** — auto 1 s delay when the cursor sits inside an open menu.
20. **Physical-pixel readout** — W×H plate shows physical px when DPI ≠ 100 %. ✓
21. **Named size presets** — 1920×1080 / 1280×720 centered-box buttons in the HUD.
22. **Saved regions** — persist named rects to session; pick from palette.
23. **Loupe hex readout** — show the sampled pixel's #RRGGBB in the cursor loupe. ✓ (was already shipped)
24. **Dim-intensity setting** for the region overlay.
25. **Capture without cursor flash** — per-shot toggle in the HUD.

## B · Still editor (Snagit-editor territory)

26. **Arrow tool** — line with head, stroke/color-aware.
27. **Rectangle / ellipse outline** tools.
28. **Blur / pixelate region** — the Inbox "Blur the token" snippet wants this to exist.
29. **Spotlight** — dim everything outside a rect.
30. **Badge style presets** — Snagit step-tool look variants (circle/square, filled/outline).
31. **Highlighter pen** — ~50 % alpha stroke mode.
32. **Stroke straighten** — near-straight freehand becomes a line.
33. **Text background box** — label look with fill + padding.
34. **Canvas padding + fill color** on crop.
35. **Edge effects** — border, torn edge, drop shadow presets.
36. **Watermark preset** — text or logo at corner with opacity.
37. **Annotation undo/redo** — Ctrl+Z / Ctrl+Y stack (per-stroke).
38. **Resize-for-export** — % or max-width field in the Still inspector (img_resize_pct exists).
39. **Export format per save** — PNG/JPEG/WebP choice.
40. **Copy original vs annotated** choice (today annotated wins).
41. **Paste image onto canvas** — combine shots, Snagit-style.
42. **Hold-Space before/after** preview of annotations.
43. **Measure tool** — px distance readout between two clicks.
44. **Ruler / grid overlay** toggle in Still canvas.
45. **Zoom-to-fit / 100 % quick keys** (Ctrl+0 / Ctrl+1).

## C · Video & GIF

46. **Window-record via pick** — same WindowPick overlay → record rect (crop record, no focus juggle). ✓
47. **Follow-cursor recording** — crop rect pans with the pointer (for zoomed tutorials).
48. **Webcam bubble** — second gdigrab/dshow source composited corner-overlay (big).
49. **Mic + system mix** — dshow device list exists; add a mix selector + level meters.
50. **Pause/resume hotkey** — dedicated digit.
51. **REC bar source line** — shows target rect/monitor + audio state.
52. **REC bar position memory** — draggable, persists.
53. **Marker hotkey during record** — drops a chapter at press.
54. **Auto-trim dead air** — drop frames <N fps-change at head/tail on finalize.
55. **Output presets** — CRF, fps, codec (H264/H265/VP9) in Settings.
56. **GIF ping-pong loop** toggle.
57. **GIF frame delete** in the filmstrip.
58. **GIF per-frame delay** editor.
59. **Re-export GIF** from an existing MP4 at new fps/width (no re-record).
60. **WebM / AV1 output** option.
61. **Stream-copy trim** — no re-encode when only cutting ends.
62. **Clip audio in preview** — today's player is silent.
63. **Frame → still** — grab the current preview frame as a new screenshot.
64. **Batch re-export** selection from Library.
65. **Recording countdown styles** — 3 / 5 / none setting.

## D · Clipboard & destinations

66. **Clipboard history** — last 10 captures in a tray submenu + palette.
67. **Copy as Markdown image** — `![](path)` for docs. ✓
68. **Copy file URI / data URI** for devs. (path copy shipped via Ctrl+C, toast, palette; URI variant open)
69. **Copy + reveal combo** action on the toast card.
70. **Auto-open editor** toggle (some flows never want Review).
71. **OS drag-out** of the capture card thumbnail into Explorer/Slack (open item).
72. **Size guard hint** — warn + auto-shrink offer when a still exceeds Discord's 8 MB.
73. **Post-capture actions menu** — copy path / reveal / open / delete right on the card. ✓ (was already shipped: Annotate · Copy Image · Copy Path · Reveal · Discard)

## E · Library

74. **Filename search** (beyond date-group browsing).
75. **Favorites / pins** — float to top.
76. **Tags** with filter chips.
77. **Export selection as ZIP**.
78. **Sort** — date/size/duration/name.
79. **Retention rules** — keep N days or N files.
80. **Duplicate detection** — content-hash same-shot warnings.
81. **Thumbnail repair** — regenerate missing thumbs.
82. **Open-with…** menu per item.
83. **Review-queue flag** — "needs attention" marker.
84. **Recently-deleted view** — surface `undo_trash` as a shelf.

## F · Hotkeys, tray, system

85. **Hotkey rebind UI** — Settings editor, applies without restart (open #81).
86. **Per-mode hotkeys** — region-still / window-still / GIF / pause.
87. **Tray recent-captures** submenu.
88. **Tray pause/resume** item during record.
89. **Tray "repeat last capture"** item. ✓
90. **Tray double-click = screenshot** option.
91. **CLI poke running GUI** — `vibecap --capture` forwards to the single instance.
92. **Watch-folder import** — drop shots into the library dir.
93. **Portable mode** — config beside the exe.
94. **Profile export/import** — settings as a file.
95. **Silent mode** — suppress toasts + flash. ✓
96. **Update toast with changelog** link.
97. **First-run health check** — ffmpeg, DPI awareness, write-perms, tray.
98. **`?` cheat sheet** — in-app shortcut overlay. ✓
99. **Stats card** — captures this week, bytes, streak.
100. **Crash-recovery** — restore unsaved annotations on next launch.

### Round-2 first cut (built this pass)

Delay timer (1), repeat-last via R / palette / tray (2, 89), window-pick→record
(3, 46), aspect chips (5), WASD nudge (8), ghost double-click (9),
clipboard-only stills (11).

### Round-2 second cut (built this pass)

Monitor pick on dead space (4), pre-warm backdrop (14), Z-cycle scroll (16),
physical-px readout (20), Markdown + path palette verbs (67, 68), silent mode
(95), `?` cheat sheet (98). Loupe hex (23) and post-capture actions (73) were
already shipped — marked, not re-built.

### Round-2 deliberate skips (for now)

Scrolling capture + OCR/grab-text (need a real engine, not a feature flag);
transparent live overlay (egui child viewports can't composite transparency
on Windows — a native layered HWND is a separate project); PrintScreen
*takeover* of the OS key (registry-level, revisit).

---

# Round 3 — 300 improvements (2026-09-20)

Third tranche. Assumes rounds 1–2 plus the five-theme design system
(Carbon / Mono Dark / Mono Light / Celestial / Celestial Pink), the mono-ui
component port (font weights, caps labels, chips, ghost icon buttons), and the
exclusion-based record flow all hold. Grounded in the tree as of `d337634`.
Numbered 1–300 for this round. Sections sized 25 each.

## A · Design system & theme polish (1–25)

1. **Per-theme density scale** — compact/cozy/comfortable spacing token per theme; Celestial can afford airier gaps than Mono.
2. **Theme-aware elevation model** — three shadow tiers (rest/raised/overlay) in tokens instead of only `popup_shadow`/`window_shadow`.
3. **Accent-hue slider for celestial modes** — rotate the aurora hue ±40° while keeping the sky structure.
4. **Theme preview in picker shows real chrome** — mini mock of rail + card + CTA inside each swatch, not just an aurora strip.
5. **Auto theme** — follow Windows light/dark for the Mono pair; celestial modes stay manual.
6. **Scheduled themes** — Light by day, Dark/Celestial by night (opt-in).
7. **Contrast audit pass** — run a contrast checker over every `TEXT_MUTED`/`TEXT_FAINT` usage; celestial muted on plum is borderline.
8. **Focus ring token** — real `FOCUS_RING` color per theme, painted on keyboard focus for every interactive widget.
9. **Disabled-state token set** — `*_DISABLED` fill/text pair instead of ad-hoc `.weak()` calls.
10. **Hover animation** — egui `ctx.animate` on button fills; 80–120 ms ease like the mockup's `--ease`.
11. **Pressed-state scale** — 0.98 shrink on primary CTAs for tactile feel.
12. **Icon stroke-width consistency** — audit `icons.rs` strokes; loupe/camera glyphs draw heavier than nav glyphs.
13. **Icon sizing token** — `ICON_SM/MD/LG` (14/18/22) instead of scattered pixel sizes.
14. **Letter-spacing token for caps labels** — `caps_label` hardcodes 1.0; make it a token so Celestial can track wider.
15. **Numeric font feature** — tabular figures for REC timer, size columns, budget readouts (Segoe UI `tnum` or a mono face).
16. **Mono face for code/path text** — paths, durations, and `kbd` chips should share one mono family token.
17. **Theme diff in screenshot tests** — golden-frame capture per theme to catch alpha/token regressions like the premultiplied bug.
18. **Carbon accent review** — Carbon currently inherits zinc accent; give it a slate-blue tint to match Tailwind slate-400 hover states.
19. **Celestial card inner-glow** — 1 px top inner highlight (`rgba(255,255,255,.06)`) like Chromie's glass cards.
20. **Toast severity left-bar** — already colored; add matching icon tint + semantic icon per severity.
21. **Empty-state art** — one small line-art glyph per empty surface (Library, Inbox, Recents) instead of bare text.
22. **Skeleton loaders** — shimmer rect where thumbs/frames are pending instead of blank tiles.
23. **Reduced-motion setting** — disable aurora pulse, hover fades, toast slide.
24. **Starfield parallax** — stars drift 1–2 px on window resize for depth (cheap: offset by rect delta).
25. **Theme export/import** — share a theme as a JSON snippet; community themes later.

## B · Layout, rail & navigation (26–50)

26. **Collapsible rail** — icon-only 48 px mode; labels on hover tooltip.
27. **Rail badges** — numeric badge on Inbox (pending count), dot on Library (new items since open). ✓ (Inbox count shipped; Library new-dot open)
28. **Rail section labels** — CAPTURE / REVIEW / SYSTEM group dividers in expanded mode.
29. **Rail drag-reorder** — let users pin favorite stages to top.
30. **Keyboard rail nav** — Ctrl+1..5 jump to stages; shown in `?` sheet. ✓ (was already shipped)
31. **Breadcrumb in Review** — `Library › clip_name` so Esc-depth is visible.
32. **Back button** — in-header ‹ Back for Review/Clip/Still; Alt+← binding. ✓ (Alt+←/→ shipped; header button open)
33. **Window-size memory per stage** — Library wants wide; Capture wants narrow.
34. **Min window size enforcement** — below 720 px the rail overlaps content; clamp or collapse.
35. **Adaptive column width** — the 720 px content column should widen on >1100 px windows.
36. **Status strip resize drag** — give the bottom bar a 2 px taller hit target.
37. **Status strip segments clickable** — click "2 recordings" → jump to Library filtered.
38. **Right-side inspector mode** — optional docked metadata panel in Review screens.
39. **Zen mode** — hide rail + status strip; palette + hotkeys only.
40. **Header title dynamic** — show contextual title (recording name in Clip, file name in Still) instead of always stage name.
41. **Subtitle slot in header** — second line under title for context ("unsaved changes", "recording 00:12").
42. **Command palette recent verbs** — MRU section above the flat list. ✓
43. **Palette fuzzy match** — substring scoring; "gif" should rank "Export GIF" first. ✓ (subsequence scoring, word-start/consecutive bonuses)
44. **Palette actions show shortcuts** — right-aligned kbd hint per row.
45. **Palette media jump** — typing a filename jumps to its review.
46. **Tab-strip alternative** — optional top tabs instead of rail for users who want Snagit familiarity.
47. **Drag window by any dead space** — today only header drags; padding zones should too.
48. **Snap-layout friendly sizing** — default size lands cleanly in Windows 11 half-snap.
49. **Restore-last-stage on launch** — setting: always Capture vs resume where you left.
50. **Stage transition direction** — slide left/right matching rail order, not a single wipe.

## C · Capture tab & flow (51–75)

51. **Per-target memory** — remember Region vs Window per session *and* per hour-of-day.
52. **Shutter button split-menu** — chevron on Screenshot offering Region/Window/Full variants without leaving the row.
53. **Capture preview strip on hover** — hovering a recent tile grows it 1.5× with play.
54. **Drag recent tile out** — straight to Explorer/Slack from the capture card.
55. **Recent tile quick-actions** — hover overlay: copy / annotate / delete on recents.
56. **Recents carousel** — horizontal scroll when >3 items instead of hiding them.
57. **"Waiting for capture" state** — while armed+hidden, the studio (if shown) should say so.
58. **Options card quick toggles** — cursor/audio/display as icon toggles, not buried in disclosure.
59. **Audio device picker** — dropdown of dshow devices when Include audio is on.
60. **Estimated file size** — live "≈4 MB/min @ 30fps" under Record. ✓ (STORAGE group in Options, scaled by fps + monitor mpx)
61. **Disk-space guard** — warn <500 MB free on the target dir before arming. ✓ (GetDiskFreeSpaceExW; warning under shutter + free-space line in STORAGE)
62. **Battery-aware hint** — on battery, suggest 24 fps / shorter clips.
63. **Capture history sparkline** — tiny 7-day activity graph on the Capture card.
64. **Quick-capture tray-free mode** — double-press hotkey within 500 ms = instant region with last settings.
65. **Countdown cancel UX** — click anywhere or Esc during countdown aborts cleanly with toast.
66. **Post-capture inline undo** — toast gets an Undo for 5 s on auto-save.
67. **Auto-scroll to options** — when Record selected, scroll options card into view.
68. **Source icons state-colored** — the From segment icons tint to accent when active.
69. **Window target shows last pick** — "Window: Chrome — DevTools" persisted on the card.
70. **Confirm-before-overwrite** — same-name collision in output dir prompts once per session.
71. **Multi-shot batch** — hold modifier + click regions repeatedly = rapid sequence of stills.
72. **Time-lapse mode** — capture frame every N sec into a video (stills → mp4).
73. **Scheduled capture** — "in 10 min, grab this window" for meetings.
74. **Clipboard watcher mode** — studio stays parked; a shot auto-opens Still review.
75. **Capture sound per action** — distinct subtle tones for still/record-start/record-stop.

## D · Region & window pick HUD (76–100)

76. **HUD size readout follows cursor** — W×H plate avoids cursor side automatically.
77. **HUD crosshair magnifier on demand** — hold Ctrl for loupe instead of always-on.
78. **Region edge snapping** — snap to window edges/screen edges within 8 px.
79. **Region guides** — smart alignment guides to other visible window rects.
80. **Dark/light HUD chrome auto** — HUD inverts on very bright backdrops for contrast.
81. **HUD button size scales with selection** — tiny regions get a compact toolbar.
82. **Region from keyboard only** — arrows move a growing box from center; Enter commits.
83. **Preset aspect preview tint** — locked-aspect regions tint the dim outside differently.
84. **Multi-monitor dim** — only the active monitor dims; others stay lit.
85. **Pick-confirm sound** — soft tick on mouse-up valid region.
86. **Region min-size guard** — <8×8 drag shows "too small" instead of capturing noise.
87. **Region grid overlay** — thirds/quarters toggle in HUD for composition.
88. **Window pick confidence flash** — highlight border pulses once on hover-lock.
89. **Window pick excludes overlays** — our own HUD/REC bar never appear in the pick list.
90. **Alt=child-window pick** — drill into tooltips/menus as separate regions.
91. **Region coordinates copy** — click W×H plate copies `x,y,w,h` for scripts.
92. **Region color-sampler mode** — click samples hex under cursor to clipboard (design pick).
93. **Freeze-frame toggle** — optional freeze of backdrop while picking (already static on Windows; make it a toggle for parity).
94. **HUD remembers toolbar side** — toolbar docks top or bottom per last use.
95. **Cancel zone hint** — first-time hint "Esc to cancel" fades after 3 uses.
96. **Region history stack** — Ctrl+Z steps back through previous rects this session.
97. **Scroll-wheel region resize** — wheel adjusts width, Shift+wheel height.
98. **Touch/stylus support** — pen drag works; palm rejection via contact size.
99. **Pick while maximized** — studio shouldn't restore to pick; verify parked path keeps working.
100. **HUD theme variant** — HUD always uses a neutral dark chrome regardless of app theme.

## E · Still review & annotation (101–125)

101. **Undo/redo stack** — per-stroke Ctrl+Z/Ctrl+Y (open since round 1). ✓ (snapshot stack now feeds Undo+Redo in both Still and the annotate modal; Clear/Reset are undoable)
102. **Real blur bake** — box-blur/mosaic into pixels on export, not a translucent overlay. ✓ (was already shipped — `pixelate_rect` bakes real mosaic)
103. **Drag-crop on canvas** — visual crop handles replace the four numeric fields. ✓ (was already shipped — crop_drag on the Still canvas)
104. **Zoom/pan canvas** — wheel zoom, space-drag pan, Ctrl+0 fit, Ctrl+1 100 %. ✓ (was mostly shipped; this pass: scroll zoom is hover-gated, `1` = true 100 %)
105. **Arrow tool** — with head size + color from the stroke state. ✓ (was already shipped)
106. **Shape tools** — rect/ellipse outline + filled modes. ✓ partial (rect shipped; ellipse open)
107. **Highlighter** — 50 % alpha stroke. ✓ (was already shipped)
108. **Step tool** — auto-numbered badges that renumber on delete. ✓ (was already shipped)
109. **Spotlight** — dim outside a rect.
110. **Measure tool** — px distance + angle readout.
111. **In-place text editing** — click canvas, type there; no separate field. ✓ (was already shipped — `text_edit_at` Area)
112. **Text background chip** — filled label look with padding + radius. ✓ (was already shipped — OVERLAY_LABEL pill)
113. **Annotation color palette** — 6 swatch row + custom hex.
114. **Stroke width presets** — 2/4/8 chips + slider.
115. **Copy original vs annotated** — explicit choice in the copy menu. ✓ ("Copy original (no markup)" in the Still ⋯ menu)
116. **Save-as-copy default** — never silently overwrite the source still. ✓ (was already shipped — "Save as copy" + explicit overwrite)
117. **Export format picker** — PNG/JPEG/WebP + quality slider.
118. **Resize-on-export** — % or max-width field with pixel preview.
119. **Paste-onto-canvas** — clipboard image becomes a movable layer.
120. **Before/after hold** — hold Space to peek the un-annotated original.
121. **Watermark preset** — corner text/logo with opacity.
122. **Canvas padding** — add uniform border pixels with fill color on export.
123. **Annotation list panel** — side list of strokes; click selects, Del removes.
124. **Snap annotations** — arrows/shapes snap to 15° angles and edges. ✓ (Shift-drag: arrows → 15°, rect/blur → square)
125. **Esc depth-fix** — Esc exits annotate → Still → Capture, never a blank tab. ✓ (was already shipped)

### Round-3 second cut (built this pass)

Real undo **and** redo on a shared snapshot stack (101), Shift-snap for
arrows/rects/blur (124), copy-original-without-markup (115), scroll-zoom
gated to canvas hover + `1` for true 100 % (104). Verified shipped, now
marked: real mosaic blur bake (102), drag-crop (103), zoom/pan (104),
arrow/rect/highlight/step tools (105–108), in-place text (111), text
background chip (112), save-as-copy (116), Esc depth (125).

## F · Clip review, video & GIF (126–150)

126. **Preview audio** — extract audio track alongside filmstrip; play in sync.
127. **Determinate extract progress** — "frame i/n" bar instead of indeterminate label. ✓ (progress channel streams decoded/total)
128. **Scrub-bar seek** — click/drag the ruler moves the preview head. ✓ (transport slider + click canvas; Space/Home/End added)
129. **Frame-step keys** — ←/→ one frame, J/K 10 frames. ✓ (←/→ existed; J/K ±10 + I/O trim-at-playhead added)
130. **In/out loop** — preview loops the marked range. ✓ (was already shipped — `L` toggles)
131. **Trim-verify export** — assert ffmpeg `-ss/-to` equals the ruler seconds.
132. **Stream-copy trim** — `-c copy` when codec allows; instant cut. ✓ (was already shipped — Trim video uses `-ss/-to -c copy`)
133. **Export preset chips** — Discord 8 MB / README 480p / lossless. ✓ (was already shipped — PRESETS group)
134. **GIF settings dialog** — fps, width, loop mode, size estimate pre-encode. ✓ (was already shipped — sliders + ~KB estimate)
135. **GIF ping-pong** — boomerang loop toggle.
136. **GIF frame ops** — delete frames, per-frame delay in filmstrip.
137. **Re-GIF existing MP4** — new settings without re-recording.
138. **WebM/AV1 export** — codec picker on the export row.
139. **Extract audio** — one-click `.m4a` from clip. ✓ (was already shipped — AUDIO group)
140. **Frame-grab** — current preview frame → new still in Library. ✓ (transport "Grab frame" → ffmpeg `-ss -vframes 1` JPG)
141. **Chapter markers** — marker hotkey during record; ticks on the ruler. ✓ (was already shipped — sidecar + ruler ticks)
142. **Marker list** — click a marker to jump the preview. ✓ (clickable timecode chips under the ruler)
143. **Auto-trim dead air** — detect frozen head/tail, offer trim. ✓ (`dead_air_bounds` on filmstrip RGBA; "Trim to content" banner)
144. **Speed ramp** — 0.5×/2× segments (stretch goal, simple `-setpts`).
145. **Clip notes** — text sidecar shown under the player.
146. **Compare mode** — split-screen before/after trim preview.
147. **Player always-visible Open** — real-player fallback button lives in chrome, not only on error. ✓ (transport bar)
148. **Preview quality toggle** — half-res filmstrip for long clips.
149. **Auto-play setting** — the autoplay we shipped becomes a Settings toggle. ✓ (session-persisted `clip_autoplay`)
150. **Clip deletion guard** — deleting a recording with unsaved trims asks once.

## G · Library (151–175)

151. **Filename search** — filter-as-you-type in the header. ✓ (was already shipped)
152. **Search sidecars** — `.txt` notes + transcript text indexed.
153. **Sort menu** — date/size/duration/name/type. ✓ (⇅ menu; non-date sorts drop group headers)
154. **Favorites** — ★ floats to top, filter chip.
155. **Tags** — free-form tags + colored filter chips.
156. **Date groups** — Today/Yesterday/This week/Earlier headers (round-1 open item). ✓ (was already shipped)
157. **List view** — dense row alternative to the tile grid.
158. **Tile size slider** — S/M/L thumbnails. ✓ (segmented S/M/L in header)
159. **Hover-scrub** — moving across a video tile plays frames (filmstrip reuse).
160. **Hover quick-actions** — copy/reveal/delete overlay on tiles.
161. **Multi-select ops** — bulk export ZIP, bulk delete, bulk tag.
162. **Export selection as ZIP** — one archive via system dialog.
163. **Drag out to Explorer** — real OS drag source (open since round 1).
164. **Drag in to import** — drop files onto Library to copy in.
165. **Duplicate detection** — content-hash warning badge.
166. **Retention rules** — keep N days/files; run on idle.
167. **Storage bar** — per-type usage + "free X by cleaning frames_temp".
168. **Recently deleted** — undo_trash surfaced as a shelf with restore.
169. **Open-with menu** — per item, system default vs pick app.
170. **Thumbnail repair** — regenerate missing/failed thumbs in background.
171. **Sidecar hygiene** — extend denylist; sweep stale `frames_temp`, `.clean.mp4`, `.ffmpeg.log`.
172. **GIF↔clip routing** — GIFs offer both "trim as clip" and "still frame".
173. **Reveal-in-folder on tile** — hover icon opens Explorer with file selected. ✓ (↗ ghost button, thumb top-right)
174. **Selection count bar** — floating action bar appears when ≥1 selected. ✓ (was already shipped)
175. **Library empty-state CTA** — "Take your first screenshot" button routes to Capture.

## H · Inbox & HITL (176–200)

176. **j/k thread nav** — keyboard-first list traversal. ✓ (was already shipped)
177. **a = first chip** — one-key approve for choice requests. ✓
178. **Esc returns to list** — consistent back-depth. ✓ (clears selection; auto-reselect suppressed)
179. **Snooze + pin** — thread-level controls with restore. ✓ (pin existed; snooze now a 15m/1h/4h menu)
180. **Tray quick-reply** — approve/deny from tray without showing window.
181. **Poll age readout** — "agent polled 12 s ago" per thread.
182. **Answered-history search** — find past Q&A + attached media.
183. **Saved snippets** — reusable replies ("blur the token", "re-record 16:9").
184. **Voice reply preview** — waveform + re-record before send.
185. **Compose lock** — new arrivals never steal selection mid-compose.
186. **Markdown-lite composer** — backticks, links render on the agent side.
187. **Deep links** — `vibecap://feedback/<id>` opens exact thread.
188. **Request grouping** — threads collapse by agent/session.
189. **Unread divider** — "new since you last looked" line.
190. **Bulk approve** — shift-select threads → approve all.
191. **SLA colors** — threads age-tint (green→amber→red) as they wait. ✓ (timestamp goes amber >10m, red >30m; relative age shown)
192. **Attachment preview** — media_path renders inline thumb in thread.
193. **Reply templates per request type** — screenshot-needed vs approval prompts get different quick replies.
194. **Notification dedupe** — repeat polls for same request don't re-toast. ✓ (was already shipped — `feedback_notified_ids`; snoozed ids now drop out so expiry re-fires)
195. **Quiet hours** — inbox toasts suppressed, badge still counts. ✓ (manual Quiet toggle in Inbox header + Settings, session-persisted; badge/tray still update)
196. **Agent identity** — which harness/model filed the request, in the header. ✓ (was already shipped — `agent_label` in thread row + detail header)
197. **Request cost** — budget spent by this thread's session so far.
198. **One-click resolve** — mark done without a reply. ✓ (was already shipped — "Dismiss" in the composer)
199. **Inbox filter chips** — pending / answered / snoozed / expired.
200. **Keyboard composer send** — Ctrl+Enter sends; documented hint in-field. ✓

## I · Tray, hotkeys & OS integration (201–225)

201. **Hotkey rebind UI** — Settings editor, applies live (round-1 open). ✓ (`rebind_global_hotkeys` unregisters/re-registers; conflict toast names the taken combo)
202. **Per-mode hotkeys** — region-still, window-still, GIF, pause each rebindable.
203. **Hotkey conflict detect** — warn when binding collides with OS/browser.
204. **Tray recent-captures** — last 5 items submenu with copy/reveal. ✓ (5 slots follow the library scan; click opens the file)
205. **Tray pause/resume** — during record.
206. **Tray double-click action** — configurable (screenshot / open / record).
207. **Tray icon state** — REC blink baked into icon while recording. ✓ (was already shipped — IconPhase rec disc + arc)
208. **Tray recording elapsed** — tooltip shows `REC 02:41`. ✓ (was already shipped — `Recording {clock}` tooltip)
209. **Single-instance GUI** — second launch focuses first unless `--mcp`/`--screenshot`. ✓ (was already shipped — `gui.lock` pid file + `activate_own_app`)
210. **CLI poke** — `vibecap --capture` forwards to the running instance.
211. **Watch-folder import** — monitor a dir, auto-add shots.
212. **Portable mode** — config/session beside the exe.
213. **Profile export/import** — settings + hotkeys as one file.
214. **Update checker** — GitHub Releases poll, opt-in, changelog toast.
215. **Auto-update channel** — staged: check → download → apply on exit.
216. **Context-menu verb** — Explorer right-click "Annotate with Vibecap" on images.
217. **Share target** — Windows share contract so apps can send Vibecap images.
218. **Startup-on-login option** — tray-only resident mode.
219. **Notification-area copy** — all "menu bar" strings fixed for Windows.
220. **Windows permissions card** — mic/loopback device, tray status, gdigrab test.
221. **First-run health check** — ffmpeg, write-perms, DPI awareness, tray — one green card.
222. **Crash-recovery** — unsaved annotations/session state restored on relaunch.
223. **? cheat-sheet kept current** — auto-generate from the binding table, not hand-maintained.
224. **Keyboard-only walkthrough** — wizard step that teaches S/R/Esc in 30 s.
225. **OS dark-mode event** — live-switch Mono themes when Windows toggles.

## J · Performance (226–250)

226. **Thumb decode off-thread** — `egui_extras` loader already async; verify no decode on UI thread for large files.
227. **Thumb disk cache** — `.vibecap/thumbs` exists; add LRU cap (e.g. 500 MB) + stale sweep.
228. **Filmstrip parallel extract** — ffmpeg `-vsync` batch or threaded frame pull.
229. **Lazy library page** — only render visible tiles; 1000-file folders shouldn't instantiate 1000 widgets.
230. **Region backdrop reuse** — keep last snap texture; skip re-grab when <2 s old.
231. **DPI-aware texture cache** — don't re-rasterize icons on scale change storms.
232. **Font load once** — semibold/bold loads measured; cache family lookups.
233. **Repaint-on-demand** — idle app shouldn't repaint 60 fps; only on input/state change.
234. **Recording finalize off-thread** — shipped for stop; extend to remux/GIF queue.
235. **GIF encode queue** — background worker with progress, not a stop-blocking transcode.
236. **Startup time budget** — cold launch → interactive <800 ms; measure and track.
237. **Memory ceiling check** — long sessions with big thumbs shouldn't exceed ~300 MB.
238. **PowerShell spawn removal** — replace `frontmost_app_name`/`window_rect_on_screen` shell-outs with `windows` crate calls (round-1 #28, still the biggest latency item).
239. **Window-list cache** — 500 ms TTL on the pick-list enumeration.
240. **ffmpeg path resolve once** — resolved at startup, not per-capture.
241. **Starfield precomputation** — star positions hashed once, not per-frame.
242. **Gradient mesh cache** — sky mesh rebuilt only on resize/theme change, not repaint. ✓ (shape-list cache keyed by rect+mode)
243. **Toast timer coalescing** — one timer drives all toast lifetimes.
244. **Session write debounce** — don't serialize+write session on every state change; batch 500 ms.
245. **Log ring-buffer** — `.ffmpeg.log` tail kept in memory for doctor, not re-read from disk.
246. **Parallel test capture** — smoke tests run gdigrab in parallel with unit tests.
247. **Binary size audit** — strip symbols, LTO release; target <15 MB installed.
248. **Cold-start no-network** — update check must never block first paint.
249. **Large-file still guard** — >25 MP stills decode at half-res for canvas, full-res on export.
250. **Idle CPU zero** — hidden/tray app should sit at 0 % CPU, verified in CI.

## K · Reliability, diagnostics & telemetry (251–275)

251. **doctor --fix** — auto-remediate missing ffmpeg PATH, bad output dir, stale session.
252. **doctor JSON mode** — `--json` for agent parsing.
253. **Last-error surface** — persistent "last capture error" in Settings + tray tooltip.
254. **Crash log capture** — panic hook writes `vibecap-crash.log` beside session.
255. **ffmpeg stderr ring** — keep last 200 lines per recording for post-mortem.
256. **moov-verify on stop** — probe the MP4 before declaring success; auto-remux retry.
257. **Session schema versioning** — migrate old session.json fields cleanly.
258. **Config validation** — bad values (negative fps, missing dir) clamp + warn, not crash.
259. **Windows CI smoke** — gdigrab one-frame test asserting file size (round-1 #99, still open).
260. **Golden-theme CI** — screenshot-diff the five themes to catch alpha regressions.
261. **Input-fuzz test** — rapid region-drag/cancel sequences can't orphan the overlay.
262. **Kill-recovery test** — terminate mid-record; next launch must finalize or clean the partial file.
263. **ffmpeg-missing UX** — capture buttons disable with a clear fix-it card, not a post-click error. ✓ (fix-it card on Capture with copyable install cmd + live Re-check via `ffmpeg_recheck()`)

### Round-3 first cut (built this pass)

ffmpeg fix-it card + recheck (263), recording MB/min estimate + free-space
readout (60, 61), Library sort menu / tile-size S·M·L / hover reveal
(153, 158, 173), palette fuzzy scoring + MRU section (43, 42), celestial
sky shape cache (242). Verified shipped-not-marked: Ctrl+1–5 (30),
Alt+←/→ (32), Inbox rail badge (27), filename search (151), date groups
(156), selection bar (174).
264. **Self-update rollback** — bad update keeps previous binary.
265. **Instance handshake** — MCP + GUI detect each other; avoid dual capture locks.
266. **Clock-skew guard** — recording timestamps survive timezone changes mid-clip.
267. **Output-dir move handling** — deleted/moved dir → recreate or prompt, never silent fail.
268. **Long-path support** — >260 char paths via `\\?\` prefix on Windows.
269. **Unicode filename safety** — emoji/non-ASCII in naming tokens don't break ffmpeg args.
270. **Concurrent capture guard** — two rapid hotkey presses can't spawn two ffmpeg procs.
271. **Tray-missing fallback** — if tray creation fails, keep a floating mini-bar alive.
272. **DPI-change mid-pick** — region rect re-maps if scaling changes while overlay is up.
273. **Monitor-hotplug handling** — disappearing display re-targets fullscreen gracefully.
274. **Timestamp monotonicity** — output names use monotonic seq when clock steps back.
275. **Telemetry opt-in** — anonymous capture-success/fail counts; off by default, no content ever leaves.

## L · CLI / MCP, settings & onboarding (276–300)

276. **`--json` on all CLI verbs** — machine-readable output for agents.
277. **CLI progress events** — `--record-status --watch` streams JSON lines.
278. **MCP tool parity check** — doctor verifies every documented tool is registered.
279. **MCP error codes** — stable `ERR_*` codes agents can branch on.
280. **CLI dry-run** — `record start --dry-run` validates ffmpeg line without running.
281. **`vibecap open <id>`** — open a Library item straight in Review from CLI.
282. **CLI list** — `vibecap list [--type video] [--limit n]` prints media.
283. **CLI annotate** — `vibecap annotate file.png --arrow x1,y1,x2,y2` headless.
284. **Settings search** — filter box over all prefs.
285. **Settings sections as rail** — left-nav inside Settings instead of scroll.
286. **Setting tooltips** — every toggle explains its effect + default.
287. **Reset-to-default per section** — not just global reset.
288. **Wizard Windows page** — ffmpeg test-shot, mic check, hotkey conflict check.
289. **Wizard MCP detect** — find Cursor/Claude/Codex configs, offer snippet paste.
290. **Wizard theme pick** — live preview of all five, sets preference at first run.
291. **Re-open wizard** — "Replay setup" in Settings.
292. **In-app changelog** — What's New card after update.
293. **Docs links in-app** — ? icon → relevant doc section per tab.
294. **Stats card** — captures/week, bytes saved, streaks (round-2 #99, still open).
295. **Budget dashboard** — per-session spend sparkline in Inbox.
296. **Naming-token builder** — visual `{app}-{date}-{seq}` composer with live preview.
297. **Export diagnostics bundle** — one click → zip of logs+session+doctor for bug reports.
298. **Community theme repo hook** — `--theme-import URL` fetch+validate.
299. **Locale-ready strings** — wrap UI strings in a `tr()` macro now so i18n isn't a rewrite later.
300. **API surface freeze** — document which CLI/MCP contracts are stable vs internal for agent authors.
