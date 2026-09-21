<#
.SYNOPSIS
    Build, test and package wart on Windows.

.DESCRIPTION
    Runs the test suite, builds a release binary, and stages it into dist/
    together with the licenses and a place to drop extra .flf fonts.

.PARAMETER Mode
    quick   - release build only, no tests (fastest iteration)
    test    - tests only
    release - tests + release build + package into dist/ (default)
    debug   - debug build, no tests, launches nothing

.PARAMETER Run
    Launch the GUI after a successful build.

.PARAMETER Example
    After building, generate a few sample exports into dist/samples/.

.PARAMETER Icons
    Re-derive assets/icon/ from doc/img/logo.png before building. Only needed
    after touching the logo; needs Python with Pillow. See tools/make-icons.py
    for the options (edge, shadow, padding, ...).

.EXAMPLE
    .\build.ps1
    .\build.ps1 -Mode quick -Run
    .\build.ps1 -Example
    .\build.ps1 -Icons -Run
#>
[CmdletBinding()]
param(
    [ValidateSet('quick', 'test', 'release', 'debug')]
    [string]$Mode = 'release',
    [switch]$Run,
    [switch]$Example,
    [switch]$Icons
)

$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot

function Write-Step($text) { Write-Host "`n==> $text" -ForegroundColor Cyan }
function Write-Ok($text)   { Write-Host "    $text" -ForegroundColor Green }
function Write-Warn($text) { Write-Host "    $text" -ForegroundColor Yellow }

$started = Get-Date

# A running wart.exe holds a lock on the output file and makes the link step
# fail with a confusing "access denied", so clear it out first.
Write-Step 'Stopping any running instance'
$running = Get-Process -Name wart -ErrorAction SilentlyContinue
if ($running) {
    $running | Stop-Process -Force
    Start-Sleep -Milliseconds 600
    Write-Ok "stopped $($running.Count) process(es)"
} else {
    Write-Ok 'none running'
}

Write-Step 'Toolchain'
$rustc = (rustc --version) 2>&1
if ($LASTEXITCODE -ne 0) { throw 'rustc not found on PATH. Install Rust from https://rustup.rs' }
Write-Ok $rustc
Write-Ok ((cargo --version) 2>&1)

if ($Icons) {
    Write-Step 'Icons'
    $icons = Join-Path $PSScriptRoot 'tools\make-icons.py'
    $python = Get-Command python -ErrorAction SilentlyContinue
    if (-not $python) { throw 'Python is needed to regenerate the icons (pip install pillow)' }
    & $python.Source $icons
    if ($LASTEXITCODE -ne 0) { throw 'icon generation failed' }
    Write-Ok 'assets\icon\ refreshed from doc\img\logo.png'
}

if ($Mode -in @('test', 'release')) {
    Write-Step 'Tests'
    cargo test
    if ($LASTEXITCODE -ne 0) { throw 'tests failed' }
}

$buildProfile = if ($Mode -eq 'debug') { 'debug' } else { 'release' }

if ($Mode -ne 'test') {
    Write-Step "Build ($buildProfile)"
    if ($buildProfile -eq 'release') {
        cargo build --release
    } else {
        cargo build
    }
    if ($LASTEXITCODE -ne 0) { throw 'build failed' }
}

$exe = Join-Path $PSScriptRoot "target\$buildProfile\wart.exe"
if (-not (Test-Path $exe)) { throw "expected binary not found: $exe" }
$exeSize = [math]::Round((Get-Item $exe).Length / 1MB, 1)
Write-Ok "wart.exe ($exeSize MB)"

if ($Mode -eq 'release') {
    Write-Step 'Packaging dist/'
    $dist = Join-Path $PSScriptRoot 'dist'
    New-Item -ItemType Directory -Force -Path $dist | Out-Null

    Copy-Item $exe (Join-Path $dist 'wart.exe') -Force
    Copy-Item (Join-Path $PSScriptRoot 'README.md') $dist -Force
    Copy-Item (Join-Path $PSScriptRoot 'assets\licenses') $dist -Recurse -Force

    # Drop-in directory for any .flf the user wants to add on top of the 328
    # built-in fonts.
    $fontDir = Join-Path $dist 'figlet'
    New-Item -ItemType Directory -Force -Path $fontDir | Out-Null
    Set-Content -Path (Join-Path $fontDir 'README.txt') -Encoding UTF8 -Value @'
Drop .flf FIGlet fonts here and they appear in the
font list alongside the 328 built-in ones. Run `wart --list-fonts` to confirm.
'@
    Write-Ok "dist\wart.exe, dist\figlet\, dist\licenses\"

    if ($Example) {
        Write-Step 'Sample exports'
        $samples = Join-Path $dist 'samples'
        # Recreated rather than reused: otherwise samples from previous runs
        # linger and it is unclear which ones this build produced.
        if (Test-Path $samples) { Remove-Item $samples -Recurse -Force }
        New-Item -ItemType Directory -Force -Path $samples | Out-Null
        $w = Join-Path $dist 'wart.exe'

        & $w --text 'WART' --font standard --format ansi --out (Join-Path $samples 'banner.ansi')
        & $w --text 'WART' --font standard --format plain --out (Join-Path $samples 'banner-plain.txt')
        & $w --text "HELLO,`nDEBIAN" --font 'ANSI Shadow' --gradient '#ff5f5f,#ffd75f' `
             --format ansi --out (Join-Path $samples 'ansi-shadow.ansi')
        & $w --text "HELLO,`nDEBIAN" --font graffiti --gradient '#00d4ff,#ff00d4' `
             --format ansi --out (Join-Path $samples 'graffiti.ansi')
        & $w --text 'WART' --mode block --charset braille --cols 90 --rainbow `
             --format ansi --out (Join-Path $samples 'braille-rainbow.ansi')
        & $w --text "$([char]0xeb99) WART" --font big --gradient '#00d4ff,#ff00d4' `
             --frame rounded --frame-padding 1 --background '#101014' `
             --format png --out (Join-Path $samples 'icon-and-banner.png')
        & $w --text 'WART' --font slant --frame powerline --gradient '#00d4ff,#ff00d4' `
             --format lua --out (Join-Path $samples 'nvim-logo.lua')
        & $w --text 'WART' --font big --gradient '#00d4ff,#ff00d4' `
             --format fastfetch --out (Join-Path $samples 'fastfetch-logo.jsonc')
        & $w --text 'WART' --mode block --charset halfblock --cols 70 `
             --gradient '#00d4ff,#ff00d4' --background '#101014' `
             --format png --out (Join-Path $samples 'logo.png')
        & $w --text 'WART' --font big --gradient '#00d4ff,#ff00d4' `
             --format html --background '#101014' --out (Join-Path $samples 'logo.html')

        Get-ChildItem $samples | ForEach-Object { Write-Ok $_.Name }
    }
}

$elapsed = [math]::Round(((Get-Date) - $started).TotalSeconds, 1)
Write-Host "`nDone in ${elapsed}s" -ForegroundColor Cyan
Write-Host "  binary: $exe" -ForegroundColor Gray

if ($Run) {
    Write-Step 'Launching'
    $launch = if ($Mode -eq 'release') { Join-Path $PSScriptRoot 'dist\wart.exe' } else { $exe }
    Start-Process -FilePath $launch
    Write-Ok "started $launch"
}
