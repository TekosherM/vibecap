# Active bugs

Unresolved production issues. Move to resolved notes / CHANGELOG when fixed.

| ID | Severity | Status | Summary |
| ---: | :--- | :--- | :--- |
| — | — | — | None open after retro contract fix (2026-08-06 review remediation). |

## Resolved in review remediation (2026-08-06)

| Was | Fix |
| :--- | :--- |
| MCP `set_retro` did not drive GUI worker | Worker reloads `retro.json` every ~2s |
| GUI wiped ring on start/exit | Seed from disk on start; Drop only stops worker |
| Headless enable produced no frames | MCP process starts local capturer on enable |
| Silent worker spawn failure | Surface error in `RetroStatus.last_error` |
| Millis-only frame names | `f_{ms}_{seq:06}.jpg` |
