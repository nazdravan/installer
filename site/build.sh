#!/usr/bin/env bash
# Construiește site/style.css din site/src/input.css cu Tailwind CSS: executabilul de sine
# stătător (fără Node), versiune fixată, descărcat o dată în .cache/.
set -euo pipefail
VERSION=4.3.3
cd "$(dirname "$0")"
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) PLATFORM=macos-arm64 ;;
    Darwin-*) PLATFORM=macos-x64 ;;
    Linux-aarch64) PLATFORM=linux-arm64 ;;
    *) PLATFORM=linux-x64 ;;
esac
BIN=../.cache/tailwindcss-$VERSION-$PLATFORM
if [ ! -x "$BIN" ]; then
    mkdir -p ../.cache
    curl -sSfL -o "$BIN" "https://github.com/tailwindlabs/tailwindcss/releases/download/v$VERSION/tailwindcss-$PLATFORM"
    chmod +x "$BIN"
fi
"$BIN" -i src/input.css -o style.css --minify
