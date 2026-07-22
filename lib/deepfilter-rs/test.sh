#!/usr/bin/env bash

# 使用 aecho 滤镜（模拟回声/延迟）
# ffmpeg -i input.wav -af "aecho=0.8:0.88:60:0.4" output.wav
#
# 添加白噪声
# ffmpeg -i input.wav -f lavfi -i "anoisesrc=color=white:amplitude=0.03" -filter_complex "[0:a][1:a]amix=inputs=2:duration=shortest" output.wav

cargo run --release --example pipelined tmp/test-white-noise.wav tmp/output.wav models/dfn3_h0

