#!/bin/bash

LOC=$(readlink -f "$0")
DIR=$(dirname "$LOC")
ROOT_DIR="$DIR/../.."

app_name="wayshot"
icon_name="brand.png"
dst_icon_name="xyz.heng30.wayshot.png"
icon_dir="$ROOT_DIR/wayshot/ui/images/png"
pkg_dir="$DIR/package"
sizes=(16x16 22x22 24x24 32x32 36x36 48x48 64x64 72x72 96x96 128x128 192x192 256x256 512x512)
repo_name="${app_name}-repo"
branch="master"

if ! command -v flatpak &>/dev/null; then
    echo "Error: flatpak not found. Install flatpak first."
    exit 1
fi

if ! command -v magick &>/dev/null; then
    echo "Error: magick (ImageMagick) not found. Install ImageMagick first."
    exit 1
fi

if ! command -v flatpak-builder &>/dev/null; then
    echo "Error: flatpak-builder not found. Install flatpak-builder first."
    exit 1
fi

if [ ! -f "$ROOT_DIR/target/release/${app_name}" ]; then
    echo "Error: $ROOT_DIR/target/release/${app_name} not found. Run 'cargo build --release' first."
    exit 1
fi

mkdir -p "$pkg_dir/bin"
cp "$ROOT_DIR/target/release/${app_name}" "$pkg_dir/bin/"
chmod a+x "$pkg_dir/bin/${app_name}"

for size in "${sizes[@]}"; do
    mkdir -p "$pkg_dir/share/icons/hicolor/${size}/apps"
    magick "${icon_dir}/${icon_name}" -resize "$size" -background none -gravity center -extent "$size" "$pkg_dir/share/icons/hicolor/${size}/apps/${dst_icon_name}"
done

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user -y flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08

flatpak-builder --force-clean --repo="$repo_name" build-dir "$DIR/${app_name}.yaml"

flatpak build-bundle "$repo_name" "xyz.heng30.wayshot.flatpak" xyz.heng30.wayshot "$branch"

rm -rf build-dir .flatpak-builder

rm -f "$pkg_dir/bin/${app_name}"
for size in "${sizes[@]}"; do
    rm -f "$pkg_dir/share/icons/hicolor/${size}/apps/${dst_icon_name}"
    rmdir "$pkg_dir/share/icons/hicolor/${size}/apps" 2>/dev/null
    rmdir "$pkg_dir/share/icons/hicolor/${size}" 2>/dev/null
done
rmdir "$pkg_dir/share/icons/hicolor" 2>/dev/null
rmdir "$pkg_dir/share/icons" 2>/dev/null
rmdir "$pkg_dir/bin" 2>/dev/null

exit $?
