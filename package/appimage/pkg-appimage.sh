#!/bin/bash

LOC=$(readlink -f "$0")
DIR=$(dirname "$LOC")
ROOT_DIR="$DIR/../.."

app_name="wayshot"
icon_name="brand.png"
icon_dir="$ROOT_DIR/wayshot/ui/images/png"
pkg_dir="$DIR/package"
appdir="$DIR/${app_name}.AppDir"
magick_tool="magick"
sizes=(16x16 22x22 24x24 32x32 36x36 48x48 64x64 72x72 96x96 128x128 192x192 256x256 512x512)

if ! command -v appimagetool &>/dev/null; then
    echo "Error: appimagetool not found. Install it from https://github.com/AppImage/AppImageKit"
    exit 1
fi

if ! command -v magick &>/dev/null; then
    if ! command -v convert &>/dev/null; then
        echo "Error: magick (ImageMagick) not found. Install ImageMagick first."
        exit 1
    else
        magick_tool="convert"
    fi
fi


if [ ! -f "$ROOT_DIR/target/release/${app_name}" ]; then
    echo "Error: $ROOT_DIR/target/release/${app_name} not found. Run 'cargo build --release' first."
    exit 1
fi

rm -rf "$appdir"
mkdir -p "$appdir/usr/bin"
mkdir -p "$appdir/usr/lib"
mkdir -p "$appdir/usr/share/applications"
mkdir -p "$appdir/usr/share/icons/hicolor"

cp "$ROOT_DIR/target/release/${app_name}" "$appdir/usr/bin/"
chmod a+x "$appdir/usr/bin/${app_name}"

for size in "${sizes[@]}"; do
    mkdir -p "$appdir/usr/share/icons/hicolor/${size}/apps"
    $magick_tool "${icon_dir}/${icon_name}" -resize "$size" -background none -gravity center -extent "$size" "$appdir/usr/share/icons/hicolor/${size}/apps/xyz.heng30.wayshot.png"
done

mkdir -p "$appdir/usr/share/metainfo"
cp "$DIR/wayshot.appdata.xml" "$appdir/usr/share/metainfo/"

cp "$DIR/package/usr/share/applications/${app_name}.desktop" "$appdir/"
cp "$DIR/package/usr/share/applications/${app_name}.desktop" "$appdir/usr/share/applications/"

cp "${icon_dir}/${icon_name}" "$appdir/xyz.heng30.wayshot.png"

EXCLUDELIST="$DIR/excludelist"

exclude_names=()
while IFS= read -r line; do
    line=$(echo "$line" | sed 's/#.*//' | xargs)
    [ -z "$line" ] && continue
    exclude_names+=("$line")
done < "$EXCLUDELIST"

should_exclude() {
    local lib_name
    lib_name=$(basename "$1")
    local name
    for name in "${exclude_names[@]}"; do
        if [ "$lib_name" = "$name" ]; then
            return 0
        fi
    done
    return 1
}

echo "Bundling shared libraries..."
ldd "$appdir/usr/bin/${app_name}" | grep "=> /" | awk '{print $3}' | sort -u | while read lib; do
    if [ -f "$lib" ]; then
        if should_exclude "$lib"; then
            echo "  Skipping (excluded): $(basename "$lib")"
        else
            cp -L "$lib" "$appdir/usr/lib/"
        fi
    fi
done

# Also bundle Qt6 plugins
qt_plugin_dirs="/usr/lib/x86_64-linux-gnu/qt6/plugins/platforms /usr/lib/x86_64-linux-gnu/qt6/plugins/imageformats /usr/lib/x86_64-linux-gnu/qt6/plugins/iconengines /usr/lib/x86_64-linux-gnu/qt6/plugins/platformthemes /usr/lib/x86_64-linux-gnu/qt6/plugins/wayland-shell-integration /usr/lib/x86_64-linux-gnu/qt6/plugins/wayland-decoration-client /usr/lib/x86_64-linux-gnu/qt6/plugins/wayland-graphics-integration-client"
for pdir in $qt_plugin_dirs; do
    if [ -d "$pdir" ]; then
        dest="$appdir/usr/lib/qt6/plugins/$(basename "$pdir")"
        mkdir -p "$dest"
        cp -L "$pdir"/*.so "$dest/" 2>/dev/null || true
        for so in "$dest"/*.so; do
            [ -f "$so" ] && ldd "$so" | grep "=> /" | awk '{print $3}' | sort -u | while read lib; do
                if [ -f "$lib" ] && [ ! -f "$appdir/usr/lib/$(basename "$lib")" ]; then
                    if should_exclude "$lib"; then
                        echo "  Skipping (excluded): $(basename "$lib")"
                    else
                        cp -L "$lib" "$appdir/usr/lib/"
                    fi
                fi
            done
        done
    fi
done

cat > "$appdir/AppRun" << 'APPRUN'
#!/bin/bash
SELF=$(readlink -f "$0")
HERE=$(dirname "$SELF")
APP_NAME="wayshot"
APP_ID="xyz.heng30.wayshot"

export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS}"
export QT_PLUGIN_PATH="${HERE}/usr/lib/qt6/plugins:${QT_PLUGIN_PATH}"

# Install desktop file and icons to user directory for taskbar icon support
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
DESKTOP_SRC="${HERE}/${APP_NAME}.desktop"
DESKTOP_DST="${DATA_HOME}/applications/${APP_NAME}.desktop"
NEED_INSTALL=false

if [ ! -f "$DESKTOP_DST" ]; then
    NEED_INSTALL=true
else
    SRC_MD5=$(md5sum "$DESKTOP_SRC" 2>/dev/null | cut -d' ' -f1)
    DST_MD5=$(md5sum "$DESKTOP_DST" 2>/dev/null | cut -d' ' -f1)
    if [ "$SRC_MD5" != "$DST_MD5" ]; then
        NEED_INSTALL=true
    fi
fi

if [ "$NEED_INSTALL" = true ]; then
    # Install desktop file with absolute Exec/Icon paths
    mkdir -p "${DATA_HOME}/applications"
    sed -e "s|^Exec=.*|Exec=${HERE}/usr/bin/${APP_NAME}|" \
        -e "s|^Icon=.*|Icon=${DATA_HOME}/icons/hicolor/512x512/apps/${APP_ID}.png|" \
        "$DESKTOP_SRC" > "$DESKTOP_DST"

    # Install icons
    ICON_SRC_DIR="${HERE}/usr/share/icons/hicolor"
    if [ -d "$ICON_SRC_DIR" ]; then
        for size_dir in "$ICON_SRC_DIR"/*/apps; do
            size=$(basename $(dirname "$size_dir"))
            mkdir -p "${DATA_HOME}/icons/hicolor/${size}/apps"
            cp -n "$size_dir"/${APP_ID}.png "${DATA_HOME}/icons/hicolor/${size}/apps/" 2>/dev/null || true
        done
    fi

    # Update icon cache
    if command -v gtk-update-icon-cache &>/dev/null; then
        gtk-update-icon-cache -f "${DATA_HOME}/icons/hicolor" 2>/dev/null || true
    fi

    # Update desktop database
    if command -v update-desktop-database &>/dev/null; then
        update-desktop-database "${DATA_HOME}/applications" 2>/dev/null || true
    fi
fi

exec "${HERE}/usr/bin/wayshot" "$@"
APPRUN
chmod a+x "$appdir/AppRun"

runtime_url="https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64"
runtime_file="$DIR/runtime-x86_64"

if [ ! -f "$runtime_file" ]; then
    echo "Downloading AppImage runtime..."
    if [ -n "$ALL_PROXY" ]; then
        curl -L --proxy "$ALL_PROXY" -o "$runtime_file" "$runtime_url"
    else
        curl -L -o "$runtime_file" "$runtime_url"
    fi
    if [ $? -ne 0 ]; then
        echo "Error: Failed to download runtime. Set ALL_PROXY and retry, or download manually from:"
        echo "  $runtime_url"
        echo "Place it at: $runtime_file"
        rm -f "$runtime_file"
        exit 1
    fi
fi

appimagetool --no-appstream --runtime-file "$runtime_file" "$appdir" "${app_name}.AppImage"

rm -rf "$appdir"

exit $?
