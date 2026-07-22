# Live2D 模型文件结构说明

## Haru 模型文件作用

`models/Haru/` 是一个标准的 Live2D Cubism 3 模型目录，所有文件路径都由 `Haru.model3.json` 作为入口索引：

| 文件/目录 | 作用 |
|---|---|
| **`Haru.model3.json`** | **模型入口/清单文件**。声明版本号、所有子文件的相对路径（Moc、纹理、物理、姿势、表情、动作）、参数分组（眨眼/口型同步）、点击区域 |
| **`Haru.moc3`** | **模型核心二进制**。包含所有网格顶点、变形器、参数绑定、绘制顺序等几何/动画数据，是整个模型的数据主体（384KB） |
| **`Haru.2048/texture_00.png`** / **`texture_01.png`** | **纹理贴图**。模型的皮肤、衣服、头发等贴图，2048 是纹理尺寸标记 |
| **`Haru.physics3.json`** | **物理模拟配置**。定义头发、衣服等部件的摆动/弹性物理参数 |
| **`Haru.pose3.json`** | **姿势配置**。定义部件间的联动关系（如左右手臂互斥——一个显示时另一个隐藏） |
| **`Haru.cdi3.json`** | **显示信息/元数据**。给参数 ID 和部件 ID 起人类可读的名字（如 `ParamAngleX` → "角度 X"），并按功能分组 |
| **`Haru.userdata3.json`** | **用户自定义数据**。给特定 ArtMesh 绑定自定义标签（这里给 3 个网格标记了 `"tai"` 值） |
| **`expressions/F01~F08.exp3.json`** | **表情定义**。每个文件定义一组参数覆盖值，用于切换面部表情（F01~F08 共 8 种） |
| **`motions/haru_g_idle.motion3.json`** 等 | **动作定义**。每个文件定义一条或多条动画曲线，按组分类（Idle=待机、TapBody=点击身体触发） |
| **`sounds/*.wav`** | **音效文件**。与特定动作关联的音效（在 `model3.json` 的 Motions 条目中通过 `Sound` 字段引用） |

## 库是否内置了模型文件元信息

**没有。** `live2d-rs` 库本身不硬编码任何模型文件名或路径。

所有文件元信息都来自运行时读取的 `*.model3.json`。库的解析流程是：

1. 用户传入 `Haru.model3.json` 的路径
2. `Model3::from_json_str()` 反序列化 JSON，提取 `FileReferences` 结构
3. 后续所有文件定位都通过 `model_dir.join(model.moc())`、`model_dir.join(texture)` 等方式，**基于 JSON 中声明的相对路径拼接**

库代码中没有任何 `include_str!`、`include_bytes!` 或硬编码的 `"Haru"` 字符串。`models/` 目录下的 8 个模型（Haru、Hiyori、Mao 等）纯粹是示例/测试数据，不是库的内置资源。

## 库如何解析 models 下的文件名

**库不解析文件名，也不依赖文件名约定。** 它完全依赖 `model3.json` 中的路径声明。

具体来说，`assets.rs` 中的 `parse_model()` 函数：

```rust
let model_dir = path.parent()?;  // 取 model3.json 所在目录作为基准

let moc_path = model_dir.join(model.moc());           // "Haru.moc3"
let textures = model.textures().iter()                 // ["Haru.2048/texture_00.png", ...]
    .map(|t| decode_texture(model_dir.join(t)));
let pose = model.pose().map(|p| model_dir.join(p));    // "Haru.pose3.json"
```

`render.rs` 中构建动作/表情路径列表也是同样方式：

```rust
fn motion_path_bufs(runtime, model_dir) {
    runtime.model().motions().values().flatten()
        .map(|ref| model_dir.join(ref.file()))   // "motions/haru_g_idle.motion3.json"
        .collect()
}
```

**关键点：**

- 文件名中的 `Haru` 前缀、`.2048` 后缀、`haru_g_` 前缀等对库来说**毫无意义**，只是模型制作者自己的命名习惯
- 库只认 JSON 里写的相对路径字符串，然后和 `model3.json` 的父目录拼接
- 你完全可以把文件重命名为任何名字，只要 `model3.json` 里的路径引用同步更新即可
