#!/bin/bash
set -e

echo "=== Building... ==="
cargo build --release --example render 2>&1

for model in models/*/*.model3.json; do
    name=$(basename "$model" .model3.json)
    echo ""
    echo "=== Rendering: $name ==="
    cargo run --release --example render -- -m 0 -e 0 -f 30 -d 3 -o "output_frames/$name" "$model"
done

echo ""
echo "=== All done ==="
