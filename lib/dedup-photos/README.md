# dedup-photos 📸

基于 Rust 的照片去重库（library crate），提供 **三重检测** 管线：

1. **SHA-256 精确去重** — 字节级完全相同的文件
2. **感知相似检测（dHash）** — 127-bit 双梯度感知哈希，汉明距离低于阈值即判为重复（能抓 WhatsApp 压缩、缩放等"看起来一样"的副本）
3. **语义检测（CLIP）** — 本地 CLIP 视觉模型嵌入，按"画面内容"分组，抓连拍/重拍（相机微移但场景相同）

重复图片会被移动到扫描目录下的 `duplicate/` 子目录（每组保留一张，保留策略可配置），并在 `duplicate/` 目录下生成去重报告，**一行一张图片**，说明每张图片因什么原因被去重。

## 作为库使用

```rust
use dedup_photos::{dedup_directory, DedupOptions, KeepStrategy, SemanticConfig};

let options = DedupOptions {
    threshold: 10,                       // dHash 汉明距离阈值
    semantic: Some(SemanticConfig {      // None 则跳过语义检测
        model_path: "clip-vit-b32-vision-q8.onnx".into(),
        threshold: 0.85,                 // 余弦相似度阈值
    }),
    keep: KeepStrategy::Largest,         // 每组保留 largest / newest / oldest
    all_files: false,                    // true 则扫描所有文件类型
    duplicate_dir_name: "duplicate".into(),
    ..DedupOptions::default()
};

let result = dedup_directory(std::path::Path::new("/path/to/photos"), &options)?;
println!("moved {} files, report at {}", result.summary.moved_files, result.report_path.display());
```

### 核心类型

| 类型 | 说明 |
|---|---|
| `dedup_directory(root, options)` | 便捷入口：扫描 → 三重检测 → 移动重复图片 → 生成报告 |
| `dedup_directory_with(root, options, progress, cancel)` | 完整入口，支持进度回调与取消 |
| `ProgressEvent` / `Stage` | 进度事件：`StageStarted` / `ItemDone` / `StageFinished`，阶段为 `Scan` / `Exact` / `Perceptual` / `Semantic` / `Move` / `Report` |
| `CancellationToken` | 协作式取消令牌（`Arc<AtomicBool>`，可 clone 跨线程），`cancel()` 后尽快返回 `DedupError::Cancelled` |
| `DedupOptions` | 阈值、语义配置、保留策略、目录名等选项 |
| `DedupReason` | 去重原因：`Exact` / `Perceptual { hamming_distance }` / `Semantic { cosine_similarity }` |
| `Duplicate` | 单张被去重图片：原始路径、移动后路径、组内保留者、原因 |
| `DedupResult` | 全部 `Duplicate` + 摘要（组数、移动数、释放空间、警告） |
| `DedupError` | thiserror 错误类型：I/O、非目录、无文件、语义模型缺失、取消 |

### 进度与取消

```rust
use dedup_photos::{dedup_directory_with, CancellationToken, ProgressEvent};

let token = CancellationToken::new();
let progress = |e: ProgressEvent| match e {
    ProgressEvent::StageStarted { stage, total } => println!("stage {stage:?} started ({total} items)"),
    ProgressEvent::ItemDone { stage, done, total } => println!("[{stage:?}] {done}/{total}"),
    ProgressEvent::StageFinished { stage, .. } => println!("stage {stage:?} finished"),
};

dedup_directory_with(root, &options, Some(&progress), Some(&token))?;
```

- 进度回调类型为 `&dyn Fn(ProgressEvent) + Sync`，可能从 rayon 工作线程并发调用；`total = 0` 表示总数未知（如扫描阶段）。
- 取消是协作式的：调用 `token.cancel()`（例如 Ctrl+C 处理器）后，检测点尽快返回 `DedupError::Cancelled`；正在运行的并行批次会跑完当前文件，已移动的文件不会回滚。

## 示例 CLI（clap）

```bash
cargo run --example dedup -- /path/to/photos
cargo run --example dedup -- /path/to/photos --semantic-model clip-vit-b32-vision-q8.onnx
cargo run --example dedup -- /path/to/photos --threshold 5 --keep newest --duplicate-dir dup
```

示例参数：`--threshold <N>`（dHash 阈值，默认 10）、`--semantic-model <PATH>`（启用语义检测）、`--semantic-threshold <F>`（默认 0.85）、`--keep largest|newest|oldest`、`--all-files`、`--duplicate-dir <NAME>`、`--progress`（打印各阶段进度）。运行中按 Ctrl+C 会取消去重（退出码 130）。

### 生成测试图片并校验结果

```bash
# 生成 6 张测试图片（含 exact 与 perceptual 重复）到指定目录，同时写入期望结果文件
cargo run --example dedup -- --generate-test-images 6 --test-image-dir /tmp/test-photos
# 运行去重并将实际报告与期望结果文件对比
cargo run --example dedup -- --verify /tmp/test-photos
echo $?   # 0 = 报告符合预期，1 = 不一致
```

- `--generate-test-images <COUNT>`：生成 COUNT 张测试图片后退出（无需提供扫描目录），并生成 `expected_results.json` 期望结果文件。布局：`gradient.png` 原图、`gradient_copy.png`（字节相同 → SHA-256 精确重复）、`gradient_small_a/b.jpg`（同内容不同尺寸 → 感知相似，需 COUNT ≥ 4 才生成两张）、其余为互不相同的 `scene_N.png` 噪声图。
- `--verify <DIR>`：对目录执行完整去重（移动重复图片、生成报告），然后对比 `expected_results.json` 与 `DedupResult`，输出 `PASS`/`FAIL` 及差异明细（组数、移动数、缺失/多余的重复条目）。注意：`--verify` 会真实移动文件，重复运行前需重新 `--generate-test-images` 重置目录；`--threshold`/`--keep` 等参数会同时作用于去重与期望判定，应保持默认值。

## 去重报告

报告写入 `<扫描目录>/duplicate/dedup_report.txt`，主体每行一张图片：

```
/path/to/photos/c.png -> /path/to/photos/duplicate/c.png: exact duplicate of /path/to/photos/a.png (SHA-256 identical)
/path/to/photos/b2.png -> /path/to/photos/duplicate/b2.png: perceptual duplicate of /path/to/photos/b1.png (Hamming distance 0/127)
/path/to/photos/d.jpg -> /path/to/photos/duplicate/d.jpg: semantic duplicate of /path/to/photos/a.jpg (cosine similarity 0.913)
```

检测按优先级分层执行，每张图片只归入一个原因：exact > perceptual > semantic。报告头部包含扫描根目录、文件数、组数、移动数与空间、阈值与保留策略等元信息。

## 语义模型

本库 **不再自动下载模型**。启用语义检测时，你需要自行准备 CLIP 视觉 ONNX 模型文件，并将路径传入 `SemanticConfig`。

模型下载地址元信息（公开常量）：

| 常量 | 值 |
|---|---|
| `dedup_photos::semantic::CLIP_MODEL_URL` | `https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model_quantized.onnx` |
| `dedup_photos::semantic::CLIP_MODEL_FILE` | `clip-vit-b32-vision-q8.onnx` |

模型文件缺失时返回 `DedupError::Semantic`，错误信息中会附带下载地址。

## 设计说明

- **防链式聚类**：每个分组只与组内代表比较，避免"相似的相似"连锁误分组
- **分层分配**：精确组内的文件不会重复进入感知/语义分组；已分组文件只作为后续阶段的比较基准
- **重复目录自排除**：扫描时自动跳过名为 `duplicate` 的目录，重复运行不会把已移动的文件再次纳入检测
- **同名冲突处理**：`duplicate/` 下已存在同名文件时自动追加 `_1`、`_2` 后缀
- **并行处理**：rayon 16 线程池并行计算哈希与嵌入

## 构建

```bash
cargo build --release
cargo test
```
