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

**Prevention:** Park off-screen and keep ordered-in (`src/app/capture_flow.rs`). `Visible(false)` is tray-hide only. Debug assert `pre_capture_outer` is cleared after every capture exit path.

## even_screen_rect must not clamp origin to 0

**Pattern:** yuv420p needs even width/height; a helper also clamped `x,y` to ≥0.

**Symptom:** Left-of-primary monitors capture the wrong crop (or fail).

**Prevention:** Even `w`/`h` only. Virtual-desktop origin may be negative. Unit test `even_crop_forces_even_dimensions`.

## Shared media cleared by process lifecycle

**Pattern:** “Shared” disk buffer between GUI and MCP is wiped in `new()` / `Drop` for privacy.

**Symptom:** Evidence disappears on restart; agent dump after app quit fails.

**Prevention:** Distinguish **explicit clear** (user disable / Clear button) from **process lifecycle**. Prefer prune-by-age/size over wipe-on-start. Document the contract in MCP.md.
