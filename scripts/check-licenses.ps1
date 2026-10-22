# Verifies the licensing boundary (see LICENSING.md):
#  * the server (pley-server) must not depend on the transcoder
#    crate or on any crate that is only available under a (L/A)GPL license;
#  * everything the transcoder uses is permissive.
# Walks the resolved dependency graph from `cargo metadata` (dev-dependencies excluded).
# Exit code 1 on violations.

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    $meta = cargo metadata --format-version 1 --quiet | ConvertFrom-Json
} finally {
    Pop-Location
}

$packages = @{}
foreach ($p in $meta.packages) { $packages[$p.id] = $p }
$nodes = @{}
foreach ($n in $meta.resolve.nodes) { $nodes[$n.id] = $n }

function Get-Closure([string]$name) {
    $start = ($meta.packages | Where-Object { $_.name -eq $name } | Select-Object -First 1).id
    $seen = @{}
    $queue = [System.Collections.Generic.Queue[string]]::new()
    $queue.Enqueue($start)
    while ($queue.Count -gt 0) {
        $id = $queue.Dequeue()
        if ($seen.ContainsKey($id)) { continue }
        $seen[$id] = $true
        foreach ($dep in $nodes[$id].deps) {
            # dep_kinds: kind = null (normal), "build" or "dev"
            $runtime = @($dep.dep_kinds | Where-Object { $_.kind -ne "dev" }).Count -gt 0
            if ($runtime) { $queue.Enqueue($dep.pkg) }
        }
    }
    return $seen.Keys | ForEach-Object { $packages[$_] }
}

# A license expression is "copyleft-only" when every alternative is GPL-family.
function Test-CopyleftOnly([string]$expr) {
    if (-not $expr) { return $false }
    $alternatives = $expr -split '\s+OR\s+|/'
    foreach ($alt in $alternatives) {
        if ($alt -notmatch 'GPL|SSPL|EUPL|OSL') { return $false }
    }
    return $true
}

$failed = $false

$server = Get-Closure "pley-server"
Write-Host "pley-server: $($server.Count) crates in the runtime dependency graph"
foreach ($p in $server) {
    if ($p.name -eq "pley-transcoder") {
        Write-Host "  ERROR: pley-server depends on pley-transcoder" -ForegroundColor Red
        $failed = $true
    }
    if (Test-CopyleftOnly $p.license) {
        Write-Host "  ERROR: $($p.name) $($p.version) is $($p.license)" -ForegroundColor Red
        $failed = $true
    }
    if (-not $p.license -and -not $p.license_file -and $p.source) {
        Write-Host "  WARN: $($p.name) $($p.version) declares no license" -ForegroundColor Yellow
    }
}

$transcoder = Get-Closure "pley-transcoder"
Write-Host "pley-transcoder: $($transcoder.Count) crates in the runtime dependency graph"
foreach ($p in $transcoder) {
    if ($p.license -match 'SSPL|EUPL|GPL-2\.0-only|GPL-2\.0$') {
        Write-Host "  ERROR: $($p.name) $($p.version) ($($p.license)) can't be used in the transcoder" -ForegroundColor Red
        $failed = $true
    }
}

$licenses = $server | Group-Object { if ($_.license) { $_.license } else { "(file) $($_.license_file)" } } | Sort-Object Count -Descending
Write-Host "Licenses used by pley-server:"
foreach ($l in $licenses) { Write-Host ("  {0,4}  {1}" -f $l.Count, $l.Name) }

if ($failed) {
    Write-Host "License check FAILED" -ForegroundColor Red
    exit 1
}
Write-Host "License check passed" -ForegroundColor Green
