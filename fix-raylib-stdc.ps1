#Requires -Version 5.1
<#
.SYNOPSIS
    Patches raylib-sys in the Cargo registry to fix LNK1181 (stdc++.lib) on MSVC.
.DESCRIPTION
    raylib-sys unconditionally requests stdc++.lib when the imgui feature is
    enabled. That library only exists on GCC/MinGW; MSVC does not have it.
    This script patches the build.rs in the Cargo registry cache and then
    clears both the compiled build-script binary and its output from
    target/release/build/ so Cargo is forced to recompile the build script
    from the patched source on the next build.

    Run from the project root with:
        .\fix-raylib-stdc.ps1

    See docs/raylib-sys-stdc-windows.md for a full explanation.
.PARAMETER ProjectRoot
    Path to the Cargo project root. Defaults to the current directory.
#>
param(
    [string]$ProjectRoot = $PWD.Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ── 1. Locate and patch build.rs in the Cargo registry ───────────────────────

$CargoHome     = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { "$env:USERPROFILE\.cargo" }
$RegistrySrc   = "$CargoHome\registry\src"

Write-Host "Searching for raylib-sys build.rs under $RegistrySrc ..."

$candidates = @(
    Get-ChildItem $RegistrySrc -Recurse -Filter "build.rs" -ErrorAction SilentlyContinue |
    Where-Object { $_.DirectoryName -match "[\\/]raylib-sys-\d" }
)

if ($candidates.Count -eq 0) {
    Write-Warning "No raylib-sys found in the Cargo registry."
    Write-Warning "Make sure raylib (with the imgui feature) is a dependency, then run 'cargo fetch'."
    exit 1
}

# The snippet that needs to be wrapped with a platform check.
$oldSnippet = '    println!("cargo:rustc-link-lib=dylib=stdc++");'

# Replacement: same println, now inside a Windows guard.
$newSnippet  = '    let target = std::env::var("TARGET").unwrap_or_default();'
$newSnippet += "`n    if !target.contains(`"windows`") {"
$newSnippet += "`n        println!(`"cargo:rustc-link-lib=dylib=stdc++`");"
$newSnippet += "`n    }"

$patchedFiles = 0
foreach ($file in $candidates) {
    $text = [System.IO.File]::ReadAllText($file.FullName)

    if ($text -match 'if !target\.contains\("windows"\)') {
        Write-Host "  Already patched: $($file.FullName)"
        continue
    }

    if (-not $text.Contains($oldSnippet)) {
        Write-Host "  Pattern not found (fixed upstream?), skipping: $($file.FullName)"
        continue
    }

    $text = $text.Replace($oldSnippet, $newSnippet)
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [System.IO.File]::WriteAllText($file.FullName, $text, $utf8NoBom)
    Write-Host "  Patched: $($file.FullName)"
    $patchedFiles++
}

# ── 2. Clear the Cargo build cache for raylib-sys ────────────────────────────
#
#  Cargo stores a build script in two separate directories:
#    build/raylib-sys-<A>  — compiled build-script binary
#    build/raylib-sys-<B>  — output produced by running that binary
#  Both must be removed to force a full rebuild from the patched source.

$targetRelease = Join-Path $ProjectRoot "target\release"
$clearedDirs   = 0

if (-not (Test-Path $targetRelease)) {
    Write-Host "No target\release directory found; skipping cache clear."
} else {
    Write-Host "Clearing raylib-sys build cache in $targetRelease ..."
    foreach ($subDir in @("build", ".fingerprint")) {
        $searchPath = Join-Path $targetRelease $subDir
        if (Test-Path $searchPath) {
            foreach ($dir in (Get-ChildItem $searchPath -Filter "raylib-sys-*")) {
                Remove-Item $dir.FullName -Recurse -Force
                Write-Host "  Removed: $($dir.FullName)"
                $clearedDirs++
            }
        }
    }
}

# ── Summary ───────────────────────────────────────────────────────────────────

Write-Host ""
if ($patchedFiles -gt 0 -or $clearedDirs -gt 0) {
    Write-Host ("Done.  {0} file(s) patched, {1} cache dir(s) cleared." -f $patchedFiles, $clearedDirs)
} else {
    Write-Host "Nothing to do - already patched and cache is clean."
}
Write-Host "Run 'cargo build --release' from a VS Developer PowerShell to rebuild."
