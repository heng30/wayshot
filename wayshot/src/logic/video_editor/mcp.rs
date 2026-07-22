use crate::{
    global_logic, global_store,
    logic::{
        video_editor::{
            command::{sync_manager_to_ui, with_history_manager},
            common_type::VideoEditorPreferenceMcpConfig,
            project::{PROJECT_STATE, add_to_recent, async_open_project_path, setup_autosave},
        },
        {toast, tr::tr},
    },
    slint_generatedAppWindow::{
        AppWindow, McpTransport as UIMcpTransport, SelectedSegmentIndex as UISelectedSegmentIndex,
        SelectedTrackIndex as UISelectedTrackIndex, VideoEditorTrackType as UIVideoEditorTrackType,
        VideoEditorTracksManager as UIVideoEditorTracksManager,
    },
    toast_success,
};
use mcp_server::{
    config::McpTransport,
    state::{
        StateAccessors, UiAction, register_dispatch_action, register_state_accessors,
        register_sync_ui_callback,
    },
};
use slint::{ComponentHandle, Weak};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use video_editor::{
    commands::HistoryManager,
    project::{ManagerData, save_project},
    tracks::manager::Manager,
};

static MCP_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn init(ui: &AppWindow) {
    global_store!(ui).set_video_editor_mcp_is_runing(MCP_RUNNING.load(Ordering::Relaxed));

    let ui_weak = ui.as_weak();
    register_sync_ui_callback(Arc::new(move || {
        _ = ui_weak.upgrade_in_event_loop(|ui| {
            sync_manager_to_ui(&ui);
        });
    }));

    let ui_weak = ui.as_weak();
    register_dispatch_action(Arc::new(move |action: UiAction| {
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            handle_ui_action(&ui, action);
        });
    }));

    register_state_accessors(StateAccessors {
        get_project_path: || {
            let state = PROJECT_STATE.lock().unwrap();
            state
                .as_ref()
                .and_then(|s| s.current_project_path.as_ref())
                .map(|p| p.to_string_lossy().to_string())
        },
        get_tracks_manager: || {
            let state = PROJECT_STATE.lock().unwrap();
            state.as_ref().map(|s| s.tracks_manager.clone())
        },
        is_unsaved: || false,
        can_undo: || with_history_manager(|s| s.history_manager.can_undo()),
        can_redo: || with_history_manager(|s| s.history_manager.can_redo()),
        with_history_manager: |f: &mut dyn FnMut(&mut Manager, &mut HistoryManager)| {
            with_history_manager(|s| f(&mut s.tracks_manager, &mut s.history_manager))
        },
    });
}

fn handle_ui_action(ui: &AppWindow, action: UiAction) {
    let logic = global_logic!(ui);

    match action {
        UiAction::NewProject => logic.invoke_video_editor_new_project(),
        UiAction::OpenProject => logic.invoke_video_editor_open_project(),
        UiAction::OpenProjectPath { path } => {
            let path = PathBuf::from(&path);
            let project_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();

            let ui_weak = ui.as_weak();
            tokio::spawn(async move {
                async_open_project_path(ui_weak, project_name.into(), path);
            });
        }
        UiAction::CreateProject { name, dir_path } => {
            let ui_weak = ui.as_weak();
            let mut path = PathBuf::from(&dir_path);
            path.push(&name);
            path.set_extension("wayshot");
            let project_name = name.clone();

            tokio::spawn(async move {
                let (manager, playlist, chapter_summary, memo) = {
                    let mut state = PROJECT_STATE.lock().unwrap();
                    if state.is_none() {
                        *state = Some(super::project::ProjectState::default());
                    }
                    if let Some(ref mut state) = *state {
                        state.current_project_path = Some(path.clone());
                        (
                            state.tracks_manager.clone(),
                            state.playlist.clone(),
                            state.chapter_summary.clone(),
                            state.memo.clone(),
                        )
                    } else {
                        unreachable!();
                    }
                };

                let manager_data = ManagerData::new(manager)
                    .with_playlist(playlist)
                    .with_chapter_summary(chapter_summary)
                    .with_memo(memo);

                match save_project(&manager_data, &path) {
                    Ok(()) => {
                        add_to_recent(path.clone());
                        setup_autosave(Some(&path));
                        _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            global_store!(ui).set_video_editor_tracks_manager(
                                UIVideoEditorTracksManager::default(),
                            );
                            global_store!(ui).set_video_editor_is_unsaved(false);
                            toast_success!(
                                ui,
                                format!("{} {}", tr("New project created"), project_name)
                            );
                        });
                    }
                    Err(e) => toast::async_toast_warn(
                        ui_weak,
                        format!("{}: {}", tr("Failed to create project"), e),
                    ),
                }
            });
        }
        UiAction::CloseProject => logic.invoke_video_editor_close_project(),
        UiAction::SaveProject => logic.invoke_video_editor_save_project(),
        UiAction::Undo => logic.invoke_video_editor_undo(),
        UiAction::Redo => logic.invoke_video_editor_redo(),

        UiAction::AddTrack { track_type } => {
            let ty = match track_type {
                1 => UIVideoEditorTrackType::Audio,
                2 => UIVideoEditorTrackType::Subtitle,
                3 => UIVideoEditorTrackType::Image,
                4 => UIVideoEditorTrackType::Text,
                _ => UIVideoEditorTrackType::Video,
            };
            logic.invoke_video_editor_add_track(ty);
        }
        UiAction::AddEmptyVideoTrack => logic.invoke_video_editor_add_empty_video_track(),
        UiAction::AddEmptyAudioTrack => logic.invoke_video_editor_add_empty_audio_track(),
        UiAction::AddEmptySubtitleTrack => logic.invoke_video_editor_add_empty_subtitle_track(),
        UiAction::AddEmptyImageTrack => logic.invoke_video_editor_add_empty_image_track(),
        UiAction::AddEmptyTextTrack => logic.invoke_video_editor_add_empty_text_track(),
        UiAction::AddSelectedTrack { index } => {
            let idx = UISelectedTrackIndex {
                index: index as i32,
                modifiers: Default::default(),
            };
            logic.invoke_video_editor_add_selected_track(idx);
        }
        UiAction::RemoveTracks => logic.invoke_video_editor_remove_tracks(),
        UiAction::InsertVideoTrack { index } => {
            logic.invoke_video_editor_insert_video_track(index as i32)
        }
        UiAction::InsertAudioTrack { index } => {
            logic.invoke_video_editor_insert_audio_track(index as i32)
        }
        UiAction::InsertSubtitleTrack { index } => {
            logic.invoke_video_editor_insert_subtitle_track(index as i32)
        }
        UiAction::InsertImageTrack { index } => {
            logic.invoke_video_editor_insert_image_track(index as i32)
        }
        UiAction::InsertTextTrack { index } => {
            logic.invoke_video_editor_insert_text_track(index as i32)
        }
        UiAction::ToggleLockedTrack { index } => {
            logic.invoke_video_editor_toggle_locked_track(index as i32)
        }
        UiAction::ToggleHidingTrack { index } => {
            logic.invoke_video_editor_toggle_hiding_track(index as i32)
        }
        UiAction::ToggleMutedTrack { index } => {
            logic.invoke_video_editor_toggle_muted_track(index as i32)
        }
        UiAction::TrackMoveUp { index } => logic.invoke_video_editor_track_move_up(index as i32),
        UiAction::TrackMoveDown { index } => {
            logic.invoke_video_editor_track_move_down(index as i32)
        }
        UiAction::MoveTrackByDrag {
            from_index,
            to_index,
        } => logic.invoke_video_editor_move_track_by_drag(from_index as i32, to_index as i32),

        // Segments
        UiAction::AddSelectedSegment {
            track_index,
            segment_index,
        } => {
            let idx = UISelectedSegmentIndex {
                index: segment_index as i32,
                track_index: track_index as i32,
                modifiers: Default::default(),
            };
            logic.invoke_video_editor_add_selected_segment(idx);
        }
        UiAction::SplitSegment => logic.invoke_video_editor_split_segment(),
        UiAction::RemoveSegments => logic.invoke_video_editor_remove_segments(),
        UiAction::ToggleSegmentEnable {
            track_index,
            segment_index,
        } => {
            let idx = UISelectedSegmentIndex {
                index: segment_index as i32,
                track_index: track_index as i32,
                modifiers: Default::default(),
            };
            logic.invoke_video_editor_segment_toggle_enable(idx);
        }
        UiAction::ToggleSegmentAudio {
            track_index,
            segment_index,
        } => {
            let idx = UISelectedSegmentIndex {
                index: segment_index as i32,
                track_index: track_index as i32,
                modifiers: Default::default(),
            };
            logic.invoke_video_editor_segment_toggle_audio(idx);
        }
        UiAction::SegmentRemoveGap {
            track_index,
            segment_index,
        } => {
            let idx = UISelectedSegmentIndex {
                index: segment_index as i32,
                track_index: track_index as i32,
                modifiers: Default::default(),
            };
            logic.invoke_video_editor_segment_remove_gap(idx);
        }
        UiAction::SegmentRemoveLeftGap {
            track_index,
            segment_index,
        } => {
            let idx = UISelectedSegmentIndex {
                index: segment_index as i32,
                track_index: track_index as i32,
                modifiers: Default::default(),
            };
            logic.invoke_video_editor_segment_remove_left_gap(idx);
        }
        UiAction::SegmentRemoveRightGap {
            track_index,
            segment_index,
        } => {
            let idx = UISelectedSegmentIndex {
                index: segment_index as i32,
                track_index: track_index as i32,
                modifiers: Default::default(),
            };
            logic.invoke_video_editor_segment_remove_right_gap(idx);
        }
        UiAction::CommitSegmentMove {
            track_index,
            segment_index,
            final_offset_ms,
        } => {
            let idx = UISelectedSegmentIndex {
                index: segment_index as i32,
                track_index: track_index as i32,
                modifiers: Default::default(),
            };
            logic.invoke_video_editor_commit_segment_move(idx, final_offset_ms);
        }

        UiAction::RemoveAllFiltersFromSegment {
            track_index,
            segment_index,
        } => {
            logic.invoke_video_editor_segment_remove_all_filters(
                track_index as i32,
                segment_index as i32,
            );
        }
        UiAction::RemoveAllFiltersFromTrack { track_index } => {
            logic.invoke_video_editor_remove_all_filters_from_track(track_index as i32);
        }

        UiAction::ImportToPlaylist => logic.invoke_video_editor_playlist_item_import(),
        UiAction::ImportToLibrary => logic.invoke_video_editor_library_item_import(),
        UiAction::PlaylistItemAddToTrack { index } => {
            logic.invoke_video_editor_playlist_item_add_to_track(index as i32)
        }
        UiAction::PlaylistItemAddToTrackEnd { index } => {
            logic.invoke_video_editor_playlist_item_add_to_track_end(index as i32)
        }
        UiAction::LibraryItemAddToTrack { index } => {
            logic.invoke_video_editor_library_item_add_to_track(index as i32)
        }
        UiAction::LibraryItemAddToTrackEnd { index } => {
            logic.invoke_video_editor_library_item_add_to_track_end(index as i32)
        }

        UiAction::PreviewPlay => logic.invoke_video_editor_item_preview_play(),
        UiAction::PreviewStop => logic.invoke_video_editor_item_preview_stop(),
        UiAction::PreviewSeek { position_ms } => {
            logic.invoke_video_editor_preview_seek(position_ms)
        }
        UiAction::TimelineSeek { position_ms } => {
            logic.invoke_video_editor_timeline_seek(position_ms)
        }

        UiAction::ExportVideo => logic.invoke_video_editor_export_video(Default::default()),
        UiAction::ExportAudio => logic.invoke_video_editor_export_audio(Default::default()),
        UiAction::ExportSubtitle => logic.invoke_video_editor_export_subtitle(Default::default()),
        UiAction::StartRecordingAudio => logic.invoke_video_editor_start_recording_audio(),
        UiAction::StopRecordingAudio => logic.invoke_video_editor_stop_recording_audio(),
    }
}

pub fn start_from_config(ui: &AppWindow) {
    let mcp_setting: VideoEditorPreferenceMcpConfig = global_store!(ui)
        .get_video_editor_preference_config()
        .mcp
        .into();

    if mcp_setting.enabled {
        mcp_server::state::set_ui_available(true);
        start_mcp_server(ui.as_weak(), mcp_setting);
    }
}

fn start_mcp_server(ui_weak: Weak<AppWindow>, config: VideoEditorPreferenceMcpConfig) {
    let mcp_config = mcp_server::McpConfig {
        enabled: config.enabled,
        transport: match config.transport {
            UIMcpTransport::Http => McpTransport::Http,
            UIMcpTransport::Both => McpTransport::Both,
            _ => McpTransport::Stdio,
        },
        port: config.port as u16,
    };

    MCP_RUNNING.store(true, Ordering::Relaxed);
    _ = ui_weak.upgrade_in_event_loop(|ui| {
        global_store!(ui).set_video_editor_mcp_is_runing(true);
    });

    tokio::spawn(async move {
        if let Err(e) = mcp_server::transport::start(mcp_config).await {
            toast::async_toast_warn(ui_weak.clone(), format!("{}: {e}", tr("MCP server error")));
        }
        MCP_RUNNING.store(false, Ordering::Relaxed);
        _ = ui_weak.upgrade_in_event_loop(|ui| {
            global_store!(ui).set_video_editor_mcp_is_runing(false);
        });
    });
}

#[allow(dead_code)]
pub fn is_mcp_running() -> bool {
    MCP_RUNNING.load(Ordering::Relaxed)
}
