#!/bin/sh
# Pornește Instalatorul de pe stick, ca root. Se rulează cu:  sh /media/<utilizator>/<stick>/instalator.sh
#
# Stick-urile FAT32 se montează fără drept de execuție, de aceea programul se copiază întâi în
# /tmp. Parola se cere o singură dată: prin fereastra sistemului (pkexec) sau, unde aceasta
# lipsește (de ex. antiX), prin sudo în terminal.
set -eu

DIR=$(cd "$(dirname "$0")" && pwd)
TMP=$(mktemp -d /tmp/instalator.XXXXXX)
trap 'rm -rf "$TMP"' EXIT INT TERM
cp "$DIR/instalator" "$TMP/instalator"
chmod 755 "$TMP" "$TMP/instalator"

if [ "$(id -u)" -eq 0 ]; then
    "$TMP/instalator" "$DIR/Linux"
elif command -v pkexec >/dev/null 2>&1 && [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
    # pkexec pornește programul cu un mediu curat: fereastra are nevoie de ecranul utilizatorului.
    pkexec env DISPLAY="${DISPLAY:-}" XAUTHORITY="${XAUTHORITY:-$HOME/.Xauthority}" \
        WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-}" XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-}" \
        "$TMP/instalator" "$DIR/Linux"
else
    sudo -E "$TMP/instalator" "$DIR/Linux"
fi
