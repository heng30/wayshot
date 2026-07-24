#!/usr/bin/env bash

LOC=$(readlink -f "$0")
DIR=$(dirname "$LOC")
cd $DIR/..

make features=windows && make packing-windows

