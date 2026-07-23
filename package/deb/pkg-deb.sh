#!/bin/bash

LOC=$(readlink -f "$0")
DIR=$(dirname "$LOC")
ROOT_DIR="$DIR/../.."

app_name="wayshot"
icon_name="brand.png"
icon_dir="$ROOT_DIR/wayshot/ui/images/png"
bin_dir="$DIR/package/usr/bin"
dst_icon_name="xyz.heng30.wayshot.png"
magick_tool="magick"
sizes=(16x16 22x22 24x24 32x32 36x36 48x48 64x64 72x72 96x96 128x128 192x192 256x256 512x512)

mkdir -p ${bin_dir}
cp $ROOT_DIR/target/release/${app_name} ${bin_dir}
chmod a+x ${bin_dir}/${app_name}

if ! command -v magick &>/dev/null; then
    if ! command -v convert &>/dev/null; then
        echo "Error: magick (ImageMagick) not found. Install ImageMagick first."
        exit 1
    else
        magick_tool="convert"
    fi
fi

for size in "${sizes[@]}"; do
    mkdir -p $DIR/package/usr/share/icons/hicolor/${size}/apps
    $magick_tool "${icon_dir}/${icon_name}" -resize "$size" -background none -gravity center -extent "$size" "$DIR/package/usr/share/icons/hicolor/${size}/apps/${dst_icon_name}"
done

dpkg-deb --root-owner-group --build package ${app_name}.deb

rm -f ${bin_dir}/${app_name}

for size in "${sizes[@]}"; do
    rm -f $DIR/package/usr/share/icons/hicolor/${size}/apps/${dst_icon_name}
done

exit $?
