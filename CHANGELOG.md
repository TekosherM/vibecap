# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-08-05

First public open-source release.

### Added
- Native desktop app (eframe/egui): capture, library, annotation studio, video editor, feedback inbox, settings
- Annotation tools: pen, arrow, rectangle, highlight, text, blur, step badges, clipboard copy
- Voice notes and text notes beside screenshots
- Timeline GIF export and wardrobe video/image tools (via ffmpeg)
- MCP stdio server with 10 tools (capture, record, GIF, live inspection, budget, feedback)
- Agent budget controls (frames / MB / minutes + eco/standard/intensive tiers)
- Human feedback loop shared between app and MCP (`~/.config/vibecap`)
- Cross-platform capture layer (`src/platform`): macOS screencapture, Windows gdigrab, Linux x11grab/grim
- Portable paths via `dirs` (Videos/Vibecap media, config dir)
- CLI: `--mcp`, `--screenshot`, `--help`, `--version`
- Docs: USAGE, MCP, ARCHITECTURE, PLATFORMS, TESTING, CONTRIBUTING
- CI matrix (macOS smoke + Windows/Linux build)
- Dual license: MIT OR Apache-2.0

### Notes
- macOS remains the primary day-to-day capture quality target
- Windows system audio and Wayland motion capture are best-effort
- Requires ffmpeg for GIF/editor; Screen Recording permission on macOS

[0.1.0]: https://github.com/TekosherM/vibecap/releases/tag/v0.1.0
