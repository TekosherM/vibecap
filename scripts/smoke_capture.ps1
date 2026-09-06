# Windows capture smoke — requires ffmpeg + a desktop session.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not $PSScriptRoot) { $Root = (Get-Location).Path }
if (Test-Path "$PSScriptRoot\..\Cargo.toml") {
    $Root = Resolve-Path "$PSScriptRoot\.."
}
Set-Location $Root
$Bin = if ($env:VIBECAP_BIN) { $env:VIBECAP_BIN } else { Join-Path $Root "target\release\vibecap.exe" }
if (-not (Test-Path $Bin)) {
    Write-Host "Building release binary..."
    cargo build --release --offline
}
$Tmp = Join-Path $env:TEMP ("vibecap_smoke_" + [guid]::NewGuid().ToString("n"))
New-Item -ItemType Directory -Force -Path $Tmp | Out-Null
try {
    & $Bin --version
    & $Bin doctor
    $out = & $Bin --screenshot --output-dir $Tmp
    if (-not $out) { throw "screenshot printed no path" }
    $path = $out.Trim()
    if (-not (Test-Path $path)) { throw "screenshot missing: $path" }
    $len = (Get-Item $path).Length
    if ($len -lt 8000) { throw "screenshot too small: $len bytes" }
    Write-Host "OK $path ($len bytes)"
    exit 0
} catch {
    Write-Host "FAIL $_"
    exit 1
} finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}
