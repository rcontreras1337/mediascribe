#!/usr/bin/env bash
# Placeholder for the macOS counterpart of fetch-ffmpeg.ps1.
#
# When we add macOS support we'll download a static build (e.g. from
# https://evermeet.cx/ffmpeg/) and place ffmpeg / ffprobe at
# src-tauri/binaries/ffmpeg-<target> / ffprobe-<target>, where <target> is
# either aarch64-apple-darwin (Apple Silicon) or x86_64-apple-darwin (Intel).
#
# Until then, this script is a no-op that exits with a friendly message.

set -euo pipefail

cat <<'EOF'
mediascribe: fetch-ffmpeg.sh is a placeholder.

macOS support is not wired up yet. When we get there this script will
download static ffmpeg + ffprobe builds and place them at
src-tauri/binaries/ with the target-triple naming Tauri expects.

For now, run the app on Windows.
EOF
exit 0
