# wayshot-cli

Wayshot 的无 GUI 命令行版本：把主程序里不依赖 Slint 界面的能力抽出来，统一用 `clap`
子命令调用，**所有命令都向 stdout 输出一行 JSON**（`--pretty` 可缩进），便于 LLM / 脚本调用。
失败时输出 `{"ok":false,"error":"..."}` 并以退出码 1 结束。

不带额外参数时 stderr 只保留警告；加 `-v` 可看完整调试日志。

**不读任何配置**：不读配置文件、不读数据库、不读 `RUST_LOG` 等环境变量，也不自动下载模型；
所有行为都由命令行参数决定（字体、模型、工程、输入输出路径全部显式传入）。

## 构建

```bash
cargo build -p wayshot-cli            # debug
cargo build -p wayshot-cli --release
./target/debug/wayshot-cli --help
```

`cli` 是主 workspace 的成员，依赖全部来自根 `Cargo.toml` 的 `[workspace.dependencies]`，
不依赖也不会构建 `wayshot`（Slint）本体。

## 命令一览

### 屏幕（Wayland wlr-screencopy）

| 命令 | 说明 |
|---|---|
| `screen list` | 列出显示器（名称 / 位置 / 尺寸 / 缩放） |
| `screen shot -o a.png [--screen eDP-1] [--cursor]` | 截屏 |
| `screen record -o a.mp4 --seconds 10 [--fps ...] [--resolution p1080] [--audio-device N] [--speaker] [--mono] [--denoise] [--camera N] [--camera-bg-model m.onnx] [--cursor-track --region-width 1280 --region-height 720]` | 录屏（x264 + 麦克风/系统声/摄像头画中画与抠像/跟随鼠标自动缩放） |

### 图片

| 命令 | 说明 |
|---|---|
| `image info a.png` | 尺寸信息 |
| `image convert a.png b.jpg` | 按扩展名转格式 |
| `image resize a.png b.png --width 1280 --height 720 [--fit stretch\|contain]` | 缩放（Lanczos3） |
| `image crop a.png b.png --x 0 --y 0 --width 100 --height 100` | 裁剪 |
| `image rotate a.png b.png --degrees 90` | 旋转 |
| `image flip a.png b.png --axis horizontal` | 翻转 |
| `image thumbnail a.png b.png --size 320` | 缩略图 |
| `image effect a.png b.png --name <特效> [--value V] [--value2 V] [--value3 V] [--text S]` | 20+ 特效 + 22 种 preset |
| `image filters` | 列出特效与 preset 名 |
| `image solid -o bg.png --width 1920 --height 1080 --color 1e1e1e` | 生成纯色图片 |
| `image stitch -o long.png a.png b.png c.png [--algorithm template\|col-sample]` | 长截图拼接（重叠检测） |

### 视频（ffmpeg）

| 命令 | 说明 |
|---|---|
| `video info a.mp4` | 流信息（编码、分辨率、帧率、时长） |
| `video frames a.mp4 -o dir [--every-secs 1\|--fps 2\|--count 20] [--start S] [--duration S] [--max-frames N]` | 抽帧 |
| `video thumbnail a.mp4 -o f.png --at 3.5` | 指定时间取帧 |
| `video clip a.mp4 -o b.mp4 --start 2 --duration 5 [--no-audio]` | 截取片段（重新编码） |
| `video audio a.mp4 -o a.wav --sample-rate 16000 --channels 1` | 提取音轨 |
| `video scenes a.mp4 [--algorithm content\|adaptive\|histogram\|threshold] [--threshold 27] [--min-duration 1]` | 场景切换检测，输出切分时间点 |
| `video erase a.mp4 -o b.mp4 --model lama_fp32.onnx --x 100 --y 900 --width 800 --height 120 [--start --duration]` | 擦除固定区域的字幕/水印（LaMa 逐帧修复，保留音轨） |

### 音频

| 命令 | 说明 |
|---|---|
| `audio info a.wav` | 采样率 / 声道 / 时长 / RMS / 峰值 / LUFS |
| `audio vad a.wav [--threshold 0.01] [--output-dir seg]` | 语音活动检测，可导出语音段 WAV |
| `audio gain a.wav b.wav --db 6` | 调音量 |
| `audio speed a.wav b.wav --speed 1.5` | 变速不变调 |
| `audio resample a.wav b.wav --sample-rate 8000 --channels 1` | 重采样 / 转声道 |
| `audio denoise a.wav b.wav --model DeepFilterNet3` | 降噪（DeepFilterNet，输出 48k 单声道） |
| `audio speakers a.wav --models ./pyannote [--merge-gap 0.5] [--output spk.json]` | 说话人分离 |
| `audio devices` | 列出录音设备 |
| `audio record -o a.wav --seconds 5 [--device N] [--gain-db 3] [--mono]` | 麦克风录音 |

### 字幕

| 命令 | 说明 |
|---|---|
| `subtitle info a.srt` | 条数与时长 |
| `subtitle to-srt a.lrc b.srt` | LRC / SRT → SRT |
| `subtitle split a.srt b.srt --max-chars 18` | 长句按标点自动对半拆分（时间轴按字数比例分配） |
| `subtitle translate a.srt b.srt --api-base URL --api-key K --model M [--prompt "翻译成英文"] [--batch-size 40]` | 用 LLM 翻译字幕 |

### 视频工程（`.wayshot`）

| 命令 | 说明 |
|---|---|
| `project create -o p.wayshot a.mp4 b.wav` | 用媒体文件建工程（每个文件一条轨道） |
| `project add-media p.wayshot c.mp4` | 追加媒体 |
| `project info p.wayshot` | 轨道 / 片段 / 时长 |
| `project segments p.wayshot [--track 0]` | 列出片段索引与时间参数 |
| `project trim p.wayshot --track 0 --segment 0 --start 2 --duration 5` | 裁剪片段（改源起点与时长） |
| `project move p.wayshot --track 0 --segment 0 --to 10` | 移动片段在时间轴的位置 |
| `project remove p.wayshot --track 0 --segment 0` | 删除片段 |
| `project filters p.wayshot [--track 0 --segment 0]` | 列出片段滤镜 + 全局滤镜（完整 JSON 参数） |
| `project filter-catalog` | 列出全部滤镜类型名（video/audio/subtitle/global） |
| `project filter-add p.wayshot --track 0 --segment 0 --json '{"enabled":true,"type":"grayscale","params":{}}' [--kind video\|audio\|subtitle\|image]` | 添加滤镜（关键帧写在 `params.keyframe_tracks`） |
| `project filter-remove p.wayshot --track 0 --segment 0 --kind video --index 0` | 删除滤镜 |
| `project global-filters p.wayshot` | 列出全局滤镜 |
| `project global-filter-add p.wayshot --json '{"enabled":true,"type":"global_speed","params":{"speed":1.5}}'` | 加全局滤镜（progress_bar / timer / global_speed / rotation / danmaku） |
| `project global-filter-remove p.wayshot --index 0` | 删全局滤镜 |
| `project memo p.wayshot [--set "备注"]` | 读写工程备注 |
| `project bookmarks p.wayshot` / `bookmark-add --time 12.5` / `bookmark-remove --index 0` | 书签增删查 |
| `project texts p.wayshot` | 列出文字（标题）片段 |
| `project text-add p.wayshot --text "标题" --start 0 --duration 3 --font-size 64 --color ffffffff [--font-family ... --font-path ... --position 0.5,0.5 --alignment 1 --json '<TextElementData>']` | 加文字/标题，自动创建文字轨道 |
| `project text-remove p.wayshot --track 0 --segment 0` | 删除文字片段 |
| `project subtitles p.wayshot` | 列出全部字幕条目（含时间轴） |
| `project subtitle-search p.wayshot "关键词"` | 字幕全文搜索 |
| `project chapter-summary p.wayshot --api-base … --api-key … --model … [--prompt "…"] [--save]` | 用 LLM 生成章节摘要，可选写回工程（GUI 的"章节摘要"） |
| `project voice-cut p.wayshot --track 1 --segment 0 [--entries '<json>'] [--asr-weights … --asr-tokenizer … --api-base … --api-key … --model …] [--reference "文稿"] [--apply]` | 语音剪辑：VAD+ASR+LLM 判定口误/废话并切掉（不加 `--apply` 只输出剪辑计划） |
| `project smart-mix p.wayshot [--audio-track N] [--visual-tracks 0,1] [--audio-entries '<json>'\|--srt a.srt\|--asr-weights …] [--visuals '<json>'\|--vision-model DIR] [--reuse-media --sequential --cover-all] --api-base … --model … [--apply]` | 智能混剪：按音频内容把画面重排到音频时间轴（对应 GUI 的 smart mix） |
| `project export p.wayshot -o out.mp4 [--width --height --fps --crf --preset --audio-bitrate] [--no-subtitles] [--low-memory]` | 渲染工程为 mp4（含字幕烧录、片段滤镜、全局滤镜、文字） |

### 本地 AI（需要模型文件，不自动下载）

| 命令 | 说明 |
|---|---|
| `ai upscale a.png b.png --model swinir.onnx` | SwinIR x4 超分 |
| `ai remove-bg a.png b.png --model modnet.onnx [--kind modnet\|rmbg14] [--mask m.png]` | 去背景（透明 PNG） |
| `ai cutout a.png b.png --model u2netp.onnx [--kind u2netp] [--threshold 160] [--binary] [--sticker ff8800] [--mask m.png]` | 抠图（U2Net / ISNet，7 种模型） |
| `ai dewatermark a.png b.png --model lama_fp32.onnx --x 0 --y 0 --width 200 --height 80` | 去水印（LaMa） |
| `ai split-stems a.mp3 -o stems --model demucs.onnx` | 人声/伴奏分离 |
| `tts --text "你好" -o a.wav --model ./VoxCPM2 [--reference spk.wav] [--cfg 2.0] [--steps 10]` | 文本转语音 / 音色克隆 |
| `ocr run a.png --model ./PaddleOCR-VL [--task spotting] [--device cpu\|cuda\|metal] [--dtype f16]` | 图片 OCR / 文本定位 |
| `transcribe a.mp4 --weights model.pt --tokenizer qwen3.json [--srt out.srt] [-o out.txt]` | 语音转文字（FunASR-Nano），可切成带时间轴 SRT |
| `music-gen generate --prompt "calm piano" -o a.wav --model ./musicgen [--variant small --decoder merged --seconds 10]` | 文本生成音乐（MusicGen） |
| `music search "稻香" [--sources netease,kuwo,kugou,qianqian] [--limit 5] [--download] [--with-assets]` | 音乐搜索/下载（含歌词封面） |
| `music sources` / `music playlist <歌单URL>` | 来源列表 / 歌单解析 |
| `llm "问题" --api-base https://api.openai.com/v1 --api-key K --model gpt-4o-mini [--system "..."]` | 通用大模型对话（OpenAI 兼容接口） |

### 其它

| 命令 | 说明 |
|---|---|
| `code a.rs -o a.png --ascii-font /path/mono.ttf --non-ascii-font /path/cjk.ttf [--language rust --theme "Solarized (dark)" --line-numbers --font-size 16 --scale 2 --terminal macos]` | 代码转图片（字体必填，`font list` 可查；输出 `.svg` 则存矢量） |
| `ttfx --text "HELLO" --effect matrix -o last.png --ascii-font /path/mono.ttf --non-ascii-font /path/cjk.ttf [--frames-dir dir] [--list-effects]` | 37 种终端文字动画特效（字体必填） |
| `animation scroll a.png -o a.mp4 --height 1080` | 长图滚动动画 |
| `animation arrow -o a.mp4 [--x 0.5 --y 0.5 --color ff3355 --line-width 6]` | 箭头指示动画 |
| `animation rect -o a.mp4 [--rect-width 0.3 --rect-height 0.2 --corner-radius 8]` | 框选动画 |
| `animation grade -o a.mp4 --mark checkmark\|cross\|circle` | 评分标记动画 |
| `animation background -o bg.mp4 [--kind bokeh] [--width 1920 --height 1080 --fps 30 --duration 10]` | 19 种程序化动态背景（grid/glitch/noise-flow/matrix-rain/fluid/galaxy/black-hole/ink/wave…） |
| `dedup run DIR [--threshold 10] [--keep largest\|newest\|oldest] [--all-files] [--model clip.onnx]` | 照片去重（SHA-256 + dHash + 可选 CLIP） |
| `similar scan --query ref.png videos... [--threshold 0.75] [--export-dir out]` | 在视频里找与参考图相似的片段并导出 |
| `download url https://... -o file` | 通用下载 |
| `image-search search "关键词" [--sources bing,baidu] [--limit 20] [--download] [--timeout-secs 20]` | 网络图片搜索/下载（41 个来源） |
| `image-search sources` | 列出图片来源 |
| `danmaku fetch BV1xx411c7mD [-o d.json] [--page 1] [--limit 5000]` | 下载 B 站弹幕 |
| `camera list` / `camera snapshot -o a.png [--index 0] [--mirror]` | 摄像头枚举 / 拍照 |
| `clipboard set-text HELLO` / `get-text` / `set-image a.png` | 剪贴板 |
| `qrcode "https://..." -o qr.png [--scale 8]` | 二维码生成 |
| `font list [--query mono]` / `font info a.ttf` | 系统字体枚举与字体文件信息 |
| `update check [--current v1.0.2]` | 版本更新检查（GitHub releases，只报告不安装） |

## 覆盖情况

Wayshot 主程序里的功能分三类：

1. **已移植**：上表全部命令，均直接调用 `lib/*` 中与 GUI 无关的实现（录屏用 `recorder`，
   导出用 `video-editor`，滤镜用 `image-effect`，AI 用对应 ONNX 库）。
2. **需要模型文件**：`ai *`、`tts`、`ocr`、`transcribe`、`music-gen`。默认不联网下载模型，模型路径由参数给出。
3. **未移植（有意）**：
   - 
     字幕检索、章节摘要：都与视觉模型强耦合；字幕擦除已做（`video erase`）。
   - `smart_clip` / `smart_mix`：需要视觉模型 + LLM 决策，且实现依赖 GUI 状态。
   - 播放器（`mp4-player`）、终端面板（`term`）、预览/时间线交互、设置与数据库（`db`）：
     本身就是 GUI 交互或状态管理，没有对应的命令行语义。

## 与主仓库的构建关系

为了让 `ocr` / `transcribe` / `music-gen` / `music` 能被依赖，修了三处上游编译问题（不改业务逻辑）：

- 根 `Cargo.toml`：`rand` 打开 `thread_rng`（`rand::rng()` 与 `StdRng` 需要），`base64` 打开 `std`（`Engine::decode` 需要）。
- `lib/musicdl-rs/src/sources/kugou.rs`：`base64::Engine::decode(&STANDARD, …)` → `STANDARD.decode(…)`。

## 片段滤镜与关键帧

滤镜 JSON 就是工程文件里的格式：`{"enabled": true, "type": "<名字>", "params": { ... }}`。
`project filter-catalog` 给出全部类型名；`project filter-add` 成功后会回显**补全默认值后的完整 JSON**，
把它改几个字段再 `filter-add`/`filter-remove` 即可迭代。关键帧在滤镜自己的 `params.keyframe_tracks` 里。

已知示例：

```json
{"enabled":true,"type":"grayscale","params":{}}
{"enabled":true,"type":"flip","params":{"direction":"Horizontal"}}
{"enabled":true,"type":"vignette","params":{"intensity":0.6}}
{"enabled":true,"type":"gain","params":{"amplitude":2.0}}
{"enabled":true,"type":"global_speed","params":{"speed":1.5}}
{"enabled":true,"type":"rotation","params":{"rotation":90.0}}
```

## 输出约定

## 已验证 / 未验证

- **真机跑通**：截屏、录屏（含摄像头画中画+光标跟踪）、图像/视频全部命令、
  工程全流程（建工程→滤镜→文字→语音剪辑→智能混剪→导出）、19 种背景动画、ttfx、二维码、代码图、
  去重、音乐搜索下载（真实下载 8 首）、图片搜索下载（pixabay 真实下载 20 张）、B 站弹幕（真实拉取）、
  摄像头拍照、字体枚举、更新检查、LLM/字幕翻译/章节摘要（mock OpenAI 服务验证协议）。
- **仅编译+错误路径验证**（本机无模型文件、HuggingFace 不可达）：`ai *`、`tts`、`ocr`、`transcribe`、
  `music-gen`、`similar`、`audio denoise`、`audio speakers`、`video erase`（`--skip-inpaint` 已验证管线）、
  `project voice-cut`/`smart-mix` 的 ASR/VL 自动路径。

## 输出约定

- 成功：`{"ok":true, ...}`，路径一律给出实际写入的绝对/相对路径。
- 失败：`{"ok":false,"error":"..."}`，退出码 1。
- x264/ffmpeg 自身日志走 stderr，不影响 JSON。
