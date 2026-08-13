use crate::{
    global_logic, global_store, global_ve_filter,
    logic::tr::tr,
    slint_generatedAppWindow::{
        AppWindow, ConfirmDialogSetting, PopupActionSetting, SelectedSegmentIndex,
    },
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
                "video-editor-transcribe-subtitles-correction" => {
                    global_logic!(ui).invoke_video_editor_transcribe_subtitles_correction();
                }
                "video-editor-transcribe-subtitles-correction-with-reference" => {
                    global_store!(ui).set_video_editor_transcribe_is_show_original_text_dialog(true);
                }
                "video-editor-transcribe-subtitles-accept-correction" => {
                    global_logic!(ui).invoke_video_editor_transcribe_subtitles_accept_correction();
                }
                "video-editor-transcribe-subtitles-remove-correction" => {
                    global_logic!(ui).invoke_video_editor_transcribe_subtitles_remove_correction();
                }
                "video-editor-transcribe-subtitles-adjust-overlap-timestamp" => {
                    global_logic!(ui).invoke_video_editor_transcribe_subtitles_adjust_overlap_timestamp();
                }
                "video-editor-transcribe-subtitles-to-lowercase" => {
                    global_logic!(ui).invoke_video_editor_transcribe_subtitles_to_lowercase();
                }
                "video-editor-transcribe-subtitles-to-simple-chinese" => {
                    global_logic!(ui).invoke_video_editor_transcribe_subtitles_to_simple_chinese();
                }
                "video-editor-transcribe-subtitles-to-primitive-numbers" => {
                    global_logic!(ui).invoke_video_editor_transcribe_subtitles_to_primitive_numbers();
                }
                "video-editor-transcribe-subtitles-remove-separator" => {
                    global_logic!(ui).invoke_video_editor_transcribe_subtitles_remove_separator();
                }
                "video-editor-transcribe-subtitle-split" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_transcribe_subtitle_split(index);
                }
                "video-editor-transcribe-subtitle-merge-above" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_transcribe_subtitle_merge_above(index);
                }
                "video-editor-transcribe-subtitle-insert-above" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_transcribe_subtitle_insert_above(index);
                }
                "video-editor-transcribe-subtitle-insert-below" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_transcribe_subtitle_insert_below(index);
                }
                "video-editor-transcribe-subtitle-remove" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_transcribe_subtitle_remove(index);
                }
                "video-editor-new-project" => {
                    global_logic!(ui).invoke_video_editor_new_project();
                }
                "video-editor-open-project" => {
                    global_logic!(ui).invoke_video_editor_open_project();
                }
                "video-editor-show-recent-dialog" => {
                    global_logic!(ui).invoke_video_editor_show_recent_dialog();
                }
                "video-editor-show-metadata-dialog" => {
                    global_logic!(ui).invoke_video_editor_show_metadata_dialog();
                }
                "video-editor-preview-show-dialog" => {
                    global_logic!(ui).invoke_video_editor_preview_show_dialog();
                }
                "video-editor-show-preference-setting-dialog" => {
                    global_logic!(ui).invoke_video_editor_show_preference_setting_dialog();
                }
                "video-editor-save-project" => {
                    global_logic!(ui).invoke_video_editor_save_project();
                }
                "video-editor-save-as-project" => {
                    global_logic!(ui).invoke_video_editor_save_as_project();
                }
                "video-editor-backup-project" => {
                    global_logic!(ui).invoke_video_editor_backup_project();
                }
                "video-editor-close-project" => {
                    global_logic!(ui).invoke_video_editor_close_project();
                }
                "video-editor-quit" => {
                    global_logic!(ui).invoke_video_editor_quit();
                }
                "video-editor-undo" => {
                    global_logic!(ui).invoke_video_editor_undo();
                }
                "video-editor-redo" => {
                    global_logic!(ui).invoke_video_editor_redo();
                }
                "video-editor-cut" => {
                    global_logic!(ui).invoke_video_editor_cut();
                }
                "video-editor-copy" => {
                    global_logic!(ui).invoke_video_editor_copy();
                }
                "video-editor-paste" => {
                    global_logic!(ui).invoke_video_editor_paste();
                }
                "video-editor-playlist-select-all" => {
                    global_logic!(ui).invoke_video_editor_playlist_toggle_select_all(true);
                }
                "video-editor-library-select-all" => {
                    global_logic!(ui).invoke_video_editor_library_toggle_select_all(true);
                }
                "video-editor-playlist-item-add-to-track" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_playlist_item_add_to_track(index);
                }
                "video-editor-playlist-item-add-to-track-end" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_playlist_item_add_to_track_end(index);
                }
                "video-editor-playlist-item-remove" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_playlist_item_remove(index);
                }
                "video-editor-playlist-item-preview" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_playlist_item_preview(index);
                }
                "video-editor-playlist-enter-edit-mode" => {
                    global_logic!(ui).invoke_video_editor_playlist_enter_edit_mode();
                }
                "video-editor-playlist-show-remove-all-confirm-dialog" => {
                    global_logic!(ui).invoke_video_editor_playlist_show_remove_all_confirm_dialog();
                }
                "video-editor-playlist-item-copy-path" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_playlist_item_copy_path(index);
                }
                "video-editor-playlist-item-open-directory" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_playlist_item_open_directory(index);
                }
                "video-editor-library-item-add-to-track" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_library_item_add_to_track(index);
                }
                "video-editor-library-item-add-to-track-end" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_library_item_add_to_track_end(index);
                }
                "video-editor-library-item-remove" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_library_item_remove(index);
                }
                "video-editor-library-item-preview" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_library_item_preview(index);
                }
                "video-editor-library-enter-edit-mode" => {
                    global_logic!(ui).invoke_video_editor_library_enter_edit_mode();
                }
                "video-editor-library-show-remove-all-confirm-dialog" => {
                    global_logic!(ui).invoke_video_editor_library_show_remove_all_confirm_dialog();
                }
                "video-editor-library-rename-folder" => {
                    global_logic!(ui).invoke_video_editor_library_rename_folder(user_data.clone().into());
                }
                "video-editor-library-remove-folder" => {
                    global_logic!(ui).invoke_video_editor_library_remove_folder(user_data.clone().into());
                }
                "video-editor-playlist-remove-folder" => {
                    global_logic!(ui).invoke_video_editor_playlist_remove_folder(user_data.clone().into());
                }
                "video-editor-playlist-rename-folder" => {
                    global_logic!(ui).invoke_video_editor_playlist_rename_folder(user_data.clone().into());
                }
                "video-editor-library-sync-folder" => {
                    global_logic!(ui).invoke_video_editor_library_sync_folder(user_data.clone().into());
                }
                "video-editor-library-refresh" => {
                    global_logic!(ui).invoke_video_editor_library_refresh();
                }
                "video-editor-playlist-sync-folder" => {
                    global_logic!(ui).invoke_video_editor_playlist_sync_folder(user_data.clone().into());
                }
                "video-editor-playlist-refresh" => {
                    global_logic!(ui).invoke_video_editor_playlist_refresh();
                }
                "video-editor-library-item-copy-path" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_library_item_copy_path(index);
                }
                "video-editor-library-item-open-directory" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_library_item_open_directory(index);
                }
                "video-editor-library-toggle-mark" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_library_toggle_mark(index);
                }
                "video-editor-export-queue-cancel" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_export_queue_cancel(index);
                }
                "video-editor-export-queue-remove" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_export_queue_remove(index);
                }
                "video-editor-add-empty-video-track" => {
                    global_logic!(ui).invoke_video_editor_add_empty_video_track();
                }
                "video-editor-add-empty-audio-track" => {
                    global_logic!(ui).invoke_video_editor_add_empty_audio_track();
                }
                "video-editor-add-empty-subtitle-track" => {
                    global_logic!(ui).invoke_video_editor_add_empty_subtitle_track();
                }
                "video-editor-add-empty-image-track" => {
                    global_logic!(ui).invoke_video_editor_add_empty_image_track();
                }
                "video-editor-add-empty-text-track" => {
                    global_logic!(ui).invoke_video_editor_add_empty_text_track();
                }
                "video-editor-add-text-segment" => {
                    global_logic!(ui).invoke_video_editor_add_text_segment();
                }
                "video-editor-track-move-up" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_track_move_up(index);
                }
                "video-editor-track-move-down" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_track_move_down(index);
                }
                "video-editor-track-move-to-top" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_track_move_to_top(index);
                }
                "video-editor-track-move-to-bottom" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_track_move_to_bottom(index);
                }
                "video-editor-insert-video-track" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_insert_video_track(index);
                }
                "video-editor-insert-audio-track" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_insert_audio_track(index);
                }
                "video-editor-insert-subtitle-track" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_insert_subtitle_track(index);
                }
                "video-editor-insert-image-track" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_insert_image_track(index);
                }
                "video-editor-insert-text-track" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_insert_text_track(index);
                }
                "video-editor-paste-filter-to-track" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_paste_filter_to_track(index);
                }
                "video-editor-remove-all-filters-from-track" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_remove_all_filters_from_track(index);
                }
                "video-editor-detach-audio-track" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_detach_audio_track(index);
                }
                "video-editor-detach-subtitle-track" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_detach_subtitle_track(index);
                }
                "video-editor-track-stretch-to-end" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_track_stretch_to_end(index);
                }
                "video-editor-track-remove-all-gap" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_logic!(ui).invoke_video_editor_track_remove_all_gap(index);
                }

                "video-editor-segment-detach-audio" => match parse_segment_user_data(&user_data) {
                    Ok((track_index, index)) => {
                        let index = SelectedSegmentIndex {
                            index,
                            track_index,
                            ..Default::default()
                        };
                        global_logic!(ui).invoke_video_editor_segment_detach_audio(index);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-segment-detach-subtitle" => match parse_segment_user_data(&user_data)
                {
                    Ok((track_index, index)) => {
                        let index = SelectedSegmentIndex {
                            index,
                            track_index,
                            ..Default::default()
                        };
                        global_logic!(ui).invoke_video_editor_segment_detach_subtitle(index);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-segment-remove-gap" => match parse_segment_user_data(&user_data) {
                    Ok((track_index, index)) => {
                        let index = SelectedSegmentIndex {
                            index,
                            track_index,
                            ..Default::default()
                        };
                        global_logic!(ui).invoke_video_editor_segment_remove_gap(index);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-segment-remove-left-gap" => match parse_segment_user_data(&user_data)
                {
                    Ok((track_index, index)) => {
                        let index = SelectedSegmentIndex {
                            index,
                            track_index,
                            ..Default::default()
                        };
                        global_logic!(ui).invoke_video_editor_segment_remove_left_gap(index);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-segment-remove-right-gap" => {
                    match parse_segment_user_data(&user_data) {
                        Ok((track_index, index)) => {
                            let index = SelectedSegmentIndex {
                                index,
                                track_index,
                                ..Default::default()
                            };
                            global_logic!(ui).invoke_video_editor_segment_remove_right_gap(index);
                        }
                        Err(e) => log::warn!("{e}"),
                    }
                }
                "video-editor-segment-resize-to-playhead" => {
                    match parse_segment_user_data(&user_data) {
                        Ok((track_index, index)) => {
                            let index = SelectedSegmentIndex {
                                index,
                                track_index,
                                ..Default::default()
                            };
                            global_logic!(ui).invoke_video_editor_segment_resize_to_playhead(index);
                        }
                        Err(e) => log::warn!("{e}"),
                    }
                }
                "video-editor-segment-resize-to-previous-segment" => {
                    match parse_segment_user_data(&user_data) {
                        Ok((track_index, index)) => {
                            let index = SelectedSegmentIndex {
                                index,
                                track_index,
                                ..Default::default()
                            };
                            global_logic!(ui).invoke_video_editor_segment_resize_to_previous_segment(index);
                        }
                        Err(e) => log::warn!("{e}"),
                    }
                }
                "video-editor-segment-resize-to-next-segment" => {
                    match parse_segment_user_data(&user_data) {
                        Ok((track_index, index)) => {
                            let index = SelectedSegmentIndex {
                                index,
                                track_index,
                                ..Default::default()
                            };
                            global_logic!(ui).invoke_video_editor_segment_resize_to_next_segment(index);
                        }
                        Err(e) => log::warn!("{e}"),
                    }
                }
                "video-editor-segment-remove-all-filters" => {
                    match parse_segment_user_data(&user_data) {
                        Ok((track_index, segment_index)) => {
                            global_logic!(ui).invoke_video_editor_segment_remove_all_filters(track_index, segment_index);
                        }
                        Err(e) => log::warn!("{e}"),
                    }

                }
                "video-editor-segment-remove-all-keyframes" => {
                    match parse_segment_user_data(&user_data) {
                        Ok((track_index, segment_index)) => {
                            global_logic!(ui).invoke_video_editor_remove_all_segment_keyframes(track_index, segment_index);
                        }
                        Err(e) => log::warn!("{e}"),
                    }
                }
                "video-editor-segment-export-gif" => match parse_segment_user_data(&user_data) {
                    Ok((track_index, index)) => {
                        let index = SelectedSegmentIndex {
                            index,
                            track_index,
                            ..Default::default()
                        };
                        global_logic!(ui).invoke_video_editor_segment_export_gif(index);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-segment-export-selected-mp4" => {
                    global_logic!(ui).invoke_video_editor_segment_export_selected_mp4();
                }
                "video-editor-segment-extract-frames" => match parse_segment_user_data(&user_data) {
                    Ok((track_index, index)) => {
                        let index = SelectedSegmentIndex {
                            index,
                            track_index,
                            ..Default::default()
                        };
                        global_logic!(ui).invoke_video_editor_segment_extract_frames(index);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-segment-export-audio" => match parse_segment_user_data(&user_data) {
                    Ok((track_index, index)) => {
                        let index = SelectedSegmentIndex {
                            index,
                            track_index,
                            ..Default::default()
                        };
                        global_logic!(ui).invoke_video_editor_segment_export_audio(index);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-segment-export-long-screenshot" => {
                    match parse_segment_user_data(&user_data) {
                        Ok((track_index, index)) => {
                            global_logic!(ui)
                                .invoke_video_editor_long_screenshot_open(track_index, index);
                        }
                        Err(e) => log::warn!("{e}"),
                    }
                }
                "video-editor-segment-toggle-enable" => match parse_segment_user_data(&user_data) {
                    Ok((track_index, index)) => {
                        let index = SelectedSegmentIndex {
                            index,
                            track_index,
                            ..Default::default()
                        };
                        global_logic!(ui).invoke_video_editor_segment_toggle_enable(index);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-segment-toggle-audio" => match parse_segment_user_data(&user_data) {
                    Ok((track_index, index)) => {
                        let index = SelectedSegmentIndex {
                            index,
                            track_index,
                            ..Default::default()
                        };
                        global_logic!(ui).invoke_video_editor_segment_toggle_audio(index);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-segment-intelligent-voice-segmentation" => match parse_segment_user_data(&user_data) {
                    Ok((track_index, index)) => {
                        let index = SelectedSegmentIndex {
                            index,
                            track_index,
                            ..Default::default()
                        };
                        global_logic!(ui).invoke_video_editor_segment_intelligent_voice_segmentation(index);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-segment-intelligent-scene-segmentation" => match parse_segment_user_data(&user_data) {
                    Ok((track_index, index)) => {
                        global_store!(ui).set_video_editor_is_show_scene_detect_dialog(true);
                        global_store!(ui).set_video_editor_scene_detect_track_index(track_index);
                        global_store!(ui).set_video_editor_scene_detect_segment_index(index);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-segment-smart-voice-clip" => match parse_segment_user_data(&user_data) {
                    Ok((track_index, index)) => {
                        global_store!(ui).set_video_editor_smart_clip_segment_track_index(track_index as i32);
                        global_store!(ui).set_video_editor_smart_clip_segment_index(index as i32);
                        global_store!(ui).set_video_editor_smart_clip_is_show_dialog(true);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-smart-clip-ai-split" => {
                    global_logic!(ui).invoke_video_editor_smart_clip_ai_split();
                }
                "video-editor-smart-clip-ai-split-with-reference" => {
                    global_store!(ui).set_video_editor_smart_clip_is_show_reference_dialog(true);
                }
                "video-editor-smart-clip-select-all" => {
                    global_logic!(ui).invoke_video_editor_smart_clip_toggle_all(true);
                }
                "video-editor-smart-clip-deselect-all" => {
                    global_logic!(ui).invoke_video_editor_smart_clip_toggle_all(false);
                }
                "video-editor-smart-clip-remove-unkept" => {
                    global_logic!(ui).invoke_video_editor_smart_clip_remove_unkept();
                }
                "video-editor-smart-mix" => {
                    global_store!(ui).set_video_editor_smart_mix_is_show_dialog(true);
                }
                "video-editor-up-filter" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_ve_filter!(ui).invoke_up_filter(index);
                }
                "video-editor-down-filter" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_ve_filter!(ui).invoke_down_filter(index);
                }
                "video-editor-move-to-top-filter" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_ve_filter!(ui).invoke_move_to_top_filter(index);
                }
                "video-editor-move-to-bottom-filter" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_ve_filter!(ui).invoke_move_to_bottom_filter(index);
                }
                "video-editor-remove-filter" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_ve_filter!(ui).invoke_remove_filter(index);
                }
                "video-editor-copy-filter" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_ve_filter!(ui).invoke_copy_filter(index);
                }
                "video-editor-cut-filter" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_ve_filter!(ui).invoke_cut_filter(index);
                }
                "video-editor-paste-filter" => {
                    global_ve_filter!(ui).invoke_paste_filter();
                }
                "video-editor-refresh-filter-list" => {
                    global_ve_filter!(ui).invoke_refresh_filter_list();
                }
                "video-editor-preset-filter-up" => match parse_segment_user_data(&user_data) {
                    Ok((filter_type_index, index)) => {
                        let filter_type =
                            global_ve_filter!(ui).invoke_filter_type_from_int(filter_type_index);
                        global_ve_filter!(ui).invoke_preset_filter_up(index, filter_type);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-preset-filter-down" => match parse_segment_user_data(&user_data) {
                    Ok((filter_type_index, index)) => {
                        let filter_type =
                            global_ve_filter!(ui).invoke_filter_type_from_int(filter_type_index);
                        global_ve_filter!(ui).invoke_preset_filter_down(index, filter_type);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-preset-filter-move-to-top" => match parse_segment_user_data(&user_data) {
                    Ok((filter_type_index, index)) => {
                        let filter_type =
                            global_ve_filter!(ui).invoke_filter_type_from_int(filter_type_index);
                        global_ve_filter!(ui).invoke_preset_filter_move_to_top(index, filter_type);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-preset-filter-move-to-bottom" => match parse_segment_user_data(&user_data) {
                    Ok((filter_type_index, index)) => {
                        let filter_type =
                            global_ve_filter!(ui).invoke_filter_type_from_int(filter_type_index);
                        global_ve_filter!(ui).invoke_preset_filter_move_to_bottom(index, filter_type);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-remove-preset-filter" => match parse_segment_user_data(&user_data) {
                    Ok((filter_type_index, index)) => {
                        let filter_type =
                            global_ve_filter!(ui).invoke_filter_type_from_int(filter_type_index);
                        global_ve_filter!(ui).invoke_remove_preset_filter(index, filter_type);
                    }
                    Err(e) => log::warn!("{e}"),
                },
                "video-editor-preset-filter-enter-edit-mode" => {
                    let index = user_data.parse::<i32>().unwrap();
                    global_ve_filter!(ui).set_preset_filter_enter_edit_mode_index(index);
                    global_ve_filter!(ui).set_preset_filter_enter_edit_mode_flag(
                        !global_ve_filter!(ui).get_preset_filter_enter_edit_mode_flag());
                },
                "video-editor-segment-remove-keyframe" => {
                    let keyframe_index = user_data.parse::<i32>().unwrap();
                    global_ve_filter!(ui).invoke_remove_keyframe_at_index(
                        global_ve_filter!(ui).get_selected_filter_index(),
                        keyframe_index,
                    );
                }
                "video-editor-segment-remove-keyframe-by-property" => {
                    // user_data format: "filter-index|property-name|time-ms"
                    let parts: Vec<&str> = user_data.split('|').collect();
                    if parts.len() == 3 {
                        let filter_index = parts[0].parse::<i32>().unwrap_or(-1);
                        let property_name = parts[1];
                        let time_ms = parts[2].parse::<i32>().unwrap_or(-1);
                        global_ve_filter!(ui).invoke_remove_keyframe(filter_index, property_name.into(), time_ms);
                    } else {
                        log::warn!("Invalid user_data format for video-editor-segment-remove-keyframe-by-property: {user_data}");
                    }
                }
                "video-editor-segment-keyframe-to-playhead" => {
                    // user_data format: "filter-index|property-name|time-ms"
                    let parts: Vec<&str> = user_data.split('|').collect();
                    if parts.len() == 3 {
                        let filter_index = parts[0].parse::<i32>().unwrap_or(-1);
                        let property_name = parts[1];
                        let old_time_ms = parts[2].parse::<i32>().unwrap_or(-1);

                        if filter_index >= 0 && old_time_ms >= 0 {
                            global_ve_filter!(ui).invoke_move_keyframe_to_playhead(
                                filter_index,
                                property_name.into(),
                                old_time_ms,
                            );
                        } else {
                            log::warn!("Invalid parameters for video-editor-segment-keyframe-to-playhead: {user_data}");
                        }
                    } else {
                        log::warn!("Invalid user_data format for video-editor-segment-keyframe-to-playhead: {user_data}");
                    }
                }
                "video-editor-create-preset-subtitle-style" => {
                    global_ve_filter!(ui).invoke_show_preset_subtitle_style_new_lineinput();
                }
                "video-editor-show-select-preset-subtitle-style" => {
                    global_ve_filter!(ui).invoke_show_preset_subtitle_style_panel();
                }
                "video-editor-segment-text-keyframe-to-playhead" => {
                    // user_data format: "track-index|segment-index|property-name|time-ms"
                    let parts: Vec<&str> = user_data.split('|').collect();
                    if parts.len() == 4 {
                        let track_index = parts[0].parse::<i32>().unwrap_or(-1);
                        let segment_index = parts[1].parse::<i32>().unwrap_or(-1);
                        let property_name = parts[2];
                        let old_time_ms = parts[3].parse::<i32>().unwrap_or(-1);

                        if track_index >= 0 && segment_index >= 0 && old_time_ms >= 0 {
                            let playhead = global_store!(ui).get_video_editor_timeline_offset();
                            global_logic!(ui).invoke_video_editor_move_text_keyframe(
                                property_name.into(),
                                old_time_ms,
                                playhead,
                            );
                        } else {
                            log::warn!("Invalid parameters for video-editor-segment-text-keyframe-to-playhead: {user_data}");
                        }
                    } else {
                        log::warn!("Invalid user_data format for video-editor-segment-text-keyframe-to-playhead: {user_data}");
                    }
                }
                "video-editor-segment-remove-text-keyframe" => {
                    let parts: Vec<&str> = user_data.split('|').collect();
                    if parts.len() == 4 {
                        let property_name = parts[2];
                        let time_ms = parts[3].parse::<i32>().unwrap_or(-1);

                        if time_ms >= 0 {
                            global_logic!(ui).invoke_video_editor_remove_text_keyframe(
                                property_name.into(),
                                time_ms,
                            );
                        } else {
                            log::warn!("Invalid parameters for video-editor-segment-remove-text-keyframe: {user_data}");
                        }
                    } else {
                        log::warn!("Invalid user_data format for video-editor-segment-remove-text-keyframe: {user_data}");
                    }
                }
                "video-editor-create-preset-text-style" => {
                    global_logic!(ui).invoke_video_editor_show_preset_text_style_new_lineinput();
                }
                "video-editor-show-select-preset-text-style" => {
                    global_logic!(ui).invoke_video_editor_show_preset_text_style_panel();
                }
                "video-editor-show-record-audio-dialog" => {
                    global_store!(ui).set_video_editor_is_show_record_audio_dialog(true);
                }
                "video-editor-show-stem-splitter-dialog" => {
                    global_store!(ui).set_video_editor_is_show_stem_splitter_dialog(true);
                }
                "video-editor-show-deep-filter-dialog" => {
                    global_store!(ui).set_video_editor_is_show_deep_filter_dialog(true);
                }
                "video-editor-show-similar-video-segment-dialog" => {
                    global_store!(ui).set_video_editor_is_show_similar_video_segment_dialog(true);
                }
                "video-editor-show-dedup-photos-dialog" => {
                    global_store!(ui).set_video_editor_is_show_dedup_photos_dialog(true);
                }
                "video-editor-show-speakers-dialog" => {
                    global_store!(ui).set_video_editor_is_show_speakers_dialog(true);
                }
                "video-editor-show-code-image-dialog" => {
                    global_store!(ui).set_video_editor_is_show_code_image_dialog(true);
                }
                "video-editor-show-pure-color-image-dialog" => {
                    global_store!(ui).set_video_editor_is_show_pure_color_image_dialog(true);
                }
                "video-editor-show-bg-remover-dialog" => {
                    global_store!(ui).set_video_editor_is_show_bg_remover_dialog(true);
                }
                "video-editor-show-dewatermark-dialog" => {
                    global_store!(ui).set_video_editor_is_show_dewatermark_dialog(true);
                }
                "video-editor-show-subtitle-remover-dialog" => {
                    global_store!(ui).set_video_editor_is_show_subtitle_remover_dialog(true);
                }
                "video-editor-show-subtitle-search-dialog" => {
                    global_logic!(ui).invoke_video_editor_subtitle_search_load();
                    global_store!(ui).set_video_editor_is_show_subtitle_search_dialog(true);
                }
                "video-editor-show-clear-vision-dialog" => {
                    global_store!(ui).set_video_editor_is_show_clear_vision_dialog(true);
                }
                "video-editor-show-bg-animation-dialog" => {
                    global_store!(ui).set_video_editor_is_show_bg_animation_dialog(true);
                }
                "video-editor-show-img-animation-dialog" => {
                    global_store!(ui).set_video_editor_is_show_img_animation_dialog(true);
                }
                "video-editor-show-global-filter-dialog" => {
                    global_store!(ui).set_video_editor_is_show_global_filter_dialog(true);
                }
                "video-editor-transcribe-audio" => {
                    global_logic!(ui).invoke_video_editor_transcribe_audio();
                }
                "video-editor-show-tts-dialog" => {
                    global_store!(ui).set_video_editor_is_show_tts_dialog(true);
                }
                "video-editor-show-ocr-dialog" => {
                    global_store!(ui).set_video_editor_is_show_ocr_dialog(true);

                }
                "video-editor-show-chapter-summary-dialog" => {
                    global_store!(ui).set_video_editor_is_show_chapter_summary_dialog(true);
                }
                "video-editor-bookmark-add" => {
                    global_logic!(ui).invoke_video_editor_bookmark_add();
                }
                "video-editor-bookmark-remove" => {
                    let time_ms = user_data.parse::<i32>().unwrap_or(-1);
                    global_logic!(ui).invoke_video_editor_bookmark_remove(time_ms);
                }
                "video-editor-bookmark-remove-all" => {
                    ui.global::<ConfirmDialogSetting>().invoke_set(
                        true,
                        tr("Warning").into(),
                        tr("Remove all bookmarks or not?").into(),
                        "video-editor-bookmark-remove-all".into(),
                        "".into(),
                    );
                }
                "video-editor-show-memo-dialog" => {
                    global_store!(ui).set_video_editor_is_show_memo_dialog(true);
                }
                "video-editor-show-subtitle-translate-dialog" => {
                    global_store!(ui).set_video_editor_is_show_subtitle_translate_dialog(true);
                }
                "video-editor-show-cutout-dialog" => {
                    global_store!(ui).set_video_editor_is_show_cutout_dialog(true);
                }
                "video-editor-show-musicgen-dialog" => {
                    global_store!(ui).set_video_editor_is_show_musicgen_dialog(true);
                }
                _ => log::warn!("Unknown popup action: {action}"),
            }
        });
}

fn parse_segment_user_data(user_data: &str) -> anyhow::Result<(i32, i32)> {
    let indexs = user_data
        .split(':')
        .map(|item| item.to_string().parse::<i32>().ok())
        .collect::<Vec<Option<i32>>>();

    if indexs.len() != 2 {
        anyhow::bail!("video-editor-segment-detach-audio invalid index: {user_data}");
    };

    let (Some(track_index), Some(seg_index)) = (indexs[0], indexs[1]) else {
        anyhow::bail!("video-editor-segment-detach-audio invalid index: {user_data}");
    };

    Ok((track_index, seg_index))
}
