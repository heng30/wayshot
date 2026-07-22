# 下面是全局规则。需要严格遵守：

- 如果只修改了`Slint`文件，使用`make slint-viewer`进行验证

- 如果只修改了`lib`目录下的库，应该使用`cargo check -p lib`进行验证

- 修改了`wayshot`目录中的`Rust`文件，使用`make check`进行验证

- 如果是`Slint`中定义的类型需要转换到`wayshot`库中的类型，使用SlintFromConvert宏

- `crate::impl_c_like_enum_convert!` 用于将`Slint`中的`Enum`类型转换到`Rust`中的类型。避免手动实现`From` trait

- `crate::impl_slint_enum_serde!` 用于给`Slint`中的`Enum`类型实现序列化和反序列化。避免手动实现序列化和方序列化trait

- 翻译那些使用了`async_toast_xxx`和`toast_xxx!`的函数中的常量字符串。使用tr函数进行翻译，并且在 `wayshot/src/logic/tr.rs` 中添加翻译。

- 添加一个Segment滤镜流程：
    - 在 `lib/video-editor/src/filters/` 中添加对应类型的滤镜代码。优先使用rayon库并行版本滤镜。
        - 注意：给合适的参数，添加关键帧
        - 注意：像素为单位的参数，应该使用`scale_pixel_for_height`进行转换，保证大小在不同分辨率都有一样的视觉效果。
        - 注意：滤镜名称如果是多个单词的，每个单词间使用空格分开，不要使用如`_`连接
    - 在 `lib/video-editor/src/project/filters.rs` 添加保存滤镜到项目代码
    - 在 `lib/mcp-server/src/service/filter.rs` 中添加滤镜对应的mcp工具
    - 在 `wayshot/src/logic/video_editor/filters/keyframe.rs` 添加关键帧代码
    - 在 `wayshot/src/logic/video_editor/filters/filter.rs` 添加滤镜代码
    - 在 `wayshot/src/logic/video_editor/conversion.rs` 添加类型转换实现
    - 在 `wayshot/src/logic/video_editor/filters/conversion.rs` 添加类型转换实现
    - 在 `wayshot/ui/panel/desktop/video-editor/filter.slint` 添加不同类型的滤镜配置定义和回调函数
    - 在 `wayshot/ui/panel/desktop/video-editor/right-panel/filter` 添加不同类型的滤镜配置文件
    - 只有需要在 `./wayshot/ui/panel/desktop/video-editor/preview` 中添加 `preview-xxx-layer.slint` 的情况下，才需要在 `./wayshot/ui/store.slint` 中的`VideoEditorLayerImage`变量中，添加滤镜配置
    - 如果是视频滤镜，同时需要在 `wayshot/ui/panel/desktop/video-editor/right-panel/filter/image.slint` 中添加滤镜调用实现
    - 如果滤镜参数是有固定范围的，如：0~1。实现自定义的Slider组件。参考 `./wayshot/ui/panel/desktop/video-editor/right-panel/filter/video/vignette.slint` 的实现。
    - 运行`make tr`获取需要翻译的为文本，并且翻译到 `./wayshot/src/logic/tr.rs`。将滤镜名称也翻译到 `./wayshot/src/logic/tr.rs`

- 添加一个全局滤镜流程：
    - 在 `lib/video-editor/src/filters/global/` 中添加对应类型的滤镜代码。
        - 注意：没有明确说明，不需要添加关键帧
        - 注意：像素为单位的参数，应该使用`scale_pixel_for_height`进行转换，保证大小在不同分辨率都有一样的视觉效果。
    - 在 `lib/video-editor/src/project/filters.rs` 中添加保存滤镜到项目代码
    - 在 `wayshot/src/db.rs` 中添加需要保存滤镜信息到数据库代码
    - 在 `wayshot/src/logic/video_editor/filters/global.rs` 中实现全局滤镜保存相关代码
    - 在 `wayshot/src/logic/video_editor/filters/global/` 中实现全局滤镜相关代码
    - 在 `wayshot/ui/panel/desktop/video-editor/tools/global-filter/` 中实现全局滤镜定义，回调和界面逻辑
        - 注意： UI面板中最后不用使好`wayshot/ui/base/slider.slint`组件，很可能会出现循环引用问题

- 添加一个工具的流程
    - 在 `wayshot/ui/store.slint` 中添加配置定义
    - 在 `wayshot/src/db.rs` 中添加配置定义
    - 在 `wayshot/src/logic/video_editor/project.rs` 中添加数据库条目id
    - 在 `wayshot/ui/logic.slint` 中添加回调函数
    - 在 `wayshot/ui/panel/desktop/video-editor/tools/` 中添加tool面板
    - 在 `wayshot/ui/panel/desktop/video-editor/top-moving-dialog.slint` 中使用tool面板
    - 在 `wayshot/ui/panel/desktop/video-editor/header.slint` 中添加tool面板入口位置
    - 在 `wayshot/ui/panel/desktop/video-editor/shortcut.slint` 中添加tool面板快捷键定义
    - 在 `wayshot/src/logic/video_editor.rs` 中添加init函数
    - 在 `wayshot/src/logic/video_editor` 目录中添加Rust 回调函数实现
    - 在 `wayshot/ui/panel/desktop/video-editor/help-dialog.slint` 中添加快捷键
