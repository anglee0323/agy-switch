# agy-switch identity

Use the name **agy-switch**, lowercase and with the hyphen, in both languages. Antigravity remains the name of Google's client; it is not renamed by this project.

The two rounded arrows represent switching between accounts. Blue and mint keep the mark distinct at small sizes and sit comfortably beside the dashboard's green/yellow/red quota bars. The app icon uses a porcelain surface and a subtle edge instead of a dark purple tile. The menu bar uses the same shape as a monochrome macOS template image, adapting to system light/dark appearance.

| Asset | Use |
| --- | --- |
| [app-icon.svg](app-icon.svg) | Editable 1024×1024 app icon master, transparent outside the rounded surface |
| [mark.svg](mark.svg) | Compact two-color mark |
| [tray.svg](tray.svg) | Monochrome template; render at 44px for the macOS menu bar |
| [wordmark.svg](wordmark.svg) | Mark and lowercase name for a light background |
| [preview.html](preview.html) | App, wordmark and light/dark menu bar preview |

Brand colors: blue `#3478F6`, mint `#1AA58B`, ink `#182838`, porcelain `#F6FAFD`. Use normal foreground text for values; brand colors do not replace semantic quota colors.

Run `npm ci` then `npm run brand:generate` to regenerate the existing app PNG/ICNS/ICO, frontend logos and tray image with the pinned Tauri CLI. This renders image assets only; it does not build, package, sign, install or launch the application. No extra graphics dependency is required.

The 4.9.0 source updates the app, menu bar, CLI, window titles and future package names. User data remains in `~/.antigravity_tools`, and the stable bundle identifier remains unchanged. The internal Rust library name is retained. Published releases and their hashes are immutable; `scripts/release-brand.mjs` distinguishes their historical filenames from future artifacts, without introducing old-name runtime aliases.

The development cask is named `agy-switch`, but still pins the public 4.8.1 archive until a new release is authorized. That archive contains the original app name and artwork. Do not claim that source previews are installed-app or Windows/Linux acceptance evidence.
