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

bin_path="$ROOT_DIR/target/release/${app_name}"
if [ ! -f "$bin_path" ]; then
    echo "Error: $bin_path not found. Build the release binary first."
    exit 1
fi

# Auto-detect shared library dependencies from the binary and update control file
deps=$(objdump -p "$bin_path" 2>/dev/null | grep NEEDED | awk '{print $2}' | sort)
pkg_deps=""
while IFS= read -r lib; do
    case "$lib" in
        libasound.so.2)        pkg_deps="$pkg_deps, libasound2" ;;
        libavcodec.so.60)       pkg_deps="$pkg_deps, libavcodec60" ;;
        libavdevice.so.60)      pkg_deps="$pkg_deps, libavdevice60" ;;
        libavformat.so.60)      pkg_deps="$pkg_deps, libavformat60" ;;
        libavutil.so.58)        pkg_deps="$pkg_deps, libavutil58" ;;
        libfontconfig.so.1)     pkg_deps="$pkg_deps, libfontconfig1" ;;
        libgcc_s.so.1)          pkg_deps="$pkg_deps, libgcc-s1" ;;
        libopus.so.0)           pkg_deps="$pkg_deps, libopus0" ;;
        libpipewire-0.3.so.0)   pkg_deps="$pkg_deps, libpipewire-0.3-0" ;;
        libQt6Core.so.6)        pkg_deps="$pkg_deps, libqt6core6" ;;
        libQt6Gui.so.6)         pkg_deps="$pkg_deps, libqt6gui6" ;;
        libQt6Widgets.so.6)     pkg_deps="$pkg_deps, libqt6widgets6" ;;
        libstdc++.so.6)         pkg_deps="$pkg_deps, libstdc++6" ;;
        libswscale.so.7)        pkg_deps="$pkg_deps, libswscale7" ;;
        libxcb.so.1)            pkg_deps="$pkg_deps, libxcb1" ;;
        libwayland-client.so.0) pkg_deps="$pkg_deps, libwayland-client0" ;;
        libc.so.6|libm.so.6|ld-linux-*.so.*) ;;
    esac
done <<< "$deps"
pkg_deps=$(echo "$pkg_deps" | sed 's/^, //')
sed -i "s/^Depends:.*/Depends: $pkg_deps/" "$DIR/package/DEBIAN/control"

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
