<div style="display: flex">
    <img src="./screenshot/1-en.png" width="400"/>
</div>

[中文文档](./README.zh-CN.md)

### Introduction
This is a screen recording tool for `Linux` `Wayland`, which uses the `wlroots` extension protocol to capture screenshots. Commonly used desktop environments include `Sway` and `Hyprland`. It is based on `Rust` and the `Slint` GUI framework.

### Features
- Single screen recording
- Single input device audio recording
- Desktop audio recording
- Microphone noise reduction
- Cursor tracking
- Manage recorded video history
- Play recorded historical videos

### How to build?
- Install `Rust`, `Cargo`, `libpipewire`, `libalsa`, `libx264` and `QT6`
- Run `make desktop-debug` to run it on desktop platform
- Run `make desktop-build-release` to build a release version desktop application for wayland wlr. Likes: `Sway` and `Hyprland`.
- Run `make desktop-build-release desktop-features=desktop-wayland-portal` to build a release version desktop application for wayland xdg-desktop-portal. Likes: `Ubuntu` and `kde`
- Run `make cursor-release` to build the program for fetching the cursor position. This program needs to be used together with the `portal` version of `wayshot`.
- Refer to [Makefile](./Makefile) for more information

### Troubleshooting
- Using the `Qt backend` can resolve the issue of fuzzy fonts on the Windows platform. It is also recommended to prioritize the `Qt backend` to maintain a consistent build environment with the developers.

- Check program output log information: `RUST_LOG=debug wayshot`。Available log level：`debug`, `info`, `warn`, `error`

- To use the cursor tracking feature with the `Wayland xdg portal` version, it needs to be used together with the `wayshot-cursor` program. The program can be downloaded from the Github page. The program must be run with administrator privileges: `sudo -E wayshot-cursor`. If you need to view logs, you can use: `RUST_LOG=debug sudo -E wayshot-cursor`. Available log levels: `debug`, `info`, `warn`, `error`

- Program version selection:
    - `portal` version: `Ubuntu` and `KDE`, etc.
    - `wlr` version: `Sway` and `Hyprland`, etc.

- Install build dependences on `ubuntu` ：
    ```bash
    sudo apt install libxcb-composite0-dev libasound2-dev libpipewire-0.3-dev \
                     libx264-dev libx11-dev libxi-dev libxtst-dev libevdev-dev \
                     qt6-base-dev qt6-tools-dev qt6-tools-dev-tools
    ```

### Reference
- [Slint Language Documentation](https://slint-ui.com/releases/1.0.0/docs/slint/)
- [slint::android](https://snapshots.slint.dev/master/docs/rust/slint/android/#building-and-deploying)
- [Running In A Browser Using WebAssembly](https://releases.slint.dev/1.7.0/docs/slint/src/quickstart/running_in_a_browser)
- [github/slint-ui](https://github.com/slint-ui/slint)
- [Viewer for Slint](https://github.com/slint-ui/slint/tree/master/tools/viewer)
- [LSP (Language Server Protocol) Server for Slint](https://github.com/slint-ui/slint/tree/master/tools/lsp)
- [developer.android.com](https://developer.android.com/guide)
- [How to Deploy Rust Binaries with GitHub Actions](https://dzfrias.dev/blog/deploy-rust-cross-platform-github-actions/)
