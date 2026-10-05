# Linux delivery

Linux has two entry points: a desktop app for account management and a terminal dashboard for interactive or scripted use. The supported package baseline is x86-64 Ubuntu 22.04, with GTK3 and WebKitGTK 4.1. Both entry points share the same account store.

## Desktop installation

These names apply to the unreleased 4.9.0 source. Public 4.8.1 desktop packages keep their original names and executable paths.

```sh
sudo apt install ./agy-switch-<version>-linux-amd64.deb
agy-switch-desktop
```

The deb installs the `agy-switch` package, `/usr/bin/agy-switch-desktop` and its application-menu entry/icon, and the `/usr/bin/agy-switch` CLI. A desktop display and D-Bus session are needed for the GUI. Modern Antigravity APP credential switching needs an unlocked Secret Service such as GNOME Keyring. A compatible KWallet service has not been separately validated.

The main app exposes the same account, quota, usage, switching strategy, update and appearance settings as Mac. A supported tray offers Quick Dashboard. If tray creation fails, the main window remains available and closing it exits. If the desktop creates an invisible tray icon, disable the tray as described below.

## Terminal-only installation

Extract `agy-switch-<version>-linux-amd64.tar.gz` and install its executable into a user directory:

```sh
mkdir -p ~/.local/bin
install -m 755 ./agy-switch ~/.local/bin/agy-switch
~/.local/bin/agy-switch
~/.local/bin/agy-switch accounts list --json
```

Add `~/.local/bin` to PATH if your shell does not already include it. No root access is needed for this executable installation. The desktop deb already provides the command globally.

The CLI does not initialize GTK, open a window or need DISPLAY/Wayland for cached reads and terminal interaction. It currently links the desktop runtime libraries, however. On the Ubuntu 22.04 baseline, install them through APT when absent:

```sh
sudo apt install libgtk-3-0 libwebkit2gtk-4.1-0 libayatana-appindicator3-1
```

This is a glibc binary, not a static/musl binary; package names and ABI compatibility differ on other distributions. Headless reads do not require a keyring. Authorization/refresh needs network access; browser authorization additionally needs a browser/callback route. Masked refresh-token entry is available in the terminal dashboard.

Default `switch --target app` requires a discoverable Antigravity APP and its credential backend. It can close/relaunch that application, so it is not advertised as a server-only agy switch. A file-only session write cannot establish the active identity when clients share a credential store; the CLI intentionally rejects that fallback. The GUI retains its existing initialized-agy fallback when no APP is installed. The smart-switch background scheduler runs in the desktop app, not in a terminal command.

## Accounts and paths

Tools data defaults to `~/.antigravity_tools`; `ABV_DATA_DIR` overrides it. Native agy initialization is discovered under `~/.gemini/antigravity-cli`. Generic Gemini CLI OAuth files are not changed. Absolute `XDG_CONFIG_HOME` is respected by native application discovery. Account import and native client locations are separate from the Tools data override.

Quotas are cached observations. Missing windows, stale records and disabled accounts are not fabricated as 100% available. Cost estimates use known model prices; they do not measure requests that Antigravity failed to record locally.

## Desktop compatibility

Windows are opaque. GNOME/KDE tray presentation depends on the desktop’s AppIndicator/StatusNotifier support; merely having a library installed does not prove a visible tray. Wayland restricts absolute placement and activation, so tray positioning needs validation on each compositor. If the desktop does not display its tray icon, use `ANTIGRAVITY_DISABLE_TRAY=1` so closing the main window exits instead of hiding it.

![Linux quick dashboard](screenshots/4.8.0/linux-quick-dashboard-light.png)

Actual Linux WebKitGTK viewport, CI debug build with synthetic quota observations. This is a compact-route capture, not a Wayland/tray-placement test. [Source and hashes](screenshots/4.8.0/README.md)

Existing overrides remain available:

- `ANTIGRAVITY_DISABLE_TRAY=1`: use the main window without a tray.
- `ANTIGRAVITY_FORCE_TRAY=1`: explicitly request tray creation on Wayland.
- `ANTIGRAVITY_FORCE_X11=1` / `ANTIGRAVITY_FORCE_WAYLAND=1`: select the intended backend where available.
- `WEBKIT_DISABLE_DMABUF_RENDERER=1`: opt into the WebKit rendering workaround where required.

The existing graphics policy avoids known GNOME/KDE Wayland issues when X11 is available and keeps native Wayland on wlroots-family desktops. These switches are troubleshooting controls, not a claim that every GPU/compositor has been tested.

## Build

```sh
sudo apt install build-essential curl pkg-config libwebkit2gtk-4.1-dev \
  libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf \
  libssl-dev git cmake clang libclang-dev xdg-utils
./scripts/build-linux-deb.sh --native
```

Node.js 22+ and Rust stable are also required. To keep the Ubuntu 22.04 baseline on a newer Linux host:

```sh
./scripts/build-linux-deb.sh --docker
```

The Docker helper uses the caller’s UID/GID and workspace/cache mounts; it does not publish a release or install the resulting package. Do not distribute a package built on a newer glibc as if it had the older baseline. Other architectures/distributions require their own native builds and acceptance.

## Verification

CI runs headless reads, real PTY interaction, deb payload checks, GTK/WebKitGTK native window tests under Xvfb, and isolated Secret Service tests. These use synthetic accounts. Run the keyring fixture only through its isolated helper:

```sh
./scripts/test-linux-credentials.sh
```

It creates a disposable HOME and independent D-Bus session; never run ignored credential tests directly on your normal desktop bus. Real login, authenticated switching, Wayland/KDE tray behavior and ARM remain separate acceptance tasks. [Native acceptance](native-gui-acceptance.md) · [Historical Linux checks](linux-validation.md)
