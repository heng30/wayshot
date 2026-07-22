#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MODEL_DIR="$SCRIPT_DIR/tmp/models"
INPUT_DIR="$MODEL_DIR/test-images"
OUTPUT_DIR="$SCRIPT_DIR/tmp/output"

mkdir -p "$OUTPUT_DIR"

run() {
    local model="$1"
    local image="$2"
    local model_name
    model_name=$(basename "$model" .onnx)
    local image_name
    image_name=$(basename "${image%.*}")
    local output="$OUTPUT_DIR/${image_name}-${model_name}.png"

    echo "=== $model_name / $image_name ==="
    cargo run --release --example cutout-cli -- \
        -i "$image" \
        -o "$output" \
        -m "$model" \
        -s
    echo
}

# u2net - general purpose salient object detection
for img in "$INPUT_DIR"/*; do
    run "$MODEL_DIR/u2net.onnx" "$img"
done

# u2netp - lightweight version of u2net
for img in "$INPUT_DIR"/*; do
    run "$MODEL_DIR/u2netp.onnx" "$img"
done

# silueta - human silhouette segmentation
for img in "$INPUT_DIR"/human.png "$INPUT_DIR"/people.png "$INPUT_DIR"/leg.png; do
    run "$MODEL_DIR/silueta.onnx" "$img"
done

# isnet-anime - anime background removal
for img in "$INPUT_DIR"/anime.jpg "$INPUT_DIR"/anime2.jpg "$INPUT_DIR"/anime3.jpg "$INPUT_DIR"/anime4.jpg; do
    run "$MODEL_DIR/isnet-anime.onnx" "$img"
done

# isnet-general-use - general use background removal
for img in "$INPUT_DIR"/*; do
    run "$MODEL_DIR/isnet-general-use.onnx" "$img"
done

# u2net_cloth_seg - clothing segmentation
for img in "$INPUT_DIR"/clothes.png "$INPUT_DIR"/human.png "$INPUT_DIR"/people.png; do
    run "$MODEL_DIR/u2net_cloth_seg.onnx" "$img"
done

# u2net_human_seg - human segmentation
for img in "$INPUT_DIR"/human.png "$INPUT_DIR"/people.png "$INPUT_DIR"/leg.png; do
    run "$MODEL_DIR/u2net_human_seg.onnx" "$img"
done

echo "All tests completed. Results in $OUTPUT_DIR"
