use crate::{
    config, global_logic, global_store, logic_cb,
    slint_generatedAppWindow::{AppWindow, RealtimeImageEffect as UIRealtimeImageEffect},
};
use image_effect::realtime::RealtimeImageEffect;
use once_cell::sync::Lazy;
use slint::ComponentHandle;
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

static REALTIME_IAMGE_EFFECT: Lazy<Arc<AtomicU8>> =
    Lazy::new(|| Arc::new(AtomicU8::new(RealtimeImageEffect::None.into())));

pub fn init(ui: &AppWindow) {
    inner_init();

    logic_cb!(show_realtime_image_effect_dialog, ui, flag);
    logic_cb!(init_realtime_image_effect_dialog, ui);
    logic_cb!(realtime_image_effect_changed, ui, effect);
}

pub fn get_realtime_image_effect() -> Arc<AtomicU8> {
    REALTIME_IAMGE_EFFECT.clone()
}

fn inner_init() {
    let effect: RealtimeImageEffect = config::all().control.realtime_image_effect.into();
    REALTIME_IAMGE_EFFECT.store(effect.into(), Ordering::Relaxed);
}

fn realtime_image_effect_changed(ui: &AppWindow, effect: UIRealtimeImageEffect) {
    REALTIME_IAMGE_EFFECT.store(
        <UIRealtimeImageEffect as Into<RealtimeImageEffect>>::into(effect).into(),
        Ordering::Relaxed,
    );

    let mut setting = global_store!(ui).get_setting_control();
    setting.realtime_image_effect = effect;

    global_store!(ui).set_setting_control(setting.clone());
    global_logic!(ui).invoke_set_setting_control(setting);
}

fn show_realtime_image_effect_dialog(ui: &AppWindow, flag: bool) {
    global_store!(ui).set_is_show_realtime_image_effect_dialog(flag);
}

fn init_realtime_image_effect_dialog(_ui: &AppWindow) -> UIRealtimeImageEffect {
    config::all().control.realtime_image_effect
}
