#!/bin/bash
# Test RTMP client with token authentication
#
# This script demonstrates how to use the query parameter feature
# to pass authentication tokens to the RTMP server

set -e

echo "=========================================="
echo "RTMP Client Token Authentication Test"
echo "=========================================="
echo ""

# Check if token is provided
if [ -z "$1" ]; then
    echo "Usage: $0 <token>"
    echo ""
    echo "Example:"
    echo "  $0 abc123"
    echo ""
    echo "This will connect to: rtmp://localhost:1935/live/stream?token=abc123"
    exit 1
fi

TOKEN="$1"

echo "Configuration:"
echo "  Server:     rtmp://localhost:1935"
echo "  App:        live"
echo "  Stream:     stream"
echo "  Token:      $TOKEN"
echo ""
echo "Full RTMP URL:"
echo "  rtmp://localhost:1935/live/stream?token=$TOKEN"
echo ""
echo "Starting RTMP client..."
echo "=========================================="
echo ""

# Run the RTMP client demo with token in query parameters
cargo run --example rtmp-client-demo -- \
    "rtmp://localhost:1935/live/stream?token=$TOKEN"

echo ""
echo "=========================================="
echo "Test completed"
echo "=========================================="
