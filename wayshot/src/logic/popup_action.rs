use crate::{
    global_logic,
    slint_generatedAppWindow::{AppWindow, PopupActionSetting},
};
use slint::ComponentHandle;

pub fn init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    ui.global::<PopupActionSetting>()
        .on_action(move |action, _user_data| {
            let ui = ui_weak.unwrap();

            match action.as_str() {
                "remove-caches" => {
                    global_logic!(ui).invoke_remove_caches();
                }
                "toggle-control-enable-stats" => {
                    global_logic!(ui).invoke_toggle_control_enable_stats();
                }
                "toggle-control-enable-preview" => {
                    global_logic!(ui).invoke_toggle_control_enable_preview();
                }
                _ => (),
            }
        });
}
