# Windows installer from a release folder (Inno Setup 6.7 or newer):
#   powershell -ExecutionPolicy Bypass -File scripts\build-installer.ps1 -Release dist\pley-<version>-windows-<arch>
#     [-OutDir dist] [-Ffmpeg <an FFmpeg build's bin folder> -Flavor <name>]
# Output: pley-server-<version>[-<flavor>]-windows-<arch>-setup.exe in -OutDir (default: dist).
# scripts\build.ps1 -Installer runs it. What the installer does is in
# deploy\windows\README.md.
param(
    [Parameter(Mandatory)][string]$Release,
    [string]$OutDir,
    # An FFmpeg build's bin folder for the installer to carry (offered first),
    # and a name for the build in the file name, such as fel.
    [string]$Ffmpeg,
    [string]$Flavor
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$Release = (Resolve-Path $Release).Path
$name = Split-Path $Release -Leaf
if ($name -notmatch '^pley-(.+)-windows-(x86_64|arm64)$') {
    throw "$name isn't a Windows release folder (pley-<version>-windows-<x86_64|arm64>)"
}
$version, $archName = $Matches[1], $Matches[2]
$arch = if ($archName -eq "arm64") { "arm64" } else { "x64" }
if (-not $OutDir) { $OutDir = Join-Path $root "dist" }
New-Item -ItemType Directory -Force $OutDir | Out-Null

$iscc = @(
    $env:ISCC,
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $iscc) { throw "Inno Setup 6.7 or newer isn't installed (winget install JRSoftware.InnoSetup), or set ISCC to its ISCC.exe" }

$defines = @("/DAppVersion=$version", "/DSourceDir=$Release", "/DArch=$arch", "/DOutputDir=$((Resolve-Path $OutDir).Path)")
if ($Ffmpeg) {
    $Ffmpeg = (Resolve-Path $Ffmpeg).Path
    foreach ($exe in "ffmpeg.exe", "ffprobe.exe") {
        if (-not (Test-Path (Join-Path $Ffmpeg $exe))) { throw "$Ffmpeg has no $exe" }
    }
    # "ffmpeg version n9.0.2-pley-dovi-el Copyright…" names it.
    $label = (& (Join-Path $Ffmpeg "ffmpeg.exe") -hide_banner -version | Select-Object -First 1) -replace '^ffmpeg version (\S+).*$', 'FFmpeg $1'
    $defines += "/DFfmpegDir=$Ffmpeg", "/DFfmpegLabel=$label"
}
if ($Flavor) { $defines += "/DFlavor=$Flavor" }
& $iscc /Q @defines (Join-Path $root "deploy\windows\pley.iss")
if ($LASTEXITCODE) { throw "the installer didn't build" }
$flavored = if ($Flavor) { "-$Flavor" } else { "" }
Write-Host "wrote $(Join-Path $OutDir "pley-server-$version$flavored-windows-$archName-setup.exe")" -ForegroundColor Green
