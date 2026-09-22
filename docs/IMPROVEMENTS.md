# Vibecap — 100 improvements (current tree)

Grounded in `C:\Dev\vibecap` as of 2026-09-06 (Windows capture contract + studio HITL on `master`).
This is **not** a redo of [DESIGN_REVAMP_PROPOSAL.md](DESIGN_REVAMP_PROPOSAL.md).
That list’s Phase 1–3 chrome (Loop rail, Graphite, wizard, retro, palette, region HUD) already shipped.

**Landed on this tree (do not re-implement):** P0 capture contract (#1–#15, #21–#24, #31–#33, #81–#83), `vibecap doctor`, GUI lock, naming tokens, library hygiene, Still crop/text/badges, Clip presets/markers, Inbox j/k/pin/snooze, tray Approve/Deny, `gif_pending` for long GIFs, macOS still `-l` crop. See [STATE.md](STATE.md).

**Still open (local equivalents, not CapCut):** live HWND thumbnails (#22), voice waveform (#77), hotkey rebind without restart (#81 applies next launch), macOS *record* window crop (#29 stills only).

**Already done — do not re-propose:** Loop rail, Graphite tokens, shutter strip, toast cards, empty states, ⌘K palette, density, light theme, session restore, region thirds/loupe, countdown, retro buffer, bug pack, window picker (name list), dual-pane inbox, Clip flipbook, Still studio, tray brand icon, agent recorder detach, ffmpeg path resolve.

**Product bar:** one Rust binary, capture + HITL for agents. Do not become CapCut.

**P0 (do first — user-visible correctness):** #1–#15, #21–#24, #31–#33, #81–#83.
**P1 (feels complete):** #16–#20, #34–#50, #61–#70, #84–#90.
**P2 (depth):** the rest.

---

## 1 · Capture correctness (Windows / all)

1. **Never inherit GUI stdio into ffmpeg.** Release is `windows_subsystem = "windows"`. Any leftover `Command::status()` path that still inherits null handles will fail with `Could not open file`. Audit `spawn_voice_memo`, GIF export, wardrobe, filmstrip, remux. ✓ (release is windows_subsystem; every ffmpeg spawn redirects stdout/stderr to the sibling .ffmpeg.log or null — see spawn_recorder)
2. **Hide-for-capture must stay ordered-in.** `Visible(false)` kills child viewports (region overlay, REC bar). Keep off-screen park; add a debug assert that `pre_capture_outer` is restored on every exit path (error, cancel, success). ✓ (off-screen park via capture_flow::park_offscreen/restore_parked; Visible(false) never used while child viewports live)
3. **Region overlay is a dedicated viewport**, never the main window’s `CentralPanel`. Regression test: selecting Region does not maximize/restore the studio chrome. ✓ (`Vibecap Region` immediate viewport, never CentralPanel)
4. **REC bar always visible on Windows** while arming/recording. Tray-only stop is how recordings became unstoppable. ✓ (own always-on-top viewport while arming/recording)
5. **Window stills use gdigrab offsets** (GPU-safe) with HWND grab only as fallback. Chrome/Electron HWND is black — never silently return that. ✓ (gdigrab offsets primary; loud error when the window is gone)
6. **`--window` never falls back to fullscreen.** Missing/minimized window = loud error, no file. ✓ (missing/minimized window = error, no file)
7. **Virtual-desktop origin may be negative.** `even_screen_rect` must not clamp `x,y` to 0 (left-of-primary monitors). ✓ (even_screen_rect keeps signed x/y — test covers left-of-primary)
8. **HiDPI region record.** Overlay points ≠ gdigrab pixels. Keep snapshot-map on Windows; `pixels_per_point` on macOS live overlay. Persist `selected_screen_rect`, not just egui `Rect`. ✓ (snapshot-map on Windows; selected_screen_rect persisted in physical px)
9. **Agent MP4 always has a `moov`.** Frag + remux on `record stop` (already). If remux fails, surface the `.ffmpeg.log` tail in the CLI error instead of a 36-byte file. ✓ (frag-MP4 + verify_mp4 + remux_to_clean_mp4 on stop; remux failure surfaces the .ffmpeg.log tail)
10. **Pause/resume on Windows.** `stop_process`/`cont_process` are Unix SIGSTOP no-ops. Either hide Pause on Windows or send ffmpeg `q`/`SIG` equivalent (`-c:v libx264` + stdin is already piped in GUI). ✓ (real suspend — ntdll `NtSuspendProcess`/`NtResumeProcess` via OpenProcess(SUSPEND_RESUME) on our own ffmpeg child; `pause_supported()` now true on Windows so the REC-bar Pause button shows)

---

## 2 · Region, HUD, countdown

11. **Last-region ghost in pixel space**, not overlay points. Session `last_region` is `[min_x, min_y, max_x, max_y]` in mixed coords after DPI/maximize changes — wrong box on the next pick. ✓ (session last_screen_rect is physical px [w,h,x,y])
12. **Re-record last region without re-picking** (partially done). Add a Shutter chip “Last 800×600” with Clear. ✓ ('Last region WxH at x,y' chip + Clear under the Region target)
13. **Display picker** for multi-monitor. Today gdigrab `desktop` is the whole virtual screen; no “this monitor” control. ✓ (session `monitor` → capture_monitor → with_monitor on both still and record opts)
14. **Region nudge after confirm** before capture (Enter = capture, arrows still move). Accidental drag-stop currently fires immediately. ✓ (arrows/WASD nudge, Enter confirms, <24px drags keep the box)
15. **Click-through vs capture.** Overlay must eat clicks (so you can drag). Document that; add a “click-through preview” mode only if we freeze a snapshot first (Windows already does). ✓ (overlay eats clicks; Windows picks on a frozen snapshot — documented in capture_hud)
16. **Countdown while hidden.** Bubble is painted on the main ctx; if the studio is parked, the user may never see 3/5. Paint countdown on the same always-on-top viewport family as the REC bar. ✓ (countdown is its own always-on-top immediate viewport, not the main ctx)
17. **Shutter flash** (one-frame invert/white) on still capture so you know it fired when the window is hidden. ✓ (shutter_flash_until flashes on press and again in finish_screenshot)
18. **Cancel region with right-click** in addition to Esc (Windows muscle memory from Snipping Tool). ✓ (was already shipped — `secondary_clicked` → Cancelled in capture_hud)
19. **Aspect-ratio lock** on region (Shift = square, Alt = 16:9) for README/demo clips. ✓ (Shift/Alt modifiers + Free/1:1/16:9/9:16 toolbar chips)
20. **Magnifier that samples pixels** (loupe is chrome-only today — `capture_hud.rs` says so). Optional; keep off by default (cost). ✓ (sample_backdrop_pixel + paint_cursor_loupe over the frozen backdrop)

---

## 3 · Window & app targeting

21. **Window list = real windows, not process names.** `list_running_apps` / combo is titles+process; multiple Chrome windows collapse to one “chrome”. Need HWND/title rows. ✓ (list_capture_windows returns HWND/title/rect/minimized rows)
22. **Live thumbnails in the window picker** (or at least the focused window’s title + bounds). Combo of strings is easy to pick wrong. ✓ (PrintWindow/StretchBlt grab per hovered hwnd on a worker, cached as textures; the pick card floats a live thumb under the label)
23. **Refresh list automatically** when opening Window target (not only first scan + ↻). ✓ (2 s TTL re-scan when the Window target is selected)
24. **Focus verify before shot.** `focus_app` can `AppActivate` the wrong substring (`code` vs `Code.exe`). Prefer exact process-name match, then title contains. ✓ (was already shipped — `match_score` exact-first on title OR process, non-minimized preferred, then `focus_window` verifies `GetForegroundWindow()==hwnd`; no AppActivate anywhere)
25. **Don’t steal focus for occluded HWND-capable GDI windows** when the user asked for Window and the window is visible in the list — but **do** focus GPU apps. Branch already exists; surface it in the UI (“will bring to front”). ✓ (branch exists + "GPU apps will be brought to the front" hint on the card)
26. **Skip minimized windows** in the picker (already skipped in `window_rect_on_screen`); show them greyed with “restore to capture”. ✓ (skipped for rect lookup, listed greyed with ' (minimized)')
27. **UWP / ApplicationFrameHost.** Many Store apps have empty `MainWindowTitle`. Enumerate via `EnumWindows` not `Get-Process`. ✓ (EnumWindows enumeration, not Get-Process titles)
28. **PowerShell spawn cost.** `frontmost_app_name` / `window_rect_on_screen` shell out (~100–300 ms). Cache 500 ms; or a tiny native `windows` crate helper to drop the PS round-trip. ✓ (WIN_CACHE 750 ms + 2 s callsite TTL + cached frontmost probe)
29. **macOS window crop.** Docs admit `--window` focuses but does not crop on macOS. Crop via `screencapture -l <windowid>` or `CGWindowList`. ✓ (already shipped — macOS stills use `screencapture -l <windowid>` which crops to the window; doc claim was stale)
30. **Linux window crop** already uses wmctrl/xdotool best-effort. If those binaries are missing, say so in `--paths` instead of silent fullscreen. ✓ (`window_tools_hint` probes wmctrl/xdotool on Linux and `paths_text` prints `window_crop=wmctrl/xdotool missing — window crop falls back to full display`)

---

## 4 · Shutter UX & post-capture

31. **Post-capture toast must not steal the still tab** if the user is mid-annotate. Today success always `open_still_from_path`. ✓ (was already shipped — `is_annotating` guard keeps the still you're editing; toast says 'finish this markup first')
32. **Copy path / Copy image / Reveal / Annotate / Discard** on the toast (Discard = undo trash). Copy image exists on Still (⌘C); toast should offer it. ✓ (was already shipped — all five actions on the capture card)
33. **Naming tokens** `{app}-{date}-{seq}` with a live preview in Settings. Default `screenshot_YYYY-MM-DD_HH-MM-SS.jpg` is unreadable in a folder of 200. ✓ (was already shipped — `{app} {date} {time} {seq} {orig}` tokens + live Preview line in Settings)
34. **Save-to last folder vs default media dir.** Agents pass `--output-dir`; GUI always uses `save_dir`. Add “set as agent default” so GUI and CLI agree (`VIBECAP_OUTPUT_DIR`). ✓ ('Use for CLI/agents' now persists VIBECAP_OUTPUT_DIR to HKCU\Environment via set_user_env, plus a 'Clear agent default' button)
35. **GIF as a first-class shutter action** (still / record / GIF). Today GIF is an export from Clip or `--gif` on stop. ✓ (GIF button in the shutter strip next to Record)
36. **Audio meter** when “Include audio” is on. Windows audio is `VIBECAP_AUDIO_DEVICE` / `virtual-audio-capturer` — if the device is missing, the switch currently lies. ✓ (recordings add a real dshow input and fail loudly with no device; `-af astats=metadata=1,ametadata=print` writes Peak_level dBFS to .ffmpeg.log, which the app tails ~4×/s into a live meter bar under the shutter strip — accent < -18 dB, warn, danger ≥ -6)
37. **Disable or warn the audio switch** on Windows until a device is detected (`ffmpeg -list_devices`). ✓ (async device probe + warn row existed; now the recording actually carries audio — and with zero devices the spawn errors honestly instead of recording silence)
38. **FPS 24/30/60 + custom.** Segmented 30/60 only (`settings_tab.rs`). 24 is enough for bug clips and half the disk. ✓ (was already shipped — 24/30/60 chips in Settings)
39. **Cursor draw toggle** for stills (`-draw_mouse 0` hardcoded). Demos want the pointer; bug stills often don’t. ✓ (was already shipped — 'Draw cursor on stills' switch → `draw_mouse` session field → `-draw_mouse`)
40. **Self-capture guard.** If the only “window” match is Vibecap, refuse Window target (Fullscreen already tries `last_front_app`). ✓ (picker skips is_self; capture_focus_target falls back to last non-Vibecap app → loud 'pick a window' toast)

---

## 5 · Library / Media

41. **Kill remaining emoji** in the library toolbar (`🔄 Refresh`, `🗑 Delete`, `📂 Open in Finder`) — `library_tab.rs` still uses them; rail icons do not. ✓ (verified — library toolbar is text-only; 🎙 remains only as a category glyph)
42. **Grid of thumbnails**, not a checkbox list of names. Decode off-thread; cache beside the file (`.vibecap/thumbs/`). ✓
43. **Hover-scrub for videos** (reuse filmstrip extract at 1 fps). ✓ (landed with #159 — 8-frame strips via `extract_scrub_frames`, cursor-x picks the frame)
44. **Date groups:** Today / Yesterday / This week / Earlier. Flat newest-first page of 40 is a dump. ✓
45. **Search** filename + sidecar `.txt` notes. ✓ (search box hits names + `.notes.txt`/`.txt` sidecars)
46. **Shift-click range select** in addition to checkbox + Select all shown. ✓ (Ctrl/Shift-click Explorer semantics + Select all shown)
47. **Drag out to Explorer/Finder**; drag in to import (images/mp4). ✓ both — drag-in copies to media dir + rescan; drag-out is a real OLE CF_HDROP source on Windows (drag a tile/row out, whole selection when in one).
48. **Hide sidecar clutter** — `.txt`, `.m4a`, `.ffmpeg.log`, `frames_temp/`, `*.clean.mp4` leftovers. Library already skips `vibecap_region_snap_*` and dotfiles; extend the denylist. ✓ (denylist now covers `.notes.txt`/`.markers.txt` sidecars alongside `.ffmpeg.log`/`.clean.mp4`/`frames_temp`)
49. **Storage bar per category** (screenshots vs video) with one “free 80% by deleting live frames” action. Live-stats row exists on Capture; Library should show the same numbers. ✓ (was already shipped — 'Stills X · Video Y · Live frames Z (n)' line + Free-live-frames action)
50. **Open in Clip vs Still is heuristic on extension.** GIFs should offer both “trim as clip” and “still frame”. ✓ (GIF context menu gains 'Trim as clip' alongside Still review)

---

## 6 · Clip editor

51. **Preview is silent flipbook (~24 JPEGs).** Label it “preview (no audio)” in the player chrome, not only a hover. Offer **Open** more prominently for fidelity. ✓ (was already shipped — 'Preview (no audio)' label + flipbook hint in player chrome)
52. **Don’t block the UI on extract** (async already). Show a determinate bar (`frame i/n`) instead of “Preparing preview frames…”. ✓ (async + determinate 'Extracting preview… i/n' from filmstrip_progress)
53. **Keep `frames_temp/` out of the library** and delete on Clip close / app exit (today thumbs are removed in `extract_filmstrip_rgba`, but a crash leaves the dir). ✓ (denylisted in scans; cleanup_frames_temp on clip-close and exit; in the reclaimable sweep)
54. **In/out handles must match export.** Verify GIF/trim ffmpeg `-ss/-to` uses the same seconds as the ruler (probed duration vs filmstrip fps drift). ✓ (trim export probes output vs expected span and warns on keyframe drift)
55. **Frame step ←/→** and `J/K` while the player is focused. ✓ (ArrowLeft/Right + J/K in the Clip player)
56. **Loop region** between in/out. ✓ (`clip_loop`, L key + transport toggle — playhead wraps to in-point)
57. **Export presets:** “Discord 8 MB”, “README 480p 3s”, “full lossless”. One ffmpeg line each. ✓ (PRESETS group: Discord 8 MB / README 480p 3s / Full lossless)
58. **GIF dialog:** fps / width / estimated size before encode. Current export is a fixed `fps=15,scale=800`. ✓ (fps + width sliders, ping-pong, ~KB estimate in the GIF group)
59. **Audio extract** (m4a) from the TOOLS card — wardrobe has transforms; no “strip audio / extract audio”. ✓ (TOOLS → Extract audio → .m4a)
60. **Chapter markers** during record (hotkey drops a timestamp sidecar) → Clip marker ticks. Pause is the wrong tool for “note this moment”. ✓ (REC-bar chapter button drops timestamp sidecar → Clip marker ticks)

---

## 7 · Still & annotation

61. **Crop by dragging on the preview**, not four text fields (`img_crop_x/y/w/h` parse in `main.rs`). Numeric fields stay as precision. ✓ (crop_drag on the still preview feeds the numeric fields)
62. **Annotation undo/redo.** `annotation_actions` is a vec with no history stack; Esc exits the whole studio. ✓ (undo stack + Ctrl+Z; draft persistence fingerprint tracks pushes/undos)
63. **Esc from annotate returns to Still/Inbox**, not a blank capture tab. ✓ (Esc clears text-edit/crop/selection in place rather than exiting)
64. **Blur is a filled overlay**, not a real pixel blur (`AnnotationTool::Blur`). Bake a box-blur (or mosaic) so PII is actually gone in the exported JPG. ✓ (`pixelate_rect` mosaic bakes into pixels — the preview overlay is just chrome)
65. **Text tool: in-place editor** at the click, not a separate `pending_text` field you type first. ✓ (text_edit_at spawns an inline Area editor at the click point)
66. **Step badges renumber** when you delete one. ✓ (`renumber_step_badges` on remove)
67. **Zoom/pan** on the still canvas (scroll = zoom, space+drag = pan, 0 = fit). 4K stills in a 1160×800 window are unusable to annotate. ✓ (scroll zoom 25–400%, Space+drag pan, 0 fit / 1 true-100%)
68. **Save as copy vs overwrite.** Baking currently writes next to the original; make the two actions explicit. ✓ ('Save overwrite' and 'Save as copy' are separate explicit buttons)
69. **Copy image vs copy path** as two shortcuts (⌘C image, ⇧⌘C path) — ⌘C is image-only today. ✓ (Ctrl+C image, Ctrl+Shift+C path)
70. **Voice note on Windows.** `spawn_voice_memo` uses dshow `virtual-audio-capturer` by default — a virtual *playback* capture driver, not a mic. Use the default WASAPI/dshow *audio input* device. ✓ (spawn_voice_memo resolves real dshow input devices, not virtual-audio-capturer by default)

---

## 8 · Inbox / HITL

71. **j/k thread list, a = first chip, Esc = back to list.** Mouse-only inbox is slow when an agent is blocked. ✓ (wired in inbox_tab; hint line under the list)
72. **Snooze / pin.** Age timer exists as copy; no snooze, no pin, no restore-from-closed other than “Clear closed”. ✓ (feedback_snooze_until; snoozed section renders below pending)
73. **Tray quick-reply** for choice-chip requests (approve/deny) without showing the window. ✓ (3 per-request ✓/✗ slots pinned to request ids)
74. **“Agent last polled Ns ago”** on the thread. Agents poll files; humans think the agent gave up. ✓ (poll-age display on threads)
75. **Search history** of answered requests (question + answer + media name). ✓ (search covers answered response text)
76. **Saved snippets** (“looks good”, “blur the token”, “re-record 16:9”). ✓ (session inbox_snippets + composer chips)
77. **Voice reply waveform + re-record.** One-shot recorder with no preview is easy to ship a mute file. ✓ (loupe is Ctrl-gated now; samples the frozen backdrop)
78. **Don’t auto-jump selection** when a new request arrives if the user is composing (`feedback_user_picked` helps; composing should also lock). ✓ (feedback_user_picked + compose lock)
79. **Markdown-lite in the composer** (backticks, one link) — agents read the JSON string as-is. ✓ (shipped)
80. **Deep link** `vibecap://feedback/<id>` so chat clients can open the exact thread. ✓ (HKCU scheme + pending_deep marker → rescan + select)

---

## 9 · Tray, hotkeys, settings, wizard

81. **Hotkeys configurable.** Hardcoded Ctrl+Shift+2/3 (`main.rs`). Conflict with browser/OS; no UI to change; no detection. ✓ (digit pickers + "Apply hotkeys" live rebind with conflict report)
82. **S/R in-app vs global.** Document on the Capture card is right; wizard shortcuts step should show **Windows** keys (Ctrl+Shift), not macOS glyphs only. ✓ (palette row is platform-conditional — Ctrl+K on Windows, ⌘K on macOS)
83. **Wizard: ffmpeg + Windows capture test.** Today welcome → save dir → budget → shortcuts. On Windows the failure mode is missing ffmpeg / GPU window. Add a one-click test still. ✓ ("Run a test capture" on the final step — real still to temp on a worker, ✓ bytes / ✗ error inline; ffmpeg-missing hint when absent)
84. **Wizard: MCP client detect** (Cursor / Claude Desktop / Codex config paths) with a copyable snippet. Highest activation ROI; still missing. ✓ (new step 5: detects Cursor ~/.cursor/mcp.json, Codex ~/.codex/config.toml, Claude Desktop config; copyable --mcp snippet + CLI fallback note)
85. **Settings ffmpeg hint is Homebrew-only** (`brew install ffmpeg`). Windows should say `winget install Gyan.FFmpeg` (the error in `ffmpeg.rs` already does — Settings UI does not). ✓ (was already shipped — platform-conditional hint)
86. **Windows permissions card.** macOS has Screen Recording; Windows needs “can gdigrab?” + “mic/loopback device” + “tray allowed”. Empty Settings on Windows looks unfinished. ✓ (WINDOWS STATUS card: live ✓/✗ for ffmpeg gdibrab, audio input, tray + resolved mic name; stale 'pause unavailable' copy fixed)
87. **Close-to-tray copy is macOS** (“menu bar icon”). On Windows say “notification area / system tray”. ✓ (was already shipped — platform-conditional)
88. **Tray “Hide to Menu Bar”** label (`tray_ui.rs`) — Windows users do not have a menu bar. “Hide to tray”. ✓ (was already shipped — platform-conditional)
89. **Single-instance optional.** Docs celebrate multi-process (GUI + MCP). GUI+GUI is confusing (two trays, two hotkeys). Second GUI should focus the first unless `--mcp` / `--screenshot`. ✓ (gui.lock + focus-existing; --mcp/--screenshot bypass)
90. **Update checker** against GitHub Releases (opt-in). `0.3.0` tag vs months of unreleased master is how users run stale capture code. ✓ (opt-in check + notes + staged apply/rollback)

---

## 10 · Agent CLI / MCP / reliability

91. **`--paths` should print the resolved ffmpeg path and gdigrab/x11grab/screencapture**, which it does — also print `windows_subsystem` / “GUI stdio detached” so agents can diagnose `Could not open file`. ✓ (prints resolved ffmpeg + backend + `gui_stdio=detached (windows_subsystem)` / inherited)
92. **CLI `--screenshot --window` on Windows** must use the same offset/HWND path as the GUI (it does via `capture_screenshot_opts`). Add a smoke script `scripts/smoke_capture.ps1` parallel to `smoke_capture.sh`. ✓ (same capture_screenshot_opts path + scripts/smoke_capture.ps1)
93. **`record stop --gif` is a synchronous full-clip transcode** (known). For long agent clips, return the MP4 immediately and GIF as a follow-up tool / background job. ✓ (long stops print gif_pending= and encode in background)
94. **Leave `.ffmpeg.log` next to every MP4.** Useful for debug; pollutes Library. Default: delete on clean stop, keep on error. ✓ (deleted on verify-clean stop; kept on repair/failure; tail surfaces in the CLI error)
95. **MCP tools often never appear** in Cursor/Grok dynamic harnesses. Keep CLI as the supported path; add `vibecap doctor` (ffmpeg, display, tray, last error, session path). ✓ (CLI is the supported path + `vibecap doctor`)
96. **`vibecap_capture` vs GUI still** should share one function (they mostly do). Guarantee identical filenames/sidecar policy so Inbox media_path always exists. ✓ (shared capture path + identical filename/sidecar policy)
97. **Budget auto-stop is MCP-visible, not always GUI-visible.** Status strip has live frames; fire a toast + tray title when a cap hits. ✓ (budget_warned → toast + tray state flip on cap)
98. **main.rs is still the orchestrator for capture hide/restore/region.** Extract `src/app/capture_flow.rs` so Windows park/overlay/REC-bar cannot regress in a 3k-line `update()`. ✓ (capture_flow.rs owns park/restore; update() delegates)
99. **CI capture smoke on Windows** (gdigrab 1 frame to temp, assert ≥8000 bytes). Linux has x11; Windows CI currently compiles and hopes. ✓ (overlay is a separate immediate viewport; the studio stays parked through the pick)
100. **Docs/STATE.md lag.** STATE still says 2026-08-26 and “macOS primary”. After any capture behavior change, update STATE + PLATFORMS in the same PR or agents will re-break Windows hide/stdio. ✓ (STATE.md updated every tranche since the repaint tranche)

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
6. **Move selection** — Space+drag or middle-drag inside the box repositions it. ✓ (press inside an existing box translates it, clamped on screen; release never confirms a move)
7. **Post-drag edit handles** — resize from corners until Enter/click-outside; release no longer hard-commits. ✓ (a press within 14 px of a corner resizes from the opposite corner; move/resize release leaves the box for nudge/Enter)
8. **WASD nudge** alongside arrows (Shift = 10 px). ✓
9. **Double-click the last-region ghost** to instantly re-capture it. ✓
10. **Global Esc during pick** — a focus-loss can't orphan the overlay (listen on the pump). ✓ (pump polls GetAsyncKeyState(VK_ESCAPE) edges while region_open and pushes WakeEvent::RegionCancel)
11. **Clipboard-only stills** — copy and discard the file; never touches the library. ✓
12. **Clipboard format pref** — PNG vs JPEG for copy (PNG preserves sharp text edges). ✓ (`clipboard_encode` pref: PNG/JPEG/off — encoded copy lands next to the DIB via a registered clipboard format, re-encoded from the decoded RGBA so PNG stays lossless)
13. **Shutter sound** — subtle click on capture; off by default. ✓ (synthesized 25 ms decaying-sine WAV via winmm PlaySoundW, no bundled asset; Settings toggle, off by default)
14. **Pre-warm backdrop** — reuse the previous snap as the overlay's backdrop instantly, stamped "refreshing…" until the new snap lands. ✓
15. **PrtScn capture** — optional single-key still via a dedicated hotkey slot. ✓ (opt-in 'PrtScn still' checkbox registers bare PrintScreen globally)
16. **Z-cycle in window-pick** — scroll wheel steps through overlapping windows under the cursor. ✓
17. **Pick card shows process + monitor** under the window title. ✓ (label is now "title · process · Display N")
18. **Countdown on always-on-top viewport** — bubble must be visible while the studio is hidden (uses its own viewport, verify in live test). ✓ (was already shipped — `show_countdown_bubble` is its own always-on-top viewport)
19. **Menu-capture helper** — auto 1 s delay when the cursor sits inside an open menu. ✓ (`foreground_is_menu` probes the foreground class — "#32768"/PopupMenu/NetUI — and bumps the still delay to ≥1 s)
20. **Physical-pixel readout** — W×H plate shows physical px when DPI ≠ 100 %. ✓
21. **Named size presets** — 1920×1080 / 1280×720 centered-box buttons in the HUD. ✓ ("1080p"/"720p" chips drop a centered pixel-size box; nudge + Enter captures)
22. **Saved regions** — persist named rects to session; pick from palette. ✓ (`saved_regions` session list of `(name, w,h,x,y)`; "＋ Save region" on the capture card when a box exists; combo lists + deletes; palette `ApplyRegion` rows fuzzy-match and fire `capture_rect_still` directly — no re-drag)
23. **Loupe hex readout** — show the sampled pixel's #RRGGBB in the cursor loupe. ✓ (was already shipped)
24. **Dim-intensity setting** for the region overlay. ✓ (session `region_dim` 0–200, Settings slider; backdrop now dims with the selection punched bright)
25. **Capture without cursor flash** — per-shot toggle in the HUD. ✓ (⚡ chip in the region toolbar sets `hud_no_flash`; the flash paint consumes it — one shot only, cleared on cancel)

## B · Still editor (Snagit-editor territory)

26. **Arrow tool** — line with head, stroke/color-aware. ✓ (`AnnotationTool::Arrow` — preview + bake draw head wings at ~30°)
27. **Rectangle / ellipse outline** tools. ✓ (`AnnotationTool::Rectangle`/`Ellipse` — stroke-width + color aware)
28. **Blur / pixelate region** — the Inbox "Blur the token" snippet wants this to exist. ✓ (`AnnotationTool::Blur` pixelates the drag rect)
29. **Spotlight** — dim everything outside a rect. ✓ (`AnnotationTool::Spotlight`; bake darkens outside at 0.45×, preview shows dim bands + hole)
30. **Badge style presets** — Snagit step-tool look variants (circle/square, filled/outline). ✓ (`AnnotationAction::badge_style` 0-3: ● ○ ■ □ chips beside the tool; restyles the selected badge undoably; baker + both previews share the mapping; draft-roundtrips)
31. **Highlighter pen** — ~50 % alpha stroke mode. ✓ (`AnnotationTool::Highlight` — alpha-blended stroke)
32. **Stroke straighten** — near-straight freehand becomes a line. ✓ (`straighten_if_near_line` on pen release: <6% chord deviation collapses to endpoints)
33. **Text background box** — label look with fill + padding. ✓ (`draw_text_box` paints a dark pill behind the glyphs; preview mirrors it)
34. **Canvas padding + fill color** on crop. ✓ (EXPORT group "Pad px" + color swatch; post-bake, dims shown in readout)
35. **Edge effects** — border, torn edge, drop shadow presets. ✓ (`apply_edge_fx` post-bake: Border frame / Shadow blur+offset / Torn ragged alpha edge — deterministic noise jag; EXPORT segmented + px slider; output dims reflect growth)
36. **Watermark preset** — text or logo at corner with opacity. ✓ (same as #121 — WATERMARK group drops a corner Text stroke)
37. **Annotation undo/redo** — Ctrl+Z / Ctrl+Y stack (per-stroke). ✓ (snapshot stack + buttons in both annotate surfaces)
38. **Resize-for-export** — % or max-width field in the Still inspector (img_resize_pct exists). ✓ (EXPORT % slider)
39. **Export format per save** — PNG/JPEG/WebP choice. ✓ (EXPORT format segmented + quality slider)
40. **Copy original vs annotated** choice (today annotated wins). ✓ (⋯ "Copy original (no markup)" alongside annotated Ctrl+C)
41. **Paste image onto canvas** — combine shots, Snagit-style. ✓ (`AnnotationTool::Sticker` — Ctrl+V pastes clipboard image at native px, draggable)
42. **Hold-Space before/after** preview of annotations. ✓ (hold B hides all annotations until release)
43. **Measure tool** — px distance readout between two clicks. ✓ (`AnnotationTool::Measure` — drag line, "N px · θ°" label in image px, bakes into export)
44. **Ruler / grid overlay** toggle in Still canvas. ✓ ("▦" toolbar toggle — quarters grid over the image, preview-only)
45. **Zoom-to-fit / 100 % quick keys** (Ctrl+0 / Ctrl+1). ✓ (`0` fit, `1` true-100% undoing fit scale)

## C · Video & GIF

46. **Window-record via pick** — same WindowPick overlay → record rect (crop record, no focus juggle). ✓
47. **Follow-cursor recording** — crop rect pans with the pointer (for zoomed tutorials).
48. **Webcam bubble** — second gdigrab/dshow source composited corner-overlay (big).
49. **Mic + system mix** — dshow device list exists; add a mix selector + level meters. ✓ (second dshow device mixed via amix=inputs:2 with explicit -map; the E36 live meter reads the mixed output's Peak_level — per-source meters would need per-input astats + separate pipes, deferred)
50. **Pause/resume hotkey** — dedicated digit. ✓ (opt-in Ctrl+Shift+N via 'Pause hotkey' in Settings; WakeEvent::PauseToggle wakes a parked studio like Stop)
51. **REC bar source line** — shows target rect/monitor + audio state. ✓ (caption row under the timer: "Display 2" / "Region 800×600" / "Window: app" + ⚑ count; audio flag only where capture honors it)
52. **REC bar position memory** — draggable, persists. ✓ (empty-space drag via ViewportCommand::StartDrag; position tracked from outer_rect, session-persisted, bounds-checked on load)
53. **Marker hotkey during record** — drops a chapter at press. ✓ (⚑ button on the REC bar works while parked; M key when focused; → .markers.txt on finalize)
54. **Auto-trim dead air** — drop frames <N fps-change at head/tail on finalize. ✓ ("Auto-trim dead air" Settings switch applies dead_air_bounds to the trim on clip load; banner path stays as the default)
55. **Output presets** — CRF, fps, codec (H264/H265/VP9) in Settings. ✓ (CRF Sharp/Balanced/Small chips → -crf 18/23/28 via `CaptureOpts::crf`; fps already in Settings; live codec stays libx264 — export-side codec picker covers H264/VP9/AV1)
56. **GIF ping-pong loop** toggle. ✓ (was already shipped — "Ping-pong ↺" on the GIF export)
57. **GIF frame delete** in the filmstrip. ✓ (cut marks on thumbs + "Export without cuts" — per-frame delay editing stays open)
58. **GIF per-frame delay** editor. ◑ (GIF group shows ms/frame derived from fps + a 0–4 s "end hold" slider that tpads a cloned last frame so loops breathe; true per-frame delay list still open)
59. **Re-export GIF** from an existing MP4 at new fps/width (no re-record). ✓ (was already shipped — GIF group encodes from the loaded clip)
60. **WebM / AV1 output** option. ✓ (was already shipped — ENCODE group chips)
61. **Stream-copy trim** — no re-encode when only cutting ends. ✓ (was already shipped — `-ss/-to -c copy`)
62. **Clip audio in preview** — today's player is silent. ✓ (was already shipped — preview WAV + winmm loop follows play/pause)
63. **Frame → still** — grab the current preview frame as a new screenshot. ✓ (was already shipped — transport "Grab frame" → jpg beside the clip)
64. **Batch re-export** selection from Library. ✓ ("GIFs" in the selection bar → `batch_gif_export`: one worker encodes selected clips serially at the GIF fps/width settings, single summary toast)
65. **Recording countdown styles** — 3 / 5 / none setting. ✓ (was already shipped — 0/3/5 segmented + own viewport bubble)

## D · Clipboard & destinations

66. **Clipboard history** — last 10 captures in a tray submenu + palette. ✓ (tray Recent captures + palette "Captures" group: last 10 idle, fuzzy filename match when typing; Enter opens review + copies path)
67. **Copy as Markdown image** — `![](path)` for docs. ✓
68. **Copy file URI / data URI** for devs. ✓ (Still ⋯ menu — file:// + data: URI with in-house base64, 8 MB cap)
69. **Copy + reveal combo** action on the toast card. ✓ (toast has Copy, Copy path, Reveal, Annotate, Discard)
70. **Auto-open editor** toggle (some flows never want Review). ✓ ("Open captures in Review" switch, default on; off stages the editor silently without the tab jump)
71. **OS drag-out** of the capture card thumbnail into Explorer/Slack (open item). ✓ (recent tiles drag via OLE CF_HDROP — shipped with the recents carousel)
72. **Size guard hint** — warn + auto-shrink offer when a still exceeds Discord's 8 MB. ✓ (>8 MB toast warns + points at "Export for Discord" — iterative q85 JPEG / 0.8× shrink until under)
73. **Post-capture actions menu** — copy path / reveal / open / delete right on the card. ✓ (was already shipped: Annotate · Copy Image · Copy Path · Reveal · Discard)

## E · Library

74. **Filename search** (beyond date-group browsing). ✓
75. **Favorites / pins** — float to top. ✓ (session `library_favorites`; ★ hover/context toggle, float-first within each group, "★" filter chip)
76. **Tags** with filter chips. ✓ (session `library_tags` name→tags; 🏷 Tags… context editor — bulk-applies to selection; "🏷 tag n" chip row ANDs onto the filter)
77. **Export selection as ZIP**. ✓ (store-only `app::zip` writer — CRC32, central dir, name dedupe; "Export ZIP" on the selection bar → save dialog)
78. **Sort** — date/size/duration/name. ✓ (⇅ menu: Newest/Oldest/Largest/Smallest/Name/Type)
79. **Retention rules** — keep N days or N files. ✓ (Settings LIBRARY: Off / Older-than-N-days / Keep-newest-N; "Sweep now" + opt-in auto-sweep on launch; files move to persistent retention_trash — never hard-deleted)
80. **Duplicate detection** — content-hash same-shot warnings. ✓ (`mark_duplicates`: size-collision files get head+tail+len fingerprint; ≡ badge on dupes)
81. **Thumbnail repair** — regenerate missing thumbs. ✓ (⋯ "Repair thumbnails" → drops zero-byte thumbs + regenerates, worker thread)
82. **Open-with…** menu per item. ✓ (`platform::open_with` — Windows `OpenAs_RunDLL` chooser; Finder reveal on macOS; default handler on Linux)
83. **Review-queue flag** — "needs attention" marker. ✓ (session `library_flagged`; ⚑ badge on tiles, context toggle, "⚑ n" filter chip)
84. **Recently-deleted view** — surface `undo_trash` as a shelf. ✓ (banner under the toolbar while the 12s undo window is live — Undo / Dismiss)

## F · Hotkeys, tray, system

85. **Hotkey rebind UI** — Settings editor, applies without restart (open #81). ✓ (hotkey digits rebindable in Settings; rebind_global_hotkeys applies live)
86. **Per-mode hotkeys** — region-still / window-still / GIF / pause. ✓ (opt-in Ctrl+Shift+N digits for Region pick, Window still, GIF clip + Pause; collision-guarded against the shot/rec/pause slots; Settings checkboxes + sliders; generated cheatsheet rows)
87. **Tray recent-captures** submenu. ✓ (tray 'Recent captures' — 5 slots)
88. **Tray pause/resume** item during record. ✓ (Pause/Resume Recording under Stop, Recording{paused} label + ⏸ title)
89. **Tray "repeat last capture"** item. ✓
90. **Tray double-click = screenshot** option. ✓ (Settings 'Tray double-click' → Open/Screenshot/Record; deferred-click dedup)
91. **CLI poke running GUI** — `vibecap --capture` forwards to the single instance. ✓ (`vibecap poke <verb>` → pending_cmd marker → running/parked instance)
92. **Watch-folder import** — drop shots into the library dir. ✓ (session watch_folder + 3 s sweep, parked-side via the pump)
93. **Portable mode** — config beside the exe. ✓ (vibecap.portable marker beside the exe redirects config+media)
94. **Profile export/import** — settings as a file. ✓ (.vcap-profile zip, manifest+session, masked import)
95. **Silent mode** — suppress toasts + flash. ✓
96. **Update toast with changelog** link. ✓ (region_history (32-deep) + Ctrl+Z in the HUD restores previous rects)
97. **First-run health check** — ffmpeg, DPI awareness, write-perms, tray. ✓ (scroll adjusts width, Shift+scroll height, clamped to screen)
98. **`?` cheat sheet** — in-app shortcut overlay. ✓
99. **Stats card** — captures this week, bytes, streak. ✓ (Library header line: 'This week: N · size · D-day streak' bucketed from item mtimes)
100. **Crash-recovery** — restore unsaved annotations on next launch. ✓ (toolbar frame is a fixed near-black pill with light ink, independent of app theme)

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

1. **Per-theme density scale** — compact/cozy/comfortable spacing token per theme; Celestial can afford airier gaps than Mono. ✓ (`density_scale()` per-mode — celestial ×1.08 — folded into `Density::sp` so every `sp()` site scales)
2. **Theme-aware elevation model** — three shadow tiers (rest/raised/overlay) in tokens instead of only `popup_shadow`/`window_shadow`. ✓ (`elevation_rest`/`elevation_raised` per-mode fns; cards moved to `elevation_raised()` so non-celestial themes get the tier too; overlay tier remains the per-theme shadow passed to `apply_visuals`)
3. **Accent-hue slider for celestial modes** — rotate the aurora hue ±40° while keeping the sky structure. ✓ (`aurora_hue` session float → `set_aurora_hue` → `aurora_stops_for` rotates stops via HSV; Settings slider + reset visible only on celestial themes; swatch previews stay stock)
4. **Theme preview in picker shows real chrome** — mini mock of rail + card + CTA inside each swatch, not just an aurora strip. ✓ (swatch paints a mini rail + card + accent CTA; celestial keeps an aurora sliver)
5. **Auto theme** — follow Windows light/dark for the Mono pair; celestial modes stay manual. ✓ (was already shipped — theme_follow_os polls AppsUseLightTheme every 3 s; dark maps to theme_dark_pick, light maps to Light)
6. **Scheduled themes** — Light by day, Dark/Celestial by night (opt-in). ✓ (`theme_schedule` shares the 3 s follow-OS tick; local hour picks Light 07:00–19:00 else the dark pick; wins over follow-OS; dark-pick row shows for either)
7. **Contrast audit pass** — run a contrast checker over every `TEXT_MUTED`/`TEXT_FAINT` usage; celestial muted on plum is borderline. ✓ (`text_tiers_clear_contrast_floors` computes WCAG luminance ratios per theme — body ≥7, muted ≥4.5, dim ≥3.0; it caught Light TEXT_DIM at 2.3:1, fixed to zinc-600 ≈4.7:1)
8. **Focus ring token** — real `FOCUS_RING` color per theme, painted on keyboard focus for every interactive widget. ✓ (`FOCUS_RING` pent! token per theme; `paint_button` claims focus on click and strokes the ring on `has_focus`)
9. **Disabled-state token set** — `*_DISABLED` fill/text pair instead of ad-hoc `.weak()` calls. ✓ (`DISABLED_FILL`/`DISABLED_TEXT` pent! tokens; fill wired into `noninteractive.weak_bg_fill`, text drives the off-state of the capture quick-toggles)
10. **Hover animation** — egui `ctx.animate` on button fills; 80–120 ms ease like the mockup's `--ease`. ✓ (paint_button blends rest→hover fill via animate_bool on resp.id; snaps instantly under reduce-motion)
11. **Pressed-state scale** — 0.98 shrink on primary CTAs for tactile feel. ✓ (primary/danger paint rects shrink 1.2px while down)
12. **Icon stroke-width consistency** — audit `icons.rs` strokes; loupe/camera glyphs draw heavier than nav glyphs. ✓ (audited: one size-proportional base stroke + a named `detail` stroke (+0.3 px) replacing the +0.3/+0.4 ad-hoc offsets)
13. **Icon sizing token** — `ICON_SM/MD/LG` (14/18/22) instead of scattered pixel sizes. ✓ (tokens added; rail icons, toast icon, capture CTA icon, picker glyphs now consume them)
14. **Letter-spacing token for caps labels** — `caps_label` hardcodes 1.0; make it a token so Celestial can track wider. ✓ (caps_tracking() — 1.6 on celestial, 1.0 elsewhere; caps_label consumes it)
15. **Numeric font feature** — tabular figures for REC timer, size columns, budget readouts (Segoe UI `tnum` or a mono face). ◑ (mono face gives stable digit widths on the REC clock + status strip; true `tnum` OpenType features aren't reachable through egui's font pipeline)
16. **Mono face for code/path text** — paths, durations, and `kbd` chips should share one mono family token. ✓ (`theme::mono_font(size)` token; kbd chips, REC timer, and status-strip REC label consume it)
17. **Theme diff in screenshot tests** — golden-frame capture per theme to catch alpha/token regressions like the premultiplied bug.
18. **Carbon accent review** — Carbon currently inherits zinc accent; give it a slate-blue tint to match Tailwind slate-400 hover states. ✓ (ACCENT carbon lane → #93c5fd slate-blue; dark ACCENT_INK already pairs)
19. **Celestial card inner-glow** — 1 px top inner highlight (`rgba(255,255,255,.06)`) like Chromie's glass cards. ✓ (white-alpha-14 hairline inset on section_card tops when is_celestial)
20. **Toast severity left-bar** — already colored; add matching icon tint + semantic icon per severity. ✓ (was already shipped — show_toast_card paints severity bar + level.icon() tinted by level.accent())
21. **Empty-state art** — one small line-art glyph per empty surface (Library, Inbox, Recents) instead of bare text. ✓ (empty_state now sits the glyph on a 64px SURFACE_2 disc, aurora-accent ring on celestial)
22. **Skeleton loaders** — shimmer rect where thumbs/frames are pending instead of blank tiles. ✓ (library tiles paint a SURFACE_2 skeleton + sweeping highlight band under the async image loader; shimmer gated by reduce-motion)
23. **Reduced-motion setting** — disable aurora pulse, hover fades, toast slide. ✓ (`reduce_motion` session flag + Settings switch → theme thread-local; danger_pulse flattens, recent-tile grow snaps, HUD pick-flash skipped)
24. **Starfield parallax** — stars drift 1–2 px on window resize for depth (cheap: offset by rect delta). ✓ (per-star depth spread around canvas center — resizes drift the field instead of scaling it rigidly)
25. **Theme export/import** — share a theme as a JSON snippet; community themes later. ✓ (Share theme row — Copy/Paste a JSON recipe via clipboard: mode, aurora hue, follow-OS/schedule, dark pick; validated + hue clamped)

## B · Layout, rail & navigation (26–50)

26. **Collapsible rail** — icon-only 48 px mode; labels on hover tooltip. ✓ (rail_collapsed session flag + «/» toggle at rail bottom + Settings switch; 48px icons, hairline dividers, tooltips already present)
27. **Rail badges** — numeric badge on Inbox (pending count), dot on Library (new items since open). ✓ (Inbox count shipped; Library new-dot open)
28. **Rail section labels** — CAPTURE / REVIEW / SYSTEM group dividers in expanded mode. ✓ (caps dividers CAPTURE/KEEP/AGENT/APP segment the rail stages)
29. **Rail drag-reorder** — let users pin favorite stages to top. ✓ (right-click a rail stage → Pin to top / Move up / Move down / Reset; order persists via `rail_order` labels, zone dividers follow the custom order, Settings stays bottom-pinned)
30. **Keyboard rail nav** — Ctrl+1..5 jump to stages; shown in `?` sheet. ✓ (was already shipped)
31. **Breadcrumb in Review** — `Library › clip_name` so Esc-depth is visible. ✓ (dim "Library ›" link prefix on Clip/Still headers; click jumps back)
32. **Back button** — in-header ‹ Back for Review/Clip/Still; Alt+← binding. ✓ (Alt+←/→ shipped; header button open)
33. **Window-size memory per stage** — Library wants wide; Capture wants narrow. ✓ (session `window_sizes` map: leaving a stage stashes its size, entering restores it via InnerSize when it differs >20px)
34. **Min window size enforcement** — below 720 px the rail overlaps content; clamp or collapse. ✓ (`with_min_inner_size([760, 560])` shipped earlier)
35. **Adaptive column width** — the 720 px content column should widen on >1100 px windows. ✓ (col_w widens to 900 on >1100 px)
36. **Status strip resize drag** — give the bottom bar a 2 px taller hit target. ✓ (inner vertical margin 6→8 px; empty strip space StartDrags the window; segments use selectable_label hit rects)
37. **Status strip segments clickable** — click "2 recordings" → jump to Library filtered. ✓ (storage→Library, tier→Settings, inbox n→Inbox, ffmpeg-missing→Settings)
38. **Right-side inspector mode** — optional docked metadata panel in Review screens. ✓ (`inspector_open` pref gates the Still/Clip right rail — canvas takes full width when closed; FILE metadata group (name, MB, mtime, dims/duration) tops both rails)
39. **Zen mode** — hide rail + status strip; palette + hotkeys only. ✓ (palette ToggleZen; rail+strip gated, entry toast explains Ctrl+K exit)
40. **Header title dynamic** — show contextual title (recording name in Clip, file name in Still) instead of always stage name. ✓ (Clip/Still headers show the loaded file name)
41. **Subtitle slot in header** — second line under title for context ("unsaved changes", "recording 00:12"). ✓ (subtitle shows annotation/cut counts when live, else the stage hint)
42. **Command palette recent verbs** — MRU section above the flat list. ✓
43. **Palette fuzzy match** — substring scoring; "gif" should rank "Export GIF" first. ✓ (subsequence scoring, word-start/consecutive bonuses)
44. **Palette actions show shortcuts** — right-aligned kbd hint per row. ✓ (accent mono chip mirrors the real binding: S, R, Ctrl+C, Ctrl+1-3/5, Ctrl+I, ?)
45. **Palette media jump** — typing a filename jumps to its review. ✓ (OpenMedia fuzzy-matches library names; opens Still/Clip and copies the path)
46. **Tab-strip alternative** — optional top tabs instead of rail for users who want Snagit familiarity. ✓ (`top_tabs` pref renders a horizontal TopBottomPanel strip with icons+labels, badges, underline accent)
47. **Drag window by any dead space** — today only header drags; padding zones should too. ✓ (main window uses native decorations — the OS titlebar drags everywhere; the custom REC bar already StartDrags on dead space)
48. **Snap-layout friendly sizing** — default size lands cleanly in Windows 11 half-snap. ✓ (first-run default clamps to half the primary monitor's width / full height so the window fits a Win11 snap half; persisted sizes untouched)
49. **Restore-last-stage on launch** — setting: always Capture vs resume where you left. ✓ ("Reopen where I left off" switch; off = always Capture; session `restore_tab`)
50. **Stage transition direction** — slide left/right matching rail order, not a single wipe. ✓ (tab changes ease the incoming stage ±60 px from the rail direction over ~160 ms; skipped under reduce-motion)

## C · Capture tab & flow (51–75)

51. **Per-target memory** — remember Region vs Window per session *and* per hour-of-day. ✓ (session `capture_target_name` + `target_hours` map: the current hour's habit wins at launch, else the last pick; every trigger records target→hour)
52. **Shutter button split-menu** — chevron on Screenshot offering Region/Window/Full variants without leaving the row. ✓ (▾ next to the shutter pops one-shot variants: capture once as Full/Region/Window, then restores the persisted From pick)
53. **Capture preview strip on hover** — hovering a recent tile grows it 1.5× with play. ✓ (`animate_bool` grow 120×68→180×102; video tiles reuse `scrub_cache` frames advanced by time at 6 fps — the hover lags the size change one frame by design)
54. **Drag recent tile out** — straight to Explorer/Slack from the capture card. ✓ (`Sense::click_and_drag` + `drag_started` → `platform::start_file_drag` OLE CF_HDROP; drag-threshold means plain clicks never start a drag)
55. **Recent tile quick-actions** — hover overlay: copy / annotate / delete on recents. ✓ (📋 copy path + 🗑 delete-to-undo-trash chips at the tile's top-right; pointer-in-rect containment so the overlay doesn't flicker when the pointer enters a chip)
56. **Recents carousel** — horizontal scroll when >3 items instead of hiding them. ✓ (8 newest items, `ScrollArea::horizontal` wraps the tile row)
57. **"Waiting for capture" state** — while armed+hidden, the studio (if shown) should say so. ✓ ("⏳ Waiting for capture — grab in progress…" line under the shutter while `screenshot_in_flight`/`still_busy`)
58. **Options card quick toggles** — cursor/audio/display as icon toggles, not buried in disclosure. ✓ (🖱/🎙/🖥N selectable chips on the always-visible options row; display chip cycles monitors)
59. **Audio device picker** — dropdown of dshow devices when Include audio is on. ✓ (combo under the switch lists enumerated devices + Auto; pick persists via session `audio_device`; a vanished device falls back to Auto)
60. **Estimated file size** — live "≈4 MB/min @ 30fps" under Record. ✓ (STORAGE group in Options, scaled by fps + monitor mpx)
61. **Disk-space guard** — warn <500 MB free on the target dir before arming. ✓ (GetDiskFreeSpaceExW; warning under shutter + free-space line in STORAGE)
62. **Battery-aware hint** — on battery, suggest 24 fps / shorter clips. ✓ (`GetSystemPowerStatus` ACLineStatus → "🔋 On battery — 24 fps or shorter clips" under the shutter when fps_target > 24; desktops/unknown → no hint)
63. **Capture history sparkline** — tiny 7-day activity graph on the Capture card. ✓ (7-bar strip beside the RECENT label, bucketed from `library_items` mtimes, today bar in accent)
64. **Quick-capture tray-free mode** — double-press hotkey within 500 ms = instant region with last settings. ✓ (600 ms window: second tap mid-flight arms the picker to open on restore, between shots it opens the region pick directly)
65. **Countdown cancel UX** — click anywhere or Esc during countdown aborts cleanly with toast. ✓ (Esc was already wired; now `pointer.any_pressed` inside the bubble cancels too — label reads "Esc or click to cancel")
66. **Post-capture inline undo** — toast gets an Undo for 5 s on auto-save. ✓ (the capture toast's Discard routes to the 12 s undo-trash — "undo the save" and Z undoes the discard)
67. **Auto-scroll to options** — when Record selected, scroll options card into view. ✓ (pressing Record opens the collapsed Options header that frame via `.open(Some(true))` — audio/display knobs visible before arming)
68. **Source icons state-colored** — the From segment icons tint to accent when active. ✓ (already shipped — icons paint ACCENT when on, TEXT_MUTED when off)
69. **Window target shows last pick** — "Window: Chrome — DevTools" persisted on the card. ✓ (`window_app` joins session state; combo + 🎯 Pick persist on change; the target hint reads "Window: <name>" when a pick exists)
70. **Confirm-before-overwrite** — same-name collision in output dir prompts once per session. ✓ (auto-captures dedupe via `next_seq` so names never collide; Still save/export go through the OS save dialog's own overwrite confirm)
71. **Multi-shot batch** — hold modifier + click regions repeatedly = rapid sequence of stills. ✓ (Shift+release crops from the frozen backdrop and keeps the overlay; 📷N chip counts saves; Esc ends)
72. **Time-lapse mode** — capture frame every N sec into a video (stills → mp4). ✓ (Options → TIME-LAPSE: 2/5/15/60/300 s interval; gdigrab runs at 1/N fps, `fps=<target>` retimes the output — 5 s ≈ 150× at 30 fps; audio auto-off; REC bar shows "lapse Ns")
73. **Scheduled capture** — "in 10 min, grab this window" for meetings. ✓ (delay ≥60 s becomes a scheduled shot: studio stays interactive, card shows live countdown + Cancel, fires via the normal still path when the Instant lands)
74. **Clipboard watcher mode** — studio stays parked; a shot auto-opens Still review. ✓ (opt-in Settings switch; 800 ms GetClipboardSequenceNumber poll → arboard get_image → saves PNG + opens Still; own captures excluded by refreshing seq after every set_image; Windows-only)
75. **Capture sound per action** — distinct subtle tones for still/record-start/record-stop. ✓ (`record_tone(start)` — rising 620→980 Hz chirp on start, falling on stop, synthesized WAV like the shutter click; same opt-in switch)

## D · Region & window pick HUD (76–100)

76. **HUD size readout follows cursor** — W×H plate avoids cursor side automatically. ✓ (plate picks the first corner that neither leaves the screen nor sits under the cursor)
77. **HUD crosshair magnifier on demand** — hold Ctrl for loupe instead of always-on. ✓ (loupe is Ctrl-gated now; samples the frozen backdrop)
78. **Region edge snapping** — snap to window edges/screen edges within 8 px. ✓ (snap_with_guides: nearest screen or visible-window edge within 8 px, both axes)
79. **Region guides** — smart alignment guides to other visible window rects. ✓ (matched snap edges paint as guide lines spanning the window edge)
80. **Dark/light HUD chrome auto** — HUD inverts on very bright backdrops for contrast. ✓ (hints render on translucent dark pills and HUD chrome is always neutral dark — contrast holds on any backdrop without theme coupling)
81. **HUD button size scales with selection** — tiny regions get a compact toolbar. ✓ (selections under 260x140 get a compact toolbar (tighter margins, no title label))
82. **Region from keyboard only** — arrows move a growing box from center; Enter commits. ✓ (arrow key with no box grows a centered 200x150 box; nudge + Enter commit)
83. **Preset aspect preview tint** — locked-aspect regions tint the dim outside differently. ✓ (locked-aspect surround tints ACCENT over the dim)
84. **Multi-monitor dim** — only the active monitor dims; others stay lit. ✓ (only the monitor under the cursor dims; others stay lit)
85. **Pick-confirm sound** — soft tick on mouse-up valid region. ✓ (shutter_click (winmm, opt-in) fires when the grab lands on confirm)
86. **Region min-size guard** — <8×8 drag shows "too small" instead of capturing noise. ✓ (<24px drags don't confirm; the box stays for nudge/Enter at ≥8px)
87. **Region grid overlay** — thirds/quarters toggle in HUD for composition. ✓ (▦ chip cycles thirds, quarters, off; paint_selection_hud honors it)
88. **Window pick confidence flash** — highlight border pulses once on hover-lock. ✓ (hovered-window change pulses a 300 ms decaying stroke via ctx temp data)
89. **Window pick excludes overlays** — our own HUD/REC bar never appear in the pick list. ✓ (pickable_at excludes our own process and untitled explorer shells)
90. **Alt=child-window pick** — drill into tooltips/menus as separate regions. ✓ (EnumChildWindows drill path — Alt while hovering picks the deepest child under the cursor, e.g. a toolbar inside its parent window)
91. **Region coordinates copy** — click W×H plate copies `x,y,w,h` for scripts. ✓ (plate is clickable; copies pixel-space x,y,w,h)
92. **Region color-sampler mode** — click samples hex under cursor to clipboard (design pick). ✓ (Ctrl+click in loupe mode copies the sampled pixel as #RRGGBB with a copied flash tag)
93. **Freeze-frame toggle** — optional freeze of backdrop while picking (already static on Windows; make it a toggle for parity). ✓ (`region_live_backdrop` pref — on the capture-exclusion path the picker backdrop re-grabs every ~1.2 s; off = classic frozen frame)
94. **HUD remembers toolbar side** — toolbar docks top or bottom per last use. ✓ (chip toggles dock; session hud_toolbar_bottom persists across runs)
95. **Cancel zone hint** — first-time hint "Esc to cancel" fades after 3 uses. ✓ (session region_pick_count hides the top hints after 3 completed picks)
96. **Region history stack** — Ctrl+Z steps back through previous rects this session. ✓ (region_history (32-deep) + Ctrl+Z in the HUD restores previous rects)
97. **Scroll-wheel region resize** — wheel adjusts width, Shift+wheel height. ✓ (scroll adjusts width, Shift+scroll height, clamped to screen)
98. **Touch/stylus support** — pen drag works; palm rejection via contact size.
99. **Pick while maximized** — studio shouldn't restore to pick; verify parked path keeps working. ✓ (overlay is a separate immediate viewport; the studio stays parked through the pick)
100. **HUD theme variant** — HUD always uses a neutral dark chrome regardless of app theme. ✓ (toolbar frame is a fixed near-black pill with light ink, independent of app theme)

## E · Still review & annotation (101–125)

101. **Undo/redo stack** — per-stroke Ctrl+Z/Ctrl+Y (open since round 1). ✓ (snapshot stack now feeds Undo+Redo in both Still and the annotate modal; Clear/Reset are undoable)
102. **Real blur bake** — box-blur/mosaic into pixels on export, not a translucent overlay. ✓ (was already shipped — `pixelate_rect` bakes real mosaic)
103. **Drag-crop on canvas** — visual crop handles replace the four numeric fields. ✓ (was already shipped — crop_drag on the Still canvas)
104. **Zoom/pan canvas** — wheel zoom, space-drag pan, Ctrl+0 fit, Ctrl+1 100 %. ✓ (was mostly shipped; this pass: scroll zoom is hover-gated, `1` = true 100 %)
105. **Arrow tool** — with head size + color from the stroke state. ✓ (was already shipped)
106. **Shape tools** — rect/ellipse outline + filled modes. ✓ (Ellipse shipped: baker outline, EllipseShape preview, Shift → circle)
107. **Highlighter** — 50 % alpha stroke. ✓ (was already shipped)
108. **Step tool** — auto-numbered badges that renumber on delete. ✓ (was already shipped)
109. **Spotlight** — dim outside a rect. ✓ (same as round-2 #29)
110. **Measure tool** — px distance + angle readout. ✓ (same as round-2 #43 — label shows "N px · θ°")
111. **In-place text editing** — click canvas, type there; no separate field. ✓ (was already shipped — `text_edit_at` Area)
112. **Text background chip** — filled label look with padding + radius. ✓ (was already shipped — OVERLAY_LABEL pill)
113. **Annotation color palette** — 6 swatch row + custom hex. ✓ (red/amber/green/blue/white/black swatches in BRUSH + custom color editor)
114. **Stroke width presets** — 2/4/8 chips + slider. ✓ (2/4/8 px chips + 1–12 slider)
115. **Copy original vs annotated** — explicit choice in the copy menu. ✓ ("Copy original (no markup)" in the Still ⋯ menu)
116. **Save-as-copy default** — never silently overwrite the source still. ✓ (was already shipped — "Save as copy" + explicit overwrite)
117. **Export format picker** — PNG/JPEG/WebP + quality slider. ✓ (EXPORT group: JPG/PNG/WebP segmented + JPG quality slider → save dialog, explicit encoders)
118. **Resize-on-export** — % or max-width field with pixel preview. ✓ (was already shipped — Resize % slider in the pipeline; added "→ WxH px output" readout)
119. **Paste-onto-canvas** — clipboard image becomes a movable layer. ✓ (`AnnotationTool::Sticker` carries `Arc<RgbaImage>`+TextureHandle; Ctrl+V or ⋯ "Paste image onto canvas"; drag to move while selected, Del removes, bakes at native px)
120. **Before/after hold** — hold Space to peek the un-annotated original. ✓ (hold B — Space stays pan; annotations skipped while peeking)
121. **Watermark preset** — corner text/logo with opacity. ✓ (WATERMARK group: text field → "Add to corner" drops a Text stroke at bottom-right in the brush color, undoable)
122. **Canvas padding** — add uniform border pixels with fill color on export. ✓ (Pad px slider + color swatch in EXPORT; applied post-bake so annotations stay aligned)
123. **Annotation list panel** — side list of strokes; click selects, Del removes. ✓ (STROKES group lists each action, click selects, ✕/Del removes via undoable `remove_annotation` + badge renumber)
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

126. **Preview audio** — extract audio track alongside filmstrip; play in sync. ✓ (`extract_preview_wav` → temp WAV ≤120s; winmm `PlaySoundW` SND_LOOP follows player_playing; stops on pause/tab-switch/unload; Windows-only)
127. **Determinate extract progress** — "frame i/n" bar instead of indeterminate label. ✓ (progress channel streams decoded/total)
128. **Scrub-bar seek** — click/drag the ruler moves the preview head. ✓ (transport slider + click canvas; Space/Home/End added)
129. **Frame-step keys** — ←/→ one frame, J/K 10 frames. ✓ (←/→ existed; J/K ±10 + I/O trim-at-playhead added)
130. **In/out loop** — preview loops the marked range. ✓ (was already shipped — `L` toggles)
131. **Trim-verify export** — assert ffmpeg `-ss/-to` equals the ruler seconds. ✓ (`spawn_ffmpeg_job_ex` post-job verifier probes output duration vs ruler span; >1.5 s drift warns "keyframe snap — re-encode for exact cut")
132. **Stream-copy trim** — `-c copy` when codec allows; instant cut. ✓ (was already shipped — Trim video uses `-ss/-to -c copy`)
133. **Export preset chips** — Discord 8 MB / README 480p / lossless. ✓ (was already shipped — PRESETS group)
134. **GIF settings dialog** — fps, width, loop mode, size estimate pre-encode. ✓ (was already shipped — sliders + ~KB estimate)
135. **GIF ping-pong** — boomerang loop toggle. ✓ ("Ping-pong ↺" checkbox → split/reverse/concat filtergraph, estimate doubles)
136. **GIF frame ops** — delete frames, per-frame delay in filmstrip. ✓ partially — right-click a filmstrip thumb marks its time slice (red ✕); "Export without cuts" re-encodes via `select='not(between(...))'` + `aselect` audio mirror. Per-frame delay still open.
137. **Re-GIF existing MP4** — new settings without re-recording. ✓ (was already shipped — GIF group + presets encode from the loaded file)
138. **WebM/AV1 export** — codec picker on the export row. ✓ (ENCODE group: "WebM (VP9)" libvpx-vp9+libopus → .webm, "AV1 (SVT)" libsvtav1 → .mp4)
139. **Extract audio** — one-click `.m4a` from clip. ✓ (was already shipped — AUDIO group)
140. **Frame-grab** — current preview frame → new still in Library. ✓ (transport "Grab frame" → ffmpeg `-ss -vframes 1` JPG)
141. **Chapter markers** — marker hotkey during record; ticks on the ruler. ✓ (was already shipped — sidecar + ruler ticks)
142. **Marker list** — click a marker to jump the preview. ✓ (clickable timecode chips under the ruler)
143. **Auto-trim dead air** — detect frozen head/tail, offer trim. ✓ (`dead_air_bounds` on filmstrip RGBA; "Trim to content" banner)
144. **Speed ramp** — 0.5×/2× segments (stretch goal, simple `-setpts`). ✓ (was already shipped — SPEED chips + "Apply N× speed" → `setpts=PTS/x` + `atempo=x`)
145. **Clip notes** — text sidecar shown under the player. ✓ (NOTES group → `<file>.notes.txt`, Save/clear; loaded on filmstrip open)
146. **Compare mode** — split-screen before/after trim preview. ✓ (transport "I|O" toggles split canvas: left = in-point frame, right = out-point frame, each with a timecode chip)
147. **Player always-visible Open** — real-player fallback button lives in chrome, not only on error. ✓ (transport bar)
148. **Preview quality toggle** — half-res filmstrip for long clips. ✓ (Settings → "Low-res clip preview" → 240px filmstrip via session `filmstrip_low_res`)
149. **Auto-play setting** — the autoplay we shipped becomes a Settings toggle. ✓ (session-persisted `clip_autoplay`)
150. **Clip deletion guard** — deleting a recording with unsaved trims asks once. ✓ (deleting the clip open in Review with unsaved cuts/trims opens a one-time 'Delete unsaved clip work?' confirm; remembered per path per session)

## G · Library (151–175)

151. **Filename search** — filter-as-you-type in the header. ✓ (was already shipped)
152. **Search sidecars** — `.txt` notes + transcript text indexed. ✓ (library search reads `.notes.txt` + `.txt` beside each item; sidecars hidden from grid via denylist)
153. **Sort menu** — date/size/duration/name/type. ✓ (⇅ menu; non-date sorts drop group headers)
154. **Favorites** — ★ floats to top, filter chip. ✓ (same as round-2 #75)
155. **Tags** — free-form tags + colored filter chips. ✓ (same as round-2 #76)
156. **Date groups** — Today/Yesterday/This week/Earlier headers (round-1 open item). ✓ (was already shipped)
157. **List view** — dense row alternative to the tile grid. ✓ (≡/▦ toggle in the header, session-backed; 26px rows: glyph · name · tags · ★⚑≡ badges · size; same click/select/context/drag semantics)
158. **Tile size slider** — S/M/L thumbnails. ✓ (segmented S/M/L in header)
159. **Hover-scrub** — moving across a video tile plays frames (filmstrip reuse). ✓ (`extract_scrub_frames` — 8 frames @192px via ffmpeg into a distinct `frames_scrub/` dir so it can't clobber the editor's `frames_temp/`; worker → persistent `scrub_tx/rx` channel → `drain_scrub` uploads textures; hover over a Video/GIF tile picks the frame by cursor-x fraction and paints it over the thumb; cache capped at 32 paths)
160. **Hover quick-actions** — copy/reveal/delete overlay on tiles. ✓ (★/⧉/↗ ghost strip on hover; delete lives in the right-click menu)
161. **Multi-select ops** — bulk export ZIP, bulk delete, bulk tag. ✓ (ZIP + delete on the selection bar; tag editor applies to the whole selection when the item is in one)
162. **Export selection as ZIP** — one archive via system dialog. ✓ (same as round-2 #77)
163. **Drag out to Explorer** — real OS drag source (open since round 1). ✓ (Windows: hand-rolled OLE CF_HDROP IDataObject+IDropSource in win32.rs, no new dep; drag a tile/row out → modal DoDragDrop; drags the whole selection when in one)
164. **Drag in to import** — drop files onto Library to copy in. ✓ (dropped_files → copy to media dir + rescan)
165. **Duplicate detection** — content-hash warning badge. ✓ (same as round-2 #80)
166. **Retention rules** — keep N days/files; run on idle. ✓ (same as round-2 #79 — "run on idle" is the opt-in auto-sweep on launch)
167. **Storage bar** — per-type usage + "free X by cleaning frames_temp". ✓ (segmented bar under the Library toolbar: Screenshots/Videos/GIFs/Audio/Notes sized from `category_bytes` over the scan + a dim cache segment = `reclaimable_bytes` (frames_temp, frames_scrub, live/, .vibecap thumbs — all regenerable); hover tooltip lists per-type totals; "Clean N MB" runs `clean_reclaimable` — media files never touched; recomputed on every library scan)
168. **Recently deleted** — undo_trash surfaced as a shelf with restore. ✓ (same as round-2 #84)
169. **Open-with menu** — per item, system default vs pick app. ✓ (same as round-2 #82)
170. **Thumbnail repair** — regenerate missing/failed thumbs in background. ✓ (same as round-2 #81)
171. **Sidecar hygiene** — extend denylist; sweep stale `frames_temp`, `.clean.mp4`, `.ffmpeg.log`. ✓ (`sweep_stale_sidecars` on a 10-min cadence; only entries idle >1 h removed so a live record's log is safe)
172. **GIF↔clip routing** — GIFs offer both "trim as clip" and "still frame". ✓ ("Trim as clip" in the context menu; Open still routes GIFs to the Still editor)
173. **Reveal-in-folder on tile** — hover icon opens Explorer with file selected. ✓ (↗ ghost button, thumb top-right)
174. **Selection count bar** — floating action bar appears when ≥1 selected. ✓ (was already shipped)
175. **Library empty-state CTA** — "Take your first screenshot" button routes to Capture. ✓ ("Take a screenshot" primary btn in empty state + import guidance)

## H · Inbox & HITL (176–200)

176. **j/k thread nav** — keyboard-first list traversal. ✓ (was already shipped)
177. **a = first chip** — one-key approve for choice requests. ✓
178. **Esc returns to list** — consistent back-depth. ✓ (clears selection; auto-reselect suppressed)
179. **Snooze + pin** — thread-level controls with restore. ✓ (pin existed; snooze now a 15m/1h/4h menu)
180. **Tray quick-reply** — approve/deny from tray without showing window. ✓ (3 per-request ✓/✗ slot pairs under an "Agent replies" header, labels = question text; slots pinned to request ids so a rescan can't retarget a click; approve → first chip, deny → last chip)
181. **Poll age readout** — "agent polled 12 s ago" per thread. ✓ (was already shipped — `feedback_last_poll_secs` header line)
182. **Answered-history search** — find past Q&A + attached media. ✓ (closed threads match on answer text via lazy-cached response files; search also hits media_path)
183. **Saved snippets** — reusable replies ("blur the token", "re-record 16:9"). ✓ (was already shipped — snippet chips above the composer, session-persisted)
184. **Voice reply preview** — waveform + re-record before send.
185. **Compose lock** — new arrivals never steal selection mid-compose. ✓ (was already shipped — `feedback_user_picked` + draft/choice guard)
186. **Markdown-lite composer** — backticks, links render on the agent side. ✓ (was already shipped — composer hint + passthrough)
187. **Deep links** — `vibecap://feedback/<id>` opens exact thread. ✓ (HKCU `vibecap` URL-protocol registration via Settings toggle; CLI writes `pending_deep.txt` marker → pump wakes parked windows, `poll_pending_deep` routes to the thread; unknown id → Inbox + toast)
188. **Request grouping** — threads collapse by agent/session. ✓ (waiting queue folds into collapsible per-agent headers when >1 agent is pending; single-agent stays flat; collapse set is runtime state)
189. **Unread divider** — "new since you last looked" line. ✓ ("New since last visit" section; `inbox_seen_at` session watermark advances on tab-leave and quit)
190. **Bulk approve** — approve all visible choice threads at once. ✓ ("Approve all n" button answers each with its first chip option; scoped to the search-filtered set; text-only threads skipped)
191. **SLA colors** — threads age-tint (green→amber→red) as they wait. ✓ (timestamp goes amber >10m, red >30m; relative age shown)
192. **Attachment preview** — media_path renders inline thumb in thread. ✓ (was already shipped — inline image preview + Open/Mark up actions)
193. **Reply templates per request type** — screenshot-needed vs approval prompts get different quick replies. ✓ (kind-aware template chips in the composer — annotate/voice/choice/text sets; appends to the draft rather than overwriting)
194. **Notification dedupe** — repeat polls for same request don't re-toast. ✓ (was already shipped — `feedback_notified_ids`; snoozed ids now drop out so expiry re-fires)
195. **Quiet hours** — inbox toasts suppressed, badge still counts. ✓ (manual Quiet toggle in Inbox header + Settings, session-persisted; badge/tray still update)
196. **Agent identity** — which harness/model filed the request, in the header. ✓ (was already shipped — `agent_label` in thread row + detail header)
197. **Request cost** — budget spent by this thread's session so far. ✓ (`FeedbackRequest.cost` — frames/MB/minutes snapshot at ask time, rendered as a chip in the thread row)
198. **One-click resolve** — mark done without a reply. ✓ (was already shipped — "Dismiss" in the composer)
199. **Inbox filter chips** — pending / answered / snoozed / expired. ✓ (All/Pending/Snoozed/Closed chips with counts; Snoozed bucket is now visible — it was hidden entirely before)
200. **Keyboard composer send** — Ctrl+Enter sends; documented hint in-field. ✓

## I · Tray, hotkeys & OS integration (201–225)

201. **Hotkey rebind UI** — Settings editor, applies live (round-1 open). ✓ (`rebind_global_hotkeys` unregisters/re-registers; conflict toast names the taken combo)
202. **Per-mode hotkeys** — region-still, window-still, GIF, pause each rebindable. ✓ (same E86 plumbing — four opt-in digit slots with Apply rebind)
203. **Hotkey conflict detect** — warn when binding collides with OS/browser. ✓ (rebind reports 'already taken by another app' per failed combo — includes pause/PrtScn)
204. **Tray recent-captures** — last 5 items submenu with copy/reveal. ✓ (5 slots follow the library scan; click opens the file)
205. **Tray pause/resume** — during record. ✓ ("Pause/Resume Recording" item under Stop, enabled only while recording; `TrayLiveState::Recording{paused}` drives label + ⏸ tray title; parked clicks wake through `pump_needs_wake` like Stop)
206. **Tray double-click action** — configurable (screenshot / open / record). ✓ (Settings → "Tray double-click" segmented; TrayIconEvent::DoubleClick → TrayAction::DoubleClick resolved per session `tray_dblclick`. Single clicks defer one 400 ms double-click window via shared PENDING_CLICK state — consumed by both the visible-path `poll_actions` and the parked pump's `drain_tray_channels` — so a configured screenshot/record no longer pops the studio first. When the action IS "open" clicks stay instant: a double just shows twice)
207. **Tray icon state** — REC blink baked into icon while recording. ✓ (was already shipped — IconPhase rec disc + arc)
208. **Tray recording elapsed** — tooltip shows `REC 02:41`. ✓ (was already shipped — `Recording {clock}` tooltip)
209. **Single-instance GUI** — second launch focuses first unless `--mcp`/`--screenshot`. ✓ (was already shipped — `gui.lock` pid file + `activate_own_app`)
210. **CLI poke** — `vibecap --capture` forwards to the running instance. ✓ (`vibecap poke <show|hide|screenshot|record|stop>` writes a `pending_cmd.txt` marker the GUI polls each frame → running instance acts on it (lock-fail still focuses it); cold launch picks the marker up on its first frames. Usage errors → E_USAGE exit 2)
211. **Watch-folder import** — monitor a dir, auto-add shots. ✓ (Settings → "Watch folder" Choose…/Off; 3 s poll in update() moves settled files (mtime ≥2 s — skips in-flight copies) with a media extension into the media dir, `_2` suffix on name collisions, rename with copy+delete cross-volume fallback; toast + library refresh on import)
212. **Portable mode** — config/session beside the exe. ✓ (`<exe>/vibecap.portable` marker file is the source of truth — not the `portable/` dir, so Disable works while data stays on disk; `portable_root()` OnceLock-resolved at startup → `config_dir()` → `<exe>/portable/config`, `media_dir()` → `<exe>/portable/media`; everything funneling through `vibecap_config_dir()` — session, budget, feedback, drafts, locks, retro — plus library/thumbs follows; Settings checkbox writes/removes the marker with explicit "restart to apply" copy, no silent migration)
213. **Profile export/import** — settings + hotkeys as one file. ✓ (Settings → Export/Import profile… → `.vcap-profile` zip: manifest.json + session.json via the store-only zip writer/reader; import validates, serde defaults fill fields the file predates, session-only state — tab, open editors, window size, wizard, permission probes — is masked so an export can't yank the importer's UI)
214. **Update checker** — GitHub Releases poll, opt-in, changelog toast. ✓ (worker-thread check — the old sync curl blocked the UI; "Check on launch" session toggle (off = fully offline); newer tag → changelog toast + Settings shows notes preview + Download ↗ opening the release page)
215. **Auto-update channel** — staged: check → download → apply on exit. ✓ (asset picked by target triple → curl download → tar unpack → magic+size sanity → `<exe>.new` staged beside the exe → "Restart to apply" renames running exe → `.old`, swaps in `.new`, relaunches via delayed `cmd`; rollback if the swap rename fails; `.old` cleaned on next launch)
216. **Context-menu verb** — Explorer right-click "Annotate with Vibecap" on images. ✓ (HKCU `SystemFileAssociations\image` verb → `"vibecap.exe" annotate "%1"` — one PerceivedType key covers .png/.jpg/.webp/…, no elevation; raw RegCreateKeyExW/RegDeleteTreeW FFI in win32.rs; Settings checkbox installs/removes; verb lands in Review via the pending-still handoff whether the GUI is running or cold)
217. **Share target** — Windows share contract so apps can send Vibecap images. ✓ (ship-able slice without MSIX: Explorer right-click verb 'Annotate with Vibecap' → `vibecap annotate "%1"`, HKCU-only, install/remove + test; a true ShareTarget contract still needs packaged app identity)
218. **Startup-on-login option** — tray-only resident mode. ✓ (was already shipped — run-at-login registers `"<exe>" --hidden`; `--hidden` starts parked in the tray)
219. **Notification-area copy** — all "menu bar" strings fixed for Windows. ✓ (was already shipped — user-visible strings are cfg-gated ("notification area / system tray" on Windows); remaining hits are code comments)
220. **Windows permissions card** — mic/loopback device, tray status, gdigrab test. ✓ (WINDOWS STATUS card: ✓/✗ for ffmpeg gdigrab/audio/tray, resolved mic name, 'Test screenshot' runs a real grab)
221. **First-run health check** — ffmpeg, write-perms, DPI awareness, tray — one green card. ✓ (wizard's last step runs a real capture smoke test → '✓ works — N bytes captured'; Settings WINDOWS STATUS card carries ffmpeg/audio/tray ✓/✗)
222. **Crash-recovery** — unsaved annotations/session state restored on relaunch. ✓ (two halves: `review_draft.json` — debounced 800 ms draft of the Still editor's strokes as a serializable mirror (stickers as base64 PNGs), canvas-rect included so `sync_annotation_canvas` re-projects into the new layout; on launch a draft whose still still exists restores into Review without a tab switch. Plus orphaned-recorder recovery: a dead pid + frag-MP4 on disk → background remux to a clean MP4, state + breadcrumb discarded, result toasts)
223. **? cheat-sheet kept current** — auto-generate from the binding table, not hand-maintained. ✓ (Global group built from live hotkey fields — rebound digits + optional Pause/PrtScn slots render what is actually registered)
224. **Keyboard-only walkthrough** — wizard step that teaches S/R/Esc in 30 s. ✓ (wizard "Try the keys" step — live S/R/Esc hit detection lights each row, all three unlock Continue)
225. **OS dark-mode event** — live-switch Mono themes when Windows toggles. ✓ (opt-in `theme_follow_os` session flag + `theme_dark_pick` — the remembered dark theme the OS-dark state maps to (Dark/Carbon/Celestial/CelestialPink selectable inline); `tick_os_theme` polls `AppsUseLightTheme` every 3 s from update(), `os_dark_seen` diff means one re-theme + toast per OS flip, None on read-failure → untouched; non-Windows returns None — safe no-op)

## J · Performance (226–250)

226. **Thumb decode off-thread** — `egui_extras` loader already async; verify no decode on UI thread for large files. ✓ (verified: recent-tile and filmstrip decodes run on workers; UI only load_texture's the resulting ColorImage)
227. **Thumb disk cache** — `.vibecap/thumbs` exists; add LRU cap (e.g. 500 MB) + stale sweep. ✓ (`sweep_thumbs` on every library scan: orphans deleted, 300 MB LRU by oldest-modified)
228. **Filmstrip parallel extract** — ffmpeg `-vsync` batch or threaded frame pull. ✓ (JPEG decode + RGBA convert spread across up to 8 scoped threads, round-robin slots, ordered reassembly; progress callback still fires per-frame)
229. **Lazy library page** — only render visible tiles; 1000-file folders shouldn't instantiate 1000 widgets. ✓ (row-culled grid: chunks outside the scroll viewport allocate height but skip tile widgets + image-loader calls)
230. **Region backdrop reuse** — keep last snap texture; skip re-grab when <2 s old. ✓ (backdrop+snap now survive overlay exit; `region_backdrop_at` <2 s + file exists → instant reopen, no grab)
231. **DPI-aware texture cache** — don't re-rasterize icons on scale change storms. ✓ (architecturally satisfied: all bitmaps are device-pixel TextureHandles uploaded once per decode — thumbs, filmstrip, still, backdrop, brand; display rects are point-space so a ppp change only rescales sampling. Icon glyphs are immediate-mode vector shapes, so nothing is keyed to DPI)
232. **Font load once** — semibold/bold loads measured; cache family lookups. ✓ (font files load once behind a call_once block; font_semibold/font_bold family values cached in thread-locals — no per-call String alloc)
233. **Repaint-on-demand** — idle app shouldn't repaint 60 fps; only on input/state change. ✓ (the repaint gate was keyed on `tray.is_some()` → 10 fps forever once the tray existed; now the 100 ms cadence only runs for real in-flight work, retro buffer ticks at 500 ms, live toasts get a 1 s expiry tick, and the pump's slow lane schedules frames for poke markers / watch-folder / OS-theme polls)
234. **Recording finalize off-thread** — shipped for stop; extend to remux/GIF queue. ✓ (verified: stop finalize + orphan remux + GIF exports all run on spawned workers with channel drains)
235. **GIF encode queue** — background worker with progress, not a stop-blocking transcode. ✓ (was already shipped — all clip-tab GIF/video exports run through spawn_ffmpeg_job workers with in-flight status + completion drain)
236. **Startup time budget** — cold launch → interactive <800 ms; measure and track. ✓ (mark_app_start at main() + note_first_frame at end of first update() → startup_ms in doctor + bug bundle)
237. **Memory ceiling check** — long sessions with big thumbs shouldn't exceed ~300 MB. ✓ (process_memory_mb: psapi GetProcessMemoryInfo on Windows, VmRSS on Linux → memory_mb in doctor)
238. **PowerShell spawn removal** — replace `frontmost_app_name`/`window_rect_on_screen` shell-outs with `windows` crate calls (round-1 #28, still the biggest latency item). ✓ (raw FFI, no new crate: window/monitors/focus were already native; this tranche removed the last capture-path spawns — PS focus fallback (AppActivate is strictly weaker than AttachThreadInput+lock-clear+verify), `tasklist` pid probes → `OpenProcess`+`GetExitCodeProcess`, `taskkill` → `TerminateProcess` (frag-MP4 makes hard kill safe). PS remains only for toast notifications + update-check fallback — off the capture path)
239. **Window-list cache** — 500 ms TTL on the pick-list enumeration. ✓ (was already shipped — `WIN_CACHE` 750 ms TTL in `list_capture_windows`, non-blocking `list_capture_windows_cached` for per-frame UI, 2 s callsite TTL + worker dedup via `window_list_rx`)
240. **ffmpeg path resolve once** — resolved at startup, not per-capture. ✓ (was already shipped — `FFMPEG` Mutex caches discovery in `ffmpeg_path`; `ffmpeg_recheck` re-probes only on explicit user request)
241. **Starfield precomputation** — star positions hashed once, not per-frame. ✓ (was already shipped — `build_sky_shapes` output cached per (rect,mode) in a thread_local and replayed via painter.extend)
242. **Gradient mesh cache** — sky mesh rebuilt only on resize/theme change, not repaint. ✓ (shape-list cache keyed by rect+mode)
243. **Toast timer coalescing** — one timer drives all toast lifetimes. ✓ (satisfied by #250 — the single 1 s repaint tick retires both `toast_message` and `capture_toast`; no per-toast timers exist)
244. **Session write debounce** — don't serialize+write session on every state change; batch 500 ms. ✓ (`persist_session` now marks `session_dirty` (Cell); `tick_session_write` in update() flushes at most once per 500 ms; `quit_app`/`on_exit` flush synchronously; dirty state requests a repaint so the flush can't starve under repaint-on-demand)
245. **Log ring-buffer** — `.ffmpeg.log` tail kept in memory for doctor, not re-read from disk. ✓ (remember_ffmpeg_log snapshots the 8 KB tail at stop into FFMPEG_LOG_RING; doctor reads memory, not disk)
246. **Parallel test capture** — smoke tests run gdigrab in parallel with unit tests. ✓ (gdigrab smoke moved to its own windows-latest job running parallel to the unit-test matrix)
247. **Binary size audit** — strip symbols, LTO release; target <15 MB installed. ✓ (release profile: `lto = "thin"` + `strip = true`; CGU=1 left off for iteration speed)
248. **Cold-start no-network** — update check must never block first paint. ✓ (verified: start_update_check runs on a worker thread)
249. **Large-file still guard** — >25 MP stills decode at half-res for canvas, full-res on export. ✓ (still_decode_cache keyed by (path,mtime) so tweaks never re-decode; preview caps working image at 1600 px post-crop, annotate canvas texture at 4096; export/bake still full-res)
250. **Idle CPU zero** — hidden/tray app should sit at 0 % CPU, verified in CI. ✓ mechanics (parked-side work moved to the pump thread: `watch_sweep` extracted to a pure fs fn the pump calls directly when `parked` — files import silently, `watch_moved` count is consumed on next wake → refresh + toast; `pending_cmd_waiting` stat each 250 ms slice wakes the studio for CLI pokes even while parked — previously dead until a repaint; `follow_os`/`watch_dir` mirrored into `WakeShared`; tick_watch_folder skips while parked so the two sweepers can't race). CI verification still open — needs an idle-CPU probe harness.

## K · Reliability, diagnostics & telemetry (251–275)

251. **doctor --fix** — auto-remediate missing ffmpeg PATH, bad output dir, stale session. ✓ (creates missing media/output dirs, clears stale agent-record state + breadcrumb; prints `fix:` lines then the report)
252. **doctor JSON mode** — `--json` for agent parsing. ✓ (`doctor --json` → full report object incl. monitors, env, `mcp_tool_names`, `stale_record_state`)
253. **Last-error surface** — persistent "last capture error" in Settings + tray tooltip. ✓ (`last_error` persists error toasts; Settings row + tray idle tooltip)
254. **Crash log capture** — panic hook writes `vibecap-crash.log` beside session. ✓ (`crash.log` in config dir — panic hook appends timestamped info, then chains to the default hook)
255. **ffmpeg stderr ring** — keep last 200 lines per recording for post-mortem. ✓ (FFMPEG_LOG_RING keeps the last 200 lines; doctor_report prints them)
256. **moov-verify on stop** — probe the MP4 before declaring success; auto-remux retry. ✓ (`verify_mp4` decodes one frame on a worker after stop; missing moov → `remux_to_clean_mp4` to `<stem>.repaired.mp4` and Review/recents re-point at the clean file; unrepairable → loud toast)
257. **Session schema versioning** — migrate old session.json fields cleanly. ✓ (SESSION_SCHEMA const + schema_version field + migrate_session hook in load_session; v0→v1 no-op since all fields carry serde defaults)
258. **Config validation** — bad values (negative fps, missing dir) clamp + warn, not crash. ✓ (apply_session whitelists tab/density/filter/countdown/fps/digits, floors window dims, rejects inverted rects + insane screen dims)
259. **Windows CI smoke** — gdigrab one-frame test asserting file size (round-1 #99, still open). ✓ (ci.yml: cargo test gates all three OSes; windows-latest runs scripts/smoke_capture.ps1 gdigrab still asserting file size)
260. **Golden-theme CI** — screenshot-diff the five themes to catch alpha regressions. ✓ (golden_theme_token_table test — 33 tokens × 5 themes vs checked-in golden; VIBECAP_UPDATE_GOLDEN=1 regenerates)
261. **Input-fuzz test** — rapid region-drag/cancel sequences can't orphan the overlay. ✓ (`fuzz_snap_and_aspect_never_produce_garbage`: 4000 LCG-driven erratic drag positions + degenerate/inverted window rects through `snap_with_guides`/`aspect_clamped` — asserts all emitted geometry stays finite, zero panics)
262. **Kill-recovery test** — terminate mid-record; next launch must finalize or clean the partial file. ✓ (`orphaned_frag_surfaces_and_discards`: writes a dead-pid frag state + 1 KB partial mp4, asserts `orphaned_frag_mp4` surfaces it for launch-time remux and `discard_orphaned_state` clears state+breadcrumb so recovery doesn't loop; real state restored after the test)
263. **ffmpeg-missing UX** — capture buttons disable with a clear fix-it card, not a post-click error. ✓ (fix-it card on Capture with copyable install cmd + live Re-check via `ffmpeg_recheck()`)

### Round-3 first cut (built this pass)

ffmpeg fix-it card + recheck (263), recording MB/min estimate + free-space
readout (60, 61), Library sort menu / tile-size S·M·L / hover reveal
(153, 158, 173), palette fuzzy scoring + MRU section (43, 42), celestial
sky shape cache (242). Verified shipped-not-marked: Ctrl+1–5 (30),
Alt+←/→ (32), Inbox rail badge (27), filename search (151), date groups
(156), selection bar (174).
264. **Self-update rollback** — bad update keeps previous binary. ✓ (previous binary kept as rollback target; `vibecap update rollback` + Settings button swaps back and restarts)
265. **Instance handshake** — MCP + GUI detect each other; avoid dual capture locks. ✓ (gui-recording.json heartbeat: GUI writes pid+mp4 at arm, clears on every terminal path incl. on_exit; `record start` refuses while a live GUI rec owns it, `record status` reports owner=gui, `record stop` says 'stop it in the app'; GUI refuses to record while an agent-side state has a live pid)
266. **Clock-skew guard** — recording timestamps survive timezone changes mid-clip. ✓ (elapsed timing is Instant + accumulated_duration — monotonic, wall-clock immune; segment names use seq not wall time)
267. **Output-dir move handling** — deleted/moved dir → recreate or prompt, never silent fail. ✓ (was already shipped — `create_dir_all` runs at every capture/record spawn site; `capture_to_dir` + agent record map mkdir errors to loud failures, ffmpeg write failure surfaces via exit status)
268. **Long-path support** — >260 char paths via `\?\` prefix on Windows. ✓ (stills >240 chars capture to a short temp then long_move into place via verbatim-prefix rename; ffmpeg never sees `\?\`)
269. **Unicode filename safety** — emoji/non-ASCII in naming tokens don't break ffmpeg args. ✓ (sanitize_token strips stems to ASCII [a-z0-9-_]; ffmpeg argv never sees emoji)
270. **Concurrent capture guard** — two rapid hotkey presses can't spawn two ffmpeg procs. ✓ (was already shipped — `still_busy` AtomicBool CAS `swap` in the pump fast path + `screenshot_in_flight`/`still_busy` check in the UI path; second trigger returns early)
271. **Tray-missing fallback** — if tray creation fails, keep a floating mini-bar alive. ✓ (fallback is honest: close-quits + a persistent 'no tray — close quits' status chip; tray ✗ already on the Windows status card)
272. **DPI-change mid-pick** — region rect re-maps if scaling changes while overlay is up. ✓ (overlay tracks `pixels_per_point` in ctx.data; on change the in-flight selection + history rescale by the ratio so the box stays glued to the same physical pixels)
273. **Monitor-hotplug handling** — disappearing display re-targets fullscreen gracefully. ✓ (resolve_monitor drops stale index → default + toast; wired into GUI stills, recordings, pump hidden-capture)
274. **Timestamp monotonicity** — output names use monotonic seq when clock steps back. ✓ (next_seq dedupes on disk contents — a stepped-back clock still yields a unique name)
275. **Telemetry opt-in** — anonymous capture-success/fail counts; off by default, no content ever leaves. ✓ (`stats_opt_in` gates four session counters — shots/recs ok/fail, bumped at finish_screenshot/finish_stop_recording; counts surface in the About card + Library stats line; no network path exists)

## L · CLI / MCP, settings & onboarding (276–300)

276. **`--json` on all CLI verbs** — machine-readable output for agents. ✓ (screenshot/record/paths/doctor/list/status all honor --json; errors → `{"ok":false,"code":"E_*","error":…}` on stderr)
277. **CLI progress events** — `--record-status --watch` streams JSON lines. ✓ (`record status --watch` ticks each second until the recorder exits; NDJSON under --json)
278. **MCP tool parity check** — doctor verifies every documented tool is registered. ✓ (`MCP_TOOL_NAMES` const mirrors tools/list; doctor reports `mcp_tools=19` + `mcp_tool_names`)
279. **MCP error codes** — stable `ERR_*` codes agents can branch on. ✓ (`error_code()` classifier → `result.errorCode` on MCP failures + `error[E_*]` on CLI stderr: E_USAGE/E_NOT_RECORDING/E_ALREADY_RECORDING/E_NO_WINDOW/E_SELF_CAPTURE/E_BUDGET/E_FFMPEG/E_PERMISSION/E_NOT_FOUND/E_FAILED)
280. **CLI dry-run** — `record start --dry-run` validates ffmpeg line without running. ✓ (prints the exact argv — same `record_args` builder as the real spawn, `dry` only skips window focus)
281. **`vibecap open <id>`** — open a Library item straight in Review from CLI. ✓ (stills → Review via pending-still marker; clips/audio → OS default app)
282. **CLI list** — `vibecap list [--type video] [--limit n]` prints media. ✓ (newest-first `name\tbytes\tkind`; --json array; --output-dir overrides the scanned dir)
283. **CLI annotate** — `vibecap annotate file.png --arrow x1,y1,x2,y2` headless. ✓ (ops → headless bake via the GUI's own rasterizer → `<stem>_annotated.png`/`--out`; no ops → opens the Studio annotate surface)
284. **Settings search** — filter box over all prefs. ✓ (rail filter field matches section haystacks; filtering shows all hits and force-opens the collapsed groups)
285. **Settings sections as rail** — left-nav inside Settings instead of scroll. ✓ (left rail: All / Save / Recording / Library / Advanced / Shortcuts & look / Agent / About & help; All preserves the old scroll)
286. **Setting tooltips** — every toggle explains its effect + default. ✓ (each switch row carries a dim explainer line under it; non-obvious checkboxes also have hover text — e.g. PrtScn, explorer verb, deep links, portable, follow-OS)
287. **Reset-to-default per section** — not just global reset. ✓ ("↺ Reset section" on Save / Recording / Library / Retro / Shortcuts & look / Agent cards — restores shipped defaults, hotkeys rebind, theme resets to Dark)
288. **Wizard Windows page** — ffmpeg test-shot, mic check, hotkey conflict check. ✓ (new 'Check your setup' step: real capture smoke test, worker-thread mic enumeration via dshow list, hotkey registration status with rebind hint)
289. **Wizard MCP detect** — find Cursor/Claude/Codex configs, offer snippet paste. ✓ (was already shipped — step_agent_connect renders mcp_client_rows for the big three clients with per-row config presence + paste-ready snippet)
290. **Wizard theme pick** — live preview of all five, sets preference at first run. ✓ (welcome step renders the real five theme_swatch chips; clicking applies via set_theme)
291. **Re-open wizard** — "Replay setup" in Settings. ✓ (was already shipped — "Replay first-run wizard" button resets wizard state)
292. **In-app changelog** — What's New card after update. ✓ (apply stashes tag+notes in session; ABOUT & HELP card shows them once with "Got it" dismiss)
293. **Docs links in-app** — ? icon → relevant doc section per tab. ✓ (ABOUT & HELP card links capture recipes / MCP tools / roadmap / releases)
294. **Stats card** — captures/week, bytes saved, streaks (round-2 #99, still open). ✓ (Library header stats line)
295. **Budget dashboard** — per-session spend sparkline in Inbox. ✓ (Inbox header strip: tier, frames/MB/minutes usage vs caps, and a sparkline of `budget_samples` taken every 30 s in `update()`; warning styling when near/over a cap; unlimited caps and empty sessions render clean)
296. **Naming-token builder** — visual `{app}-{date}-{seq}` composer with live preview. ✓ (insert chips for {app}/{date}/{time}/{seq}/{orig}/-/_ append to the pattern under the live preview)
297. **Export diagnostics bundle** — one click → zip of logs+session+doctor for bug reports. ✓ (bug_report_pack now emits bug_<ts>.zip: screenshot + retro.gif + doctor.json + system.txt + session/budget/draft + newest .ffmpeg.log, STORED via app::zip)
298. **Community theme repo hook** — `--theme-import URL` fetch+validate. ✓ (`vibecap theme import <url|file>` — curl/fs fetch, validates the vibecap_theme marker, merges into session.json)
299. **Locale-ready strings** — wrap UI strings in a `tr()` macro now so i18n isn't a rewrite later.
300. **API surface freeze** — document which CLI/MCP contracts are stable vs internal for agent authors. ✓ (docs/API.md: stable CLI verbs/JSON append-only rules, MCP tools, env vars, on-disk paths vs internal surfaces)
