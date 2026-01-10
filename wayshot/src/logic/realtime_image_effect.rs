use crate::{
    config, logic_cb,
    slint_generatedAppWindow::{AppWindow, RealTimeImageEffect as UIRealTimeImageEffect},
};
use image_effect::realtime::RealTimeImageEffect;
use once_cell::sync::Lazy;
use slint::ComponentHandle;
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

static REALTIME_IAMGE_EFFECT: Lazy<Arc<AtomicU8>> =
    Lazy::new(|| Arc::new(AtomicU8::new(RealTimeImageEffect::None.into())));

pub fn init(ui: &AppWindow) {
    inner_init();
    logic_cb!(realtime_image_effect_changed, ui, effect);
}

pub fn get_realtime_image_effect() -> Arc<AtomicU8> {
    REALTIME_IAMGE_EFFECT.clone()
}

fn inner_init() {
    let effect: RealTimeImageEffect = config::all().control.realtime_image_effect.into();
    REALTIME_IAMGE_EFFECT.store(effect.into(), Ordering::Relaxed);
}

fn realtime_image_effect_changed(_ui: &AppWindow, effect: UIRealTimeImageEffect) {
    REALTIME_IAMGE_EFFECT.store(
        <UIRealTimeImageEffect as Into<RealTimeImageEffect>>::into(effect).into(),
        Ordering::Relaxed,
    );
}
