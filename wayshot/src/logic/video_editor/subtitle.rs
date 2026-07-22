use super::{
    command::{sync_manager_to_ui, with_history_manager},
    common_type::SubtitleStyleConfig,
    track::get_selected_segment_indices,
};
use crate::{
    global_store, global_ve_filter,
    logic::tr::tr,
    logic_cb,
    slint_generatedAppWindow::{AppWindow, VideoEditorSubtitle as UIVideoEditorSubtitle},
};
use slint::ComponentHandle;
use std::{path::PathBuf, time::Duration};
use video_editor::{
    Error,
    commands::{
        filter::AddFilterCommand,
        subtitle::{AddSubtitleCommand, UpdateSubtitleCommand},
    },
    filters::subtitle::style::{
        BackgroundColorFilter, BorderRadiusFilter, FontPathFilter, FontSizeFilter,
        MarginHorizontalFilter, MarginVerticalFilter, OutlineColorFilter, OutlineWidthFilter,
        PaddingFilter, PrimaryColorFilter,
    },
    filters::traits::{SubtitleEntry, SubtitleFilter},
    tracks::track::Track,
};
use video_utils::subtitle::{srt_timestamp_to_ms, valid_srt_timestamps};

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_add_subtitle, ui, subtitle);
    logic_cb!(video_editor_update_subtitle, ui, subtitle);
}

fn video_editor_add_subtitle(ui: &AppWindow, subtitle: UIVideoEditorSubtitle) {
    let current_track_index = global_store!(ui).get_video_editor_current_edited_track_index();

    if current_track_index < 0 {
        crate::toast_warn!(ui, tr("No current edited track found"));
        return;
    }

    if let Err(e) = valid_srt_timestamps(&subtitle.start_timestamp, &subtitle.end_timestamp) {
        crate::toast_warn!(ui, format!("{}: {}", tr("Invalid subtitle timestamps"), e));
        return;
    }

    let start_ms = srt_timestamp_to_ms(&subtitle.start_timestamp).unwrap();
    let end_ms = srt_timestamp_to_ms(&subtitle.end_timestamp).unwrap();
    let track_index = current_track_index as usize;

    let entry = SubtitleEntry {
        start: Duration::from_millis(start_ms as u64),
        end: Duration::from_millis(end_ms as u64),
        // Convert \n to \N (ASS line break format) for multi-line support
        text: subtitle.subtitle.replace('\n', "\\N"),
    };

    let subtitle_style: SubtitleStyleConfig = global_ve_filter!(ui).get_subtitle_style().into();

    let result: Result<(), Error> = with_history_manager(|state| {
        let track = state
            .tracks_manager
            .get(track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(track_index, state.tracks_manager.len()))?;

        if !matches!(track, Track::Subtitle(_)) {
            return Err(Error::InvalidConfig(
                "Current track is not a subtitle track. Please select a subtitle track first"
                    .into(),
            ));
        }

        state
            .history_manager
            .begin_batch("Add subtitle with font style".to_string());

        let add_cmd = AddSubtitleCommand::new(track_index, entry.clone());
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(add_cmd))?;

        let inserted_index = state
            .tracks_manager
            .get(track_index)
            .and_then(|t| {
                if let Track::Subtitle(inner) = t {
                    inner
                        .track
                        .segments
                        .iter()
                        .position(|seg| seg.timeline_offset == entry.start)
                } else {
                    None
                }
            })
            .ok_or_else(|| Error::InvalidConfig("Failed to find inserted subtitle index".into()))?;

        let filters = create_filters_from_subtitle_style(&subtitle_style);
        for filter in filters {
            let filter_cmd = AddFilterCommand::new_subtitle(track_index, inserted_index, filter);
            state
                .history_manager
                .execute(&mut state.tracks_manager, Box::new(filter_cmd))?;
        }

        state.history_manager.end_batch()?;

        Ok(())
    });

    match result {
        Ok(_) => {
            sync_manager_to_ui(ui);
            crate::toast_success!(ui, tr("Subtitle added"));
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn create_filters_from_subtitle_style(
    config: &SubtitleStyleConfig,
) -> Vec<Box<dyn SubtitleFilter>> {
    let mut filters: Vec<Box<dyn SubtitleFilter>> = Vec::new();

    if !config.font_path.is_empty() {
        filters.push(Box::new(FontPathFilter::new(
            PathBuf::from(&config.font_path),
            config.font_family.clone(),
            config.font_style.clone(),
        )));
    }

    if config.font_size > 0 {
        filters.push(Box::new(FontSizeFilter::new(config.font_size as u32)));
    }

    filters.push(Box::new(PaddingFilter::new(config.padding)));

    filters.push(Box::new(MarginVerticalFilter::new(Some(
        config.margin_vertical as u32,
    ))));

    filters.push(Box::new(MarginHorizontalFilter::new(
        config.margin_horizontal,
    )));

    filters.push(Box::new(OutlineWidthFilter::new(config.outline_width)));

    filters.push(Box::new(BorderRadiusFilter::new(config.border_radius)));

    filters.push(Box::new(PrimaryColorFilter::from_rgba(
        config.primary_color_r as u8,
        config.primary_color_g as u8,
        config.primary_color_b as u8,
        config.primary_color_a as u8,
    )));

    filters.push(Box::new(OutlineColorFilter::from_rgba(
        config.outline_color_r as u8,
        config.outline_color_g as u8,
        config.outline_color_b as u8,
        config.outline_color_a as u8,
    )));

    filters.push(Box::new(BackgroundColorFilter::from_rgba(
        config.background_color_r as u8,
        config.background_color_g as u8,
        config.background_color_b as u8,
        config.background_color_a as u8,
    )));

    filters
}

fn video_editor_update_subtitle(ui: &AppWindow, subtitle: UIVideoEditorSubtitle) {
    let current_track_index = global_store!(ui).get_video_editor_current_edited_track_index();

    if current_track_index < 0 {
        crate::toast_warn!(ui, tr("No current edited track found"));
        return;
    }

    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        crate::toast_warn!(ui, tr("Please select a subtitle segment to update"));
        return;
    }

    let (selected_track_index, segment_index) = selected_segments[selected_segments.len() - 1];
    if selected_track_index != current_track_index as usize {
        crate::toast_warn!(ui, tr("Selected segment is not in the current track"));
        return;
    }

    if let Err(e) = valid_srt_timestamps(&subtitle.start_timestamp, &subtitle.end_timestamp) {
        crate::toast_warn!(ui, format!("{}: {}", tr("Invalid subtitle timestamps"), e));
        return;
    }

    let track_index = current_track_index as usize;

    let new_entry = with_history_manager(|state| {
        let track = state
            .tracks_manager
            .get(track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(track_index, state.tracks_manager.len()))?;

        if !matches!(track, Track::Subtitle(_)) {
            return Err(Error::InvalidConfig(
                "Current track is not a subtitle track. Please select a subtitle track first"
                    .into(),
            ));
        }

        let segment = track.get_segment(segment_index)?;
        let start = segment.timeline_offset;
        let end = start + segment.duration;

        Ok(SubtitleEntry {
            start,
            end,
            text: subtitle.subtitle.replace('\n', "\\N"),
        })
    });

    let Ok(new_entry) = new_entry else {
        crate::toast_warn!(ui, new_entry.unwrap_err().to_string());
        return;
    };

    let result: Result<(), Error> = with_history_manager(|state| {
        let command = UpdateSubtitleCommand::new(track_index, segment_index, new_entry);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))?;

        Ok(())
    });

    match result {
        Ok(_) => {
            sync_manager_to_ui(ui);
            crate::toast_success!(ui, tr("Subtitle updated"));
        }
        Err(e) => {
            crate::toast_warn!(ui, e.to_string());
        }
    }
}
