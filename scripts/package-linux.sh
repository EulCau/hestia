#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
BUNDLES="${BUNDLES:-deb,rpm,appimage}"
OUT_DIR="${OUT_DIR:-$ROOT_DIR/dist/packages/linux/$TARGET}"

usage() {
  cat <<'EOF'
Usage:
  TARGET=x86_64-unknown-linux-gnu scripts/package-linux.sh
  TARGET=aarch64-unknown-linux-gnu scripts/package-linux.sh
  BUNDLES=deb,rpm,appimage scripts/package-linux.sh

Environment:
  TARGET   Rust target triple. Supported examples:
           x86_64-unknown-linux-gnu
           aarch64-unknown-linux-gnu
  BUNDLES  Tauri bundle list for Linux. Default: deb,rpm,appimage
  OUT_DIR  Copy final artifacts here.

Notes:
  - Build on the matching architecture when possible.
  - Cross-building aarch64 requires Rust target, linker, WebKitGTK, and system
    package tooling for that target.
  - Wayland and X11 use the same package. For WebKitGTK/Wayland rendering
    issues, launch with WEBKIT_DISABLE_COMPOSITING_MODE=1.
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

cd "$ROOT_DIR"
mkdir -p "$OUT_DIR"

rustup target add "$TARGET"
npm install
npm run build
npm run tauri -- build --target "$TARGET" --bundles "$BUNDLES"

find "$ROOT_DIR/src-tauri/target/$TARGET/release/bundle" -type f \
  \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) \
  -exec cp -v {} "$OUT_DIR/" \;

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
  (cd "$ARCH_DIR" && makepkg -f)
  find "$ARCH_DIR" -maxdepth 1 -type f -name '*.pkg.tar.*' -exec cp -v {} "$OUT_DIR/" \;
else
  echo "makepkg not found; skipped Arch Linux .pkg.tar.zst build." >&2
fi

echo "Artifacts copied to $OUT_DIR"
