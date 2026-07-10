param(
  [string]$Target = "x86_64-pc-windows-msvc",
  [string]$Bundles = "nsis,msi",
  [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
if ($OutDir -eq "") {
  $OutDir = Join-Path $Root "dist/packages/windows/$Target"
}

Set-Location $Root
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

rustup target add $Target
npm install
npm run build
npm run tauri -- build --target $Target --bundles $Bundles

$BundleRoot = Join-Path $Root "src-tauri/target/$Target/release/bundle"
Get-ChildItem -Path $BundleRoot -Recurse -File |
  Where-Object { $_.Extension -in ".msi", ".exe" } |
  ForEach-Object { Copy-Item $_.FullName -Destination $OutDir -Force }

Write-Host "Artifacts copied to $OutDir"
