# Packaging

## Targets

Linux:

```bash
TARGET=x86_64-unknown-linux-gnu scripts/package-linux.sh
TARGET=aarch64-unknown-linux-gnu scripts/package-linux.sh
```

The Linux script builds Tauri `deb`, `rpm`, and `AppImage` bundles and, when `makepkg` is available, an Arch Linux `.pkg.tar.zst`.

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
