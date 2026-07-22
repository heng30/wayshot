#!/usr/bin/env bash

cargo run --release --example inference --  -m ./LFM2.5-VL-450M-ONNX -i ./tmp/test.png -p "描述图片" --precision fp16

