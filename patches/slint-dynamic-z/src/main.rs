slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let app = Example::new()?;
    app.show()?;

    // 自动验证模式：ZTEST_AUTO=1 时，1.5s 后切换 red_on_top，4s 后退出。
    // 期间打印两个矩形的运行时 z 值，验证动态 z 绑定生效。
    // 注意：timer 必须存活到 run_event_loop 结束（main 返回），否则会被提前 drop。
    let mut _auto_timers: Vec<slint::Timer> = Vec::new();
    if std::env::var("ZTEST_AUTO").is_ok() {
        let weak = app.as_weak();
        let toggle = slint::Timer::default();
        toggle.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(1500),
            move || {
                let app = weak.unwrap();
                println!(
                    "BEFORE toggle: red_on_top={} red_z={} blue_z={}",
                    app.get_red_on_top(),
                    app.get_red_z(),
                    app.get_blue_z(),
                );
                app.set_red_on_top(true);
                println!(
                    "AFTER  toggle: red_on_top={} red_z={} blue_z={}",
                    app.get_red_on_top(),
                    app.get_red_z(),
                    app.get_blue_z(),
                );
            },
        );
        _auto_timers.push(toggle);

        let quit = slint::Timer::default();
        quit.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(4000),
            move || {
                slint::quit_event_loop().unwrap();
            },
        );
        _auto_timers.push(quit);
    }

    slint::run_event_loop()
}
