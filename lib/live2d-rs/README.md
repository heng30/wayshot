# live2d-rs

纯 CPU 的 Live2D/Cubism 渲染器。无需 GPU、无需窗口，按指定帧率将 Live2D 模型渲染为 RGBA 图片帧。

## 特性

- **纯 CPU 渲染** — 不依赖 GPU、wgpu 或任何图形 API，可在任何环境运行
- **无头渲染** — 不需要窗口或显示器，输出为原始 RGBA 像素数据
- **动作播放** — 支持 `.motion3.json` 动作文件，含淡入淡出
- **表情切换** — 支持 `.exp3.json` 表情文件，含混合模式（叠加/乘算/覆写）
- **姿态控制** — 支持参数覆盖、部件透明度、姿态（pose）切换
- **多种混合模式** — 正常（Normal）、加算（Additive）、乘算（Multiplicative）
- **Multiply/Screen 颜色** — 与 Live2D 官方渲染器一致的着色逻辑

## 快速开始

### 作为库使用

```rust
use live2d_rs::{Live2dRenderer, Options};

// 创建渲染器，加载模型，指定输出分辨率
let mut renderer = Live2dRenderer::new("models/Haru/Haru.model3.json", 512, 512)?;

// 播放动作和表情
renderer.play_motion("models/Haru/motions/haru_g_idle.motion3.json")?;
renderer.play_expression("models/Haru/expressions/F01.exp3.json")?;

// 幂等渲染：给定 fps 和时间，获取对应的 RGBA 帧
// 同样的参数总是返回同样的结果，不受调用历史影响
let frame0 = renderer.render_at(30.0, 0.0);    // 第 0 帧
let frame1 = renderer.render_at(30.0, 1.0/30.0); // 第 1 帧
let frame0_again = renderer.render_at(30.0, 0.0); // 仍然是第 0 帧，和上面一致

// 或使用增量动画推进
let rgba: Vec<u8> = renderer.render_frame(1.0 / 30.0);

// 或渲染静态姿态
let rgba: Vec<u8> = renderer.render_static();

// 手动控制参数
renderer.set_parameter("ParamEyeLOpen", 0.0);
```

### 命令行工具

```bash
# 查看帮助
cargo run --example render -- --help

# 列出模型可用的动作和表情
cargo run --example render -- --list models/Haru/Haru.model3.json

# 渲染一帧静态图片
cargo run --example render -- models/Haru/Haru.model3.json

# 播放动作 + 表情，30fps 渲染 3 秒，输出到指定目录
cargo run --example render -- -m 0 -e 0 -f 30 -d 3 -o output models/Haru/Haru.model3.json

# 自定义分辨率和背景色
cargo run --example render -- -w 256 --height 256 --background 808080FF models/Haru/Haru.model3.json
```

### 运行测试

```bash
bash test.sh
```

会对 `models/` 下的所有模型（Haru、Hiyori、Mao、Mark、Natori、Ren、Rice、Wanko）各渲染 3 秒动画，按模型名称保存到 `output_frames/` 目录。

## 命令行参数

| 参数 | 短 | 说明 | 默认值 |
|------|----|------|--------|
| `MODEL` | — | `.model3.json` 文件路径（必填） | — |
| `--output` | `-o` | 输出目录 | `.` |
| `--width` | `-w` | 输出宽度（像素） | `512` |
| `--height` | — | 输出高度（像素） | `512` |
| `--fps` | `-f` | 帧率 | `30` |
| `--duration` | `-d` | 时长（秒，0 = 仅渲染一帧静态图） | `0` |
| `--motion` | `-m` | 动作索引（0 起，可用 `--list` 查看） | — |
| `--expression` | `-e` | 表情索引（0 起，可用 `--list` 查看） | — |
| `--background` | — | 背景色（8 位十六进制 RGBA，如 `FF0000FF`） | `00000000` |
| `--fill` | — | 模型填充因子（控制模型在画布中的大小） | `1.85` |
| `--list` | — | 列出模型可用的动作和表情后退出 | — |

## API 参考

### `Live2dRenderer`

核心渲染器结构体。

| 方法 | 说明 |
|------|------|
| `new(model_path, width, height)` | 创建渲染器并加载模型 |
| `new_with_options(model_path, width, height, options)` | 使用自定义选项创建渲染器 |
| `render_frame(delta_seconds)` | 推进动画并渲染一帧，返回 RGBA 数据 |
| `render_static()` | 渲染当前状态（不推进动画），返回 RGBA 数据 |
| `render_at(fps, current_time)` | **幂等渲染**：给定 fps 和时间，返回对应的 RGBA 帧。同样的参数总是返回同样的结果 |
| `play_motion(path)` | 播放动作文件 |
| `play_expression(path)` | 播放表情文件 |
| `stop_motion()` | 停止当前动作 |
| `stop_expressions()` | 停止所有表情 |
| `set_parameter(id, value)` | 设置参数值 |
| `resize(width, height)` | 修改输出分辨率 |
| `motion_paths()` | 获取模型中所有动作文件路径 |
| `expression_paths()` | 获取模型中所有表情文件路径 |
| `runtime()` / `runtime_mut()` | 访问底层 ModelRuntime |

### `Options`

渲染器选项。

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| `background` | `[u8; 4]` | 背景色 RGBA | `[0, 0, 0, 0]`（透明） |
| `model_view_fill` | `f32` | 模型填充因子 | `1.85` |

## 项目结构

```
src/
├── lib.rs          # 公共 API 导出
├── render.rs       # CPU 软件光栅化器 + Live2dRenderer
├── error.rs        # 错误类型
├── assets.rs       # 模型加载（model3.json + .moc3 + 纹理）
├── runtime.rs      # ModelRuntime：参数控制、姿态、mesh 构建
├── motion.rs       # MotionPlayer：动作播放
├── expression.rs   # ExpressionManager：表情管理
├── core/           # 数学、插值、混合、参数
├── json/           # JSON 解析器（model3, motion3, expression3, pose3, physics3）
└── moc3/           # .moc3 二进制格式解析器
```

## 渲染流程

1. **动画推进** — 重置参数 → 推进动作/表情 → 应用覆盖 → 应用姿态 → 更新 mesh
2. **CPU 光栅化** — 按 draw order 遍历 mesh，三角形光栅化，双线性纹理采样
3. **混合** — 正常（预乘 alpha 混合）、加算、乘算三种模式
4. **着色** — Multiply/Screen 颜色处理，与 Live2D 官方 WGSL shader 数学逻辑一致
5. **输出** — 预乘 alpha 浮点帧缓冲 → straight alpha u8 RGBA

## 依赖

- `serde` + `serde_json` — JSON 解析
- `image` — PNG 纹理解码

无 GPU 依赖，无窗口系统依赖。

## 致谢

核心模型加载、moc3 解析、运行时逻辑提取自 [Mocari](https://github.com/Eatgrapes/Mocari) 项目（MIT 协议）。

