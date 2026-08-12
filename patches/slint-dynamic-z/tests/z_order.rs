slint::include_modules!();

// 渲染层验证：软件渲染器 + testing 后端渲染 Example 组件，
// 检查重叠区域像素颜色随 red_on_top（动态 z）变化。

use slint::platform::PlatformError;

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

fn pixel_at(buf: &slint::SharedPixelBuffer<slint::Rgba8Pixel>, x: u32, y: u32) -> (u8, u8, u8) {
    let p = buf.as_slice()[(y * buf.width() + x) as usize];
    (p.r, p.g, p.b)
}

fn is_red(c: (u8, u8, u8)) -> bool {
    c.0 > 180 && c.1 < 100 && c.2 < 100
}

fn is_blue(c: (u8, u8, u8)) -> bool {
    c.0 < 100 && c.1 < 100 && c.2 > 150
}

#[test]
fn dynamic_z_changes_paint_order() -> Result<(), PlatformError> {
    init_backend();

    let app = Example::new()?;
    app.show()?;
    app.window().request_redraw();

    // 初始 red_on_top=false：red z=0（下），blue z=10（上）
    // 重叠区中心 (180,155) 应该是蓝色
    let snap1 = app.window().take_snapshot()?;
    assert_eq!((snap1.width(), snap1.height()), (400, 300));
    let overlap1 = pixel_at(&snap1, 180, 155);
    assert!(is_blue(overlap1), "overlap should be BLUE (blue z=10 on top), got {overlap1:?}");
    // 非重叠区仍各自可见
    assert!(is_red(pixel_at(&snap1, 60, 60)), "red-only region should be RED");
    assert!(is_blue(pixel_at(&snap1, 280, 240)), "blue-only region should be BLUE");

    // 切换 red_on_top=true：red z=10（上），blue z=0（下）
    app.set_red_on_top(true);
    println!("before snap2: red_z={} blue_z={}", app.get_red_z(), app.get_blue_z());
    app.window().request_redraw();
    let snap2 = app.window().take_snapshot()?;
    println!("after  snap2: red_z={} blue_z={}", app.get_red_z(), app.get_blue_z());
    let overlap2 = pixel_at(&snap2, 180, 155);
    assert!(is_red(overlap2), "overlap should be RED (red z=10 on top), got {overlap2:?}");

    println!("PASS: overlap {:?} -> {:?} after toggling red_on_top", overlap1, overlap2);
    Ok(())
}

/// 静态 z 兼容性：编译期常量 z 仍按静态排序渲染
#[test]
fn static_z_still_works() -> Result<(), PlatformError> {
    init_backend();

    let app = Example::new()?;
    app.show()?;
    app.window().request_redraw();
    let snap = app.window().take_snapshot()?;
    // 与上一个测试初始状态一致：蓝色在上
    assert!(is_blue(pixel_at(&snap, 180, 155)));
    Ok(())
}

/// 带透明度（编译器合成 Opacity 包装元素）的元素也参与动态 z 排序：
/// 包装元素的 z 必须与面板本体同步，否则渲染层级不会变化。
#[test]
fn dynamic_z_with_opacity_wrapper() -> Result<(), PlatformError> {
    init_backend();

    let app = Example::new()?;
    app.show()?;
    app.window().request_redraw();

    // green_bar 区域 (160,20)-(340,90) 与 red_rect 重叠区中心 (200,50)。
    // red_on_top=false：red z=0 > green z=-30 → red 在上 → 纯红
    let snap1 = app.window().take_snapshot()?;
    let p1 = pixel_at(&snap1, 200, 50);
    assert!(is_red(p1), "red z=0 should be above glass z=-30, got {p1:?}");

    // red_on_top=true：red z=10 < green z=30 → green 在上 → 半透明混合色（非纯红）
    app.set_red_on_top(true);
    app.window().request_redraw();
    let snap2 = app.window().take_snapshot()?;
    let p2 = pixel_at(&snap2, 200, 50);
    assert!(!is_red(p2), "glass z=30 should cover red z=10, got {p2:?}");

    println!("PASS: opacity+wrapped z ordering {:?} -> {:?}", p1, p2);
    Ok(())
}
