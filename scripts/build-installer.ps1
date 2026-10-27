# Windows installer from a release folder (Inno Setup 6.7 or newer):
#   powershell -ExecutionPolicy Bypass -File scripts\build-installer.ps1 -Release dist\pley-<version>-windows-<arch> [-OutDir dist]
# Output: pley-server-<version>-windows-<arch>-setup.exe in -OutDir (default: dist).
# scripts\build.ps1 -Installer runs it. What the installer does is in
# deploy\windows\README.md.
param(
    [Parameter(Mandatory)][string]$Release,
    [string]$OutDir
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$Release = (Resolve-Path $Release).Path
$name = Split-Path $Release -Leaf
if ($name -notmatch '^pley-(.+)-windows-(x86_64|arm64)$') {
    throw "$name isn't a Windows release folder (pley-<version>-windows-<x86_64|arm64>)"
}
$version = $Matches[1]
$arch = if ($Matches[2] -eq "arm64") { "arm64" } else { "x64" }
if (-not $OutDir) { $OutDir = Join-Path $root "dist" }
New-Item -ItemType Directory -Force $OutDir | Out-Null

$iscc = @(
    $env:ISCC,
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $iscc) { throw "Inno Setup 6.7 or newer isn't installed (winget install JRSoftware.InnoSetup), or set ISCC to its ISCC.exe" }

& $iscc /Q "/DAppVersion=$version" "/DSourceDir=$Release" "/DArch=$arch" "/DOutputDir=$((Resolve-Path $OutDir).Path)" (Join-Path $root "deploy\windows\pley.iss")
if ($LASTEXITCODE) { throw "the installer didn't build" }
Write-Host "wrote $(Join-Path $OutDir ($name -replace '^pley-', 'pley-server-'))-setup.exe" -ForegroundColor Green
