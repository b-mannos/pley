# Release build: web UI + both binaries + licensing files, ready to ship.
#   powershell -ExecutionPolicy Bypass -File scripts\build.ps1 [-SkipWeb] [-Target <rust target triple>] [-Archive] [-Installer]
#     [-Ffmpeg <an FFmpeg build's bin folder>] [-Flavor <name>]
# Output: dist\pley-server-<version>-windows-<arch>\ (plus a .zip with -Archive, and
# the installer pley-server-<version>-windows-<arch>-setup.exe with -Installer)
param(
    [switch]$SkipWeb,
    [string]$Target,
    [switch]$Archive,
    [switch]$Installer,
    # For the installer: the FFmpeg it carries and a name for the build (see build-installer.ps1).
    [string]$Ffmpeg,
    [string]$Flavor
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$version = (Select-String -Path "$root/apps/server/Cargo.toml" -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value

if (-not $SkipWeb) {
    Push-Location $root
    try {
        if (-not (Test-Path node_modules)) { bun install --frozen-lockfile; if ($LASTEXITCODE) { throw "bun install failed" } }
        bun run --cwd apps/web build
        if ($LASTEXITCODE) { throw "web build failed" }
    } finally {
        Pop-Location
    }
}
# The UI is embedded at compile time; make sure cargo re-embeds the fresh build.
(Get-Item "$root/apps/server/src/web.rs").LastWriteTime = Get-Date

$cargoArgs = @("build", "--release", "--locked", "-p", "pley-server", "-p", "pley-transcoder")
if ($Target) {
    $cargoArgs += @("--target", $Target)
    $bin = "$root/target/$Target/release"
    $arch = $Target.Split("-")[0]
} else {
    $bin = "$root/target/release"
    $arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x86_64" }
}
if ($arch -eq "aarch64") { $arch = "arm64" }

Push-Location $root
try {
    cargo @cargoArgs
    if ($LASTEXITCODE) { throw "cargo build failed" }
} finally {
    Pop-Location
}

$name = "pley-server-$version-windows-$arch"
$out = "$root/dist/$name"
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item "$bin/pley.exe", "$bin/pley-transcoder.exe" $out
Copy-Item "$root/apps/server/LICENSE" "$out/LICENSE.txt"
Copy-Item "$root/LICENSING.md", "$root/README.md", "$root/pley.example.toml" $out


if ($Archive) {
    $zipPath = "$root/dist/$name.zip"
    if (Test-Path $zipPath) { Remove-Item $zipPath }
    # Forward slashes in entry names (Compress-Archive on PowerShell 5.1 writes backslashes).
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::Open($zipPath, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        $prefix = (Resolve-Path $out).Path.TrimEnd('\', '/').Length + 1
        foreach ($file in Get-ChildItem $out -Recurse -File) {
            $entry = "$name/" + $file.FullName.Substring($prefix).Replace('\', '/')
            [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $file.FullName, $entry) | Out-Null
        }
    } finally {
        $zip.Dispose()
    }
    Write-Host "wrote $zipPath"
}

if ($Installer) {
    $carrying = @{}
    if ($Ffmpeg) { $carrying.Ffmpeg = $Ffmpeg }
    if ($Flavor) { $carrying.Flavor = $Flavor }
    & "$PSScriptRoot/build-installer.ps1" -Release $out @carrying
}

Write-Host "release ready in $out" -ForegroundColor Green
Get-ChildItem $out | Select-Object Name, Length | Format-Table -AutoSize
