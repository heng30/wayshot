#!/usr/bin/env bash

mv -f ./test-images/duplicate/*.png ./test-images/
rm -rf ./test-images/duplicate
cargo run --example dedup -- test-images --semantic-model ./vision_model_quantized.onnx --progress

