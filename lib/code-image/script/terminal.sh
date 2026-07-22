#!/usr/bin/env bash

for style in macos macos-dark windows windows-dark gnome iterm; do
    cargo run --release -- \
        -l \
        -a ./tmp/JetBrainsMono-Regular.ttf \
        -n ./tmp/SourceHanSansCN.otf \
        --font-size 16 \
        --terminal $style \
        --terminal-title $style \
        -o tmp/output-terminal-${style}.png \
        ./script/terminal.rs
done
