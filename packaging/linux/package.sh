#!/usr/bin/env bash
# Construiește instalatorul pentru Linux x86_64 și pregătește conținutul stick-ului.
# Rulează pe Linux (în workflow-ul de release: ubuntu-22.04, ca Furnica și Cangurul).
#
# Rezultatul: dist/stick-linux/ și dist/instalator-<versiune>-linux.zip
#
# Pachetele .deb ale Furnicii și Cangurului se caută în $ASSETS (dacă e setat), apoi în
# ../furnica/dist/release și ../cangurul/dist/release.
# Necesită: cargo, zip; pentru build: libxkbcommon-dev (vezi README).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
VERSION="$(sed -n '/^\[package\]/,/^\[/{s/^version = "\(.*\)"/\1/p;}' Cargo.toml | head -1)"
STICK=$ROOT/dist/stick-linux
ZIP=$ROOT/dist/instalator-$VERSION-linux.zip
HERE=$ROOT/packaging/linux

command -v zip >/dev/null || { echo "Lipsește zip (sudo apt install zip)." >&2; exit 1; }

cargo build --release --locked

rm -rf "$STICK" "$ZIP"
mkdir -p "$STICK"
install -m 755 "${CARGO_TARGET_DIR:-target}/release/instalator" "$STICK/instalator"
install -m 755 "$HERE/instalator.sh" "$STICK/instalator.sh"
install -m 644 "$HERE/CITESTE.txt" "$STICK/CITESTE.txt"
cp -R stick/Linux "$STICK/Linux"
find "$STICK" -name .DS_Store -delete

for app in furnica cangurul; do
    Name="$(tr '[:lower:]' '[:upper:]' <<<"${app:0:1}")${app:1}"
    toml="$STICK/Linux/$Name/program.toml"
    deb=$(ls -t ${ASSETS:+"$ASSETS"/$app-*-linux-x86_64.deb} \
        "$ROOT/../$app/dist/release/$app"-*-linux-x86_64.deb 2>/dev/null | head -1 || true)
    if [ -n "$deb" ]; then
        v=$(basename "$deb" | sed "s/^$app-\(.*\)-linux-x86_64.deb$/\1/")
        cp "$deb" "$STICK/Linux/$Name/"
        sed -i.bak -e "s/^version = .*/version = \"$v\"/" -e "s/^file = .*/file = \"$(basename "$deb")\"/" "$toml"
        rm "$toml.bak"
        echo "$Name $v: $(basename "$deb")"
    else
        echo "ATENȚIE: lipsește $app-*-linux-x86_64.deb. Descărcați-l de pe" >&2
        echo "  https://github.com/nazdravan/$app/releases în $STICK/Linux/$Name/ și verificați program.toml." >&2
    fi
done

mkdir -p "$ROOT/dist"
(cd "$STICK" && zip -qr -9 "$ZIP" .)
echo "Gata: $STICK și $ZIP"
