#!/bin/bash
# test.sh — Generate all 24 flip animation combinations (4 corners × 2 axes × 3 directions).
#
# Usage: ./test.sh [image_path]
#
# If no image_path is given, generates a solid-color test image (400×600 blue).
#
# Outputs:
#   test.png                                 — the test image (copy)
#   output/<corner>_<axis>_<direction>.webp  — WebP animation for each combination
#   output/<corner>_<axis>_<direction>/      — PNG frame sequence for each combination
#
# Example:
#   ./test.sh                    # generates test image automatically
#   ./test.sh my_photo.png       # uses your own image

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

IMAGE_PATH="${1:-}"

# Generate test image if none provided
if [ -z "$IMAGE_PATH" ]; then
    echo "No image path provided, generating test image..."
    IMAGE_PATH="test.png"
    cargo run --release --example flip_demo -F animation -- gen-test "$IMAGE_PATH"
fi

if [ ! -f "$IMAGE_PATH" ]; then
    echo "ERROR: Image not found: $IMAGE_PATH"
    exit 1
fi

echo "Using image: $IMAGE_PATH"
echo ""

# Copy test image for reference (skip if already in output/)
if [ "$(realpath "$IMAGE_PATH")" != "$(realpath "test.png" 2>/dev/null || echo "")" ]; then
    mkdir -p output
    cp "$IMAGE_PATH" "test.png"
fi

CORNERS=(br bl tr tl)
AXES=(horizontal vertical)
DIRECTIONS=(forward backward roundtrip)

total=0
for _corner in "${CORNERS[@]}"; do
  for _axis in "${AXES[@]}"; do
    for _dir in "${DIRECTIONS[@]}"; do
      total=$((total + 1))
    done
  done
done

count=0
for corner in "${CORNERS[@]}"; do
  for axis in "${AXES[@]}"; do
    for dir in "${DIRECTIONS[@]}"; do
      count=$((count + 1))
      name="${corner}_${axis}_${dir}"
      echo "[$count/$total] $name"
      cargo run --release --example flip_demo -F animation -- \
        flip \
        "$IMAGE_PATH" \
        --corner "$corner" \
        --axis "$axis" \
        --direction "$dir" \
        --output "output/${name}.webp" \
        --png-dir "output/${name}" \
        --duration 800 \
        --frames 60
    done
  done
done

echo ""
echo "All $total combinations done!"
