<p align="center">
  <img src="screenshot/screenshot-light.png" alt="Wayshot - Video Creation Tool (light)" width="350">
  <img src="screenshot/screenshot-dark.png" alt="Wayshot - Video Creation Tool (dark)" width="350">
</p>

<p align="center">
    <a href="https://github.com/heng30/wayshot/releases"><img src="https://img.shields.io/github/v/release/heng30/wayshot" alt="Release"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-APACHE2.0%20%7C%20GPLv3-blue.svg" alt="License"></a>
    <a href="Platform"><img src="https://img.shields.io/badge/Platform-Linux%20%7C%20Windows-blue.svg" alt="Platform"></a>
    <a href="https://doc.rust-lang.org/edition-guide/rust-2024/"><img src="https://img.shields.io/badge/Rust-2024_edition-orange" alt="Rust 2024"></a>
    <a href="Stars"><img src="https://img.shields.io/github/stars/heng30/wayshot?style=social" alt="Stars"></a>
</p>

<p align="center">
  <strong>Video creation tool: Video editing (with extensive AI-assisted features), screen recording, streaming, and screen sharing.</strong>
</p>

<p align="center">
  <a href="README.md">English</a> | <a href="README.zh-CN.md">简体中文</a> | <a href="https://www.bilibili.com/video/BV1wyMB6KEur">演示视频</a>
</p>


## Introduction
This is a **video editing**, **screen recording**, **streaming**, and **screen sharing** tool. Built with `Rust` and the `Slint` GUI framework. Supported operating systems: `Linux` and `Windows`.

----

## Features

### Recording
- Screen recording, cursor tracking, audio recording (with noise reduction), desktop audio capture, camera capture, camera background removal, and real-time image filters
- Manage and play recorded videos
- Screen sharing (WebRTC) and RTMP streaming

### Video Editing

#### Timeline & Tracks
- Multi-track timeline with 5 track types: Video, Audio, Subtitle, Image, Text
- Automatic track priority ordering: Subtitle > Text > Video/Image > Audio
- Segment operations: Add, insert, move, copy, delete, split, merge
- Segment trimming: Drag left/right edges to trim, stretch to extend
- Track operations: Add, delete, move, copy, lock, hide, mute
- Smart snapping: Auto-snap to segment edges and playhead position
- Link mode: Single-track link, all-tracks link
- One-click gap removal: Remove gaps between segments (left/right/all)
- Detach audio/subtitle: Extract audio or subtitle from video segment to a separate track

#### Undo & Redo
- Full undo/redo system based on the Command Pattern
- Batch operations (atomic multi-command composition)
- Configurable history limit (default 1000 steps)

#### Video Filters (46 types)
| Category | Filters |
|----------|---------|
| Transform | Transform (position/scale/rotation), Crop, Flip, Zoom, Speed, Frame Extract |
| Transition | Fly In (8 directions), Fade In/Out, Slide (4 directions), Wipe (4 directions), Genie, Page Flip, Split |
| Mask | Linear Mask, Circle Mask, Rectangle Mask, Mirror Mask |
| Effect | Chroma Key (green screen), Vignette, Fisheye, Wave, Grain, Old Film, Sketch, Grayscale, Edge Detect, Sharpen, Directional Blur, Gaussian Blur, Breathing, Lighting |
| Overlay | Mosaic, Draw Circle, Draw Rectangle, Local Magnify, Magnifier, Text Highlight, Border, Background, Shadow, Grid, Device Frame, Opacity |
| AI/Advanced | Focus, Live2D Animation Overlay, HSL Adjust |

#### Audio Filters (12 types)
- Gain, Fade In, Fade Out, Normalize, Mute (left/right/both channels), Copy Channel, Limiter, Noise Gate, Compressor, Voice Changer, Speed
- AI Denoising (DeepFilter)

#### Global Filters (5 types)
- Progress Bar, Global Rotation, Timer/Clock, Global Speed, Danmaku (bullet comments) Overlay

#### Keyframe Animation
- Add keyframe animations to filter properties
- Value types: Float, Float2 (position/scale), Color (RGBA), Bool
- Keyframe operations: Add, delete, move, update
- Visual keyframe markers on the timeline, drag to adjust timing

#### Subtitle Editing
- Manually add/modify/delete subtitle entries
- Subtitle style system: Font, size, color, border, border radius, alignment, margins, padding, etc.
- Export subtitles: SRT, VTT, ASS formats
- AI subtitle translation
- AI subtitle removal (based on LaMa inpainting model)

#### Text Overlay
- Text overlay editor: Content, position, rotation, opacity
- Font selection, size, color, outline, background, border, alignment
- Keyframe animation for position/rotation/opacity
- Preset text style save and load

#### Export
- **Video Export**: MP4 (H.264 encoding), supports x264/openh264/ffmpeg backends
  - Resolution: Original, 480P-4K, Portrait, Square, Instagram Portrait
  - Frame rate: 24/25/30/60 FPS
  - Quality presets: Low/Medium/High/Ultra (CRF 28/23/18/15)
  - Audio: Configurable channels and sample rate
  - Low memory mode
  - Subtitle burning
- **Audio Export**: AAC, FLAC, MP3, OGG, WAV
- **Subtitle Export**: SRT, VTT, ASS
- Export queue: Multiple export tasks in parallel, progress display, cancellable

#### Project Management
- Project file save/load (JSON format)
- Auto-save and crash recovery
- Recent projects list
- Project notes (Memo)
- Chapter markers

#### AI Tools
| Tool | Description |
|------|-------------|
| Smart Clip | AI identifies highlights and auto-clips |
| Smart Mix | Three-phase AI pipeline: Speech transcription → Visual recognition → Semantic matching, auto-combines audio and video |
| Scene Detection | Auto-detect video scene boundaries |
| Transcription | FunASR speech-to-text |
| Subtitle Translation | AI subtitle translation |
| Subtitle Remover | AI removal of hardcoded subtitles/watermarks |
| Background Remover | AI background removal |
| Cutout | AI object cutout/segmentation |
| Dewatermark | AI watermark removal |
| Clear Vision | AI super-resolution enhancement (SwinIR) |
| Deep Filter | DeepFilter audio denoising |
| Stem Splitter | AI music stem separation (vocals/drums/bass etc.) |
| Speakers | AI speaker diarization |
| OCR | Optical character recognition |
| TTS | VoxCPM text-to-speech synthesis |
| Music Generation | MusicGen music generation |
| Similar Video Segment | Embedding-based similar video segment detection |
| Chapter Summary | AI-generated video chapter summary generation |

#### Other Tools
- Background animations (18 types): Black Hole, Bokeh, Flow Field, Fluid, Galaxy, Glitch, Grid, Ink, Kaleidoscope, Light Effects, Matrix Rain, Noise Flow, Particle Life, Particle Network, Shape, Triangle, Wave, etc.
- Image animations: Arrow, Grade Mark, Rectangle Draw, Scroll
- Code to Image: Syntax-highlighted code screenshots
- Pure Color Image generation
- Long Screenshot stitching
- Online search for images and audio
- In-editor audio recording
- Metadata viewer
- MCP Server: Exposes 50+ editor operations via Model Context Protocol, enabling AI agent control of the editor (AI operation results are not great, currently incomplete)

#### Keyboard Shortcuts
- Project: Ctrl+N/O/S/W/Q (New/Open/Save/Close/Quit)
- Edit: Ctrl+Z/Shift+Z (Undo/Redo), Ctrl+C/X/V (Copy/Cut/Paste)
- Timeline: Space (Play/Pause), S (Split), M (Merge), Delete (Remove), Home/End (Jump)
- Tools: 30+ shortcuts covering all AI tools
- Panels: Alt+1/2/3/4 to switch left panel tabs

----

## How to Build?
- Install `Rust`, `Cargo`, and `qt6`
- Run `make debug` to debug the desktop platform application
- Run `make build-release` to build a release version desktop application for Wayland wlr. E.g.: `Sway` and `Hyprland`.
- Run `make build-release features=wayland-portal` to build a release version desktop application for Wayland XDG Desktop Portal. E.g.: `Ubuntu` and `KDE`.
- Run `make build-release features=windows` to build a release version desktop application for `Windows`.
- Run `make cursor-release` to build the program for fetching the cursor position. This program needs to be used together with the `portal` version of `wayshot`.
- Refer to [Makefile](./Makefile) for more information

----

## Troubleshooting
- Using the `Qt backend` can resolve the issue of fuzzy fonts on the Windows platform. It is also recommended to prioritize the `Qt backend` to maintain a consistent build environment with the developers.

- Check program output log information: `RUST_LOG=debug wayshot`. Available log levels: `debug`, `info`, `warn`, `error`

- To use the cursor tracking feature with the `Wayland xdg portal` version, it needs to be used together with the `wayshot-cursor` program. The program can be downloaded from the Github page. The program must be run with administrator privileges: `sudo -E wayshot-cursor`. If you need to view logs, you can use: `RUST_LOG=debug sudo -E wayshot-cursor`. Available log levels: `debug`, `info`, `warn`, `error`

- Program version selection:
    - `portal` version: `Ubuntu` and `KDE`, etc.
    - `wlr` version: `Sway` and `Hyprland`, etc.

- Install build dependencies on `Ubuntu`:
    ```bash
    sudo apt install \
      libxcb-composite0-dev imagemagick libasound2-dev libpipewire-0.3-dev libx264-dev libx11-dev \
      libxi-dev libxtst-dev libevdev-dev libfontconfig-dev libavcodec-dev libavformat-dev libavutil-dev \
      libswscale-dev libavfilter-dev libavdevice-dev libssl-dev clang libclang-dev libx264-dev libx265-dev \
      libfdk-aac-dev libmp3lame-dev libopus-dev libvpx-dev libvorbis-dev qt6-base-dev qt6-tools-dev qt6-tools-dev-tools
    ```

- `Windows` build dependencies:
    - `Windows` compiles [`ffmpeg-next`](https://github.com/zmwangx/rust-ffmpeg/wiki/Notes-on-building)
        - Install LLVM (through official installer, Visual Studio, Chocolatey, or any other means), and add LLVM's bin path to PATH, or set LIBCLANG_PATH to that (see clang-sys documentation for additional info).
        - Download [ffmpeg](https://ffmpeg.org/download.html) and [x264.dll](https://github.com/heng30/wayshot/tree/main/wayshot/windows/dll/libx264.dll)
        - Install FFmpeg (complete with headers) through any means, e.g. downloading a pre-built ["full_build-shared"](https://www.gyan.dev/ffmpeg/builds/) version from https://ffmpeg.org/download.html. Set FFMPEG_DIR to the directory containing include and lib.
        - Add FFmpeg's `bin` path to the `PATH` environment variable.
        - `git bash` example:
        ```bash
        export FFMPEG_DIR=C:/ffmpeg-8.0.1-full_build-shared
        export LIBCLANG_PATH="C:/Program Files/Microsoft Visual Studio/2022/Community/VC/Tools/Llvm/x64/bin"
        export PATH=$PATH:"C:/ffmpeg-8.0.1-full_build-shared/bin":"/path/to/x264.dll"
        make build-release features=windows
        ```

    - Set up Qt dependencies
        - [How to find Qt](https://docs.rs/qttypes/latest/qttypes/#finding-qt)
        - Install `Qt6`, and add the directory containing `qmake` to `PATH`: `export PATH=$PATH:"C:/Qt/6.11.1/msvc2022_64/bin"`

----

## How to Configure `STUN` and `TURN` Servers
- Download and install [coturn](https://github.com/coturn/coturn)

- Generate certificate and key: `openssl req -x509 -newkey rsa:1024 -keyout /tmp/turn_key.pem -out /tmp/turn_cert.pem -days 9999 -nodes`

- Edit the configuration.
    - Default location: `/etc/turnserver.conf` or `/etc/coturn/turnserver.conf`

    - Example configuration:
    ```bash
    listening-ip=0.0.0.0
    listening-port=3478
    relay-ip=192.168.10.8
    external-ip=192.168.10.8

    tls-listening-port=5349
    cert=/tmp/turn_cert.pem
    pkey=/tmp/turn_key.pem

    realm=example.com

    lt-cred-mech
    user=foo:123456

    # no-auth
    no-cli
    verbose
    ```

- Testing
    - `turnserver -c /etc/turnserver.conf`
    - Test using [Trickle ICE](https://webrtc.github.io/samples/src/content/peerconnection/trickle-ice/)
    - `TURN` server address format: `turn:192.168.10.1:3478`

----

## Reference
- [Slint Language Documentation](https://slint-ui.com/releases/1.0.0/docs/slint/)
- [github/slint-ui](https://github.com/slint-ui/slint)
- [Viewer for Slint](https://github.com/slint-ui/slint/tree/master/tools/viewer)
- [LSP (Language Server Protocol) Server for Slint](https://github.com/slint-ui/slint/tree/master/tools/lsp)
- [How to Deploy Rust Binaries with GitHub Actions](https://dzfrias.dev/blog/deploy-rust-cross-platform-github-actions/)

----

## Donate
<div align="center">
  <table style="border-spacing: 40px 0;">
    <tr>
      <td align="center">
        <img src="wayshot/ui/images/png/wechat-pay.png" alt="wechat-pay" width="200"><br>
        <p>Wechat Pay</p>
      </td>
      <td align="center">
        <img src="wayshot/ui/images/png/metamask-pay.png" alt="metamask-pay" width="200"><br>
        <p>MetaMask</p>
      </td>
    </tr>
  </table>
</div>
