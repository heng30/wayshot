#!/bin/bash

LOC=$(readlink -f "$0")
DIR=$(dirname "$LOC")
ROOT_DIR="$DIR/../.."

app_name="wayshot"
icon_name="brand.png"
icon_dir="$ROOT_DIR/wayshot/ui/images/png"
pkg_dir="$DIR/package"
appdir="$DIR/${app_name}.AppDir"
sizes=(16x16 22x22 24x24 32x32 36x36 48x48 64x64 72x72 96x96 128x128 192x192 256x256 512x512)

if ! command -v appimagetool &>/dev/null; then
    echo "Error: appimagetool not found. Install it from https://github.com/AppImage/AppImageKit"
    exit 1
fi

if [ ! -f "$ROOT_DIR/target/release/${app_name}" ]; then
    echo "Error: $ROOT_DIR/target/release/${app_name} not found. Run 'cargo build --release' first."
    exit 1
fi

rm -rf "$appdir"
mkdir -p "$appdir/usr/bin"
mkdir -p "$appdir/usr/share/applications"
mkdir -p "$appdir/usr/share/icons/hicolor"

cp "$ROOT_DIR/target/release/${app_name}" "$appdir/usr/bin/"
chmod a+x "$appdir/usr/bin/${app_name}"

for size in "${sizes[@]}"; do
    mkdir -p "$appdir/usr/share/icons/hicolor/${size}/apps"
    magick "${icon_dir}/${icon_name}" -resize "$size" -background none -gravity center -extent "$size" "$appdir/usr/share/icons/hicolor/${size}/apps/xyz.heng30.wayshot.png"
done

cp "$DIR/package/usr/share/applications/${app_name}.desktop" "$appdir/"
cp "$DIR/package/usr/share/applications/${app_name}.desktop" "$appdir/usr/share/applications/"

cp "${icon_dir}/${icon_name}" "$appdir/xyz.heng30.wayshot.png"

cat > "$appdir/AppRun" << 'APPRUN'
#!/bin/bash
SELF=$(readlink -f "$0")
HERE=$(dirname "$SELF")
export PATH="${HERE}/usr/bin:${PATH}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS}"
exec "${HERE}/usr/bin/wayshot" "$@"
APPRUN
chmod a+x "$appdir/AppRun"

appimagetool "$appdir" "${app_name}.AppImage"

rm -rf "$appdir"

exit $?
