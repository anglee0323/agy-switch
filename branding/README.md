# agy-switch identity

The main application navigation displays **AntiGravity Switch** in both languages. The repository, command and package identifier remain **agy-switch**, lowercase with the hyphen. Antigravity remains the name of Google's client; it is not renamed by this project.

The owner selected the desktop artwork `AntiGravity_Switch_Icon_Concept2.png`: two overlapping spectral wave peaks on a white rounded tile. The supplied 512×512 PNG is retained unchanged as the authoritative app artwork; larger sizes are raster resamples, not newly drawn detail. The two waves represent active and standby accounts and the handoff between them. This is the project's design metaphor, not a claim about the official Antigravity logo's physical meaning.

The menu bar uses an original solid double-wave silhouette with a small transparent clearance at the overlap. Its 22×22 vector viewport renders to a 44px macOS template image. `currentColor` supplies the source ink and the existing native template mechanism adapts it to the system appearance. It has no colored background tile or faint secondary wave.

| Asset | Use |
| --- | --- |
| [app-icon.png](app-icon.png) | Owner-selected 512×512 raster master, copied unchanged from the Desktop |
| [app-icon.svg](app-icon.svg) | Self-contained raster wrapper for the existing renderer, 1024×1024 output |
| [mark.svg](mark.svg) | Compact spectral double-wave mark |
| [tray.svg](tray.svg) | Monochrome template; render at 44px for the macOS menu bar |
| [wordmark.svg](wordmark.svg) | Selected app artwork and full display name for a light background |
| [preview.html](preview.html) | App, wordmark and light/dark menu bar preview |

The selected artwork transitions from blue through green and amber to coral at the wave peaks. Use normal foreground text for values; brand colors do not replace semantic quota colors.

Run `npm ci` then `npm run brand:generate` to regenerate the existing app PNG/ICNS/ICO, frontend logos and tray image with the pinned Tauri CLI. This renders image assets only; it does not build, package, sign, install or launch the application. No extra graphics dependency is required.

The 4.9.0 source updates the app, menu bar, CLI, window titles and future package names. User data remains in `~/.antigravity_tools`, and the stable bundle identifier remains unchanged. The internal Rust library name is retained. Published releases and their hashes are immutable; `scripts/release-brand.mjs` distinguishes their historical filenames from future artifacts, without introducing old-name runtime aliases.

The development cask is named `agy-switch`, but still pins the public 4.8.1 archive until a new release is authorized. That archive contains the original app name and artwork. Do not claim that source previews are installed-app or Windows/Linux acceptance evidence.
