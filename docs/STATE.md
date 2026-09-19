# Current state (read this on resume)

Last updated: 2026-09-06. Source of truth is `master` on GitHub, not a chat transcript.

Windows capture (2026-09-06): hide-for-capture **minimizes** (taskbar button stays). `Visible(false)` dropped the taskbar entry and stalled Inbox; off-screen park was clamped back into gdigrab. Park/restore: `src/app/capture_flow.rs` + `src/platform/win32.rs`. ffmpeg stills detach stdio. Region overlay is a dedicated always-on-top viewport (virtual-desktop bounds); mouse-up captures. See `docs/IMPROVEMENTS.md`.

Perf/UI-thread (2026-09-06): stop-recording no longer blocks — `finalize_recorder` runs on a worker (`record_finalize_rx` → `finish_stop_recording`; UI shows "Saving…", `TrayLiveState::Finalizing`). Voice memo finalize is a worker too. `status_snapshot` caches the dir walks/budget read ~2s (live REC/inbox fields overlay per frame). `refresh_library` scans the media dir on a worker (`library_scan_rx`, pending-flag rescan). `poll_frontmost_app`, `refresh_window_list`, and the DirectShow device probe are worker-based; the window ComboBox reads `list_capture_windows_cached()` (never spawns PS per frame). ffmpeg records with `-preset veryfast` (medium dropped gdigrab frames under load).

Visual pass 2 (2026-09-06): capture target is now three icon cards (new `Icon::Monitor`/`Region`/`Window` in icons.rs) — accent-tinted when selected. A "RECENT — TAP TO REVIEW" thumb strip sits under the capture buttons (`recent_thumbs_rx` worker: ensure_thumb + decode off-thread → ColorImage → texture on UI; click routes to the right Review editor, videos get a ▶ overlay). Empty library shows a "captures land in Review → Library" hint. Active rail stage gets a 3px accent tick on its left edge. Every stage title now carries a one-line subtitle (`AppTab::subtitle`). Library heading simplified to "Library".

Snipping-tool UX pass (2026-09-06): rail labels are plain language now — Capture · Review · Library · Inbox, with Settings pinned to the rail bottom and a ↓ flow hint between Capture and Review. Capture page reads as a funnel: "What do you want to grab?" → big target segmented (🖥/✂/🪟) → big Screenshot (primary accent) / Record / GIF strip → Options disclosure → "Advanced · live session & agent budget" collapsed (force-opens on budget-over or retro running). Settings folds Windows-capture internals + retro buffer under "Advanced" and agent budget under "For agents". Capture toast's primary action is now "Annotate". Jargon renamed user-facing (Shutter→Capture, Media→Library; enum names unchanged). Fixed a per-frame `live_usage_snapshot`+`load_budget` in the Settings budget card — uses `live_stats_snapshot`.

Tray/keys/autostart pass (2026-09-06): recording now shows a pulsing red REC disc plus a 60 s progress arc in the tray icon (`IconPhase`; `set_title` is macOS-only so the icon is the counter on Windows) — tooltip still carries m:ss. New global hotkey Ctrl+Alt+V toggles the window (`toggle_window`; ignored while parked mid-capture). In-window keys: Ctrl+1–5 jump to Loop stages, Alt+←/→ stage back/forward (`tab_back`/`tab_fwd`/`prev_tab`). First-run wizard gained a "Start Vibecap when I sign in" step (step 4 of 5, `set_run_at_login` → `HKCU\...\Run\Vibecap = "<exe>" --hidden`); Settings has the same toggle. Library rows and the Still/Clip headers gained "Open" (default app via `open::that`) and a primary "✓ Done" (copy → back to Shutter).

Review-stage merge (2026-09-06): the Loop rail is now five stages — Shutter · Review · Media · Inbox · Settings. `AppTab::Clip`/`Still` remain as internal editors; both map to `LoopStage::Review` and `tab_for_loop` dispatches Review via `review_tab()` (whichever editor has content, else last-used via `last_review_tab`, else Still). Stage titles read "Review · Clip" / "Review · Still". Tray and palette have a single GoReview; session restore accepts "review". Empty editors show funnel CTAs ("Nothing to review yet — take a screenshot / record a clip"). Web studio: `Stage` is now shutter · review · pack · inbox · agent · settings — Media+Still merged into `review` (grid when nothing selected; StillStage annotate or ClipReview video player when one is, with "← Library" back), Sources folded into the Agent stage.

Win32-native helpers (2026-09-06): the PowerShell helpers (`window_rect_on_screen`, `windows_enum_windows`, `windows_list_monitors`, `windows_foreground_process_name`, `windows_focus_app`) each spawned powershell + Add-Type (~300-800ms, three chained per windowed shot). Now direct FFI in `src/platform/win32.rs` (`find_window_rect`, `enum_windows`, `enum_monitors`, `foreground_process_name`, `focus_window`). Matching is exact→fuzzy on title/process — same window picked by focus and rect. `focus_window` clears SPI_SETFOREGROUNDLOCKTIMEOUT + AttachThreadInput; PS AppActivate remains only as a focus fallback. Focus settle sleep 600→350ms on Windows (focus is verified before returning). Old PS bodies removed — parse fns are `#[cfg(test)]`.

Library tile polish (2026-09-19): removed the `loop_position_badge` pill from Library tiles — its colored fill + clipped label sat over the filename row. Stage is now plain text in the meta line (`type · size · stage`, only when non-Capture). Deleted the badge component, its `ui::` re-export, and the unused `LOOP_*` theme colors in `theme.rs`. Tile contract: click opens Review, Ctrl+click toggles, Shift+click range, right-click menu — no overlaid CTAs.

Theme system (2026-09-19): ported the Browmie/Chromie **mono-ui** spec (`C:\Dev\Chromie\docs\mockups\mono-ui.html`) + its **Celestial** theme into `theme.rs`. `ThemeMode` is now Dark / Light / **Celestial**; a `tri!` macro gives tokens a third celestial arm while `dual!` falls back to dark values. Dark/Light retokenized to the Mono zinc palette (#0b0b0c/#f4f4f5 canvases, hairline solid borders, ink = primary). Celestial: void #0A0820 canvas, indigo surfaces, pink #EC4F8E accent, teal #26D6C0 secondary, starfield + aurora strip painted in CentralPanel, aurora rail tick. Radii now 12/9/6. Settings theme control is a 3-swatch picker with live previews (`theme_swatch` in settings_tab.rs); ToggleTheme palette cycles all three; persisted as "celestial" in session.

## Where we are

| | |
| :--- | :--- |
| Tag | [v0.3.0](https://github.com/TekosherM/vibecap/releases/tag/v0.3.0) |
| Tip | `master` — Windows capture contract + studio HITL (unreleased vs tag 0.3.0) |
| Connector that works | **CLI** `record start` / `--screenshot` / `record stop` / `doctor`. Linux = ffmpeg x11grab. Windows = ffmpeg gdigrab. |
| Lumen Cart connector | Open web studio tab + `vibecap_job` |
| MCP tools (when harness surfaces them) | `vibecap_record_start/stop/status`, `vibecap_record_video`, `vibecap_capture`, `vibecap_export_gif`, live-inspection + budget + feedback tools |
| Connector that often fails | Native `vibecap --mcp` in Cursor / Grok Bot dynamic-tool harnesses (tools never appear) |

## Agent recipe (signed-in desktop / Chrome flow)

```
vibecap record start --output-dir ./frames --display "$DISPLAY"
vibecap --screenshot --output-dir ./frames
vibecap record stop
```

See `docs/AGENTS.md`. Do **not** use `GET /api/agent/still` without `display`/`output_dir` — that is the demo shutter.

## Agent recipe (Lumen Cart evidence)

```
GET  /api/agent/hooks
POST /api/agent/call  {"tool":"vibecap_job"}
GET  /api/agent/still/{id}.jpg
GET  /api/agent/clip/{id}.webm
```

Job records, walks Lumen Cart (coupon 422 → tax 500 → pay 402, 3 stills), ingests frontend/backend/database/logs, stops, packs. Clip is persisted (`captures.clip_url`) and survives reload.

## Done (do not redo)

- Unbounded record + snap-while-REC
- Walk / coupon / tax / pay + auto stills
- `vibecap_job` one-shot
- WebM persist + Pack Download JSON / stills / clip
- Job re-runnable (resets checkout)
- Screen/camera banner: pixels only; JSON still taps Lumen Cart
- Native `--help` / crate 0.3.0: if MCP never attaches, use the web studio
- Agent record path fixed: start/stop, still-to-dir, real Linux screen (PR #22)
- MCP: `vibecap_record_start/stop/status` + `vibecap_capture` with display/window/output_dir; `record_stop` returns persisted clip_path
- Web HTTP studio shells to the same CLI when `display`/`output_dir` args are passed
- Desktop UI polish pass 2026-08-25: library heading de-accented, capture live-stats row, inbox auto-select respects explicit picks, ⌘I opens Inbox, import rot pruned (cargo check clean of unused imports)
- GUI screenshot focus hardening 2026-08-26: bare-desktop shots refused with guidance when no focus target; `focus_app` verifies + retries + reports failure
- Agent recorder detach 2026-08-26: `record start` returns immediately, ffmpeg in own process group, stderr → `.ffmpeg.log`; honest crash status; absolute output paths (stop/status cwd-independent). Verified end-to-end on macOS: piped-shell start returns in ~140ms, mp4 finalizes, GIF companion works
- Windows capture repair (2026-09-06): dedicated region overlay, ordered-in park, always-on REC bar, gdigrab offsets + HWND fallback, no silent fullscreen for missing window, negative virtual-desktop coords, HiDPI overlay map, frag remux, stdio detach (`run_ffmpeg`)
- `vibecap doctor` / `--doctor` / `--paths` prints ffmpeg path + GUI stdio + window-crop hint
- GUI single-instance lock; second GUI focuses the first. `--mcp` / CLI stay multi-process
- Naming tokens `{app}-{date}-{time}-{seq}`; Settings live preview; `VIBECAP_OUTPUT_DIR` “Use for CLI/agents”
- Library: date groups, disk thumbs (`.vibecap/thumbs/`), sidecar denylist, Shift-click range, drag-in import
- Still: crop-drag, in-place text, badge renumber, save overwrite vs copy, scroll zoom / Space pan
- Clip: preview labeled no-audio, in/out loop, GIF fps/width, Discord/README/lossless presets, `M` chapter markers
- Inbox: j/k, snippets, pin/snooze, composing lock, last-polled stamp, tray Approve/Deny first pending, `vibecap://feedback/<id>`
- Pause hidden on Windows (SIGSTOP is Unix-only). Voice memo prefers a real dshow *input* device
- Known remaining (do not rediscover): live HWND thumbnails; library hover-scrub is a poster (scrub in Clip); no OS drag-out to Explorer; no voice waveform; hotkey digit change applies on next GUI launch; macOS *record* `--window` still focuses then captures the display (stills crop via `screencapture -l`)
- CI: Linux `libxdo-dev`, web typecheck, MCP smoke accepts 0.3.x, Windows `scripts/smoke_capture.ps1` (continue-on-error)
- Native binaries on v0.3.0 (macOS arm/intel, Linux, Windows). Tip of `master` is unreleased capture/studio work — run `cargo build --release`, not the tag, for Windows gdigrab.
- Funnel shell (2026-09): stage rail hidden by default (☰/Ctrl+B toggles); the three funnel stages stack as one column — active stage expands, others become clickable stripes with an animated squeeze/reveal (Capture top · Review middle · Library bottom); Inbox/Settings get a back-to-Capture stripe + header quick icons; status strip renders only when actionable (REC / ffmpeg missing / inbox pending); autostart Run-key now via advapi32 FFI (no reg.exe console flash); tray/palette/Settings Quit does a guaranteed process exit (kill recorder child, persist session) — ViewportCommand::Close could no-op while minimized.
- UI redesign pass 2 (2026-09): Segoe UI/Cascadia loaded as app fonts + roomier chrome spacing; Review·Still is canvas-left/inspector-right (tools, brush, look, crop in a 244px rail, zoom +/− in canvas header); Review·Clip is player-left/inspector-right (trim/GIF/presets/audio/transform in rail); Library is a date-grouped thumbnail grid — click opens in Review, right-click for Open/Reveal/Copy/Delete; clip transport keys no longer fire while typing in fields.
- Region-capture reliability (2026-09): overlay tracks `was_dragging` so a release at the screen edge still confirms when `drag_stopped` is missed; after the overlay closes the studio re-asserts foreground for 4 frames (`region_refocus_frames` → `restore_studio_to_taskbar` AttachThreadInput path) — the child viewport's teardown could steal focus back, which was the "stuck until you click" report.
- Menu compaction pass (2026-09): Review·Still header is Done/Copy/Save + a ⋯ menu (Select image, Save as copy, Open, Reveal, Reset edits); Review·Clip is Done/Open + ⋯ (Select video, Reload, Open, Reveal); Library moved search into the header row, Refresh/Free-live-frames/Clear-list into ⋯, and the selection bar (Reveal/Delete/Clear) only renders when something is selected.
- Snagit-class capture latency (2026-09): capture hides use synchronous `SW_HIDE` (`hide_studio_window`, ~100 ms settle vs the old 450 ms minimize), native GDI `BitBlt` stills try first with ffmpeg fallback, `next_seq` overlaps the settle, and the focus wait is skipped when the target is already frontmost. Region pick goes further: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` on the studio replaces the hide entirely — and the selector now opens **instantly** (~16 ms, dim fill) because the "Vibecap Region" overlay HWND gets its own exclusion via `set_title_capture_excluded` before the snap fires (`region_overlay_state` atomic: 1=excluded, 2=give up→close overlay→delayed path; the worker waits on it, plus a `snap_is_uniform_dim` contamination guard for the residual timeout case). Freeze backdrop swaps in ~250 ms. Cancel during the dim phase can't resurrect the pick (drain guards on `pending_region_kind`); drain only calls show_window when the owner actually needs restoring (no focus-steal of the open overlay). Click-to-pick a window: Capture → Window → "🎯 Pick" runs the overlay in `RegionPickKind::WindowPick` mode — throttled `window_at_point` hit-test (Z-order, skips self/minimized/shell surfaces) + click confirms the window rect and crops the same freeze snap, no focus juggling; picked name feeds `window_app`. Tray-hide keeps `minimize` (taskbar must persist); recording-arm keeps deferred minimize (REC bar needs a live loop); `toggle_window` ignores summons mid-pick so the owner can't hide under the overlay. Not live-verified yet.
- Round-2 tranche (2026-09): `docs/IMPROVEMENTS.md` gained a second 100-item list aimed at beating Snagit/Snipping Tool; first cut shipped seven. (1) Capture delay — `capture_delay_secs` 0/3/5/10 combo on Capture; the still worker sleeps it after the hide (menu/tooltip shots), and `CaptureCfg.delay_secs` mirrors it for the pump's hidden fast path. (2) Repeat last capture — `repeat_last_capture` re-arms the same recording when `last_capture` is a Clip, re-grabs `selected_screen_rect` via a temp snap+crop worker (studio capture-excluded, not hidden), else falls back to a fullscreen still; wired to `PaletteAction::RepeatLast` and tray "Repeat last capture" (pump wakes a tray-hidden studio for it since `parked` is false). Inside the overlay `R` or a double-click on the last-region ghost confirms it directly. (3) `RegionPickKind::WindowRecord` + "🎯 Rec" button — same click-pick overlay, confirm sets `capture_target=Region` so the picked pixels are the record crop (no re-resolve by name). (4) Aspect chips Free/1:1/16:9/9:16 in the overlay toolbar → `region_aspect_lock`, `aspect_clamped` pins drag height (Shift/Alt still override). (5) WASD nudges alongside arrows. (6) `clipboard_only` switch — finish_screenshot copies, deletes the file, skips Review/library/card. Delay and clipboard-only are runtime-only (not session-persisted). Still not live-verified.
- Round-2 second cut (2026-09): pick-mode upgrades — `pickable_at` returns the whole pickable Z-stack at a point so scroll-cycling works (`window_pick_cycle`, reset when the cursor moves >6 px; `windows_at_point` is the live helper, `top_window_at` is now test-only); dead space under the cursor offers the whole monitor (`monitor_at_point` → "Display" pick, skips `window_app` adoption via `window_pick_hover_monitor`). Pre-warm backdrop: `start_region_pick` keeps the previous snap + marks `region_backdrop_stale` — the overlay shows the old freeze instantly stamped "⟳ refreshing…" instead of a blank dim; confirms are blocked while stale (crop pixels must map to the snap that gets cropped) and the drain clears the flag on arrival. W×H plate shows physical px when `pixels_per_point` ≠ 1. Palette gained "Copy last capture as Markdown" (`![](path)`) and "Copy last capture path". `silent_mode` switch on Capture suppresses non-error toasts (`ToastLevel::Error` still surfaces) and the shutter flash. `?`/F1 opens a grouped shortcut cheat sheet (`ui::palette::show_cheatsheet`). Loupe hex (#23) and post-capture card actions (#73) turned out already shipped — marked in the doc. Still not live-verified.
- Live-feedback fix pass (2026-09): recording arm no longer minimizes the studio — `set_studio_capture_excluded(true)` keeps it visible but invisible to gdigrab, so `update()` never stalls mid-arm (a minimized HWND gets no WM_PAINT → `drain_record_spawn` never ran → `recording_arming` stuck + headless ffmpeg until reopen; this was the "video won't start/close, must restart app" report). `record_excluded` + `region_affinity` are co-owned via `sync_capture_exclusion` (neither owner can drop the other's exclusion); released on every exit (cancel armed/active, spawn error, finish). The "Vibecap Recorder" bar gets its own exclusion (`rec_bar_exclude_attempts`, ≤8 tries, stops on Denied). Fallback when affinity is denied: the old park+minimize path. Clip tab: `filmstrip_error` is now actually rendered (it was set but never displayed — dead Play button with no explanation); Play with zero frames opens the file externally instead of no-op; preview auto-plays when frames land; Esc backs out to Capture; filmstrip density 24→64 frames (fps clamp 4→6) so playback visibly animates. Capture tab reordered: shutter strip (Screenshot/Record/GIF) is now the FIRST element, target selection demoted to a compact icon+label segmented row under "From" with a one-line hint — the three big cards that read as primary actions are gone. Not yet live-verified on hardware.

## Still true (not bugs to “fix” by pretending)

- Native MCP is two processes. Do not rewrite it into the GUI in a drive-by.
- JSON hooks bind to **Lumen Cart**, not a random shared Chrome tab.
- Auth is off. Rows unowned. Neon if `DATABASE_URL`, else PGLite.

## Save rule for this repo

Commit and push to `master` **before** the turn ends. Chat is not durable. If a turn dies, clone `TekosherM/vibecap` @ `master` and continue from this file.
