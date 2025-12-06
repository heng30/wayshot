# WRTC Examples

This directory contains example programs demonstrating various features of the wrtc library.

## Opus Codec Demo

The `opus_demo.rs` example demonstrates how to use the Opus audio codec for encoding and decoding audio data.

### Features

- Reads WAV audio files
- Encodes audio data using Opus codec
- Decodes encoded audio back to PCM
- Writes decoded audio to new WAV file
- Shows compression ratio statistics
- Comprehensive logging with env_logger

### Usage

```bash
# Run with default logging (info level)
cargo run --example opus_demo

# Run with debug logging for detailed frame information
RUST_LOG=debug cargo run --example opus_demo

# Run with only warnings and errors
RUST_LOG=warn cargo run --example opus_demo

# Run with custom log filtering
RUST_LOG=opus_demo=debug cargo run --example opus_demo
```

### What it does

1. **Reads** `data/test.wav` (48kHz, stereo, 5 seconds)
2. **Encodes** audio to Opus packets (250 frames)
3. **Decodes** Opus packets back to audio
4. **Saves** result to `/tmp/opus-coder.wav`
5. **Shows** compression statistics (~31:1 compression ratio)

### Dependencies

- `hound` - WAV file I/O
- `env_logger` - Structured logging
- `log` - Logging facade
- `wrtc::opus` - Opus codec implementation

### Log Levels

- **INFO**: Basic progress information (default)
- **DEBUG**: Detailed frame-by-frame encoding/decoding info
- **WARN**: Error messages for failed operations
- **ERROR**: Critical errors that stop execution