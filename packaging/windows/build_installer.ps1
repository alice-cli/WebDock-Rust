# Build portable zip + Inno Setup installer for WebRust.
# Run on Windows after: cargo build -p webdock-server --release
param(
  [string]$Version = "0.1.0",
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
)

$ErrorActionPreference = "Stop"
$dist = Join-Path $RepoRoot "dist"
$stage = Join-Path $dist "webrust"
$exe = Join-Path $RepoRoot "target\release\WebRust.exe"

if (-not (Test-Path $exe)) {
  throw "Missing $exe — run cargo build -p webdock-server --release first"
}

New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item $exe (Join-Path $stage "WebRust.exe") -Force
if (Test-Path (Join-Path $RepoRoot "webui")) {
  Copy-Item (Join-Path $RepoRoot "webui") (Join-Path $stage "webui") -Recurse -Force
}
foreach ($f in @("README.md", "LICENSE")) {
  $p = Join-Path $RepoRoot $f
  if (Test-Path $p) { Copy-Item $p $stage -Force }
}

$zip = Join-Path $dist "WebRust-windows-$Version.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path $stage -DestinationPath $zip -Force
Write-Host "Portable: $zip"

$iscc = @(
  "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
  "${env:LocalAppData}\Programs\Inno Setup 6\ISCC.exe",
  "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $iscc) {
  Write-Warning "Inno Setup 6 not found — skipping .exe installer (zip only)"
  exit 0
}

$iss = Join-Path $RepoRoot "packaging\windows\webrust.iss"
& $iscc $iss "/DMyAppVersion=$Version" "/DMyAppSource=$stage"
if ($LASTEXITCODE -ne 0) { throw "ISCC failed: $LASTEXITCODE" }
Write-Host "Installer: dist\WebRust-Setup-$Version.exe"
