#!/usr/bin/env bash

LOC=$(readlink -f "$0")
DIR=$(dirname "$LOC")
cd $DIR/..

make && make packing-linux
make features=wayland-portal && make packing-linux features=wayland-portal

