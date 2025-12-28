#!/bin/bash
# Xiu RTMP Server for Testing
# Receives RTMP stream on rtmp://localhost:1935/live/stream
# Provides HTTP-FLV and HLS streaming

echo "Starting Xiu RTMP server on rtmp://localhost:1935/live/stream"
echo "Press Ctrl+C to stop"
echo ""

# Start xiu RTMP server
# RTMP on port 1935, HLS on port 8081, HTTP-FLV on port 8082,
xiu -r 1935  -s 8081 -f 8082 -l info &
XIU_PID=$!

echo "Xiu RTMP server started (PID: $XIU_PID)"
echo "RTMP: rtmp://localhost:1935/live/stream"
echo "HLS: http://localhost:8081/live/stream/index.m3u8"
echo "HTTP-FLV: http://localhost:8082/live/stream.flv"
echo ""
echo "Waiting for client connection..."

# Function to cleanup on exit
cleanup() {
    echo ""
    echo "Stopping server..."
    kill $XIU_PID 2>/dev/null
    wait $XIU_PID 2>/dev/null
    echo "Server stopped"
    exit 0
}

# Trap SIGINT and SIGTERM
trap cleanup SIGINT SIGTERM

# Wait for xiu process
wait $XIU_PID
cleanup
