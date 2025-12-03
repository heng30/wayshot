A WebRTC server implementation that streams H264 video files to WHEP clients.

### Basic Usage

```bash
# Run with default settings (creates test file if needed)
cargo run --example whep_demo
```

### Advanced Usage

```bash
# With custom H264 file
H264_FILE_PATH=/path/to/your/video.h264 cargo run --example whep_demo

# With custom settings
WRTC_SERVER_ADDRESS=0.0.0.0:9090 \
H264_FPS=25 \
WRTC_APP_NAME=live \
WRTC_STREAM_NAME=my_stream \
cargo run --example whep_demo
```

### Environment Variables

- `WRTC_SERVER_ADDRESS`: Server bind address (default: `127.0.0.1:8080`)
- `H264_FILE_PATH`: Path to H264 file (default: `./test_video.h264`)
- `H264_FPS`: Frames per second for playback (default: `30`)
- `WRTC_APP_NAME`: WebRTC app name (default: `live`)
- `WRTC_STREAM_NAME`: WebRTC stream name (default: `test_stream`)

```bash
RUST_LOG=debug WRTC_SERVER_ADDRESS=0.0.0.0:9090 H264_FILE_PATH=/home/blue/Code/rust/wayshot/lib/wrtc/data/test.h264 cargo run --example whep_demo
```
