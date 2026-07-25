#!/bin/bash

LOC=$(readlink -f "$0")
DIR=$(dirname "$LOC")
ROOT_DIR="$DIR/../.."

app_name="wayshot"
icon_name="brand.png"
dst_icon_name="xyz.heng30.wayshot.png"
icon_dir="$ROOT_DIR/wayshot/ui/images/png"
pkg_dir="$DIR/package"
lib_dir="$DIR/extra-libs"
# Auto-detect Qt6 plugins path
qt_plugins_src=""
_qt_plugins_candidate=$(qmake6 -query QT_INSTALL_PLUGINS 2>/dev/null || true)
for candidate in \
    "$_qt_plugins_candidate" \
    /usr/lib/x86_64-linux-gnu/qt6/plugins \
    /usr/lib/qt6/plugins; do
    if [ -d "$candidate/wayland-decoration-client" ]; then
        qt_plugins_src="$candidate"
        break
    fi
done
if [ -z "$qt_plugins_src" ]; then
    echo "Error: Qt6 plugins directory not found. Install Qt6 Wayland dev packages."
    exit 1
fi
echo "Using Qt6 plugins from: $qt_plugins_src"
qt_plugins_dir="$DIR/qt6-plugins"
magick_tool="magick"
sizes=(16x16 22x22 24x24 32x32 36x36 48x48 64x64 72x72 96x96 128x128 192x192 256x256 512x512)
repo_name="${app_name}-repo"
branch="master"

if ! command -v flatpak &>/dev/null; then
    echo "Error: flatpak not found. Install flatpak first."
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
    $magick_tool "${icon_dir}/${icon_name}" -resize "$size" -background none -gravity center -extent "$size" "$pkg_dir/share/icons/hicolor/${size}/apps/${dst_icon_name}"
done

# Collect all shared libraries
rm -rf "$lib_dir"
mkdir -p "$lib_dir"

skip_libs="libc.so.6 libcrypt.so.1 libdl.so.2 libm.so.6 libmvec.so.1 libpthread.so.0 libresolv.so.2 librt.so.1 libthread_db.so.1 libutil.so.1 ld-linux-x86_64-so.2 ld-linux.so.2 libasound.so.2 libpipewire-0.3.so.0 libwayland-client.so.0 libwayland-cursor.so.0 libwayland-egl.so.1"

echo "Collecting shared libraries from binary..."
ldd "$ROOT_DIR/target/release/${app_name}" | grep "=> /" | awk '{print $3}' | sort -u | while read lib; do
    libname=$(basename "$lib")
    for skip in $skip_libs; do
        if [ "$libname" = "$skip" ]; then
            echo "  Skipping (core): $libname"
            continue 2
        fi
    done
    if [ -f "$lib" ] && [ ! -f "$lib_dir/$libname" ]; then
        echo "  Bundling: $libname"
        cp -L "$lib" "$lib_dir/"
    fi
done

# Collect Qt plugins and their dependencies
echo "Collecting Qt plugins..."
rm -rf "$qt_plugins_dir"
mkdir -p "$qt_plugins_dir"

for plugin_dir in platforms wayland-shell-integration xcbglintegrations imageformats \
    platforminputcontexts platformthemes wayland-graphics-integration-client \
    wayland-decoration-client; do
    if [ -d "$qt_plugins_src/$plugin_dir" ]; then
        echo "  Copying plugin dir: $plugin_dir"
        mkdir -p "$qt_plugins_dir/$plugin_dir"
        cp -L "$qt_plugins_src/$plugin_dir"/*.so "$qt_plugins_dir/$plugin_dir/"

        # Collect dependencies of Qt plugins
        for plugin in "$qt_plugins_src/$plugin_dir"/*.so; do
            ldd "$plugin" 2>/dev/null | grep "=> /" | awk '{print $3}' | sort -u | while read lib; do
                libname=$(basename "$lib")
                for skip in $skip_libs; do
                    if [ "$libname" = "$skip" ]; then
                        continue 2
                    fi
                done
                if [ -f "$lib" ] && [ ! -f "$lib_dir/$libname" ]; then
                    echo "  Bundling (plugin dep): $libname"
                    cp -L "$lib" "$lib_dir/"
                fi
            done
        done
    fi
done

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user -y flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08

flatpak-builder --force-clean --repo="$repo_name" build-dir "$DIR/${app_name}.yaml"

flatpak build-bundle "$repo_name" "${app_name}.flatpak" xyz.heng30.wayshot "$branch"

rm -rf build-dir .flatpak-builder "$lib_dir" "$qt_plugins_dir"

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
