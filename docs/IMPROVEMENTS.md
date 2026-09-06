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
