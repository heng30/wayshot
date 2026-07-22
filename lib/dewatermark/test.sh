#!/usr/bin/env bash

cargo run --release --example dewatermark_cli -- tmp/test.png --model /home/blue/models/LaMa-ONNX/lama_fp32.onnx --position bottom-right -o tmp/output.png
