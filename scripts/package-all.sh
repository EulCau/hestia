#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
  TARGET="$target" "$ROOT_DIR/scripts/package-linux.sh"
done

cat <<'EOF'
Windows installers must be built on a Windows machine with:
  pwsh scripts/package-windows.ps1 -Target x86_64-pc-windows-msvc
  pwsh scripts/package-windows.ps1 -Target aarch64-pc-windows-msvc
EOF
