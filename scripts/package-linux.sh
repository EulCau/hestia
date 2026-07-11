#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
BUNDLES="${BUNDLES:-deb,rpm}"
INCLUDE_APPIMAGE="${INCLUDE_APPIMAGE:-0}"
STRICT_APPIMAGE="${STRICT_APPIMAGE:-0}"
OUT_DIR="${OUT_DIR:-$ROOT_DIR/dist/packages/linux/$TARGET}"
TAURI_BIN="${TAURI_BIN:-$ROOT_DIR/frontend/node_modules/.bin/tauri}"

usage() {
  cat <<'EOF'
Usage:
  TARGET=x86_64-unknown-linux-gnu scripts/package-linux.sh
  TARGET=aarch64-unknown-linux-gnu scripts/package-linux.sh
  BUNDLES=deb,rpm scripts/package-linux.sh
  INCLUDE_APPIMAGE=1 scripts/package-linux.sh

Environment:
  TARGET   Rust target triple. Supported examples:
           x86_64-unknown-linux-gnu
           aarch64-unknown-linux-gnu
  BUNDLES  Tauri bundle list for Linux. Default: deb,rpm
  OUT_DIR  Copy final artifacts here.
  INCLUDE_APPIMAGE
           Set to 1 to attempt AppImage after native packages.
  STRICT_APPIMAGE
           Set to 1 to fail if AppImage bundling fails.

Notes:
  - Build on the matching architecture when possible.
  - Cross-building aarch64 requires Rust target, linker, WebKitGTK, and system
    package tooling for that target.
  - Wayland and X11 use the same package. For WebKitGTK/Wayland rendering
    issues, launch with WEBKIT_DISABLE_COMPOSITING_MODE=1.
  - AppImage bundling depends on linuxdeploy/AppImage tooling and can fail on
    otherwise valid distro package builds. It is optional by default.
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

cd "$ROOT_DIR"
mkdir -p "$OUT_DIR"

rustup target add "$TARGET"
if [[ ! -x "$TAURI_BIN" ]]; then
  npm --prefix frontend install
fi
npm run build
"$TAURI_BIN" build --target "$TARGET" --bundles "$BUNDLES"

find "$ROOT_DIR/src-tauri/target/$TARGET/release/bundle" -type f \
  \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) \
  -exec cp -v {} "$OUT_DIR/" \;

if [[ "$INCLUDE_APPIMAGE" == "1" || ",$BUNDLES," == *",appimage,"* ]]; then
  if "$TAURI_BIN" build --target "$TARGET" --bundles appimage; then
    find "$ROOT_DIR/src-tauri/target/$TARGET/release/bundle" -type f \
      -name '*.AppImage' \
      -exec cp -v {} "$OUT_DIR/" \;
  else
    echo "AppImage bundling failed. Native Linux packages were still built." >&2
    echo "Install linuxdeploy/AppImage tooling or retry with STRICT_APPIMAGE=1 to require it." >&2
    if [[ "$STRICT_APPIMAGE" == "1" ]]; then
      exit 1
    fi
  fi
fi

if command -v makepkg >/dev/null 2>&1; then
  ARCH_DIR="$ROOT_DIR/dist/pkgbuild/hestia-$TARGET"
  mkdir -p "$ARCH_DIR"
  VERSION="$(node -p "require('./package.json').version")"
  INSTALL_BIN="$ROOT_DIR/src-tauri/target/$TARGET/release/hestia"
  cat > "$ARCH_DIR/PKGBUILD" <<EOF
pkgname=hestia
pkgver=$VERSION
pkgrel=1
pkgdesc='Personal AI Companion Runtime Platform'
arch=('x86_64' 'aarch64')
url='https://example.invalid/hestia'
license=('MIT' 'Apache')
depends=('webkit2gtk-4.1' 'gtk3' 'ayatana-appindicator')
optdepends=('llama.cpp: local llama-server backend' 'ollama: local model backend')
source=('hestia')
sha256sums=('SKIP')

package() {
  install -Dm755 "\$srcdir/hestia" "\$pkgdir/usr/bin/hestia"
}
EOF
  cp "$INSTALL_BIN" "$ARCH_DIR/hestia"
  if (cd "$ARCH_DIR" && makepkg -fd); then
    find "$ARCH_DIR" -maxdepth 1 -type f -name '*.pkg.tar.*' -exec cp -v {} "$OUT_DIR/" \;
  else
    echo "Arch package build failed. deb/rpm artifacts remain available in $OUT_DIR." >&2
  fi
else
  echo "makepkg not found; skipped Arch Linux .pkg.tar.zst build." >&2
fi

echo "Artifacts copied to $OUT_DIR"
