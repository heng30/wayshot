# Download the 3 modified slint crates (1.17.1) into vendor/ and apply patches. (Windows PowerShell)
#
# Usage: .\patches\vendor.ps1 [wayshot]
#   (no args) -> vendor/ next to this patch repo (patches/slint-dynamic-z/vendor)
#   wayshot   -> vendor/ in the wayshot project root (three levels up)
# Requirements: Git for Windows (provides patch.exe and tar)
$ErrorActionPreference = 'Stop'

$CUR_DIR = $PSScriptRoot
$ROOT_DIR = Join-Path $CUR_DIR '..'
if ($args.Count -eq 1 -and $args[0] -eq 'wayshot') {
    $ROOT_DIR = Join-Path $CUR_DIR '..\..\..'
}

$VENDOR_DIR = Join-Path $ROOT_DIR 'vendor'
$VERSION = '1.17.1'
$CRATES = @('i-slint-core', 'i-slint-compiler', 'i-slint-backend-qt')

Write-Host "CUR_DIR: $CUR_DIR"
Write-Host "ROOT_DIR: $ROOT_DIR"
Write-Host "VENDOR_DIR: $VENDOR_DIR"

New-Item -ItemType Directory -Force -Path $VENDOR_DIR | Out-Null
foreach ($c in $CRATES) {
    # Prefer the local cargo cache, otherwise download from crates.io
    $localCrate = Get-ChildItem (Join-Path $env:USERPROFILE '.cargo\registry\cache') -Filter "$c-$VERSION.crate" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($localCrate) {
        $src = $localCrate.FullName
        Write-Host "==> $c : using local cache $src"
    } else {
        Write-Host "==> $c : downloading from crates.io"
        $src = Join-Path $env:TEMP "$c-$VERSION.crate"
        Invoke-WebRequest -Uri "https://static.crates.io/crates/$c/$c-$VERSION.crate" -OutFile $src -UseBasicParsing
    }
    $dest = Join-Path $VENDOR_DIR $c
    Remove-Item -Recurse -Force $dest -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $dest | Out-Null
    tar -xzf $src -C $dest --strip-components=1
    if ($LASTEXITCODE -ne 0) { throw "tar extraction failed: $c" }
}

# Find patch.exe (bundled with Git for Windows)
$patch = Get-Command patch -ErrorAction SilentlyContinue
if (-not $patch) {
    $gitPatch = 'C:\Program Files\Git\usr\bin\patch.exe'
    if (Test-Path $gitPatch) { $patch = Get-Command $gitPatch }
}
if (-not $patch) {
    throw 'patch command not found. Install Git for Windows (bundles GNU patch).'
}

Write-Host '==> applying patches'
Push-Location $VENDOR_DIR
try {
    foreach ($p in Get-ChildItem (Join-Path $CUR_DIR '*.patch') | Sort-Object Name) {
        Write-Host "    $($p.Name)"
        & $patch.Source -p1 -i $p.FullName
        if ($LASTEXITCODE -ne 0) { throw "patch failed: $($p.Name)" }
    }
}
finally {
    Pop-Location
}

Write-Host '==> done'
