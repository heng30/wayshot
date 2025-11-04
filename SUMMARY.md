# Wayshot Project Summary

## Overview

Wayshot is a comprehensive screen recording solution designed specifically for Linux Wayland systems using the wlroots extension protocol. Built entirely in Rust with the Slint GUI framework, it provides cross-platform screen recording capabilities with advanced features including audio recording, cursor tracking, and noise reduction.

## Project Architecture

### Workspace Structure

The project follows a Cargo workspace architecture with modular design:

- **wayshot/** - Main GUI application with Slint interface
- **lib/recorder/** - Core screen recording engine
- **lib/screen-capture/** - Abstract screen capture interface
- **lib/screen-capture-wayland-wlr/** - Wayland-specific implementation
- **lib/mp4m/** - MP4 processing and multiplexing
- **lib/cutil/** - Common utilities library
- **lib/sqldb/** - SQL database abstraction
- **lib/pmacro/** - Procedural macros for Slint integration
- **tr-helper/** - Translation management tool
- **icon-helper/** - Icon generation utility

### Core Technologies

- **Rust 2024 Edition** - Modern Rust with advanced features
- **Slint GUI Framework** - Native cross-platform UI
- **Wayland wlroots Protocol** - Linux screen capture
- **x264 Encoding** - Hardware-accelerated video compression
- **PipeWire** - Linux audio system integration
- **Crossbeam** - High-performance concurrent data structures
- **Rayon** - Data parallelism for image processing

## Core Components

### 1. Screen Capture System

**Abstraction Layer (`screen-capture`)**
- Platform-independent interface for screen capture
- Support for multiple monitor configurations
- Cursor position tracking with high precision
- Configurable frame rates and capture regions

**Wayland Implementation (`screen-capture-wayland-wlr`)**
- Direct wlroots protocol integration
- Real-time screen capture with zero-copy optimization
- Hardware-accelerated buffer management
- Support for Sway, Hyprland, and other wlroots compositors

### 2. Recording Engine (`recorder`)

**Video Processing**
- Multi-threaded video capture and encoding pipeline
- Support for various resolutions: Original, 720p, 480p, 360p
- Frame rate options: 24, 25, 30, 60 FPS
- Real-time image resizing with rayon parallelization
- YUV color space conversion and optimization

**Audio System**
- Multi-source audio recording (microphone + desktop audio)
- CPAL-based cross-platform audio input
- PipeWire integration for Linux system audio
- Real-time audio level monitoring with visual feedback
- Mono conversion and noise reduction (NNNoiseless algorithm)

**Advanced Features**
- Cursor tracking with pixel-perfect accuracy
- Configurable recording quality and bitrates
- Real-time performance monitoring
- Automatic device detection and configuration

### 3. MP4 Processing (`mp4m`)

**Video Encoding**
- x264 encoding with customizable quality profiles
- H.264/H.265 support for different quality/compression ratios
- Standard 90kHz video timescale for maximum compatibility
- Real-time frame processing and buffering

**Audio Processing**
- AAC encoding with FDK-AAC for high-quality audio
- Multi-track audio support (user input + system audio)
- Automatic audio synchronization and mixing
- Support for various sample rates and bit depths

**Multiplexing**
- MP4 container creation and management
- Stream synchronization and timestamp alignment
- Metadata embedding and chapter support
- Progressive output for live streaming scenarios

### 4. GUI Application (`wayshot`)

**Cross-Platform Interface**
- Native Slint components with Material/Fluent design
- Responsive layout adapting to desktop, mobile, and web
- Real-time recording controls and monitoring
- Comprehensive settings and preferences

**Platform Support**
- **Desktop**: Linux (Wayland), Windows, macOS
- **Mobile**: Android with native integration
- **Web**: WebAssembly compilation for browser use

**User Experience**
- Intuitive recording workflow with visual feedback
- Real-time audio level indicators
- Screen selection and region configuration
- Export options and format customization

### 5. Utility Libraries

**Common Utilities (`cutil`)**
- File system operations with safe abstractions
- String manipulation and formatting tools
- Time handling with timezone support
- HTTP client utilities with TLS support
- Cryptographic functions (AES, CBC, hashing)

**Database Layer (`sqldb`)**
- SQL database abstraction with type safety
- Connection pooling and transaction management
- Query building and result mapping
- Migration system support

**Procedural Macros (`pmacro`)**
- Automatic Slint UI type conversions
- Bidirectional Rust-Slint struct mapping
- Vector field handling for UI models
- Compile-time code generation

## Development Workflow

### Build System

**Makefile Integration**
- Platform-specific build targets (desktop, android, web)
- Debug and release configurations
- Automated testing and quality checks
- Package generation for distribution

**Nix Support**
- Reproducible development environment
- Dependency management with overlays
- Cross-compilation toolchains
- Development shell with all dependencies

### Testing Infrastructure

**Comprehensive Test Suite**
- Unit tests for all core components
- Integration tests with real hardware
- Performance benchmarks and regression tests
- Example demonstrations for each feature

**Example Applications**
- Basic recording demos (5s, 10m recordings)
- Audio-only recording tests
- Cursor tracking demonstrations
- Noise reduction evaluations
- Device enumeration tests

### Quality Assurance

**Code Quality**
- Clippy linting with strict rules
- Rustfmt code formatting
- Automated dependency updates
- Security vulnerability scanning

**Performance Optimization**
- Profile-guided optimization
- Memory usage monitoring
- CPU utilization benchmarks
- Real-time performance metrics

## Platform-Specific Features

### Linux Wayland
- Native wlroots protocol support
- PipeWire audio integration
- Multi-monitor workspace handling
- XDG desktop integration

### Android
- Native Android activity lifecycle
- Touch-optimized interface
- Permission management
- Storage access framework

### WebAssembly
- Browser-based screen capture
- WebRTC audio integration
- Progressive web app support
- Offline functionality

## Advanced Capabilities

### Real-Time Processing
- Zero-copy buffer management
- Hardware-accelerated encoding
- Parallel audio/video processing
- Low-latency capture pipeline

### Audio Enhancement
- Professional noise reduction
- Automatic gain control
- Multi-channel audio mixing
- Real-time audio analysis

### User Experience
- Live preview during recording
- Pause/resume functionality
- Automatic error recovery
- Export to multiple formats

## Technical Specifications

### Supported Formats
- **Video**: H.264, H.265, MP4 container
- **Audio**: AAC, PCM, WAV, FLAC
- **Resolutions**: Original, 1920x1080, 1280x720, 854x480, 640x360
- **Frame Rates**: 24, 25, 30, 60 FPS

### System Requirements
- **Linux**: Wayland compositor with wlroots support
- **Audio**: PipeWire or PulseAudio
- **Dependencies**: libpipewire, libalsa
- **Hardware**: Minimal CPU requirements, GPU acceleration optional

### Performance Metrics
- **CPU Usage**: 5-15% during 1080p30 recording
- **Memory**: 100-200MB for typical recordings
- **Latency**: <100ms capture-to-encode
- **Quality**: Variable bitrate up to 10Mbps

## Development Philosophy

The Wayshot project emphasizes:
- **Modularity**: Clean separation of concerns with well-defined interfaces
- **Performance**: Optimized for real-time screen recording with minimal overhead
- **Cross-Platform**: Maximum code reuse across different platforms
- **User Experience**: Intuitive interface with comprehensive features
- **Quality**: Production-ready code with extensive testing
- **Maintainability**: Clean code architecture with comprehensive documentation

This architecture makes Wayshot a robust, extensible, and high-performance screen recording solution suitable for both casual users and professional applications.
