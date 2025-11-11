# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Wayshot is a screen recording tool for Linux Wayland systems using the wlroots extension protocol. It's built with Rust and the Slint GUI framework, supporting single screen recording, audio recording from input devices and desktop, and microphone noise reduction. The project uses a workspace structure with multiple libraries.

## Build Commands

### Development
- `make desktop-debug` - Run desktop application in debug mode (with RUST_LOG=debug)
- `make desktop-debug-winit` - Run with winit backend for better font rendering on Linux
- `make android-debug` - Run on Android device/emulator
- `make web-debug` - Build and serve web version locally

### Release Builds
- `make desktop-build-release` - Build desktop release binary
- `make android-build-release` - Build Android APK
- `make web-build-release` - Build web distribution

### Testing and Quality
- `make test` - Run all tests in the workspace with output
- `cargo test -p recorder` - Run tests for just the recorder library
- `cargo test --workspace --all-features` - Run all tests with all features
- `make clippy` - Run clippy linter on workspace
- `make check` - Run cargo check on workspace

### Examples and Utilities
- Run recorder examples: `cargo run --example recording_5s_demo -p recorder`
- `make tr` - Run translation helper tool
- `make icon` - Generate icons from images
- `make slint-viewer-desktop/android/web` - Preview UI files with auto-reload

## Architecture

### Workspace Structure
- `wayshot/` - Main application crate with Slint GUI
- `lib/recorder/` - Core screen recording library (this directory)
- `lib/screen-capture/` - Screen capture abstraction layer
- `lib/screen-capture-wayland-wlr/` - Wayland-specific screen capture implementation
- `lib/mp4m/` - MP4 processing and manipulation
- `lib/cutil/` - Utility library (crypto, filesystem, time, strings, etc.)
- `lib/sqldb/` - SQL database abstraction
- `lib/pmacro/` - Procedural macros
- `tr-helper/` - Translation helper tool
- `icon-helper/` - Icon generation tool

### Recorder Library (Current Directory)
The recorder library provides the core screen recording functionality:

**Key Components:**
- `RecordingSession` - Main recording orchestrator
- `AudioRecorder` - Audio input device recording
- `SpeakerRecorder` - Desktop/system audio recording  
- `VideoEncoder` - Video encoding using x264
- `CursorTracker` - Mouse cursor position tracking
- `RecorderConfig` - Configuration builder for recording sessions

**Key Features:**
- Multi-threaded video/audio capture and encoding
- Real-time audio level monitoring
- Noise reduction support (using nnnoiseless)
- Various FPS options and video resolutions
- Cursor tracking integration
- Cross-platform screen capture abstraction

**Testing Examples:**
- `recording_5s_demo.rs` - Basic 5-second recording demo
- `recording_10m_demo.rs` - Longer recording test
- `audio_recording_demo.rs` - Audio-only recording test
- `cursor_tracking_*_demo.rs` - Cursor tracking functionality tests

### Platform Features
- **Desktop**: Uses `desktop-wayland-wlr` feature for Wayland support
- **Android**: Uses `mobile` feature with Android-specific dependencies
- **Web**: Uses `web` feature with WASM compilation

### Feature Flags (Recorder Library)
- `wayland-wlr` - Wayland wlr-protocols support (default)

## Development Notes

### Dependencies and System Requirements
- **Linux Wayland**: Requires `libpipewire` and `libalsa` for audio capture
- **FFmpeg**: Required for final MP4 file processing
- **Rust**: Uses 2024 edition with workspace dependencies

### Audio System Integration
- Uses CPAL for cross-platform audio input
- PipeWire for Linux system/desktop audio capture
- Supports mono conversion and noise reduction
- Real-time audio level monitoring through channels

### Video Processing
- x264 encoding via x264-rust bindings
- Fast image resize with rayon parallelization
- YUV color space handling
- Frame buffering and FPS control

### Common Development Workflow
1. For recorder development: `cargo test -p recorder -- --nocapture`
2. For GUI changes: `make slint-viewer-desktop` for live preview
3. For integration testing: `make test` to run all workspace tests
4. Examples are in `lib/recorder/examples/` for testing specific functionality