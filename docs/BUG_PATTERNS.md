# Bug patterns (Vibecap)

Reusable defect classes. Update when a confirmed bug reveals a pattern.

## File config vs long-lived in-memory state

**Pattern:** MCP or another process writes `*.json` under `~/.config/vibecap/`; a GUI worker holds `Arc<Mutex<T>>` loaded once and never reloads.

**Symptom:** Tool claims “app will pick this up”; nothing changes until restart.

**Prevention:** Workers re-read config on an interval (or watch mtime). Tool text must not claim live pickup without a reload path. Add a contract test or smoke note.

**Example fix:** retro worker reloads `retro.json` every ~2s (`src/app/retro.rs`).

## Shared media cleared by process lifecycle

**Pattern:** “Shared” disk buffer between GUI and MCP is wiped in `new()` / `Drop` for privacy.

**Symptom:** Evidence disappears on restart; agent dump after app quit fails.

**Prevention:** Distinguish **explicit clear** (user disable / Clear button) from **process lifecycle**. Prefer prune-by-age/size over wipe-on-start. Document the contract in MCP.md.
