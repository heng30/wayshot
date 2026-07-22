use super::{
    command::{refresh_preview, sync_and_refresh, sync_manager_to_ui, with_history_manager},
    common_type::SubtitleStyleConfig,
    filters::subtitle::create_subtitle_style_filters_from_config,
    media_list_common::{self as common, MediaListContext},
    project::PROJECT_STATE,
    segment::refresh_affected_segments,
    track::reset_editor_selection_state,
};
use crate::{
    global_logic, global_store, global_ve_filter,
    logic::{toast, tr::tr},
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, ConfirmDialogSetting as UIConfirmDialogSetting,
        SelectedSegmentIndex as UISelectedSegmentIndex,
        VideoEditorPlaylistItem as UIVideoEditorPlaylistItem,
        VideoEditorTrackType as UIVideoEditorTrackType,
    },
    store_video_editor_playlist,
};
use slint::{ComponentHandle, Model, SharedString, VecModel, Weak};
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use video_editor::{
    commands::{
        AffectedSegment, AffectedSegments, BatchCommand, ExecuteResult,
        filter::AddFilterCommand,
        segment::{AddSegmentCommand, InsertSegmentAtTimeCommand, SplitSegmentCommand},
        track::InsertTrackCommand,
    },
    media::{SUPPORT_EXT, media_type::MediaType},
    metadata::{Metadata, MetadataType, get_metadata},
    preview::cache::get_global_audio_display_cache,
    tracks::{
        segment::Segment,
        subtitle_track::extract_subtitles_as_segments,
        track::{Track, TrackPriority},
    },
};

const CTX: MediaListContext = MediaListContext::Playlist;

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_playlist_item_import, ui);
    logic_cb!(video_editor_playlist_folder_import, ui);
    logic_cb!(video_editor_playlist_item_remove, ui, index);
    logic_cb!(video_editor_playlist_item_add_to_track, ui, index);
    logic_cb!(video_editor_playlist_item_add_to_track_end, ui, index);
    logic_cb!(video_editor_playlist_item_preview, ui, index);
    logic_cb!(video_editor_playlist_enter_edit_mode, ui);
    logic_cb!(video_editor_playlist_exit_edit_mode, ui);
    logic_cb!(video_editor_playlist_toggle_select_all, ui, selected);
    logic_cb!(video_editor_playlist_remove_all, ui);
    logic_cb!(video_editor_playlist_show_remove_all_confirm_dialog, ui);
    logic_cb!(
        video_editor_playlist_show_remove_selections_confirm_dialog,
        ui
    );
    logic_cb!(video_editor_playlist_remove_selections, ui);
    logic_cb!(video_editor_playlist_toggle_selection, ui, index);
    logic_cb!(video_editor_find_drag_target_track, ui, relative_y);
    logic_cb!(
        video_editor_drag_item_add_to_track,
        ui,
        track_index,
        timeline_offset_ms
    );
    logic_cb!(video_editor_extract_frame_to_playlist, ui);
    logic_cb!(video_editor_playlist_item_copy_path, ui, index);
    logic_cb!(video_editor_playlist_item_open_directory, ui, index);
    logic_cb!(video_editor_playlist_search, ui, index, keyword);
    logic_cb!(video_editor_playlist_all_files, ui);
    logic_cb!(video_editor_playlist_all_videos, ui);
    logic_cb!(video_editor_playlist_all_audios, ui);
    logic_cb!(video_editor_playlist_all_images, ui);
    logic_cb!(video_editor_playlist_all_subtitles, ui);
    logic_cb!(video_editor_playlist_create_folder, ui);
    logic_cb!(video_editor_playlist_rename_folder, ui, id);
    logic_cb!(video_editor_playlist_remove_folder, ui, id);
    logic_cb!(video_editor_playlist_navigate_to_folder, ui, id);
    logic_cb!(video_editor_playlist_navigate_up, ui);
    logic_cb!(
        video_editor_playlist_move_item_to_folder,
        ui,
        item_id,
        folder_id
    );
    logic_cb!(video_editor_playlist_sync_folder, ui, id);
    logic_cb!(video_editor_playlist_refresh, ui);
    logic_cb!(
        video_editor_playlist_move_folder,
        ui,
        folder_id,
        new_parent_id
    );
}

fn video_editor_playlist_item_import(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let current_folder_id = CTX.get_current_folder_id(ui);
    tokio::spawn(async move {
        let Some(file_paths) = picker_files(
            ui_weak.clone(),
            &tr("Select media files"),
            &tr("Media Files"),
            SUPPORT_EXT,
        ) else {
            return;
        };

        toast::async_toast_info(
            ui_weak.clone(),
            tr("Importing files, this may take a while..."),
        );

        for file_path in file_paths {
            tokio::spawn(import_file_to_playlist(
                ui_weak.clone(),
                file_path,
                current_folder_id.clone(),
            ));
        }
    });
}

fn video_editor_playlist_folder_import(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let current_folder_id = CTX.get_current_folder_id(ui);
    tokio::spawn(async move {
        let Some(dir_path) = picker_folder(ui_weak.clone(), &tr("Select directory to import"))
        else {
            return;
        };

        toast::async_toast_info(
            ui_weak.clone(),
            tr("Importing folder, this may take a while..."),
        );

        let dir_path_clone = dir_path.clone();
        let dir_entries =
            tokio::task::spawn_blocking(move || collect_media_dir_entries(&dir_path_clone))
                .await
                .unwrap_or_default();

        if dir_entries.is_empty() {
            toast::async_toast_warn(ui_weak.clone(), tr("No media files found in directory"));
            return;
        }

        // Create folder hierarchy and import files into their respective folders
        let file_imports = {
            let mut state = PROJECT_STATE.lock().unwrap();
            if let Some(ref mut s) = *state {
                // Create root folder from source
                let root_folder_id = match s
                    .playlist
                    .create_folder_from_source(dir_path.clone(), current_folder_id.clone())
                {
                    Ok(id) => id,
                    Err(e) => {
                        log::warn!("Failed to create folder from source: {}", e);
                        // Fallback: import to current folder
                        current_folder_id.unwrap_or_default()
                    }
                };

                let mut dir_to_folder_id = HashMap::new();
                dir_to_folder_id.insert(dir_path.clone(), root_folder_id.clone());

                let mut file_imports: Vec<(PathBuf, String)> = Vec::new();

                for (file_path, relative_dir) in dir_entries {
                    let folder_id = if relative_dir == PathBuf::new() {
                        root_folder_id.clone()
                    } else {
                        // Ensure all parent directories exist in the folder tree
                        let mut current_path = dir_path.clone();
                        let mut parent_folder_id = root_folder_id.clone();

                        for component in relative_dir.components() {
                            current_path = current_path.join(component);
                            if let Some(fid) = dir_to_folder_id.get(&current_path) {
                                parent_folder_id = fid.clone();
                            } else {
                                match s.playlist.create_folder_from_source(
                                    current_path.clone(),
                                    Some(parent_folder_id.clone()),
                                ) {
                                    Ok(new_id) => {
                                        dir_to_folder_id
                                            .insert(current_path.clone(), new_id.clone());
                                        parent_folder_id = new_id;
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to create subfolder: {}", e);
                                        break;
                                    }
                                }
                            }
                        }
                        parent_folder_id
                    };

                    file_imports.push((file_path, folder_id));
                }

                file_imports
            } else {
                Vec::new()
            }
        };

        for (file_path, folder_id) in file_imports {
            tokio::spawn(import_file_to_playlist(
                ui_weak.clone(),
                file_path,
                Some(folder_id.into()),
            ));
        }
    });
}

/// Collect media files from a directory recursively, returning each file along with
/// its relative directory path (relative to the root). Skips symlinks to prevent circular recursion.
pub fn collect_media_dir_entries(root: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut result = Vec::new();
    let mut stack = vec![(root.to_path_buf(), PathBuf::new())];
    let mut visited = HashSet::new();

    // Canonicalize the root dir to add to visited
    if let Ok(canonical) = root.canonicalize() {
        visited.insert(canonical);
    }

    while let Some((current_dir, relative_dir)) = stack.pop() {
        let entries = match std::fs::read_dir(&current_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();

            // Skip symlinks to prevent circular recursion
            if path.is_symlink() {
                continue;
            }

            if path.is_dir() {
                // Skip if already visited (handles hardlink cycles, etc.)
                if let Ok(canonical) = path.canonicalize() {
                    if visited.contains(&canonical) {
                        continue;
                    }
                    visited.insert(canonical);
                }

                let dir_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let new_relative = relative_dir.join(dir_name);
                stack.push((path, new_relative));
            } else if path.is_file() {
                // Check if file extension is supported
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if SUPPORT_EXT.contains(&ext.as_str()) {
                    result.push((path, relative_dir.clone()));
                }
            }
        }
    }

    result
}

pub async fn import_file_to_playlist(
    ui_weak: Weak<AppWindow>,
    file_path: PathBuf,
    folder_id: Option<String>,
) {
    let (playlist_items, playlist_item) = {
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            match s.playlist.add_file(file_path.clone(), folder_id) {
                Ok(item_id) => {
                    let playlist_items =
                        s.playlist.items().into_iter().cloned().collect::<Vec<_>>();
                    let playlist_item = s
                        .playlist
                        .get_node(&item_id)
                        .and_then(|n| n.as_file())
                        .cloned();
                    (Some(playlist_items), playlist_item)
                }
                Err(video_editor::Error::DuplicateEntry(msg)) => {
                    if let Some(name) = file_path.file_name().and_then(|s| s.to_str()) {
                        toast::async_toast_warn(
                            ui_weak.clone(),
                            format!("{}: {} {}", name, tr("already exists in"), msg),
                        );
                    }
                    return;
                }
                Err(e) => {
                    toast::async_toast_warn(
                        ui_weak.clone(),
                        format!("{}: {}", tr("Failed to add to playlist"), e),
                    );
                    return;
                }
            }
        } else {
            (None, None)
        }
    };

    let Some(playlist_item) = playlist_item else {
        toast::async_toast_warn(ui_weak.clone(), tr("Failed to get playlist item"));
        return;
    };

    // Preload audio cache for video/audio files
    let metadata = get_metadata(&file_path).ok();
    if let Some(ref metadata) = metadata
        && let Some(audio_meta) = metadata.audios.first()
    {
        let file_path = file_path.clone();
        let audio_meta = audio_meta.clone();
        tokio::task::spawn_blocking(move || {
            let cache = get_global_audio_display_cache();
            if let Err(e) = cache.load_and_cache(&file_path, audio_meta.index, &audio_meta) {
                log::warn!(
                    "Failed to preload audio cache for {}: {:?}",
                    file_path.display(),
                    e
                );
            } else {
                log::debug!("Preloaded audio cache for {}", file_path.display());
            }
        });
    }

    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        crate::toast_success!(ui, format!("{} {}", tr("Imported"), playlist_item.name));

        if playlist_items.is_some() {
            common::refresh_ui_filtered(CTX, &ui);
        }
    });
}

fn video_editor_playlist_item_remove(ui: &AppWindow, index: i32) {
    common::item_remove(CTX, ui, index);
}

fn video_editor_playlist_item_add_to_track(ui: &AppWindow, index: i32) {
    video_editor_playlist_clear_selection(ui);

    let Some(item) = store_video_editor_playlist!(ui).row_data(index as usize) else {
        log::warn!("invalid playlist index: {index}");
        return;
    };

    let file_path = PathBuf::from(item.file_path.as_str());
    async_add_item_to_track(ui.as_weak(), item.name.to_string(), file_path);
}

fn video_editor_playlist_item_add_to_track_end(ui: &AppWindow, index: i32) {
    video_editor_playlist_clear_selection(ui);

    let Some(item) = store_video_editor_playlist!(ui).row_data(index as usize) else {
        log::warn!("invalid playlist index: {index}");
        return;
    };

    let file_path = PathBuf::from(item.file_path.as_str());
    async_add_item_to_track_end(ui.as_weak(), item.name.to_string(), file_path);
}

fn video_editor_playlist_item_preview(ui: &AppWindow, index: i32) {
    common::item_preview(CTX, ui, index);
}

pub fn async_add_item_to_track(ui_weak: Weak<AppWindow>, name: String, file_path: PathBuf) {
    let subtitle_style: SubtitleStyleConfig = global_ve_filter!(ui_weak.unwrap())
        .get_subtitle_style()
        .into();

    tokio::spawn(async move {
        let metadata = match get_metadata(&file_path) {
            Ok(meta) => Arc::new(meta),
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {}", tr("Failed to load metadata"), e),
                );
                return;
            }
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let current_track_index =
                global_store!(ui).get_video_editor_current_edited_track_index();
            let timeline_offset_ms = global_store!(ui).get_video_editor_timeline_offset();
            let timeline_offset = Duration::from_millis(timeline_offset_ms as u64);
            let new_track_insert_index =
                global_store!(ui).get_video_editor_item_new_track_insert_index();
            let created_new_tracks = Cell::new(false);

            let result = with_history_manager(|state| {
                // 现有轨道中添加
                if current_track_index >= 0
                    && let Some(current_track) =
                        state.tracks_manager.get(current_track_index as usize)
                    && track_matches_metadata_type(current_track, &metadata)
                {
                    let track_index = current_track_index as usize;
                    let track = state.tracks_manager.get(track_index).expect("track exists");

                    // Special handling for subtitle files: parse and add all subtitle entries
                    if metadata.get_type() == MetadataType::Subtitle {
                        let subtitle_meta = metadata.subtitles.first().ok_or_else(|| {
                            video_editor::Error::InvalidConfig("No subtitle stream".to_string())
                        })?;

                        let segments = extract_subtitles_as_segments(
                            &metadata.path,
                            subtitle_meta.index,
                            metadata.clone(),
                            state.tracks_manager.get_global_speed(),
                        )?;

                        if segments.is_empty() {
                            return Ok::<ExecuteResult, video_editor::Error>(ExecuteResult {
                                affected_segments: AffectedSegments::default(),
                            });
                        }

                        let track_end = track
                            .segments()
                            .last()
                            .map(|last_seg| last_seg.timeline_offset + last_seg.duration)
                            .unwrap_or(Duration::ZERO);

                        let global_speed = state.tracks_manager.get_global_speed();

                        // Adjust timeline_offset to start at track end position (append behavior)
                        let adjusted_segments: Vec<Arc<Segment>> = segments
                            .into_iter()
                            .map(|seg| {
                                let mut adjusted = (*seg).clone();
                                adjusted.timeline_offset = track_end + seg.timeline_offset;
                                adjusted.global_speed = global_speed;
                                Arc::new(adjusted)
                            })
                            .collect();

                        state
                            .history_manager
                            .begin_batch(format!("Add {} to track", name));

                        let base_segment_index = track.segments_count();
                        let mut execute_result = ExecuteResult {
                            affected_segments: AffectedSegments::default(),
                        };

                        for (i, segment) in adjusted_segments.iter().enumerate() {
                            let seg_result = state.history_manager.execute(
                                &mut state.tracks_manager,
                                Box::new(AddSegmentCommand::new(track_index, segment.clone())),
                            )?;
                            execute_result
                                .affected_segments
                                .segments
                                .extend(seg_result.affected_segments.segments);

                            let filters =
                                create_subtitle_style_filters_from_config(&subtitle_style);
                            for filter in filters {
                                let filter_result = state.history_manager.execute(
                                    &mut state.tracks_manager,
                                    Box::new(AddFilterCommand::new_subtitle(
                                        track_index,
                                        base_segment_index + i,
                                        filter,
                                    )),
                                )?;
                                execute_result
                                    .affected_segments
                                    .segments
                                    .extend(filter_result.affected_segments.segments);
                            }
                        }

                        state.history_manager.end_batch()?;
                        return Ok(execute_result);
                    }

                    // Add segment to existing track at playhead position (video/audio/image)
                    let segment_duration = if metadata.is_image() && metadata.duration.is_zero() {
                        Duration::from_secs(5)
                    } else {
                        metadata.duration
                    };

                    // Determine insertion point and whether we need to split
                    let mut need_split = false;
                    let mut split_segment_index = 0;
                    let mut insert_index = track.segments_count();
                    let mut split_time = Duration::ZERO;

                    for (i, segment) in track.segments().iter().enumerate() {
                        let segment_start = segment.timeline_offset;
                        let segment_end = segment.timeline_offset + segment.duration;

                        if timeline_offset >= segment_start && timeline_offset < segment_end {
                            // Playhead is within this segment, need to split
                            insert_index = i + 1;
                            need_split = true;
                            split_segment_index = i;
                            split_time = timeline_offset - segment_start;
                            break;
                        } else if timeline_offset < segment_start {
                            // Playhead is before this segment
                            insert_index = i;
                            break;
                        }
                    }

                    // Create the new segment at playhead position
                    let mut segment = Segment::new(
                        timeline_offset,
                        segment_duration,
                        metadata.clone(),
                        state.tracks_manager.get_global_speed(),
                    );
                    segment.generate_uuid();
                    let segment = Arc::new(segment);

                    let mut batch_command = BatchCommand::new(format!("Add {} to track", name));

                    if need_split {
                        batch_command.add_command(Box::new(SplitSegmentCommand::new(
                            track_index,
                            split_segment_index,
                            split_time,
                        )));

                        // 被分割的左边segment由SplitSegmentCommand进行管理
                        batch_command.add_extra_affected_segment(
                            AffectedSegment::with_both_thumbnails(
                                track_index,
                                split_segment_index + 2, // 被分割的右边
                            ),
                        );
                    }

                    batch_command.add_command(Box::new(InsertSegmentAtTimeCommand::new(
                        track_index,
                        insert_index,
                        segment.clone(),
                        true, // always enable shift_timeline to handle overlap/gap correctly
                    )));

                    let execute_result = state
                        .history_manager
                        .execute(&mut state.tracks_manager, Box::new(batch_command))?;

                    return Ok(execute_result);
                }

                // 新建轨道
                created_new_tracks.set(true);
                let global_speed = state.tracks_manager.get_global_speed();
                let tracks = Track::new(&metadata.path, global_speed)
                    .map_err(|e| video_editor::Error::InvalidConfig(e.to_string()))?;

                let is_subtitle = metadata.get_type() == MetadataType::Subtitle;

                state
                    .history_manager
                    .begin_batch(format!("Add {} to track", name));

                let new_tracks_count = tracks.len();
                let mut execute_result = ExecuteResult {
                    affected_segments: AffectedSegments::default(),
                };

                // 计算起始插入位置。如果 new_track_insert_index >= 0，使用它；否则插入到最后
                let mut insert_index = if new_track_insert_index >= 0 {
                    new_track_insert_index as usize
                } else {
                    state.tracks_manager.len()
                };

                for track in tracks {
                    let result = state.history_manager.execute(
                        &mut state.tracks_manager,
                        Box::new(InsertTrackCommand::new(track, insert_index)),
                    )?;
                    execute_result
                        .affected_segments
                        .segments
                        .extend(result.affected_segments.segments);

                    insert_index += 1;
                }

                if is_subtitle {
                    let all_subtitle_indices: Vec<usize> = state
                        .tracks_manager
                        .iter()
                        .enumerate()
                        .filter_map(|(idx, t)| matches!(t, Track::Subtitle(_)).then_some(idx))
                        .collect();

                    // Skip old subtitle tracks, take only the newly added ones at the end
                    let skip_count = all_subtitle_indices.len().saturating_sub(new_tracks_count);
                    let new_subtitle_indices: Vec<usize> =
                        all_subtitle_indices.into_iter().skip(skip_count).collect();

                    for track_idx in new_subtitle_indices {
                        let track = state.tracks_manager.get(track_idx).unwrap();
                        let segments_count = track.segments_count();
                        for seg_idx in 0..segments_count {
                            let filters =
                                create_subtitle_style_filters_from_config(&subtitle_style);
                            for filter in filters {
                                let filter_cmd =
                                    AddFilterCommand::new_subtitle(track_idx, seg_idx, filter);
                                let filter_result = state
                                    .history_manager
                                    .execute(&mut state.tracks_manager, Box::new(filter_cmd))?;
                                execute_result
                                    .affected_segments
                                    .segments
                                    .extend(filter_result.affected_segments.segments);
                            }
                        }
                    }
                }

                state.history_manager.end_batch()?;

                Ok(execute_result)
            });

            match result {
                Ok(execute_result) => {
                    sync_manager_to_ui(&ui);
                    refresh_affected_segments(&ui, execute_result.affected_segments);
                    global_store!(ui).set_video_editor_item_new_track_insert_index(-1);

                    if created_new_tracks.get() {
                        reset_editor_selection_state(&ui);
                    }

                    if matches!(metadata.get_type(), MetadataType::Video) || metadata.is_image() {
                        refresh_preview(&ui);
                    }

                    crate::toast_success!(
                        ui,
                        format!("{} {} {}", tr("Added"), name, tr("to track"))
                    );
                }
                Err(e) => crate::toast_warn!(ui, e.to_string()),
            }
        });
    });
}

fn track_matches_metadata_type(track: &Track, metadata: &Metadata) -> bool {
    match track {
        Track::Video(_) => metadata.get_type() == MetadataType::Video,
        Track::Audio(_) => metadata.get_type() == MetadataType::Audio,
        Track::Subtitle(_) => metadata.get_type() == MetadataType::Subtitle,
        Track::Image(_) => metadata.is_image(),
        Track::Text(_) => metadata.is_text(),
    }
}

pub fn async_add_item_to_track_end(ui_weak: Weak<AppWindow>, name: String, file_path: PathBuf) {
    let subtitle_style: SubtitleStyleConfig = global_ve_filter!(ui_weak.unwrap())
        .get_subtitle_style()
        .into();

    tokio::spawn(async move {
        let metadata = match get_metadata(&file_path) {
            Ok(meta) => Arc::new(meta),
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {}", tr("Failed to load metadata"), e),
                );
                return;
            }
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let current_track_index =
                global_store!(ui).get_video_editor_current_edited_track_index();

            let result = with_history_manager(|state| {
                if current_track_index >= 0
                    && let Some(current_track) =
                        state.tracks_manager.get(current_track_index as usize)
                    && track_matches_metadata_type(current_track, &metadata)
                {
                    let track_index = current_track_index as usize;
                    let track = state.tracks_manager.get(track_index).expect("track exists");

                    let track_end = track
                        .segments()
                        .last()
                        .map(|last_seg| last_seg.timeline_offset + last_seg.duration)
                        .unwrap_or(Duration::ZERO);

                    if metadata.get_type() == MetadataType::Subtitle {
                        let subtitle_meta = metadata.subtitles.first().ok_or_else(|| {
                            video_editor::Error::InvalidConfig("No subtitle stream".to_string())
                        })?;

                        let segments = extract_subtitles_as_segments(
                            &metadata.path,
                            subtitle_meta.index,
                            metadata.clone(),
                            state.tracks_manager.get_global_speed(),
                        )?;

                        if segments.is_empty() {
                            return Ok::<ExecuteResult, video_editor::Error>(ExecuteResult {
                                affected_segments: AffectedSegments::default(),
                            });
                        }

                        let adjusted_segments: Vec<Arc<Segment>> = segments
                            .into_iter()
                            .map(|seg| {
                                let mut adjusted = (*seg).clone();
                                adjusted.timeline_offset = track_end + seg.timeline_offset;
                                Arc::new(adjusted)
                            })
                            .collect();

                        state
                            .history_manager
                            .begin_batch(format!("Add {} to track end", name));

                        let base_segment_index = track.segments_count();
                        let mut execute_result = ExecuteResult {
                            affected_segments: AffectedSegments::default(),
                        };

                        for (i, segment) in adjusted_segments.iter().enumerate() {
                            let seg_result = state.history_manager.execute(
                                &mut state.tracks_manager,
                                Box::new(AddSegmentCommand::new(track_index, segment.clone())),
                            )?;
                            execute_result
                                .affected_segments
                                .segments
                                .extend(seg_result.affected_segments.segments);

                            let filters =
                                create_subtitle_style_filters_from_config(&subtitle_style);
                            for filter in filters {
                                let filter_result = state.history_manager.execute(
                                    &mut state.tracks_manager,
                                    Box::new(AddFilterCommand::new_subtitle(
                                        track_index,
                                        base_segment_index + i,
                                        filter,
                                    )),
                                )?;
                                execute_result
                                    .affected_segments
                                    .segments
                                    .extend(filter_result.affected_segments.segments);
                            }
                        }

                        state.history_manager.end_batch()?;
                        return Ok(execute_result);
                    }

                    // Add segment at track end position (video/audio/image)
                    let segment_duration = if metadata.is_image() && metadata.duration.is_zero() {
                        Duration::from_secs(5)
                    } else {
                        metadata.duration
                    };

                    let mut segment = Segment::new(
                        track_end,
                        segment_duration,
                        metadata.clone(),
                        state.tracks_manager.get_global_speed(),
                    );
                    segment.generate_uuid();
                    let segment = Arc::new(segment);

                    state
                        .history_manager
                        .begin_batch(format!("Add {} to track end", name));

                    let execute_result = state.history_manager.execute(
                        &mut state.tracks_manager,
                        Box::new(AddSegmentCommand::new(track_index, segment.clone())),
                    )?;

                    state.history_manager.end_batch()?;

                    return Ok(execute_result);
                }

                Err(video_editor::Error::InvalidConfig(
                    "No valid track selected for this media type".to_string(),
                ))
            });

            match result {
                Ok(execute_result) => {
                    sync_manager_to_ui(&ui);
                    refresh_affected_segments(&ui, execute_result.affected_segments);
                    global_store!(ui).set_video_editor_item_new_track_insert_index(-1);

                    if matches!(metadata.get_type(), MetadataType::Video) || metadata.is_image() {
                        refresh_preview(&ui);
                    }

                    crate::toast_success!(
                        ui,
                        format!("{} {} {}", tr("Added"), name, tr("to track end"))
                    );
                }
                Err(e) => crate::toast_warn!(ui, e.to_string()),
            }
        });
    });
}

fn video_editor_playlist_enter_edit_mode(ui: &AppWindow) {
    common::enter_edit_mode(CTX, ui);
}

fn video_editor_playlist_exit_edit_mode(ui: &AppWindow) {
    common::exit_edit_mode(CTX, ui);
}

fn video_editor_playlist_toggle_select_all(ui: &AppWindow, selected: bool) {
    common::toggle_select_all(CTX, ui, selected);
    if selected {
        let count = store_video_editor_playlist!(ui)
            .iter()
            .filter(|item| item.item_id != "..")
            .count() as i32;
        global_store!(ui).set_video_editor_playlist_selected_count(count);
    } else {
        global_store!(ui).set_video_editor_playlist_selected_count(0);
    }
    global_store!(ui).set_video_editor_media_library_selected_count(0);
}

fn video_editor_playlist_remove_all(ui: &AppWindow) {
    common::remove_all(CTX, ui);
}

fn video_editor_playlist_show_remove_all_confirm_dialog(ui: &AppWindow) {
    ui.global::<UIConfirmDialogSetting>().invoke_set(
        true,
        tr("Warning").into(),
        tr("Remove all items from playlist or not?").into(),
        "video-editor-playlist-remove-all".into(),
        "".into(),
    );
}

fn video_editor_playlist_show_remove_selections_confirm_dialog(ui: &AppWindow) {
    ui.global::<UIConfirmDialogSetting>().invoke_set(
        true,
        tr("Warning").into(),
        tr("Remove selected items from playlist or not?").into(),
        "video-editor-playlist-remove-selections".into(),
        "".into(),
    );
}

fn video_editor_playlist_remove_selections(ui: &AppWindow) {
    common::remove_selections(CTX, ui);
    global_store!(ui).set_video_editor_playlist_selected_count(0);
    global_store!(ui).set_video_editor_media_library_selected_count(0);
}

fn video_editor_playlist_toggle_selection(ui: &AppWindow, index: i32) {
    common::toggle_selection(CTX, ui, index);
}

fn video_editor_drag_item_add_to_track(ui: &AppWindow, track_index: i32, timeline_offset_ms: i32) {
    reset_editor_selection_state(ui);

    let left_panel_index = global_store!(ui).get_video_editor_left_panel_selected_index();
    let selected_count = if left_panel_index == 1 {
        global_store!(ui).get_video_editor_media_library_selected_count()
    } else {
        global_store!(ui).get_video_editor_playlist_selected_count()
    };

    // Collect all selected items, also count selected folders for toast warning
    let (items, folder_count) = if selected_count > 0 {
        if left_panel_index == 1 {
            let store = global_store!(ui).get_video_editor_media_library();
            let store = store
                .as_any()
                .downcast_ref::<VecModel<UIVideoEditorPlaylistItem>>()
                .expect("We know we set a VecModel earlier");
            let count = store.row_count();
            let mut file_items: Vec<(String, PathBuf)> = Vec::new();
            let mut folders = 0;
            for i in 0..count {
                if let Some(item) = store.row_data(i)
                    && item.is_selected
                {
                    if item.is_folder {
                        folders += 1;
                    } else {
                        file_items.push((
                            item.name.to_string(),
                            PathBuf::from(item.file_path.as_str()),
                        ));
                    }
                }
            }
            (file_items, folders)
        } else {
            let count = store_video_editor_playlist!(ui).row_count();
            let mut file_items: Vec<(String, PathBuf)> = Vec::new();
            let mut folders = 0;
            for i in 0..count {
                if let Some(item) = store_video_editor_playlist!(ui).row_data(i)
                    && item.is_selected
                {
                    if item.is_folder {
                        folders += 1;
                    } else {
                        file_items.push((
                            item.name.to_string(),
                            PathBuf::from(item.file_path.as_str()),
                        ));
                    }
                }
            }
            (file_items, folders)
        }
    } else {
        // No selection, use the drag item
        let drag_item = global_store!(ui).get_video_editor_drag_item();
        let folders = if drag_item.is_folder { 1 } else { 0 };
        let file_items = if drag_item.is_folder {
            vec![]
        } else {
            vec![(
                drag_item.name.to_string(),
                PathBuf::from(drag_item.file_path.as_str()),
            )]
        };
        (file_items, folders)
    };

    if folder_count > 0 {
        crate::toast_warn!(ui, tr("Folders cannot be added to tracks"));
    }

    if items.is_empty() {
        return;
    }

    if left_panel_index == 1 {
        crate::logic::video_editor::library::video_editor_library_clear_selection(ui);
    } else {
        video_editor_playlist_clear_selection(ui);
    }

    let ui_weak = ui.as_weak();
    let is_add_to_new_track = global_store!(ui).get_video_editor_item_is_add_to_new_track();

    if is_add_to_new_track || track_index < 0 {
        global_store!(ui).set_video_editor_current_edited_track_index(-1);
        tokio::spawn(async move {
            async_add_items_to_track(ui_weak, items).await;
        });
    } else if items.len() == 1 {
        let (name, file_path) = items.into_iter().next().unwrap();
        global_store!(ui).set_video_editor_current_edited_track_index(track_index);
        global_store!(ui).set_video_editor_timeline_offset(timeline_offset_ms);
        async_add_item_to_track(ui_weak, name, file_path);
    } else {
        tokio::spawn(async move {
            async_add_items_to_existing_track(ui_weak, track_index, timeline_offset_ms, items)
                .await;
        });
    }
}

fn video_editor_find_drag_target_track(ui: &AppWindow, relative_y: i32) -> i32 {
    with_history_manager(|state| {
        let tracks_count = state.tracks_manager.len();
        if tracks_count == 0 {
            return -1;
        }

        let mut accumulated_height: i32 = 0;
        for target_idx in 0..tracks_count {
            let track = state.tracks_manager.get(target_idx).unwrap();
            let track_height = global_logic!(ui)
                .invoke_video_editor_get_track_height_pixels(UIVideoEditorTrackType::from(track));

            if relative_y >= accumulated_height && relative_y < accumulated_height + track_height {
                return target_idx as i32;
            }
            accumulated_height += track_height;
        }

        -1
    })
}

fn video_editor_playlist_item_copy_path(ui: &AppWindow, index: i32) {
    common::item_copy_path(CTX, ui, index);
}

fn video_editor_playlist_item_open_directory(ui: &AppWindow, index: i32) {
    common::item_open_directory(CTX, ui, index);
}

pub fn picker_files(
    ui: Weak<AppWindow>,
    title: &str,
    filter_name: &str,
    filter_extensions: &[&str],
) -> Option<Vec<PathBuf>> {
    let mut file_dialog = native_dialog::DialogBuilder::file().set_title(title);

    if !filter_extensions.is_empty() {
        file_dialog = file_dialog.add_filter(filter_name, filter_extensions);
    }

    match file_dialog.open_multiple_file().show() {
        Ok(paths) if !paths.is_empty() => Some(paths),
        Err(e) => {
            toast::async_toast_warn(
                ui,
                format!("{}. {}: {}", tr("Choose file failed"), tr("Reason"), e),
            );
            None
        }
        _ => None,
    }
}

pub fn picker_folder(ui: Weak<AppWindow>, title: &str) -> Option<PathBuf> {
    let file_dialog = native_dialog::DialogBuilder::file().set_title(title);

    match file_dialog.open_single_dir().show() {
        Ok(Some(path)) => Some(path),
        Ok(None) => None,
        Err(e) => {
            toast::async_toast_warn(
                ui,
                format!("{}. {}: {}", tr("Choose directory failed"), tr("Reason"), e),
            );
            None
        }
    }
}

fn video_editor_extract_frame_to_playlist(ui: &AppWindow) {
    let row_count = crate::store_video_editor_selected_segments_index!(ui).row_count();
    if row_count == 0 {
        crate::toast_warn!(ui, tr("No segment selected"));
        return;
    }

    let first_selected = crate::store_video_editor_selected_segments_index!(ui)
        .row_data(row_count - 1)
        .unwrap();
    let track_index = first_selected.track_index as usize;
    let segment_index = first_selected.index as usize;

    let playhead_ms = global_store!(ui).get_video_editor_timeline_offset();
    let playhead = Duration::from_millis(playhead_ms as u64);

    let result = with_history_manager(|state| {
        let track = state.tracks_manager.get(track_index).ok_or_else(|| {
            video_editor::Error::IndexOutOfBounds(track_index, state.tracks_manager.len())
        })?;
        let segment = match track {
            Track::Video(t) => t.track.segments.get(segment_index),
            Track::Image(t) => t.track.segments.get(segment_index),
            _ => None,
        }
        .ok_or_else(|| {
            video_editor::Error::InvalidConfig(
                tr("Only video or image segments can extract frames").into(),
            )
        })?;

        let segment_start = segment.timeline_offset;
        let segment_end = segment_start + segment.duration;
        if playhead < segment_start || playhead >= segment_end {
            return Err(video_editor::Error::InvalidConfig(
                tr("Playhead is not within the selected segment").into(),
            ));
        }

        let source_name = segment.metadata.path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let frame = segment.frame_image_at_timeline_offset(playhead)?;
        Ok((frame, source_name))
    });

    match result {
        Ok((frame_image, source_name)) => {
            let ui_weak = ui.as_weak();
            tokio::spawn(async move {
                save_extracted_frame_and_add_to_playlist(ui_weak, frame_image, playhead_ms, source_name).await;
            });
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to extract frame"), e)),
    }
}

async fn save_extracted_frame_and_add_to_playlist(
    ui_weak: Weak<AppWindow>,
    frame_image: image::RgbaImage,
    playhead_ms: i32,
    source_name: String,
) {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{}_frame_{}_{}.png", source_name, timestamp, playhead_ms);
    let cache_dir = crate::config::all().cache_dir.join("extracted_frames");

    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        toast::async_toast_warn(
            ui_weak.clone(),
            format!("{}: {}", tr("Failed to create cache directory"), e),
        );
        return;
    }

    let frame_path = cache_dir.join(&filename);
    if let Err(e) = frame_image.save(&frame_path) {
        toast::async_toast_warn(
            ui_weak.clone(),
            format!("{}: {}", tr("Failed to save frame"), e),
        );
        return;
    }

    import_file_to_playlist(ui_weak.clone(), frame_path, None).await;
}

fn video_editor_playlist_search(ui: &AppWindow, index: i32, keyword: SharedString) {
    common::search(CTX, ui, index, keyword);
}

fn video_editor_playlist_all_files(ui: &AppWindow) {
    common::filter_by_type(CTX, ui, None);
}

fn video_editor_playlist_all_videos(ui: &AppWindow) {
    common::filter_by_type(CTX, ui, Some(MediaType::Video));
}

fn video_editor_playlist_all_audios(ui: &AppWindow) {
    common::filter_by_type(CTX, ui, Some(MediaType::Audio));
}

fn video_editor_playlist_all_images(ui: &AppWindow) {
    common::filter_by_type(CTX, ui, Some(MediaType::Image));
}

fn video_editor_playlist_all_subtitles(ui: &AppWindow) {
    common::filter_by_type(CTX, ui, Some(MediaType::Subtitle));
}

fn video_editor_playlist_create_folder(ui: &AppWindow) {
    common::create_folder(CTX, ui);
}

fn video_editor_playlist_rename_folder(ui: &AppWindow, id: SharedString) {
    common::rename_folder(CTX, ui, id);
}

fn video_editor_playlist_remove_folder(ui: &AppWindow, id: SharedString) {
    common::remove_folder(CTX, ui, id);
}

fn video_editor_playlist_sync_folder(ui: &AppWindow, id: SharedString) {
    let ui_weak = ui.as_weak();
    let folder_id = id.to_string();

    tokio::spawn(async move {
        toast::async_toast_info(
            ui_weak.clone(),
            tr("Syncing folder, this may take a while..."),
        );

        let result = tokio::task::spawn_blocking(move || {
            let mut state = PROJECT_STATE.lock().unwrap();
            if let Some(ref mut s) = *state {
                s.playlist.sync_folder(&folder_id)
            } else {
                Err(video_editor::Error::InvalidConfig(
                    "Project state not initialized".to_string(),
                ))
            }
        })
        .await
        .unwrap_or_else(|e| Err(video_editor::Error::InvalidConfig(e.to_string())));

        _ = ui_weak.upgrade_in_event_loop(move |ui| match result {
            Ok(sync_result) => {
                if !sync_result.removed.is_empty() || !sync_result.added.is_empty() {
                    let msg = if sync_result.added.is_empty() {
                        format!(
                            "{}: {} {}",
                            tr("Synced"),
                            sync_result.removed.len(),
                            tr("removed")
                        )
                    } else if sync_result.removed.is_empty() {
                        format!(
                            "{}: {} {}",
                            tr("Synced"),
                            sync_result.added.len(),
                            tr("added")
                        )
                    } else {
                        format!(
                            "{}: {} {}, {} {}",
                            tr("Synced"),
                            sync_result.removed.len(),
                            tr("removed"),
                            sync_result.added.len(),
                            tr("added")
                        )
                    };
                    crate::toast_success!(ui, msg);
                } else {
                    crate::toast_success!(ui, tr("Already up to date"));
                }
                common::refresh_ui_filtered(CTX, &ui);
            }
            Err(e) => {
                crate::toast_warn!(ui, format!("{}: {}", tr("Sync failed"), e));
            }
        });
    });
}

fn video_editor_playlist_refresh(ui: &AppWindow) {
    common::refresh_media_list(CTX, ui);
}

fn video_editor_playlist_navigate_to_folder(ui: &AppWindow, id: SharedString) {
    common::navigate_to_folder(CTX, ui, id);
}

fn video_editor_playlist_navigate_up(ui: &AppWindow) {
    common::navigate_up(CTX, ui);
}

fn video_editor_playlist_move_item_to_folder(
    ui: &AppWindow,
    item_id: SharedString,
    folder_id: SharedString,
) {
    common::move_item_to_folder(CTX, ui, item_id, folder_id);
}

fn video_editor_playlist_move_folder(
    ui: &AppWindow,
    folder_id: SharedString,
    new_parent_id: SharedString,
) {
    common::move_folder(CTX, ui, folder_id, new_parent_id);
}

pub fn video_editor_playlist_clear_selection(ui: &AppWindow) {
    common::clear_selection(CTX, ui);
}

pub async fn async_add_items_to_existing_track(
    ui_weak: Weak<AppWindow>,
    track_index: i32,
    timeline_offset_ms: i32,
    items: Vec<(String, PathBuf)>,
) {
    let mut metadata_list: Vec<Arc<Metadata>> = Vec::new();
    for (name, path) in &items {
        match get_metadata(path) {
            Ok(meta) => metadata_list.push(Arc::new(meta)),
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {}: {}", tr("Failed to load metadata"), name, e),
                );
                return;
            }
        }
    }

    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        let timeline_offset = Duration::from_millis(timeline_offset_ms as u64);
        let subtitle_style: SubtitleStyleConfig = global_ve_filter!(ui).get_subtitle_style().into();

        let result: Result<ExecuteResult, video_editor::Error> = with_history_manager(|state| {
            let track_idx = track_index as usize;
            if track_idx >= state.tracks_manager.len() {
                return Err(video_editor::Error::IndexOutOfBounds(
                    track_idx,
                    state.tracks_manager.len(),
                ));
            }

            // Check track type compatibility for all items first (before borrowing mutably)
            let track_type_compatible: Vec<bool> = metadata_list
                .iter()
                .map(|meta| {
                    let track = state.tracks_manager.get(track_idx).unwrap();
                    track_matches_metadata_type(track, meta)
                })
                .collect();

            state
                .history_manager
                .begin_batch(format!("Add {} items to existing track", items.len()));

            let global_speed = state.tracks_manager.get_global_speed();

            // Start from the given timeline_offset for the first segment
            let mut current_offset = timeline_offset;

            let mut execute_result = ExecuteResult {
                affected_segments: AffectedSegments::default(),
            };

            for (i, metadata) in metadata_list.iter().enumerate() {
                // Skip items that don't match the track type
                if !track_type_compatible[i] {
                    continue;
                }

                if metadata.get_type() == MetadataType::Subtitle {
                    let subtitle_meta = metadata.subtitles.first().ok_or_else(|| {
                        video_editor::Error::InvalidConfig("No subtitle stream".to_string())
                    })?;
                    let segments = extract_subtitles_as_segments(
                        &metadata.path,
                        subtitle_meta.index,
                        metadata.clone(),
                        global_speed,
                    )?;

                    for seg in &segments {
                        let mut adjusted = (**seg).clone();
                        adjusted.timeline_offset = current_offset + seg.timeline_offset;
                        adjusted.global_speed = global_speed;
                        let segment = Arc::new(adjusted);

                        let base_seg_idx = state
                            .tracks_manager
                            .get(track_idx)
                            .unwrap()
                            .segments_count();
                        let seg_result = state.history_manager.execute(
                            &mut state.tracks_manager,
                            Box::new(AddSegmentCommand::new(track_idx, segment)),
                        )?;
                        execute_result
                            .affected_segments
                            .segments
                            .extend(seg_result.affected_segments.segments);

                        let filters = create_subtitle_style_filters_from_config(&subtitle_style);
                        for filter in filters {
                            let filter_result = state.history_manager.execute(
                                &mut state.tracks_manager,
                                Box::new(AddFilterCommand::new_subtitle(
                                    track_idx,
                                    base_seg_idx,
                                    filter,
                                )),
                            )?;
                            execute_result
                                .affected_segments
                                .segments
                                .extend(filter_result.affected_segments.segments);
                        }
                    }

                    if let Some(last) = segments.last() {
                        current_offset += last.timeline_offset + last.duration;
                    }
                } else {
                    let segment_duration = if metadata.is_image() && metadata.duration.is_zero() {
                        Duration::from_secs(5)
                    } else {
                        metadata.duration
                    };

                    let mut segment = Segment::new(
                        current_offset,
                        segment_duration,
                        metadata.clone(),
                        global_speed,
                    );
                    segment.generate_uuid();
                    let segment = Arc::new(segment);

                    let seg_result = state.history_manager.execute(
                        &mut state.tracks_manager,
                        Box::new(AddSegmentCommand::new(track_idx, segment)),
                    )?;
                    execute_result
                        .affected_segments
                        .segments
                        .extend(seg_result.affected_segments.segments);

                    current_offset += segment_duration;
                }
            }

            state.history_manager.end_batch()?;
            Ok(execute_result)
        });

        match result {
            Ok(execute_result) => {
                sync_and_refresh(&ui, execute_result.affected_segments, Some(true));
                global_store!(ui).set_video_editor_item_new_track_insert_index(-1);
                reset_editor_selection_state(&ui);
                crate::toast_success!(
                    ui,
                    format!("{} {} {}", tr("Added"), items.len(), tr("items to track"))
                );
            }
            Err(e) => crate::toast_warn!(ui, e.to_string()),
        }
    });
}

/// Batch add multiple items as new tracks, grouped by media type.
/// Same-type items are placed sequentially on the same track.
pub async fn async_add_items_to_track(ui_weak: Weak<AppWindow>, items: Vec<(String, PathBuf)>) {
    let mut metadata_list: Vec<Arc<Metadata>> = Vec::new();
    for (name, path) in &items {
        match get_metadata(path) {
            Ok(meta) => metadata_list.push(Arc::new(meta)),
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {}: {}", tr("Failed to load metadata"), name, e),
                );
                return;
            }
        }
    }

    // Group by media type
    let mut type_groups: HashMap<String, Vec<Arc<Metadata>>> = HashMap::new();
    for meta in &metadata_list {
        let mt = format!("{:?}", meta.get_type());
        type_groups.entry(mt).or_default().push(meta.clone());
    }

    // Sort groups by track priority (lower value = higher priority = processed first)
    let mut groups: Vec<(TrackPriority, Vec<Arc<Metadata>>)> = type_groups
        .into_iter()
        .map(|(_, group)| {
            let first = &group[0];
            let priority = TrackPriority::from(first.get_type());
            (priority, group)
        })
        .collect();
    groups.sort_by_key(|(priority, _)| *priority);

    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        let subtitle_style: SubtitleStyleConfig = global_ve_filter!(ui).get_subtitle_style().into();
        let new_track_insert_index =
            global_store!(ui).get_video_editor_item_new_track_insert_index();

        let result: Result<ExecuteResult, video_editor::Error> = with_history_manager(|state| {
            state
                .history_manager
                .begin_batch(format!("Add {} items to track", items.len()));

            let global_speed = state.tracks_manager.get_global_speed();
            let mut execute_result = ExecuteResult {
                affected_segments: AffectedSegments::default(),
            };

            // Process each group: find position → insert track → add remaining segments.
            // This ensures find_valid_insert_position always sees the latest state,
            // so Image/Video tracks (same priority) are correctly placed.
            for (_priority, group) in &groups {
                let first_meta = &group[0];
                let tracks = Track::new(&first_meta.path, global_speed)
                    .map_err(|e| video_editor::Error::InvalidConfig(e.to_string()))?;

                for (track_i, track) in tracks.into_iter().enumerate() {
                    let requested_index = if new_track_insert_index >= 0 {
                        new_track_insert_index as usize
                    } else {
                        state.tracks_manager.len()
                    };

                    // Find actual position before inserting (reflects current state)
                    let actual_idx = state
                        .tracks_manager
                        .find_valid_insert_position(requested_index, &track);

                    let result = state.history_manager.execute(
                        &mut state.tracks_manager,
                        Box::new(InsertTrackCommand::new(track, requested_index)),
                    )?;
                    execute_result
                        .affected_segments
                        .segments
                        .extend(result.affected_segments.segments);

                    // Only add remaining segments to the primary track (first from Track::new)
                    if track_i == 0 {
                        if first_meta.get_type() == MetadataType::Subtitle {
                            let track = state.tracks_manager.get(actual_idx).unwrap();
                            let segments_count = track.segments_count();
                            for seg_idx in 0..segments_count {
                                let filters =
                                    create_subtitle_style_filters_from_config(&subtitle_style);
                                for filter in filters {
                                    let filter_result = state.history_manager.execute(
                                        &mut state.tracks_manager,
                                        Box::new(AddFilterCommand::new_subtitle(
                                            actual_idx, seg_idx, filter,
                                        )),
                                    )?;
                                    execute_result
                                        .affected_segments
                                        .segments
                                        .extend(filter_result.affected_segments.segments);
                                }
                            }
                        }

                        let first_duration =
                            if first_meta.is_image() && first_meta.duration.is_zero() {
                                Duration::from_secs(5)
                            } else {
                                first_meta.duration
                            };
                        let mut current_offset = first_duration;

                        for metadata in group.iter().skip(1) {
                            if metadata.get_type() == MetadataType::Subtitle {
                                let subtitle_meta =
                                    metadata.subtitles.first().ok_or_else(|| {
                                        video_editor::Error::InvalidConfig(
                                            "No subtitle stream".to_string(),
                                        )
                                    })?;
                                let segments = extract_subtitles_as_segments(
                                    &metadata.path,
                                    subtitle_meta.index,
                                    metadata.clone(),
                                    global_speed,
                                )?;

                                for seg in &segments {
                                    let mut adjusted = (**seg).clone();
                                    adjusted.timeline_offset = current_offset + seg.timeline_offset;
                                    adjusted.global_speed = global_speed;
                                    let segment = Arc::new(adjusted);

                                    let base_seg_idx = state
                                        .tracks_manager
                                        .get(actual_idx)
                                        .unwrap()
                                        .segments_count();
                                    let seg_result = state.history_manager.execute(
                                        &mut state.tracks_manager,
                                        Box::new(AddSegmentCommand::new(actual_idx, segment)),
                                    )?;
                                    execute_result
                                        .affected_segments
                                        .segments
                                        .extend(seg_result.affected_segments.segments);

                                    let filters =
                                        create_subtitle_style_filters_from_config(&subtitle_style);
                                    for filter in filters {
                                        let filter_result = state.history_manager.execute(
                                            &mut state.tracks_manager,
                                            Box::new(AddFilterCommand::new_subtitle(
                                                actual_idx,
                                                base_seg_idx,
                                                filter,
                                            )),
                                        )?;
                                        execute_result
                                            .affected_segments
                                            .segments
                                            .extend(filter_result.affected_segments.segments);
                                    }
                                }
                                if let Some(last) = segments.last() {
                                    current_offset += last.timeline_offset + last.duration;
                                }
                            } else {
                                let segment_duration =
                                    if metadata.is_image() && metadata.duration.is_zero() {
                                        Duration::from_secs(5)
                                    } else {
                                        metadata.duration
                                    };

                                let mut segment = Segment::new(
                                    current_offset,
                                    segment_duration,
                                    metadata.clone(),
                                    global_speed,
                                );
                                segment.generate_uuid();
                                let segment = Arc::new(segment);

                                let seg_result = state.history_manager.execute(
                                    &mut state.tracks_manager,
                                    Box::new(AddSegmentCommand::new(actual_idx, segment)),
                                )?;
                                execute_result
                                    .affected_segments
                                    .segments
                                    .extend(seg_result.affected_segments.segments);

                                current_offset += segment_duration;
                            }
                        }
                    }
                }
            }

            state.history_manager.end_batch()?;
            Ok(execute_result)
        });

        match result {
            Ok(execute_result) => {
                sync_and_refresh(&ui, execute_result.affected_segments, Some(true));
                global_store!(ui).set_video_editor_item_new_track_insert_index(-1);
                reset_editor_selection_state(&ui);
                crate::toast_success!(
                    ui,
                    format!("{} {} {}", tr("Added"), items.len(), tr("items to track"))
                );
            }
            Err(e) => crate::toast_warn!(ui, e.to_string()),
        }
    });
}
