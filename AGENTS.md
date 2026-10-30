# AGENTS.md

本文件由 agent 启动时自动加载。动手前必读「构建与验证」与「跨模块硬约定」；具体功能的逐文件改动清单在
`GLOBAL_RULES.md`（滤镜、工具、翻译等流程的真源，加功能前先照着走一遍）。

## 项目概览

Rust + Slint 的视频创作工具：录屏（Wayland wlr/portal、Windows）、视频剪辑（多轨时间线、46+ 滤镜、
字幕/转录、AI 辅助）、直播推流与 WebRTC 屏幕共享。Rust 2024 edition，Cargo workspace，
默认成员只有 `wayshot`（根目录 `cargo build` 只构建主程序）。

## 仓库地图

| 位置 | 内容 |
|---|---|
| `wayshot/` | 主 GUI（crate `wayshot`）：`src/lib.rs` 是入口 + `slint::include_modules!()`；`src/logic/` 是 UI 回调与业务逻辑；`ui/` 是全部 Slint 组件；`build.rs` 编译 Slint、生成 `src/version.rs` 与 Windows 图标资源 |
| `cli/` | 无 GUI 命令行（crate `wayshot-cli`）：截图/录屏/音频/视频/生码图，面向 LLM 调用 |
| `lib/` | 53 个内部 crate，见下节分组 |
| `wayshot-cursor/`、`tr-helper/`、`icon-helper/` | 独立小工具：全局光标位置采集（rdev + unix socket）、翻译文本提取、图标生成 |
| `vendor/` | 4 个本地改过的 crate（`speakrs`、`esaxx-rs`、`voxcpm-rs`、`deep_filter`），经根 `Cargo.toml` 的 `[patch.crates-io]` 生效 |
| `patches/` | 给 slint 1.17.1 打补丁的 vendor 脚本；slint 已升 1.18，日常构建不需要它 |
| `package/` | deb / appimage / flatpak / archlinux / windows 打包，由 `Makefile` 的 `packing-*` 调用 |
| `changelog/` | 每个发布版本一个文件 `vX.Y.Z` |
| `docs/mcp-api.md` | 视频编辑器 MCP 服务 API（16 类、50+ 工具，stdio / HTTP） |
| `docs/adr/` | 架构决策记录 |
| `CONTEXT.md` | Slint UI 组件术语表，改组件命名/语义前先读 |
| `GLOBAL_RULES.md` | 分步流程规则：新增片段滤镜 / 全局滤镜 / 工具、翻译、各范围的验证命令 |
| `ui-test`（`lib/ui-test`） | Slint 无头契约测试 |

`lib/` 分组（新增 crate 需先加进根 `Cargo.toml` 的 `members`/`workspace.dependencies`）：

- 编辑器核心：`video-editor`（工程模型、时间线、滤镜实现）；`mcp-server` 把编辑能力暴露为 MCP 工具
- 编解码与播放：`video-encoder`、`video-utils`、`mp4m`、`mp4-player`、`srtmp`、`ttfx-rs`
- 采集录制：`screen-capture`、`screen-capture-wayland-wlr`、`screen-capture-wayland-portal`、
  `screen-capture-windows`、`recorder`、`record-audio`、`audio-utils`、`camera`、`screenshot`
- 渲染/动效：`draw-utils`、`image-effect`、`image-animation`、`background-animation`、`lighting`、
  `live2d-rs`、`turn-rs`、`code-image`
- AI 能力：`bot`（LLM 流式客户端）、`background-remover`、`cutout`、`dewatermark`、`denoise`、
  `deepfilter-rs`、`clear-vision`、`paddle-ocr-rs`、`lfm-vl-onnx`、`stem-splitter`、
  `subtitle-remover`、`similar-video-segment`、`dedup-photos`、`musicgen-rs`、`fun-ast-nano`、`tensor-utils`
- 网络：`wrtc`、`turn-rs`、`downloader`、`imagedl-rs`、`musicdl-rs`、`bili-danmaku`
- 基础设施：`sqldb`（SQLite 抽象）、`cutil`（fs/str/time/crypto 等）、`term`（终端模拟器）、
  `bytesio`、`pmacro`（Slint 类型转换宏）

`lib/lfm-vl-candle` 被根 `Cargo.toml` 的 `exclude` 排除，不在 workspace 内。

## 构建与验证

```bash
make check        # cargo check --no-default-features --features=wayland-wlr,database,qrcode,center-window --bin wayshot
make build        # debug 构建主程序
make              # release 构建（lto + strip + opt-level=s）
make debug        # cargo run，RUST_LOG=debug
make debug-winit  # 同上但 SLINT_BACKEND=winit-femtovg
make run-release  # release 运行（RUST_LOG=info）
make slint-viewer # 只开 UI：slint-viewer -I wayshot/ui wayshot/ui/desktop-window.slint
make tr           # 提取待翻译文本（排除 vendor）
make icon-strip   # 同上，加 --strip
make clippy       # cargo clippy --features=wayland-wlr,database,qrcode,center-window
```

- **`SLINT_STYLE=fluent`**：Makefile 的 build / run / check 命令都带它。手工调 cargo 也要带，否则 Slint 换样式，
  看到的界面不是用户看到的界面。
- **平台 feature 三选一**：`wayland-wlr`（默认）、`wayland-portal`、`windows`，互斥，
  切换只改 `features=`。跨平台改动要在本地分别 check；CI（`.github/workflows/linux.yml`）也是
  `make check` 两个 Linux feature 各跑一次，windows.yml 只构建不跑测试。
- 按改动范围选最小验证：
  - 只改 `*.slint` → `make slint-viewer` 目视，或 `make check`（build.rs 会编译全部 Slint）
  - 只改某个 `lib/<crate>` → `cargo check -p <包名>`（目录名≠包名的只有 `cli` → `wayshot-cli`），
    带测试的顺手 `cargo test -p <包名>`
  - 改 `wayshot/src/**` → `make check`
  - 改 `cli/` → `cargo check -p wayshot-cli`
- UI 契约可用无头后端断言：`cargo test -p ui-test`（`i-slint-backend-testing`，不依赖显示环境）。
  该后端每进程只能 `init` 一次，所有断言必须写在同一个 `#[test]` 里，见 `lib/ui-test/tests/update_dialog.rs`。
- `cli/target/`、`cli/Cargo.lock` 是本地构建残留，不要提交。

## Slint ↔ Rust 接缝

- `slint::include_modules!()` 把 `ui/desktop-window.slint` 生成为 `crate::slint_generatedAppWindow`，
  Rust 侧一切 UI 类型都以那里为准。
- 全局单例：`ui/store.slint`（状态）、`ui/logic.slint`（回调）、`ui/base/util.slint`（工具）。
  Rust 侧用 `global_store!(ui)` / `global_logic!(ui)` / `global_util!(ui)` 宏取（`logic.rs` 里定义），
  不要手写 `ui.global::<...>()`。
- Slint 类型 ↔ Rust 类型：`pmacro` 的 `#[derive(SlintFromConvert)]` + `#[from("UI类型")]`，
  向量字段用 `#[vec(from = "...")]`。不要手写双向 `From`。
- C-like 枚举互转用 `crate::impl_c_like_enum_convert!`；需要序列化的 Slint 枚举用
  `crate::impl_slint_enum_serde!`。不要手写这两个 trait。
- 回调实现放 `wayshot/src/logic/<功能>.rs`，由 `logic::init(ui)` 统一注册；新增模块记得挂上去。

## 数据与持久化

- `feature database` 打开 `wayshot/src/db.rs`（`sqldb` / SQLite）：偏好设置、历史、视频编辑器的
  工具与滤镜参数都落这里。新增可持久化配置要同步 `db.rs` 与 `wayshot/src/logic/video_editor/project.rs`
  （逐文件清单见 `GLOBAL_RULES.md`）。
- 视频工程文件是 JSON（`ProjectFile`，由 `lib/video-editor/src/project/project.rs` 序列化/反序列化），
  媒体路径按工程目录写相对路径；改字段要同时给 `ProjectFile` 版本/兼容留退路（旧工程要能打开）。
- 运行期产物 `target/`、`tmp/`、`log/`、`output/`、`dist/`、`worktree/`、`.pi/` 均在 `.gitignore`，不属于源码。

## 国际化

- 界面字符串一律 `crate::logic::tr::tr("...")`；译文表唯一真源是 `wayshot/src/logic/tr.rs`
  （`tr-helper/build.rs` 直接 include 它生成自己的表，不要去改 `tr-helper/src/tr.rs`）。
- 用 `toast_xxx!` / `async_toast_xxx` 输出到界面的常量字符串同样要过 `tr()`。
- 加完文案跑 `make tr` 对照缺失条目，补进 `tr.rs`；Slint 里的新文案也照此办理。

## 跨模块硬约定

- **构建命令自带 `SLINT_STYLE=fluent`**：少一处就会得到另一套样式，而这种偏差能编译、能跑，只有人眼看得出来。
- **upscale/像素参数按高度缩放**：滤镜里以像素为单位的参数要走 `scale_pixel_for_height`，
  否则同一参数在 1080p 和 4K 下视觉大小不同（`GLOBAL_RULES.md` 已列为硬性注意项）。
- **滤镜/工具的"多处同步"是契约**：一个滤镜要同时存在于 `lib/video-editor`（实现 + 工程序列化）、
  `lib/mcp-server`（工具注册）、`wayshot` 的类型转换与 UI 面板、`db.rs`/`project.rs`（持久化）、
  `tr.rs`（文案），漏任一处都表现为"能编译、功能半残"。照 `GLOBAL_RULES.md` 的清单核对，不要即兴增删。
- **GPU/CPU 重活用并行版**：视频滤镜优先用 rayon 并行实现；别在 UI 线程里做逐帧处理。
- **平台代码用 feature 隔离**，不要在 `#[cfg(target_os)]` 之外引入 Linux/Windows 专属 API 到共用路径。
- **`vendor/` 与 `[patch.crates-io]` 是必需的**：这 4 个 crate 带着本地修改，升级依赖时不要顺手把它们
  换成 crates.io 原版，也不要删除 `patch` 段。
- Slint 组件里避免 `import` `wayshot/ui/base/slider.slint` 造成的循环引用（全局滤镜面板尤其），
  按 `GLOBAL_RULES.md` 的提示改用局部实现。

## 提交纪律

- 提交信息用中文；按改动性质逐行加前缀：新增 `[+]`、删除 `[-]`、修改 `[*]`。
- 一行一件事，行与行之间不留空行；多类改动就多行，顺序为 新增 > 删除 > 修改。
- 用显式路径暂存（`git add <具体文件>`），不要 `git add -A` / `git add .`；
  提交前 `git status` 确认暂存区只含本次改动。
- 纯格式化改动与逻辑改动分开提交。
- 发版相关改动（`changelog/vX.Y.Z`、打包脚本）单独提交。

## 注释与格式

- 注释语言跟随文件现状：本仓代码以英文注释为主，改哪个文件就用哪种，不要中英混排。
- 只写代码表达不了的 why（算法取舍、平台怪癖、上游 bug 的绕法），不复述代码在做什么。
- `///` 与它修饰的 item 是位置绑定的：搬/复制/抽取代码时连注释一起搬，搬完确认原位置没有
  孤儿注释、注释里的参数/返回值在紧随其后的签名里都真实存在（编译器和 rustfmt 都不会替你检查）。
- 无 `rustfmt.toml`、无 git hook：提交前自己跑 `cargo fmt`，别把格式化噪音混进逻辑提交。

## 版本与 changelog

- **版本真源是 git tag**：`wayshot/build.rs` 执行 `git describe --tags --abbrev=0` 写进
  `wayshot/src/version.rs`（当前 `v1.0.3`）。`Cargo.toml` 里的 `version = "0.1.0"` 是 workspace 占位，
  不代表发布版本，不要靠改它发版。
- 发版时新增 `changelog/v<version>`，格式照上一份：`## English` + `## 简体中文`，
  内含 Highlights / Features 小节；`Makefile` 的 `packing-*` 用 `git describe` 命名产物。
