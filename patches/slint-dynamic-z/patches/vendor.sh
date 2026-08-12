#!/usr/bin/env bash
# Download the 3 modified slint crates (1.17.1) into vendor/ and apply patches.
#
# Usage: ./patches/vendor.sh
set -euo pipefail
CUR_DIR="$(realpath "$(dirname "$0")")"

ROOT_DIR="$CUR_DIR/.."
[ "$#" -eq 1 ] && [ "$1" = "wayshot" ] && ROOT_DIR="$CUR_DIR/../../../"

VENDOR_DIR="$ROOT_DIR/vendor"
VERSION=1.17.1
CRATES="i-slint-core i-slint-compiler i-slint-backend-qt"

echo "CUR_DIR: " $CUR_DIR
echo "ROOT_DIR: " $ROOT_DIR
echo "VENDOR_DIR: " $VENDOR_DIR

mkdir -p $VENDOR_DIR

for c in $CRATES; do
    # Prefer the local cargo cache, otherwise download from crates.io
    local_crate=$(ls "$HOME/.cargo/registry/cache/"*/"$c-$VERSION.crate" 2>/dev/null | head -1 || true)
    if [ -n "$local_crate" ]; then
        src="$local_crate"
        echo "==> $c: using local cache $src"
    else
        echo "==> $c: downloading from crates.io"
        curl -fL "https://static.crates.io/crates/$c/$c-$VERSION.crate" -o "/tmp/$c-$VERSION.crate"
        src="/tmp/$c-$VERSION.crate"
    fi
    rm -rf "$VENDOR_DIR/$c"
    mkdir -p "$VENDOR_DIR/$c"
    tar xzf "$src" -C "$VENDOR_DIR/$c" --strip-components=1
done

echo "==> applying patches"
(
    cd $VENDOR_DIR
    for p in $CUR_DIR/*.patch; do
        echo "    $p"
        patch -p1 < "$p"
    done
)

echo "==> done"
