#!/usr/bin/env bash

cargo run -- \
    -l \
    -a ./tmp/JetBrainsMono-Regular.ttf \
    -n ./tmp/SourceHanSansCN.otf \
    -o ./tmp/output-go.png \
    ./script/main.go

