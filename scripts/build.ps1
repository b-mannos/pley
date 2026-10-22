# Release build: web UI + both binaries + licensing files, ready to ship.
#   powershell -ExecutionPolicy Bypass -File scripts\build.ps1 [-SkipWeb]
# Output: dist\pley-<version>-windows-x86_64\
param([switch]$SkipWeb)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$version = (Select-String -Path "$root/crates/pley-server/Cargo.toml" -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value

if (-not $SkipWeb) {
    Push-Location "$root/web"
    try {
        if (-not (Test-Path node_modules)) { npm ci --no-fund --no-audit; if ($LASTEXITCODE) { throw "npm ci failed" } }
        npm run build
        if ($LASTEXITCODE) { throw "web build failed" }
    } finally {
        Pop-Location
    }
}
# The UI is embedded at compile time; make sure cargo re-embeds the fresh build.
(Get-Item "$root/crates/pley-server/src/web.rs").LastWriteTime = Get-Date

Push-Location $root
try {
    cargo build --release -p pley-server -p pley-transcoder
    if ($LASTEXITCODE) { throw "cargo build failed" }
} finally {
    Pop-Location
}

$out = "$root/dist/pley-$version-windows-x86_64"
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item "$root/target/release/pley.exe", "$root/target/release/pley-transcoder.exe" $out
Copy-Item "$root/crates/pley-server/LICENSE" "$out/LICENSE.txt"
Copy-Item "$root/LICENSING.md", "$root/README.md", "$root/pley.example.toml" $out


Write-Host "release ready in $out" -ForegroundColor Green
Get-ChildItem $out | Select-Object Name, Length | Format-Table -AutoSize
