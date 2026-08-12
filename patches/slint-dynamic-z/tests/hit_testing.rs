slint::include_modules!();

// 输入命中顺序验证：点击重叠区域时，命中的应是视觉上层的方块
// （hit-testing 随动态 z 排序），且点击哪个方块哪个置顶。

use slint::platform::{PointerEventButton, WindowEvent};
use slint::LogicalPosition;

fn init_backend() {
    slint::platform::set_platform(Box::new(
        i_slint_backend_testing::TestingBackend::new(
            i_slint_backend_testing::TestingBackendOptions {
                mock_time: true,
                threading: false,
                renderer_name: Some("software".into()),
            },
        ),
    ))
    .expect("platform already initialized");
}

fn click(app: &Example, x: f32, y: f32) {
    let pos = LogicalPosition::new(x, y);
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: pos,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: pos,
        button: PointerEventButton::Left,
    });
}

/// 蓝色在上时（red_on_top=false），点击重叠区应命中蓝色块 → red_on_top 保持 false
/// 红色在上时（red_on_top=true），点击重叠区应命中红色块 → red_on_top 保持 true
#[test]
fn hit_testing_follows_dynamic_z() {
    init_backend();

    let app = Example::new().unwrap();
    app.show().unwrap();

    // 初始：蓝色 z=10 在上。重叠区中心 (180,155) 命中蓝色 → 蓝色置顶（保持 false）
    click(&app, 180.0, 155.0);
    assert!(
        !app.get_red_on_top(),
        "click on overlap with BLUE on top should hit BLUE (red_on_top stays false), got red_on_top={}",
        app.get_red_on_top()
    );

    // 红色置顶
    app.set_red_on_top(true);

    // 红色 z=10 在上。重叠区点击应命中红色 → 红色保持置顶
    click(&app, 180.0, 155.0);
    assert!(
        app.get_red_on_top(),
        "click on overlap with RED on top should hit RED (red_on_top stays true), got red_on_top={}",
        app.get_red_on_top()
    );

    // 点击红色块独占区（红色上方区域）→ 红色置顶
    click(&app, 60.0, 60.0);
    assert!(app.get_red_on_top());

    // 点击蓝色块独占区 → 蓝色置顶
    click(&app, 280.0, 240.0);
    assert!(!app.get_red_on_top(), "click on BLUE-only region should raise BLUE");
}
