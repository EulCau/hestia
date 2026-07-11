#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

case "$(uname -m)" in
  x86_64|amd64) host_target="x86_64-unknown-linux-gnu" ;;
  aarch64|arm64) host_target="aarch64-unknown-linux-gnu" ;;
  *) host_target="${TARGET:-x86_64-unknown-linux-gnu}" ;;
esac

TARGET="$host_target" "$ROOT_DIR/scripts/package-linux.sh"

if [[ "${PACKAGE_CROSS:-0}" == "1" ]]; then
  for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
    if [[ "$target" == "$host_target" ]]; then
      continue
    fi
    if ! TARGET="$target" "$ROOT_DIR/scripts/package-linux.sh"; then
      echo "Cross package failed for $target. Install target WebKitGTK/linker dependencies and retry." >&2
    fi
  done
else
  cat <<'EOF'
Skipped non-host Linux architectures. To attempt cross packages:
  PACKAGE_CROSS=1 npm run package:all
EOF
fi

cat <<'EOF'
Windows installers must be built on a Windows machine with:
  pwsh scripts/package-windows.ps1 -Target x86_64-pc-windows-msvc
  pwsh scripts/package-windows.ps1 -Target aarch64-pc-windows-msvc
EOF
