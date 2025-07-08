#!/usr/bin/env bash
# Construiește Instalator.exe pentru Windows 8 / 10 / 11 (un singur .exe pe 32 de biți, care
# merge și pe 64 de biți) și pregătește conținutul stick-ului. Merge pe macOS și pe Linux.
#
# Rezultatul: dist/stick-windows/ și dist/instalator-<versiune>-windows.zip
#
# Arhivele portabile ale Furnicii și Cangurului se caută în $ASSETS (dacă e setat, de ex. în
# workflow-ul de release), apoi în ../furnica/dist/release și ../cangurul/dist/release.
#
# Necesită: curl, zstd, 7-Zip, compilatorul MinGW pentru 32 de biți
#   macOS:         brew install mingw-w64 sevenzip zstd
#   Debian/Ubuntu: apt install gcc-mingw-w64-i686 7zip zstd
# Rust nightly se instalează prin rustup (dacă lipsește rustup, în .cache/).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
VERSION="$(sed -n '/^\[package\]/,/^\[/{s/^version = "\(.*\)"/\1/p;}' Cargo.toml | head -1)"
CACHE=$ROOT/.cache/win7
STICK=$ROOT/dist/stick-windows
ZIP=$ROOT/dist/instalator-$VERSION-windows.zip
TARGET=i686-win7-windows-gnu
HERE=$ROOT/packaging/windows

# Aceleași versiuni fixate ca la Cangurul (../cangurul/packaging/windows7/package.sh): ținta
# win7 este „tier 3”, un nightly nou poate strica build-ul.
NIGHTLY=nightly-2026-10-19
MSYS=https://repo.msys2.org/mingw/mingw32
MSYS_PKGS=(
    mingw-w64-i686-crt-git-13.0.0.r380.gb83511db8-1-any.pkg.tar.zst
    mingw-w64-i686-gcc-16.2.0-6-any.pkg.tar.zst
    mingw-w64-i686-winpthreads-git-12.0.0.r747.g1a99f8514-1-any.pkg.tar.zst
)
MESA_VERSION=26.2.4
MESA_URL=https://github.com/pal1000/mesa-dist-win/releases/download/$MESA_VERSION/mesa3d-$MESA_VERSION-release-msvc.7z

need() { command -v "$1" >/dev/null || { echo "Lipsește $1. $2" >&2; exit 1; }; }
need i686-w64-mingw32-gcc "macOS: brew install mingw-w64; Debian/Ubuntu: apt install gcc-mingw-w64-i686"
need zstd "macOS: brew install zstd; Debian/Ubuntu: apt install zstd"
SEVENZ=$(command -v 7zz || command -v 7z || true)
[ -n "$SEVENZ" ] || { echo "Lipsește 7-Zip (7zz sau 7z). macOS: brew install sevenzip" >&2; exit 1; }
mkdir -p "$CACHE"

# 1. Rust nightly cu sursele bibliotecii standard (ținta win7 cere -Z build-std).
#    rustup trebuie să fie înaintea unui Rust instalat altfel (de ex. Homebrew) în PATH.
if [ -x "$HOME/.cargo/bin/rustup" ]; then
    export PATH=$HOME/.cargo/bin:$PATH
elif ! command -v rustup >/dev/null; then
    export CARGO_HOME=$CACHE/cargo RUSTUP_HOME=$CACHE/rustup
    if [ ! -x "$CARGO_HOME/bin/rustup" ]; then
        case "$(uname -s)-$(uname -m)" in
            Darwin-arm64) HOST=aarch64-apple-darwin ;;
            Darwin-*) HOST=x86_64-apple-darwin ;;
            *) HOST=x86_64-unknown-linux-gnu ;;
        esac
        curl -sSf -o "$CACHE/rustup-init" "https://static.rust-lang.org/rustup/dist/$HOST/rustup-init"
        chmod +x "$CACHE/rustup-init"
        "$CACHE/rustup-init" -y --no-modify-path --profile minimal --default-toolchain none
    fi
    export PATH=$CARGO_HOME/bin:$PATH
fi
rustup toolchain install $NIGHTLY --profile minimal -c rust-src

# 2. Bibliotecile MinGW legate de msvcrt.dll (MinGW-ul din distribuții folosește UCRT).
MSVCRT=$CACHE/msys32
if [ ! -d "$MSVCRT/mingw32/lib" ]; then
    mkdir -p "$MSVCRT"
    for p in "${MSYS_PKGS[@]}"; do
        curl -sSfL -o "$MSVCRT/$p" "$MSYS/$p"
        zstd -dc "$MSVCRT/$p" | tar -xf - -C "$MSVCRT" mingw32/lib
    done
fi
LIB=$MSVCRT/mingw32/lib
GCC_LIB=$(echo "$LIB"/gcc/i686-w64-mingw32/*)

# 3. Programul.
export RUSTFLAGS="-C target-feature=+crt-static -C link-arg=-B$LIB -C link-arg=-B$GCC_LIB -C link-arg=-L$LIB -C link-arg=-L$GCC_LIB"
cargo +$NIGHTLY build --release --locked -Z build-std --target $TARGET

# 4. Mesa (OpenGL software) pentru calculatoarele fără driver video.
if [ ! -f "$CACHE/mesa/x86/libgallium_wgl.dll" ]; then
    curl -sSfL -o "$CACHE/mesa.7z" "$MESA_URL"
    "$SEVENZ" e -y -o"$CACHE/mesa/x86" "$CACHE/mesa.7z" x86/opengl32.dll x86/libgallium_wgl.dll >/dev/null
fi

# 5. Conținutul stick-ului.
rm -rf "$STICK" "$ZIP"
mkdir -p "$STICK/soft-gl"
cp "target/$TARGET/release/instalator.exe" "$STICK/Instalator.exe"
cp "$CACHE"/mesa/x86/*.dll "$STICK/soft-gl/"
cp -R stick/Windows "$STICK/Windows"
find "$STICK" -name .DS_Store -delete
# Notepad-ul vechi recunoaște UTF-8 doar cu BOM și vrea CRLF.
{ printf '\xEF\xBB\xBF'; sed 's/$/\r/' "$HERE/CITESTE.txt"; } > "$STICK/CITESTE.txt"

# Arhivele portabile ale Furnicii și Cangurului, dacă au fost construite alături.
for app in furnica cangurul; do
    Name="$(tr '[:lower:]' '[:upper:]' <<<"${app:0:1}")${app:1}"
    toml="$STICK/Windows/$Name/program.toml"
    zip=$(ls -t ${ASSETS:+"$ASSETS"/$app-*-windows7-x86-portable.zip} \
        "$ROOT/../$app/dist/release/$app"-*-windows7-x86-portable.zip 2>/dev/null | head -1 || true)
    if [ -n "$zip" ]; then
        v=$(basename "$zip" | sed "s/^$app-\(.*\)-windows7-x86-portable.zip$/\1/")
        cp "$zip" "$STICK/Windows/$Name/"
        sed -i.bak -e "s/^version = .*/version = \"$v\"/" -e "s/^file = .*/file = \"$(basename "$zip")\"/" "$toml"
        rm "$toml.bak"
        echo "$Name $v: $(basename "$zip")"
    else
        # În workflow-ul de release (ASSETS setat) un stick fără program nu se publică.
        if [ -n "${ASSETS:-}" ]; then
            echo "Lipsește $app în $ASSETS." >&2
            exit 1
        fi
        echo "ATENȚIE: lipsește arhiva $app-*-windows7-x86-portable.zip. Descărcați-o de pe" >&2
        echo "  https://github.com/nazdravan/$app/releases în $STICK/Windows/$Name/ și verificați program.toml." >&2
    fi
done

mkdir -p "$ROOT/dist"
(cd "$STICK" && "$SEVENZ" a -tzip -mx9 "$ZIP" . >/dev/null)
echo "Gata: $STICK și $ZIP"
