use crate::{
    global_logic, global_util, logic::video_editor::media_list_common::MediaListContext,
    logic::video_editor::media_list_common::remove_folder_confirmed,
    slint_generatedAppWindow::AppWindow,
};
use slint::ComponentHandle;

pub fn init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    global_util!(ui).on_handle_confirm_dialog(move |handle_type, user_data| {
        let ui = ui_weak.unwrap();

        match handle_type.as_str() {
            "remove-caches" => {
                global_logic!(ui).invoke_remove_caches();
            }
            "uninstall" => {
                global_logic!(ui).invoke_uninstall();
            }
            "close-window" => {
                global_util!(ui).invoke_close_window();
            }
            "remove-no-found-histories" => {
                global_logic!(ui).invoke_remove_no_found_histories();
            }
            "remove-all-histories" => {
                global_logic!(ui).invoke_remove_all_histories();
            }
            "remove-history" => {
                let index = user_data.parse::<i32>().unwrap_or(-1);
                global_logic!(ui).invoke_remove_history(index);
            }
            "video-editor-transcribe-subtitles-remove-all" => {
                global_logic!(ui).invoke_video_editor_transcribe_subtitles_remove_all();
            }
            "video-editor-transcribe-subtitles-clear" => {
                global_logic!(ui).invoke_video_editor_transcribe_subtitles_clear();
            }
            "video-editor-playlist-remove-all" => {
                global_logic!(ui).invoke_video_editor_playlist_remove_all();
            }
            "video-editor-library-remove-all" => {
                global_logic!(ui).invoke_video_editor_library_remove_all();
            }
            "video-editor-playlist-remove-selections" => {
                global_logic!(ui).invoke_video_editor_playlist_remove_selections();
            }
            "video-editor-library-remove-selections" => {
                global_logic!(ui).invoke_video_editor_library_remove_selections();
            }
            "video-editor-playlist-remove-folder" => {
                remove_folder_confirmed(MediaListContext::Playlist, &ui, &user_data);
            }
            "video-editor-library-remove-folder" => {
                remove_folder_confirmed(MediaListContext::Library, &ui, &user_data);
            }
            "video-editor-tts-remove-all-speech" => {
                global_logic!(ui).invoke_video_editor_tts_remove_all_speech();
            }
            "video-editor-tts-remove-all-entries" => {
                global_logic!(ui).invoke_video_editor_tts_remove_all_entries();
            }
            "video-editor-global-filter-progress-bar-remove-all-items" => {
                global_logic!(ui).invoke_video_editor_global_filter_progress_bar_remove_all_items();
            }
            "video-editor-global-filter-timer-remove-all-items" => {
                global_logic!(ui).invoke_video_editor_global_filter_timer_remove_all_items();
            }
            "video-editor-global-filter-danmaku-remove-all-segments" => {
                global_logic!(ui).invoke_video_editor_global_filter_danmaku_remove_all_segments();
            }
            "video-editor-global-filter-danmaku-remove-all-items" => {
                let index = user_data.parse::<i32>().unwrap_or(-1);
                global_logic!(ui).invoke_video_editor_global_filter_danmaku_remove_all_items(index);
            }
            "video-editor-chapter-summary-remove-all" => {
                global_logic!(ui).invoke_video_editor_chapter_summary_remove_all();
            }
            "video-editor-bookmark-remove-all" => {
                global_logic!(ui).invoke_video_editor_bookmark_remove_all();
            }
            "video-editor-similar-video-segment-remove-all" => {
                global_logic!(ui).invoke_video_editor_similar_video_segment_remove_all();
            }
            "video-editor-dedup-photos-remove-all" => {
                global_logic!(ui).invoke_video_editor_dedup_photos_remove_all();
            }
            "video-editor-smart-mix-start" => {
                global_logic!(ui).invoke_video_editor_smart_mix_start();
            }
            "video-editor-smart-mix-remove-all" => {
                global_logic!(ui).invoke_video_editor_smart_mix_remove_all();
            }
            _ => log::warn!("Unknown confirm handle type: {handle_type}"),
        }
    });
}
