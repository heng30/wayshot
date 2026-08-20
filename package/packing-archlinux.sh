#!/usr/bin/env bash

LOC=$(readlink -f "$0")
DIR=$(dirname "$LOC")
cd $DIR/..

make && make archlinux
make features=wayland-portal && make archlinux features=wayland-portal

