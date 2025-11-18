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

### How to build?
- Install`Rust`, `Cargo`, `libpipewire`, `libalsa`, `libx264`
- Run `make desktop-debug` to run it on desktop platform
- Run `make desktop-build-release` to build a release version desktop application
- Refer to [Makefile](./Makefile) for more information

### Troubleshooting
- Using the `Qt backend` can resolve the issue of fuzzy fonts on the Windows platform. It is also recommended to prioritize the `Qt backend` to maintain a consistent build environment with the developers.
- `ffmpeg` needs to be installed. It is used to combine the recorded video and audio into the final `MP4` file.

### Reference
- [Slint Language Documentation](https://slint-ui.com/releases/1.0.0/docs/slint/)
- [slint::android](https://snapshots.slint.dev/master/docs/rust/slint/android/#building-and-deploying)
- [Running In A Browser Using WebAssembly](https://releases.slint.dev/1.7.0/docs/slint/src/quickstart/running_in_a_browser)
- [github/slint-ui](https://github.com/slint-ui/slint)
- [Viewer for Slint](https://github.com/slint-ui/slint/tree/master/tools/viewer)
- [LSP (Language Server Protocol) Server for Slint](https://github.com/slint-ui/slint/tree/master/tools/lsp)
- [developer.android.com](https://developer.android.com/guide)
- [How to Deploy Rust Binaries with GitHub Actions](https://dzfrias.dev/blog/deploy-rust-cross-platform-github-actions/)
