use crate::{
    global_logic,
    slint_generatedAppWindow::{AppWindow, PopupActionSetting},
};
use slint::ComponentHandle;

pub fn init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    ui.global::<PopupActionSetting>()
        .on_action(move |action, user_data| {
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
                "show-realtime-image-effect-dialog" => {
                    global_logic!(ui).invoke_show_realtime_image_effect_dialog(true);
                }
                "transcribe-subtitles-correction" => {
                    global_logic!(ui).invoke_transcribe_subtitles_correction();
                }
                "transcribe-subtitles-remove-correction" => {
                    global_logic!(ui).invoke_transcribe_subtitles_remove_correction();
                }
                "transcribe-subtitles-adjust-overlap-timestamp" => {
                    global_logic!(ui).invoke_transcribe_subtitles_adjust_overlap_timestamp();
                }
                "transcribe-subtitles-to-lowercase" => {
                    global_logic!(ui).invoke_transcribe_subtitles_to_lowercase();
                }
                "transcribe-subtitles-to-simple-chinese" => {
                    global_logic!(ui).invoke_transcribe_subtitles_to_simple_chinese();
                }
                "transcribe-subtitles-remove-separator" => {
                    global_logic!(ui).invoke_transcribe_subtitles_remove_separator();
                }
                "transcribe-subtitle-split" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_transcribe_subtitle_split(index);
                }
                "transcribe-subtitle-merge-above" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_transcribe_subtitle_merge_above(index);
                }
                "transcribe-subtitle-insert-above" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_transcribe_subtitle_insert_above(index);
                }
                "transcribe-subtitle-insert-below" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_transcribe_subtitle_insert_below(index);
                }
                "transcribe-subtitle-remove" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_transcribe_subtitle_remove(index);
                }
                _ => log::warn!("Unknown popup action: {action}"),
            }
        });
}
