#!/bin/bash
# Pack the locally built release binary into an Arch Linux package (.pkg.tar.zst).
# Usage: run from package/archlinux, or via `make archlinux`.

set -e

LOC=$(readlink -f "$0")
DIR=$(dirname "$LOC")
ROOT_DIR="$DIR/../.."
app_name="wayshot"
icon_name="brand.png"
icon_dir="$ROOT_DIR/wayshot/ui/images/png"
dst_icon_name="xyz.heng30.wayshot.png"
sizes=(16x16 22x22 24x24 32x32 36x36 48x48 64x64 72x72 96x96 128x128 192x192 256x256 512x512)

if ! command -v makepkg &>/dev/null; then
    echo "Error: makepkg not found. Run this script on Arch Linux (makepkg is provided by the pacman package)."
    exit 1
fi

if command -v magick &>/dev/null; then
    magick_tool="magick"
elif command -v convert &>/dev/null; then
    magick_tool="convert"
else
    echo "Error: magick (ImageMagick) not found. Install imagemagick first."
    exit 1
fi

bin_path="$ROOT_DIR/target/release/${app_name}"
if [ ! -f "$bin_path" ]; then
    echo "Error: $bin_path not found. Run 'make build-release' first."
    exit 1
fi

version=$(git -C "$ROOT_DIR" describe --tags --abbrev=0 2>/dev/null | sed 's/^v//')
if [ -z "$version" ]; then
    echo "Error: cannot determine version (git describe failed)."
    exit 1
fi
pkgrel=1

build_dir=$(mktemp -d)
trap 'rm -rf "$build_dir"' EXIT

cat > "$build_dir/PKGBUILD" <<EOF
# Maintainer: heng30 <rongheng30@gmail.com>
pkgname=${app_name}
pkgver=${version}
pkgrel=${pkgrel}
pkgdesc="Video creation tool: Video editing, screen recording, streaming, and screen sharing."
arch=('x86_64')
url="https://github.com/Heng30/wayshot"
license=('Apache-2.0' 'GPL-3.0-or-later')
depends=('alsa-lib' 'ffmpeg' 'fontconfig' 'gcc-libs' 'libglvnd' 'libxcb' 'libxkbcommon' 'openssl' 'opus' 'pipewire' 'qt6-base' 'wayland' 'x264')
makedepends=('imagemagick')
options=(!strip)
source=()
sha256sums=()

package() {
    install -Dm755 "${ROOT_DIR}/target/release/${app_name}" "\$pkgdir/usr/bin/${app_name}"
    install -Dm644 "${DIR}/package/usr/share/applications/${app_name}.desktop" "\$pkgdir/usr/share/applications/${app_name}.desktop"
    install -Dm644 "${ROOT_DIR}/LICENSE-APACHE" "\$pkgdir/usr/share/licenses/${app_name}/LICENSE-APACHE"
    install -Dm644 "${ROOT_DIR}/LICENSE-GPL" "\$pkgdir/usr/share/licenses/${app_name}/LICENSE-GPL"
    install -Dm644 "${ROOT_DIR}/package/appimage/wayshot.appdata.xml" "\$pkgdir/usr/share/metainfo/${app_name}.appdata.xml"

    local size
    for size in ${sizes[@]}; do
        install -Dm644 <(${magick_tool} "${icon_dir}/${icon_name}" -resize "\$size" -background none -gravity center -extent "\$size" png:-) "\$pkgdir/usr/share/icons/hicolor/\$size/apps/${dst_icon_name}"
    done
}
EOF

rm -f "$DIR/${app_name}-"*.pkg.tar.zst
cd "$build_dir"
makepkg -f
mv "${app_name}-${version}-${pkgrel}-x86_64.pkg.tar.zst" "$DIR/"
echo "OK: $DIR/${app_name}-${version}-${pkgrel}-x86_64.pkg.tar.zst"
echo "Install with: sudo pacman -U ${app_name}-${version}-${pkgrel}-x86_64.pkg.tar.zst"
