# Packaging

## Targets

Linux:

```bash
TARGET=x86_64-unknown-linux-gnu scripts/package-linux.sh
TARGET=aarch64-unknown-linux-gnu scripts/package-linux.sh
INCLUDE_APPIMAGE=1 scripts/package-linux.sh
```

The Linux script builds Tauri `deb` and `rpm` bundles and, when `makepkg` is available, an Arch Linux `.pkg.tar.zst`.

The Arch package step uses `makepkg -d` so the build host does not need to install every runtime dependency just to assemble the package. Package managers still resolve the declared dependencies when users install the produced package. The generated Arch package depends on `webkit2gtk-4.1`, `gtk3`, and `libayatana-appindicator`.

Linux packages install a desktop launcher in the standard application menu location. Tauri handles this for `deb` and `rpm` bundles. The Arch package installs `/usr/share/applications/hestia.desktop` and hicolor PNG icons under `/usr/share/icons/hicolor/*/apps/hestia.png`.

AppImage is optional because it depends on `linuxdeploy`/AppImage tooling that is often absent or broken on otherwise valid distro build hosts. Use `INCLUDE_APPIMAGE=1` to attempt it. Set `STRICT_APPIMAGE=1` if AppImage failure should fail the whole command.

`npm run package:all` builds the host Linux architecture by default. To attempt both x86_64 and aarch64 Linux packages from one machine:

```bash
PACKAGE_CROSS=1 npm run package:all
```

Cross-building aarch64 still requires target Rust std, linker, WebKitGTK development packages, and distro packaging tools for that target. Building on native architecture is recommended.

The scripts use the Tauri CLI from `frontend/node_modules/.bin/tauri` to avoid depending on a root-level npm install.

Windows:

```powershell
pwsh scripts/package-windows.ps1 -Target x86_64-pc-windows-msvc
pwsh scripts/package-windows.ps1 -Target aarch64-pc-windows-msvc
```

The Windows script builds NSIS and MSI installers.

## Wayland and X11

The same Linux package is used for Wayland and X11. If a user hits WebKitGTK/Wayland compositing issues, launch with:

```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 hestia
```

## User Data Paths

Development builds keep local mutable data in the repository for easier inspection. Release builds use system user data paths:

| System | User data root |
|---|---|
| Linux | `$XDG_DATA_HOME/hestia` or `~/.local/share/hestia` |
| Windows | `%APPDATA%\hestia` |
| macOS | `~/Library/Application Support/hestia` |

The `HESTIA_USER_DIR` environment variable overrides the user data root for testing packages.

Stored user data includes:

- `config/user.toml`
- `roles/{role_id}.json`
- `roles/{role_id}/avatar/`
- `memory/{role_id}/memories.json`

Bundled defaults are compiled into the application as fallbacks, so packaged builds do not need writable repository-local `config/`, `personality/`, or `usr/` directories.
