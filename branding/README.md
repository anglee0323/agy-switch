# agy-switch identity

The main application navigation displays **AntiGravity Switch** in both languages. The repository, command and package identifier remain **agy-switch**, lowercase with the hyphen. Antigravity remains the name of Google's client; it is not renamed by this project.

The desktop artwork uses the official Antigravity arch as a visual reference: one full-color arch faces up and an identical copy is rotated 180° and shifted slightly down, leaving a small transparent gap at the center. The pair sits on the existing white rounded tile. This is an original project mark inspired by the reference style, not official Antigravity branding or a claim about its physical meaning.

The menu bar uses the same stacked arch silhouette in one color with the center gap preserved. Its 22×22 vector viewport renders to a 44px macOS template image. `currentColor` supplies the source ink and the existing native template mechanism adapts it to the system appearance. It has no colored background tile.

| Asset | Use |
| --- | --- |
| [app-icon.png](app-icon.png) | 512×512 generated app artwork with the stacked full-color arch mark |
| [app-icon.svg](app-icon.svg) | Self-contained raster wrapper for the generated artwork, 1024×1024 output |
| [mark.svg](mark.svg) | Compact full-color stacked arch mark |
| [tray.svg](tray.svg) | Monochrome stacked arch template; render at 44px for the macOS menu bar |
| [wordmark.svg](wordmark.svg) | Generated app artwork and full display name for a light background |
| [preview.html](preview.html) | App, wordmark and light/dark menu bar preview |

The selected artwork transitions from blue through green and amber to coral at the wave peaks. Use normal foreground text for values; brand colors do not replace semantic quota colors.

Run `npm ci` then `npm run brand:generate` to regenerate the existing app PNG/ICNS/ICO, frontend logos and tray image with the pinned Tauri CLI. This renders image assets only; it does not build, package, sign, install or launch the application. No extra graphics dependency is required.

The 4.9.0 source updates the app, menu bar, CLI, window titles and future package names. User data remains in `~/.antigravity_tools`, and the stable bundle identifier remains unchanged. The internal Rust library name is retained. Published releases and their hashes are immutable; `scripts/release-brand.mjs` distinguishes their historical filenames from future artifacts, without introducing old-name runtime aliases.

The app displays **AntiGravity Switch** in its navigation, menu header, Dock name and tray tooltip. The release ZIP from 4.9.1 contains `AntiGravity Switch.app`; executable and package filenames remain `agy-switch`. Homebrew pins the verified public archive. See [installation](../docs/homebrew.md) and [maintainer procedures](../docs/maintainers/README.md).
