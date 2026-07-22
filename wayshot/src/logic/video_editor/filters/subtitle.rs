use crate::{
    db::VIDEO_EDITOR_TABLE,
    from_filter_json, global_ve_filter,
    logic::{
        tr::tr,
        video_editor::{
            command::{
                refresh_preview, sync_and_refresh, sync_manager_to_ui, with_history_manager,
            },
            common_type::{
                PresetSubtitleStyleConfig, PresetSubtitleStyleData, SubtitleStyleConfig,
            },
            filters::filter::save_preset_subtitle_styles_to_db,
            project::{PRESET_SUBTITLE_STYLES_ID, SUBTITLE_STYLE_ID},
            segment::refresh_affected_segments,
        },
    },
    slint_generatedAppWindow::{
        AppWindow, BackgroundColorDetail as UIBackgroundColorDetail,
        BorderRadiusDetail as UIBorderRadiusDetail, FontPathDetail as UIFontPathDetail,
        FontSizeDetail as UIFontSizeDetail, MarginHorizontalDetail as UIMarginHorizontalDetail,
        MarginVerticalDetail as UIMarginVerticalDetail, OutlineColorDetail as UIOutlineColorDetail,
        OutlineWidthDetail as UIOutlineWidthDetail, PaddingDetail as UIPaddingDetail,
        PresetSubtitleStyle as UIPresetSubtitleStyle, PrimaryColorDetail as UIPrimaryColorDetail,
        SubtitleStyle as UISubtitleStyle, TextAlignmentDetail as UITextAlignmentDetail,
    },
    ve_filter_cb,
};
use slint::{ComponentHandle, Model, SharedString, VecModel};
use std::{path::PathBuf, sync::Arc};
use video_editor::{
    commands::{
        BatchCommand,
        filter::{InsertFilterCommand, RemoveFilterCommand},
    },
    filters::{
        subtitle::style::{
            BackgroundColorFilter, BorderRadiusFilter, FontPathFilter, FontSizeFilter,
            MarginHorizontalFilter, MarginVerticalFilter, OutlineColorFilter, OutlineWidthFilter,
            PaddingFilter, PrimaryColorFilter, TextAlignment, TextAlignmentFilter,
        },
        traits::{SubtitleFilter, SubtitleFilterWrapper},
    },
    tracks::track::Track,
};

#[macro_export]
macro_rules! ve_filter_preset_subtitle_styles {
    ($ui:expr) => {
        crate::global_ve_filter!($ui)
            .get_preset_subtitle_styles()
            .as_any()
            .downcast_ref::<VecModel<UIPresetSubtitleStyle>>()
            .expect("We know we set a VecModel<UIPresetSubtitleStyle> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    ve_filter_cb!(
        video_editor_update_subtitle_style_from_track,
        ui,
        track_index
    );

    ve_filter_cb!(show_preset_subtitle_style_panel, ui);
    ve_filter_cb!(show_preset_subtitle_style_new_lineinput, ui);
    ve_filter_cb!(create_preset_subtitle_style, ui, name);
    ve_filter_cb!(remove_preset_subtitle_style, ui, index);
    ve_filter_cb!(apply_preset_subtitle_style, ui, track_index, style);

    ve_filter_cb!(from_font_size_json, ui, json);
    ve_filter_cb!(from_padding_json, ui, json);
    ve_filter_cb!(from_margin_vertical_json, ui, json);
    ve_filter_cb!(from_margin_horizontal_json, ui, json);
    ve_filter_cb!(from_outline_width_json, ui, json);
    ve_filter_cb!(from_border_radius_json, ui, json);
    ve_filter_cb!(from_primary_color_json, ui, json);
    ve_filter_cb!(from_outline_color_json, ui, json);
    ve_filter_cb!(from_background_color_json, ui, json);
    ve_filter_cb!(from_font_path_json, ui, json);

    ve_filter_cb!(from_text_alignment_json, ui, json);
    ve_filter_cb!(modify_text_alignment_filter, ui, index, config);

    ve_filter_cb!(modify_font_size_filter, ui, index, config);
    ve_filter_cb!(modify_padding_filter, ui, index, config);
    ve_filter_cb!(modify_margin_vertical_filter, ui, index, config);
    ve_filter_cb!(modify_margin_horizontal_filter, ui, index, config);
    ve_filter_cb!(modify_outline_width_filter, ui, index, config);
    ve_filter_cb!(modify_border_radius_filter, ui, index, config);
    ve_filter_cb!(modify_primary_color_filter, ui, index, config);
    ve_filter_cb!(modify_outline_color_filter, ui, index, config);
    ve_filter_cb!(modify_background_color_filter, ui, index, config);
    ve_filter_cb!(modify_font_path_filter, ui, index, config);
}

fn inner_init(ui: &AppWindow) {
    load_subtitle_style_from_db(ui);
    load_preset_subtitle_styles_from_db(ui);
}

// `index` is the current editing track index.
// When updating, need to update ALL segments in the current editing track.
// If no filter exists, add it; if filter exists, modify it.
// Also save VEFilter.subtitle-style to database.
macro_rules! impl_modify_subtitle_filter {
    ($func_name:ident, $filter_type:ty, $ui_type:ty) => {
        fn $func_name(ui: &AppWindow, track_index: i32, config: $ui_type) {
            let track_idx = track_index as usize;

            let segment_count = with_history_manager(|state| {
                state
                    .tracks_manager
                    .get(track_idx)
                    .map(|t| t.segments_count())
                    .unwrap_or(0)
            });

            if segment_count == 0 {
                save_subtitle_style_to_db(ui);
                return;
            }

            let new_filter: $filter_type = config.into();
            let mut batch_command = BatchCommand::new(format!(
                "Update {} filter for all segments",
                <$filter_type>::NAME
            ));

            // For each segment, add or update filter
            for seg_idx in 0..segment_count {
                let existing_index: Option<usize> = with_history_manager(|state| {
                    let track = state.tracks_manager.get(track_idx)?;
                    let segment = track.get_segment(seg_idx).ok()?;
                    segment
                        .subtitle_filters
                        .iter()
                        .position(|f| f.inner.name() == <$filter_type>::NAME)
                });

                match existing_index {
                    Some(idx) => {
                        // Remove and re-insert to update
                        batch_command.add_command(Box::new(
                            video_editor::commands::filter::RemoveFilterCommand::new_subtitle(
                                track_idx, seg_idx, idx,
                            ),
                        ));
                        batch_command.add_command(Box::new(InsertFilterCommand::new_subtitle(
                            track_idx,
                            seg_idx,
                            idx,
                            Box::new(new_filter.clone()),
                        )));
                    }
                    None => {
                        // Add new filter
                        batch_command.add_command(Box::new(
                            video_editor::commands::filter::AddFilterCommand::new_subtitle(
                                track_idx,
                                seg_idx,
                                Box::new(new_filter.clone()),
                            ),
                        ));
                    }
                }
            }

            // Execute batch command
            let result = with_history_manager(|state| {
                state
                    .history_manager
                    .execute(&mut state.tracks_manager, Box::new(batch_command))
            });

            match result {
                Ok(execute_result) => {
                    sync_and_refresh(ui, execute_result.affected_segments, Some(true));
                    save_subtitle_style_to_db(ui);
                }
                Err(e) => {
                    crate::toast_warn!(ui, format!("{}: {}", tr("Failed to update filter"), e))
                }
            }
        }
    };
}

impl_modify_subtitle_filter!(modify_font_size_filter, FontSizeFilter, UIFontSizeDetail);
impl_modify_subtitle_filter!(modify_padding_filter, PaddingFilter, UIPaddingDetail);
impl_modify_subtitle_filter!(
    modify_margin_vertical_filter,
    MarginVerticalFilter,
    UIMarginVerticalDetail
);
impl_modify_subtitle_filter!(
    modify_margin_horizontal_filter,
    MarginHorizontalFilter,
    UIMarginHorizontalDetail
);
impl_modify_subtitle_filter!(
    modify_outline_width_filter,
    OutlineWidthFilter,
    UIOutlineWidthDetail
);
impl_modify_subtitle_filter!(
    modify_border_radius_filter,
    BorderRadiusFilter,
    UIBorderRadiusDetail
);
impl_modify_subtitle_filter!(
    modify_primary_color_filter,
    PrimaryColorFilter,
    UIPrimaryColorDetail
);
impl_modify_subtitle_filter!(
    modify_outline_color_filter,
    OutlineColorFilter,
    UIOutlineColorDetail
);
impl_modify_subtitle_filter!(
    modify_background_color_filter,
    BackgroundColorFilter,
    UIBackgroundColorDetail
);
impl_modify_subtitle_filter!(modify_font_path_filter, FontPathFilter, UIFontPathDetail);
impl_modify_subtitle_filter!(
    modify_text_alignment_filter,
    TextAlignmentFilter,
    UITextAlignmentDetail
);

from_filter_json!(from_font_size_json, FontSizeFilter, UIFontSizeDetail);
from_filter_json!(from_padding_json, PaddingFilter, UIPaddingDetail);
from_filter_json!(
    from_margin_vertical_json,
    MarginVerticalFilter,
    UIMarginVerticalDetail
);
from_filter_json!(
    from_margin_horizontal_json,
    MarginHorizontalFilter,
    UIMarginHorizontalDetail
);
from_filter_json!(
    from_outline_width_json,
    OutlineWidthFilter,
    UIOutlineWidthDetail
);
from_filter_json!(
    from_border_radius_json,
    BorderRadiusFilter,
    UIBorderRadiusDetail
);
from_filter_json!(
    from_primary_color_json,
    PrimaryColorFilter,
    UIPrimaryColorDetail
);
from_filter_json!(
    from_outline_color_json,
    OutlineColorFilter,
    UIOutlineColorDetail
);
from_filter_json!(
    from_background_color_json,
    BackgroundColorFilter,
    UIBackgroundColorDetail
);
from_filter_json!(from_font_path_json, FontPathFilter, UIFontPathDetail);
from_filter_json!(
    from_text_alignment_json,
    TextAlignmentFilter,
    UITextAlignmentDetail
);

fn save_subtitle_style_to_db(ui: &AppWindow) {
    let subtitle_style: SubtitleStyleConfig =
        crate::global_ve_filter!(ui).get_subtitle_style().into();
    tokio::spawn(async move {
        let data = serde_json::to_string(&subtitle_style).unwrap_or_default();
        if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, SUBTITLE_STYLE_ID, &data).await {
            log::warn!("Failed to save subtitle style to database: {}", e);
        }
    });
}

fn load_subtitle_style_from_db(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, SUBTITLE_STYLE_ID).await {
            Ok(item) => serde_json::from_str::<SubtitleStyleConfig>(&item.data).unwrap_or_default(),
            Err(_) => {
                _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, SUBTITLE_STYLE_ID, "{}").await;
                Default::default()
            }
        };
        let ui_subtitle_style = config.into();
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            crate::global_ve_filter!(ui).set_subtitle_style(ui_subtitle_style);
        });
    });
}

fn video_editor_update_subtitle_style_from_track(ui: &AppWindow, track_index: i32) {
    if track_index < 0 {
        return;
    }

    let track_idx = track_index as usize;
    let is_subtitle = with_history_manager(|state| {
        state
            .tracks_manager
            .get(track_idx)
            .map(|t| matches!(t, Track::Subtitle(_)))
            .unwrap_or(false)
    });

    if !is_subtitle {
        return;
    }

    let subtitle_style = with_history_manager(|state| {
        state.tracks_manager.get(track_idx).and_then(|track| {
            if let Track::Subtitle(inner) = track {
                inner
                    .track
                    .segments
                    .first()
                    .map(|seg| extract_subtitle_style_from_filters(&seg.subtitle_filters))
            } else {
                None
            }
        })
    });

    if let Some(style) = subtitle_style {
        crate::global_ve_filter!(ui).set_subtitle_style(style.into());
    }
}

fn extract_subtitle_style_from_filters(
    filters: &[Arc<SubtitleFilterWrapper>],
) -> SubtitleStyleConfig {
    let mut config = SubtitleStyleConfig::default();

    for filter_wrapper in filters {
        let filter = &filter_wrapper.inner;
        let filter_name = filter.name();

        if filter_name == FontPathFilter::NAME {
            if let Some(f) = filter.as_any().downcast_ref::<FontPathFilter>() {
                config.font_path = f.font_path.to_string_lossy().to_string();
                config.font_family = f.font_family.clone();
            }
        } else if filter_name == FontSizeFilter::NAME {
            if let Some(f) = filter.as_any().downcast_ref::<FontSizeFilter>() {
                config.font_size = f.font_size as i32;
            }
        } else if filter_name == PaddingFilter::NAME {
            if let Some(f) = filter.as_any().downcast_ref::<PaddingFilter>() {
                config.padding = f.padding.unwrap_or(4) as i32;
            }
        } else if filter_name == MarginVerticalFilter::NAME {
            if let Some(f) = filter.as_any().downcast_ref::<MarginVerticalFilter>() {
                config.margin_vertical = f.margin.unwrap_or(30) as i32;
            }
        } else if filter_name == MarginHorizontalFilter::NAME {
            if let Some(f) = filter.as_any().downcast_ref::<MarginHorizontalFilter>() {
                config.margin_horizontal = f.margin.unwrap_or(0) as i32;
            }
        } else if filter_name == OutlineWidthFilter::NAME {
            if let Some(f) = filter.as_any().downcast_ref::<OutlineWidthFilter>() {
                config.outline_width = f.width.unwrap_or(2) as i32;
            }
        } else if filter_name == BorderRadiusFilter::NAME {
            if let Some(f) = filter.as_any().downcast_ref::<BorderRadiusFilter>() {
                config.border_radius = f.radius.unwrap_or(0) as i32;
            }
        } else if filter_name == PrimaryColorFilter::NAME {
            if let Some(f) = filter.as_any().downcast_ref::<PrimaryColorFilter>()
                && let Some(color) = f.color
            {
                config.primary_color_r = color[0] as i32;
                config.primary_color_g = color[1] as i32;
                config.primary_color_b = color[2] as i32;
                config.primary_color_a = color[3] as i32;
            }
        } else if filter_name == OutlineColorFilter::NAME {
            if let Some(f) = filter.as_any().downcast_ref::<OutlineColorFilter>()
                && let Some(color) = f.color
            {
                config.outline_color_r = color[0] as i32;
                config.outline_color_g = color[1] as i32;
                config.outline_color_b = color[2] as i32;
                config.outline_color_a = color[3] as i32;
            }
        } else if filter_name == BackgroundColorFilter::NAME {
            if let Some(f) = filter.as_any().downcast_ref::<BackgroundColorFilter>()
                && let Some(color) = f.color
            {
                config.background_color_r = color[0] as i32;
                config.background_color_g = color[1] as i32;
                config.background_color_b = color[2] as i32;
                config.background_color_a = color[3] as i32;
            }
        } else if filter_name == TextAlignmentFilter::NAME {
            if let Some(f) = filter.as_any().downcast_ref::<TextAlignmentFilter>() {
                config.text_alignment = f
                    .alignment
                    .map(|a| match a {
                        TextAlignment::Left => 0,
                        TextAlignment::Center => 1,
                        TextAlignment::Right => 2,
                    })
                    .unwrap_or(1);
            }
        }
    }

    config
}

pub fn create_subtitle_style_filters_from_config(
    config: &SubtitleStyleConfig,
) -> Vec<Box<dyn SubtitleFilter>> {
    use video_editor::filters::subtitle::style::*;

    vec![
        Box::new(FontPathFilter::new(
            PathBuf::from(&config.font_path),
            config.font_family.clone(),
            config.font_style.clone(),
        )),
        Box::new(FontSizeFilter::new(config.font_size as u32)),
        Box::new(MarginVerticalFilter::new(Some(
            config.margin_vertical as u32,
        ))),
        Box::new(MarginHorizontalFilter::new(config.margin_horizontal)),
        Box::new(PaddingFilter::new(config.padding)),
        Box::new(OutlineWidthFilter::new(config.outline_width)),
        Box::new(BorderRadiusFilter::new(config.border_radius)),
        Box::new(PrimaryColorFilter::from_rgba(
            config.primary_color_r as u8,
            config.primary_color_g as u8,
            config.primary_color_b as u8,
            config.primary_color_a as u8,
        )),
        Box::new(OutlineColorFilter::from_rgba(
            config.outline_color_r as u8,
            config.outline_color_g as u8,
            config.outline_color_b as u8,
            config.outline_color_a as u8,
        )),
        Box::new(BackgroundColorFilter::from_rgba(
            config.background_color_r as u8,
            config.background_color_g as u8,
            config.background_color_b as u8,
            config.background_color_a as u8,
        )),
        Box::new(TextAlignmentFilter::new(match config.text_alignment {
            0 => TextAlignment::Left,
            1 => TextAlignment::Center,
            2 => TextAlignment::Right,
            _ => TextAlignment::Center,
        })),
    ]
}

fn show_preset_subtitle_style_panel(ui: &AppWindow) {
    global_ve_filter!(ui).set_is_show_preset_subtitle_style_panel(true);
}

fn show_preset_subtitle_style_new_lineinput(ui: &AppWindow) {
    global_ve_filter!(ui).set_is_show_preset_subtitle_style_new_lineinput(true);
}

fn create_preset_subtitle_style(ui: &AppWindow, name: SharedString) {
    if name.is_empty() {
        crate::toast_warn!(ui, tr("Name cannot be empty"));
        return;
    }

    let style_data = UIPresetSubtitleStyle {
        name,
        style: global_ve_filter!(ui).get_subtitle_style().into(),
    };

    ve_filter_preset_subtitle_styles!(ui).push(style_data);
    let config = collect_preset_subtitle_styles_from_ui(ui);
    save_preset_subtitle_styles_to_db(ui.as_weak(), config);
    crate::toast_success!(ui, tr("Preset subtitle style created"));
}

fn remove_preset_subtitle_style(ui: &AppWindow, index: i32) {
    let idx = index as usize;
    if idx < ve_filter_preset_subtitle_styles!(ui).row_count() {
        ve_filter_preset_subtitle_styles!(ui).remove(idx);
        let config = collect_preset_subtitle_styles_from_ui(ui);
        save_preset_subtitle_styles_to_db(ui.as_weak(), config);
        crate::toast_success!(ui, tr("Preset subtitle style removed"));
    }
}

fn load_preset_subtitle_styles_from_db(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, PRESET_SUBTITLE_STYLES_ID).await
        {
            Ok(item) => {
                serde_json::from_str::<PresetSubtitleStyleConfig>(&item.data).unwrap_or_default()
            }
            Err(_) => {
                _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, PRESET_SUBTITLE_STYLES_ID, "{}").await;
                PresetSubtitleStyleConfig::default()
            }
        };

        let ui_styles: Vec<UIPresetSubtitleStyle> =
            config.styles.into_iter().map(|s| s.into()).collect();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            ve_filter_preset_subtitle_styles!(ui).set_vec(ui_styles);
        });
    });
}

fn collect_preset_subtitle_styles_from_ui(ui: &AppWindow) -> PresetSubtitleStyleConfig {
    let data: Vec<PresetSubtitleStyleData> = ve_filter_preset_subtitle_styles!(ui)
        .iter()
        .map(|s| s.into())
        .collect();

    PresetSubtitleStyleConfig { styles: data }
}

fn apply_preset_subtitle_style(ui: &AppWindow, track_index: i32, style: UISubtitleStyle) {
    if track_index < 0 {
        return;
    }

    let track_idx = track_index as usize;
    let config: SubtitleStyleConfig = style.into();

    let segment_count = with_history_manager(|state| {
        state
            .tracks_manager
            .get(track_idx)
            .map(|t| t.segments_count())
            .unwrap_or(0)
    });

    if segment_count == 0 {
        return;
    }

    let mut batch_command = BatchCommand::new("Apply preset subtitle style".to_string());

    for seg_idx in 0..segment_count {
        let existing_count = with_history_manager(|state| {
            state
                .tracks_manager
                .get(track_idx)
                .and_then(|track| track.get_segment(seg_idx).ok())
                .map(|seg| seg.subtitle_filters.len())
                .unwrap_or(0)
        });

        for filter_idx in (0..existing_count).rev() {
            batch_command.add_command(Box::new(RemoveFilterCommand::new_subtitle(
                track_idx, seg_idx, filter_idx,
            )));
        }

        let filters = create_subtitle_style_filters_from_config(&config);
        for (filter_idx, filter) in filters.into_iter().enumerate() {
            batch_command.add_command(Box::new(InsertFilterCommand::new_subtitle(
                track_idx, seg_idx, filter_idx, filter,
            )));
        }
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_manager_to_ui(ui);
            refresh_affected_segments(ui, execute_result.affected_segments);
            refresh_preview(ui);
            save_subtitle_style_to_db(ui);
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to apply preset style"), e)),
    }
}
