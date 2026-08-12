# Dynamic Z Demo（slint 1.17.1 + backend-qt 定制版）

最简 slint 框架，演示**动态 z**：运行时改变元素的 `z` 值即可动态调整绘制层级与点击命中顺序。

## 定制方式

- `Cargo.toml` 将 `slint` / `slint-build` 锁定为 `=1.17.1`，保证 crates.io 拉取的库与补丁基线一致
- `[patch.crates-io]` 只 patch 被修改过的 3 个库，源码在 `vendor/`：
  `i-slint-core`、`i-slint-compiler`、`i-slint-backend-qt`
- 补丁文件在 [`patches/`](patches/)（3 个 diff，共 27 个文件），**兼容 slint 1.17.0 与 1.17.1**（在 1.17.1 源码上直接应用）

| Patch | 内容 |
|---|---|
| `0001-i-slint-core-dynamic-z.patch` | `ItemVTable` 与全部 item 结构体增加 `z: Property<f32>`；渲染按 z 排序；鼠标 hit-testing 按 z 降序命中 |
| `0002-i-slint-compiler-dynamic-z.patch` | `z_order` pass 保留 z 绑定；`materialize_fake_properties` 不物化 z；`builtins.slint` 的 `Empty` 基类声明 `z` 属性 |
| `0003-i-slint-backend-qt-dynamic-z.patch` | qt_widgets 的 12 个原生 widget item 增加 `z` 字段支持 |

## 重建 vendor（下载 3 个库 + 应用补丁）

```bash
./patches/vendor.sh          # Linux / macOS / Git Bash
.\patches\vendor.ps1         # Windows PowerShell（需 Git for Windows）
```

脚本：优先使用本地 cargo 缓存，否则从 crates.io 下载 3 个 1.17.1 的 `.crate` 包，
解压到 `vendor/` 后应用补丁。`vendor/` 中其他包不受影响。

## 使用动态 z

任意元素都可以写：

```slint
Rectangle {
    z: some_condition ? 10 : 0;   // 任意运行时表达式
}
```

改变表达式依赖的属性后，绘制顺序与点击命中顺序立即变化（z 大者在上）。

## 验证

```bash
cargo test        # 渲染像素测试 + 命中顺序测试 + 静态 z 兼容测试
cargo run         # 交互 demo：点击方块将其置顶
```
