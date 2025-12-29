use crate::{
    config,
    logic::tr::tr,
    logic_cb,
    slint_generatedAppWindow::{AppWindow, SettingPushStream as UISettingPushStream},
};
use recorder::PushStreamConfig;
use slint::{ComponentHandle, SharedString};

pub fn init(ui: &AppWindow) {
    logic_cb!(push_stream_verify_setting, ui, setting);
}

fn push_stream_verify_setting(_ui: &AppWindow, config: UISettingPushStream) -> SharedString {
    if !config.server_addr.starts_with("rtmp://") {
        return tr("Invalid RTMP server url format. Should start with `rtmp://`").into();
    }

    SharedString::default()
}

impl From<config::PushStream> for PushStreamConfig {
    fn from(c: config::PushStream) -> PushStreamConfig {
        PushStreamConfig::new(c.server_addr, c.app, c.stream_key)
            .with_save_mp4(c.save_mp4)
            .with_query_params(c.query_params)
    }
}
