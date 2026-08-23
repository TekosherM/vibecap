# Agent harness — capture without a pre-wired MCP client

Cursor, Grok Bot, and other **dynamic-tool** harnesses often **do not** surface
`vibecap --mcp` as a live connector. Agents then only see the binary and
`--screenshot`. This is the supported path for that situation.

Do not mix this with the Lumen Cart demo shutter. A still of
`checkout.lumen.test/pay` is the studio subject, not the Chrome window you are
driving.

## Capture-only recipe

```bash
# once
cargo install --path .          # or: cargo build --release
# Linux: ffmpeg on PATH, DISPLAY set (agent backend = ffmpeg x11grab)
echo "$DISPLAY"                 # e.g. :0 or :1

OUT=./frames                    # caller-specified; always pass this
mkdir -p "$OUT"

vibecap record start --output-dir "$OUT" --display "$DISPLAY"
# …drive the signed-in flow (10–20+ minutes is fine)…
vibecap --screenshot --output-dir "$OUT" --display "$DISPLAY"
# optional: crop to a window title
# vibecap --screenshot --output-dir "$OUT" --window "Chrome"
vibecap record stop             # MP4 in $OUT; add --gif for a companion GIF
```

Same verbs as flags: `--record-start`, `--record-stop`, `--record-status`, `--paths`.

| Flag | Meaning |
| :--- | :--- |
| `--output-dir`, `-o` | Where stills / MP4 land. Also `VIBECAP_OUTPUT_DIR`. |
| `--display`, `-d` | X11 `DISPLAY` (also `VIBECAP_DISPLAY` / `$DISPLAY`). |
| `--window`, `--app` | Focus + Linux crop to that window title. |
| `--gif` | Companion GIF on stop. |
| `--paths` | Print the **one** default media dir for this OS. |

Default when `--output-dir` is omitted: `dirs::video_dir()/Vibecap`, else
`~/Movies/Vibecap` on macOS, else `~/Vibecap`. Do not guess. Use `--paths`.

## If MCP *does* attach

Portable config (checked in): [`.cursor/mcp.json`](../.cursor/mcp.json) runs
[`scripts/vibecap-mcp.sh`](../scripts/vibecap-mcp.sh) so the client does not
need a machine-specific `mcp.json`.

```
vibecap_record_start  { display, window, output_dir, gif? }
vibecap_capture       { display, window, output_dir }
vibecap_record_stop   { gif? }
vibecap_record_status
```

`vibecap_record_video` with **no** `duration_secs` starts unbounded (then stop).
With `duration_secs` (max 600) it still records a short clip.

## Web studio HTTP

`cd web && npm run dev` remains the connector for **Lumen Cart evidence**
(`vibecap_job`) and for JSON hooks.

To capture the **real display** through the same HTTP surface (no tab, no demo
shutter), pass `display` / `window` / `output_dir`. The server shells out to the
same `vibecap` CLI (ffmpeg x11grab):

```
POST /api/agent/call
{"tool":"vibecap_capture","args":{"display":":0","output_dir":"./frames"}}
```

Without those args, `snapshot` / `capture` is the shutter — demo pay screen if
the studio is on Demo.

## Linux

Supported agent backend: **ffmpeg x11grab** (what `--help` prints). Not a hidden
fallback.

| Need | Notes |
| :--- | :--- |
| `ffmpeg` | `sudo apt install ffmpeg`. Override with `VIBECAP_FFMPEG`. |
| `DISPLAY` | X11 / XWayland. `echo $DISPLAY`. Pass `--display` if the agent’s env differs. |
| Window crop | `wmctrl` / `xdotool` (best-effort). Otherwise focus + full display. |
| Wayland | Stills may fall back to `grim` only when no display was named and x11grab fails. Prefer an X11 session for long recordings. |

Web studio and CLI share this capturer whenever `display` / `window` / `output_dir`
is set.

Full tool list: [MCP.md](MCP.md). Web job: [WEB.md](WEB.md).
