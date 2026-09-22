# API Surface — stable vs internal (E300)

What agent authors and integrators can rely on, and what can change
without notice. Semver rules apply only to **Stable** surfaces.

## Stable (safe to build on)

### CLI

| Verb / flag | Notes |
| :--- | :--- |
| `vibecap --screenshot` | `-o/--output-dir`, `-d/--display`, `--window/--app`, `--json`, `--dry-run` semantics frozen |
| `vibecap record start|stop|status` | `--gif` follow-up (`gif_pending=`), `--watch` NDJSON, `--dry-run` argv print |
| `vibecap doctor` | `--json` report shape is append-only; `--fix` stays conservative |
| `vibecap list` / `open` / `annotate` / `--paths` | Output fields append-only |
| `vibecap --mcp` | MCP server entry point |

- `--json` success payloads are **append-only**: new keys may appear;
  existing keys keep their type and meaning.
- Errors on stderr: `{"code":"E_*","error":…}` — codes are stable,
  messages may change.
- Exit codes: 0 success, non-zero failure.

### MCP tools

All tools listed in `docs/MCP.md` are stable by name and parameter.
Return payloads are append-only JSON.

### Environment variables

`VIBECAP_OUTPUT_DIR`, `VIBECAP_FFMPEG`, `VIBECAP_DISPLAY`,
`VIBECAP_AUDIO_DEVICE` — honored permanently.

### On-disk locations

- `vibecap --paths` output (media dir, ffmpeg, backend).
- `<config>/session.json` — user settings; `schema_version` migrates.
- `<media>/.vibecap/` — thumbs + scratch; safe to delete, regenerates.
- `<media>/*.ffmpeg.log`, `*.markers.txt`, `*.clean.mp4` — sidecars.

## Internal (can change between releases)

- CLI flag spellings not listed above; `--help` text layout.
- `session.json` field set (migrations preserve behavior, not shape).
- Retro-buffer on-disk format (`<config>/retro_buffer/`).
- Tray menu item wording/order; palette action names.
- Rust module layout, function names, `ThemeMode` internals.
- `gui.lock` / `pending_still.path` / `pending_cmd` marker formats.

## Versioning

The binary reports `vibecap --version`. CLI/MCP breaking changes require
a minor bump minimum and a `docs/` note; the JSON append-only rule means
most additions are non-breaking.
