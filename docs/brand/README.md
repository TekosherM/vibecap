# Vibecap brand kit

## Assets

| File | Use |
| :--- | :--- |
| `logo-horizontal-dark.jpg` | Marketing, README, website header |
| `app-icon-1024.jpg` / `.png` | App Store / source master |
| `AppIcon.icns` | macOS `Vibecap.app` bundle |
| `mark-mono-pair.jpg` | Favicon, docs, print, light/dark UI |

## Design language

- **Canvas:** graphite dark (`#121316` family)
- **Live accent:** amber wedge (`#f59e4b`) = recording / agent waiting only
- **Mark:** aperture shutter (capture) with one live segment
- **Wordmark:** clean geometric sans; optional “Safelight Studio” subtitle

## Rebuild app icon

```bash
# Master → 1024 square + full iconset + AppIcon.icns + assets/app_icon.png (Dock/eframe)
MASTER=docs/brand/app-icon-master.png
sips -z 1024 1024 "$MASTER" --out docs/brand/app-icon-1024.png
# sizes: 16,32@1x/2x,128,256@1x/2x,512@1x/2x,1024@2x
# (see scripts/install_macos_app.sh / prior regen steps)
iconutil -c icns docs/brand/AppIcon.iconset -o docs/brand/AppIcon.icns
sips -z 256 256 docs/brand/app-icon-1024.png --out assets/app_icon.png
./scripts/install_macos_app.sh
# If Dock caches the old mark: killall Dock
```

**Note:** The running app Dock icon comes from eframe `ViewportBuilder::with_icon`  
(`assets/app_icon.png` embedded at compile time). Finder / Applications use `AppIcon.icns`.
