# Bug patterns (Vibecap)

Reusable defect classes. Update when a confirmed bug reveals a pattern.

## File config vs long-lived in-memory state

**Pattern:** MCP or another process writes `*.json` under `~/.config/vibecap/`; a GUI worker holds `Arc<Mutex<T>>` loaded once and never reloads.

**Symptom:** Tool claims “app will pick this up”; nothing changes until restart.

**Prevention:** Workers re-read config on an interval (or watch mtime). Tool text must not claim live pickup without a reload path. Add a contract test or smoke note.

**Example fix:** retro worker reloads `retro.json` every ~2s (`src/app/retro.rs`).

## Windows GUI-subsystem stdio inherited by ffmpeg

**Pattern:** Release is `windows_subsystem = "windows"`. `Command::status()` / inherited stdin/stdout are NUL. ffmpeg then fails with `Could not open file` even though the same args work in a console.

**Symptom:** CLI ffmpeg works; `vibecap.exe` stills are empty or missing. Direct `ffmpeg -f gdigrab …` works.

**Prevention:** Every ffmpeg job goes through `platform::run_ffmpeg` (stdin/stdout null, stderr piped, `CREATE_NO_WINDOW`). Never add a new `Command::status()` capture path. `vibecap doctor` prints `gui_stdio=detached`.

## Visible(false) destroys child viewports

**Pattern:** Hide-for-capture used `ViewportCommand::Visible(false)` so the studio is not in the shot.

**Symptom:** Region overlay paints inside the main window; REC bar never appears; recordings become unstoppable except via tray.

**Prevention:** On Windows, **minimize** for capture and tray-hide (`ShowWindow(SW_MINIMIZE)`). That keeps the taskbar button and the event loop (Inbox polling). `Visible(false)` is macOS tray-hide only. Never overwrite saved geometry with a 120×80 park. Debug assert `pre_capture_outer` is cleared after every capture exit path.

## even_screen_rect must not clamp origin to 0

**Pattern:** yuv420p needs even width/height; a helper also clamped `x,y` to ≥0.

**Symptom:** Left-of-primary monitors capture the wrong crop (or fail).

**Prevention:** Even `w`/`h` only. Virtual-desktop origin may be negative. Unit test `even_crop_forces_even_dimensions`.

## Windows clamps off-screen park back into the shot

**Pattern:** Hide-for-capture moved the studio to `(-12000,-12000)` at 120×80 so it would not appear in gdigrab.

**Symptom:** Screenshots still contain Vibecap; after restore the window is missing from the taskbar or stuck tiny/off-screen.

**Prevention:** Do not park off-screen on Windows 10/11 — DWM relocates fully off-screen windows. Minimize instead. Only snapshot geometry when the window is still studio-sized (`should_snapshot_geometry`).

## Shared media cleared by process lifecycle

**Pattern:** “Shared” disk buffer between GUI and MCP is wiped in `new()` / `Drop` for privacy.

**Symptom:** Evidence disappears on restart; agent dump after app quit fails.

**Prevention:** Distinguish **explicit clear** (user disable / Clear button) from **process lifecycle**. Prefer prune-by-age/size over wipe-on-start. Document the contract in MCP.md.

## Simple `-af`/`-vf` conflicts with a `-filter_complex` stream

**Pattern:** ffmpeg argv built by string concat across multiple sites — a
shared output section appends `-af <meter>` while an earlier section
conditionally emits `-filter_complex ... [a]` for the mic+system mix.

**Symptom:** ffmpeg refuses to start: "Simple and complex filtering cannot
be used together for the same stream." Mixed recordings break entirely
while single-device recordings work — argv tests pass either way.

**Prevention:** When a stream comes from `-filter_complex`, put extra
filters INSIDE the graph (`asplit` tap + `anullsink` sink) and gate the
simple `-af`/`-vf` on the graph's absence. Argv-shape tests alone can't
catch this — smoke the generated argv against real ffmpeg (a lavfi
testsrc+sine one-liner proves it in seconds).

## ffmpeg `astats` emits `-inf` on digital silence

**Pattern:** Meter/parser assumes `key=value` values are numeric.

**Symptom:** Parse returns None → caller keeps the last loud level → the
live meter freezes exactly when the source goes silent.

**Prevention:** Treat `-inf`/`inf`/`nan` as sentinel values mapped to the
display floor, distinct from "no data yet". Test silence explicitly.
