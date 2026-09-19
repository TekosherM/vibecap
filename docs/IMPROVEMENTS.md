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
4. **Monitor pick** — in pick mode, hover dead space → highlight the whole monitor; click = capture that display.
5. **Aspect-lock toolbar chips** (Free/1:1/16:9/9:16) in the region HUD, keeping Shift/Alt modifiers. ✓
6. **Move selection** — Space+drag or middle-drag inside the box repositions it.
7. **Post-drag edit handles** — resize from corners until Enter/click-outside; release no longer hard-commits.
8. **WASD nudge** alongside arrows (Shift = 10 px). ✓
9. **Double-click the last-region ghost** to instantly re-capture it. ✓
10. **Global Esc during pick** — a focus-loss can't orphan the overlay (listen on the pump).
11. **Clipboard-only stills** — copy and discard the file; never touches the library. ✓
12. **Clipboard format pref** — PNG vs JPEG for copy (PNG preserves sharp text edges).
13. **Shutter sound** — subtle click on capture; off by default.
14. **Pre-warm backdrop** — reuse the previous snap as the overlay's backdrop instantly, stamped "refreshing…" until the new snap lands.
15. **PrtScn capture** — optional single-key still via a dedicated hotkey slot.
16. **Z-cycle in window-pick** — scroll wheel steps through overlapping windows under the cursor.
17. **Pick card shows process + monitor** under the window title.
18. **Countdown on always-on-top viewport** — bubble must be visible while the studio is hidden (uses its own viewport, verify in live test).
19. **Menu-capture helper** — auto 1 s delay when the cursor sits inside an open menu.
20. **Physical-pixel readout** — W×H plate shows physical px when DPI ≠ 100 %.
21. **Named size presets** — 1920×1080 / 1280×720 centered-box buttons in the HUD.
22. **Saved regions** — persist named rects to session; pick from palette.
23. **Loupe hex readout** — show the sampled pixel's #RRGGBB in the cursor loupe.
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
67. **Copy as Markdown image** — `![](path)` for docs.
68. **Copy file URI / data URI** for devs.
69. **Copy + reveal combo** action on the toast card.
70. **Auto-open editor** toggle (some flows never want Review).
71. **OS drag-out** of the capture card thumbnail into Explorer/Slack (open item).
72. **Size guard hint** — warn + auto-shrink offer when a still exceeds Discord's 8 MB.
73. **Post-capture actions menu** — copy path / reveal / open / delete right on the card.

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
95. **Silent mode** — suppress toasts + flash.
96. **Update toast with changelog** link.
97. **First-run health check** — ffmpeg, DPI awareness, write-perms, tray.
98. **`?` cheat sheet** — in-app shortcut overlay.
99. **Stats card** — captures this week, bytes, streak.
100. **Crash-recovery** — restore unsaved annotations on next launch.

### Round-2 first cut (built this pass)

Delay timer (1), repeat-last via R / palette / tray (2, 89), window-pick→record
(3, 46), aspect chips (5), WASD nudge (8), ghost double-click (9),
clipboard-only stills (11).

### Round-2 deliberate skips (for now)

Scrolling capture + OCR/grab-text (need a real engine, not a feature flag);
transparent live overlay (egui child viewports can't composite transparency
on Windows — a native layered HWND is a separate project); PrintScreen
*takeover* of the OS key (registry-level, revisit).
