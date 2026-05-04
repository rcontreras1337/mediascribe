<#
.SYNOPSIS
    Downloads ffmpeg + ffprobe static binaries for Windows and places them
    at src-tauri/binaries/ with the target-triple naming Tauri's sidecar
    feature expects.

.DESCRIPTION
    Tauri's `bundle.externalBin` looks up sidecar binaries by host target
    triple. On x86_64 Windows + MSVC toolchain that resolves to
    `ffmpeg-x86_64-pc-windows-msvc.exe`. This script downloads the latest
    "release-essentials" build from gyan.dev (well-maintained, static, MIT-
    licensed re-pack), extracts ffmpeg.exe + ffprobe.exe and copies them
    over with the right names.

    Run this once after cloning the repo and before `npm run tauri dev` or
    `npm run tauri build`. The binaries are git-ignored on purpose (~150 MB).

.NOTES
    macOS counterpart will live in `scripts/fetch-ffmpeg.sh` once we tackle
    the Mac side.
#>

[CmdletBinding()]
param(
    [string]$Target = "x86_64-pc-windows-msvc",
    [string]$DownloadUrl = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
)

$ErrorActionPreference = "Stop"

# Resolve repo root from this script's location: scripts/fetch-ffmpeg.ps1 -> ..
$RepoRoot = Split-Path -Parent $PSScriptRoot
$BinariesDir = Join-Path $RepoRoot "src-tauri\binaries"
$WorkDir = Join-Path $env:TEMP "mediascribe-ffmpeg-fetch"

Write-Host "Repo root        : $RepoRoot"
Write-Host "Binaries target  : $BinariesDir"
Write-Host "Target triple    : $Target"
Write-Host ""

New-Item -ItemType Directory -Path $BinariesDir -Force | Out-Null
New-Item -ItemType Directory -Path $WorkDir -Force | Out-Null

$ZipPath = Join-Path $WorkDir "ffmpeg-release-essentials.zip"
$ExtractDir = Join-Path $WorkDir "extracted"

Write-Host "Downloading ffmpeg from $DownloadUrl ..."
Write-Host "(this is ~120 MB, give it a minute)"
Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing
Write-Host "Downloaded: $((Get-Item $ZipPath).Length / 1MB) MB"
Write-Host ""

Write-Host "Extracting..."
if (Test-Path $ExtractDir) { Remove-Item $ExtractDir -Recurse -Force }
Expand-Archive -Path $ZipPath -DestinationPath $ExtractDir -Force

# The zip contains a top-level versioned directory (e.g. ffmpeg-7.1-essentials_build/)
# and the executables live in its `bin/` subfolder.
$VersionedDir = Get-ChildItem -Path $ExtractDir -Directory | Select-Object -First 1
if (-not $VersionedDir) {
    throw "Did not find an extracted directory inside $ExtractDir"
}
$BinDir = Join-Path $VersionedDir.FullName "bin"
$SrcFfmpeg = Join-Path $BinDir "ffmpeg.exe"
$SrcFfprobe = Join-Path $BinDir "ffprobe.exe"

if (-not (Test-Path $SrcFfmpeg))  { throw "ffmpeg.exe not found at $SrcFfmpeg" }
if (-not (Test-Path $SrcFfprobe)) { throw "ffprobe.exe not found at $SrcFfprobe" }

$DstFfmpeg = Join-Path $BinariesDir ("ffmpeg-{0}.exe" -f $Target)
$DstFfprobe = Join-Path $BinariesDir ("ffprobe-{0}.exe" -f $Target)

Copy-Item -Path $SrcFfmpeg -Destination $DstFfmpeg -Force
Copy-Item -Path $SrcFfprobe -Destination $DstFfprobe -Force

Write-Host ""
Write-Host "Installed sidecar binaries:"
Get-ChildItem $BinariesDir | Format-Table Name, @{N="MB";E={[math]::Round($_.Length/1MB,1)}}

# Cleanup work dir
Remove-Item $WorkDir -Recurse -Force -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "Done. You can now run 'npm run tauri dev' or 'npm run tauri build'."
