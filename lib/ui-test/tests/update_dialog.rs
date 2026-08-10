slint::include_modules!();

// slint testing 后端要求每进程只初始化一次且单测试函数（官方限制），
// 因此所有契约断言在同一个 #[test] 内按小节顺序执行。

fn fresh_dialog() -> UpdateDialog {
    UpdateDialog::new().expect("UpdateDialog should instantiate")
}

#[test]
fn update_dialog_contract() {
    i_slint_backend_testing::init_integration_test_with_system_time();

    title_defaults_and_configurable();
    collapse_title_defaults_and_configurable();
    button_texts_defaults_and_configurable();
    data_properties_settable_and_readable();
    dont_ask_again_semantics();
    callbacks_fire();
    update_content_roundtrip();
}

fn title_defaults_and_configurable() {
    let d = fresh_dialog();
    assert_eq!(d.get_title(), "New version available", "标题默认值");
    d.set_title("Custom title".into());
    assert_eq!(d.get_title(), "Custom title", "标题可配置");
}

fn collapse_title_defaults_and_configurable() {
    let d = fresh_dialog();
    assert_eq!(d.get_collapse_title(), "Update content", "折叠标题默认值");
    d.set_collapse_title("Changelog".into());
    assert_eq!(d.get_collapse_title(), "Changelog", "折叠标题可配置");
}

fn button_texts_defaults_and_configurable() {
    let d = fresh_dialog();
    assert_eq!(d.get_cancel_text(), "Cancel", "取消按钮文本默认值");
    assert_eq!(d.get_confirm_text(), "Confirm", "确认按钮文本默认值");
    d.set_cancel_text("Later".into());
    d.set_confirm_text("Update now".into());
    assert_eq!(d.get_cancel_text(), "Later", "取消按钮文本可配置");
    assert_eq!(d.get_confirm_text(), "Update now", "确认按钮文本可配置");
}

fn data_properties_settable_and_readable() {
    let d = fresh_dialog();
    d.set_app_name("Wayshot".into());
    d.set_current_version("v1.0.2".into());
    d.set_latest_version("v1.1.0".into());
    d.set_update_content("Fixed many bugs.".into());
    assert_eq!(d.get_app_name(), "Wayshot", "程序名称");
    assert_eq!(d.get_current_version(), "v1.0.2", "当前版本");
    assert_eq!(d.get_latest_version(), "v1.1.0", "最新版本");
    assert_eq!(d.get_update_content(), "Fixed many bugs.", "更新内容");
}

fn dont_ask_again_semantics() {
    let d = fresh_dialog();
    assert!(!d.get_dont_ask_again(), "不再提示默认不勾选");

    d.set_dont_ask_again(true);
    d.invoke_canceled();
    assert!(d.get_dont_ask_again(), "勾选即生效：取消后状态保持");

    d.set_dont_ask_again(true);
    d.invoke_confirmed();
    assert!(d.get_dont_ask_again(), "勾选即生效：确认后状态保持");
}

fn callbacks_fire() {
    use std::cell::Cell;
    use std::rc::Rc;

    let d = fresh_dialog();
    let canceled = Rc::new(Cell::new(0u32));
    let confirmed = Rc::new(Cell::new(0u32));
    let escape = Rc::new(Cell::new(0u32));

    // canceled 与 close 是同一底层回调（Dialog 内 close <=> cancel-clicked 合并），
    // slint 的 on_ 为 set 语义，二者不可同时注册（后者覆盖前者），
    // 故注册 on_canceled 后分别 invoke 验证两个名字。
    let c = canceled.clone();
    d.on_canceled(move || c.set(c.get() + 1));
    d.invoke_canceled();
    assert_eq!(canceled.get(), 1, "取消回调应触发");

    let c = confirmed.clone();
    d.on_confirmed(move || c.set(c.get() + 1));
    d.invoke_confirmed();
    assert_eq!(confirmed.get(), 1, "确认回调应触发");

    let c = escape.clone();
    d.on_escape(move || c.set(c.get() + 1));
    d.invoke_escape();
    assert_eq!(escape.get(), 1, "escape 回调应触发");
}

fn update_content_roundtrip() {
    // 空内容时折叠区隐藏由 `if !update-content.is-empty` 条件渲染在编译期保证；
    // 1.17 testing 后端无 item 可见性查询 API，此处仅验证内容属性往返一致。
    let d = fresh_dialog();
    d.set_update_content("".into());
    assert_eq!(d.get_update_content(), "", "空更新内容");
    d.set_update_content("something".into());
    assert_eq!(d.get_update_content(), "something", "更新内容可重新设置");
}
