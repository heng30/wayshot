use crate::{
    db::DanmakuGlobalFilterConfigData,
    global_store,
    logic::{
        toast,
        tr::tr,
        video_editor::{
            command::refresh_preview, filters::global::save_global_filter_config,
            project::PROJECT_STATE,
        },
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, DanmakuDistributionMode as UIDanmakuDistributionMode,
        DanmakuItem as UIDanmakuItem, DanmakuSegment as UIDanmakuSegment,
        DanmakuStyle as UIDanmakuStyle,
    },
};
use bili_danmaku::{DEFAULT_TIMEOUT, get_all_danmaku_with_limit};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::sync::Arc;
use video_editor::filters::{global::DanmakuFilter, traits::GlobalFilterWrapper};

#[macro_export]
macro_rules! store_video_editor_global_filter_danmaku_segments {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_global_filter_config()
            .danmaku
            .items
            .as_any()
            .downcast_ref::<VecModel<UIDanmakuSegment>>()
            .expect("We know we set a VecModel<UIDanmakuSegment> earlier")
    };
}

#[macro_export]
macro_rules! store_video_editor_global_filter_danmaku_segment_items {
    ($segment:expr) => {
        $segment
            .items
            .as_any()
            .downcast_ref::<VecModel<UIDanmakuItem>>()
            .expect("We know we set a VecModel<UIDanmakuItem> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_global_filter_danmaku_add_segment, ui);
    logic_cb!(video_editor_global_filter_danmaku_remove_segment, ui, index);
    logic_cb!(video_editor_global_filter_danmaku_insert_segment, ui, index);
    logic_cb!(
        video_editor_global_filter_danmaku_move_segment,
        ui,
        from_index,
        to_index
    );
    logic_cb!(video_editor_global_filter_danmaku_remove_all_segments, ui);
    logic_cb!(
        video_editor_global_filter_danmaku_update_offsets,
        ui,
        index,
        start_offset,
        end_offset
    );
    logic_cb!(
        video_editor_global_filter_danmaku_update_scroll_speed,
        ui,
        index,
        speed
    );
    logic_cb!(
        video_editor_global_filter_danmaku_update_style,
        ui,
        index,
        style
    );
    logic_cb!(
        video_editor_global_filter_danmaku_update_distribution,
        ui,
        index,
        distribution
    );
    logic_cb!(
        video_editor_global_filter_danmaku_update_track_count,
        ui,
        index,
        count
    );
    logic_cb!(
        video_editor_global_filter_danmaku_update_track_distribution,
        ui,
        index,
        distribution
    );
    logic_cb!(
        video_editor_global_filter_danmaku_update_position,
        ui,
        index,
        position
    );
    logic_cb!(video_editor_global_filter_danmaku_toggle_enable, ui);
    logic_cb_pure!(video_editor_global_filter_danmaku_is_valid, ui, index, flag);
    logic_cb!(
        video_editor_global_filter_danmaku_add_items,
        ui,
        segment_index,
        text
    );
    logic_cb!(
        video_editor_global_filter_danmaku_remove_item,
        ui,
        segment_index,
        item_index
    );
    logic_cb!(
        video_editor_global_filter_danmaku_remove_all_items,
        ui,
        segment_index
    );
    logic_cb!(
        video_editor_global_filter_danmaku_copy_all_items,
        ui,
        segment_index
    );
    logic_cb!(
        video_editor_global_filter_danmaku_update_item_text,
        ui,
        segment_index,
        item_index,
        text
    );
    logic_cb!(
        video_editor_global_filter_danmaku_fetch_from_bilibili,
        ui,
        url,
        counts
    );
}

pub fn init_from_project(ui: &AppWindow) {
    clear_ui_state(ui);

    let state = PROJECT_STATE.lock().unwrap();
    if let Some(ref s) = *state {
        let danmaku_filter = s
            .tracks_manager
            .get_global_filters()
            .iter()
            .find(|f| f.inner.name() == DanmakuFilter::NAME);

        if let Some(filter) = danmaku_filter
            && let Some(danmaku) = filter.inner.as_any().downcast_ref::<DanmakuFilter>()
        {
            let danmaku: DanmakuGlobalFilterConfigData = danmaku.clone().into();
            let mut config = global_store!(ui).get_video_editor_global_filter_config();
            config.danmaku = danmaku.into();
            config.danmaku.enabled = filter.enabled();

            global_store!(ui).set_video_editor_global_filter_config(config);
        }
    }
}

pub fn sync_danmaku_to_project_state(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_global_filter_config();
    let danmaku_config = &config.danmaku;
    let enabled = danmaku_config.enabled;

    {
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            let filter_idx = s
                .tracks_manager
                .get_global_filters()
                .iter()
                .position(|f| f.inner.name() == DanmakuFilter::NAME);

            let new_filter: DanmakuFilter =
                DanmakuGlobalFilterConfigData::from(danmaku_config.clone()).into();

            let new_wrapper = GlobalFilterWrapper::new(enabled, Box::new(new_filter));
            if let Some(idx) = filter_idx {
                s.tracks_manager.global_filters[idx] = Arc::new(new_wrapper);
            } else {
                s.tracks_manager.add_global_filter(Arc::new(new_wrapper));
            }
        }
    }

    save_global_filter_config(ui);
    refresh_preview(ui);
}

fn video_editor_global_filter_danmaku_add_segment(ui: &AppWindow) {
    let default_style = global_store!(ui)
        .get_video_editor_global_filter_config()
        .danmaku
        .default_style;

    let new_segment = UIDanmakuSegment {
        start_offset: 0,
        end_offset: 0,
        scroll_speed: 200.0,
        distribution: UIDanmakuDistributionMode::Uniform,
        track_count: 0,
        track_distribution: UIDanmakuDistributionMode::Uniform,
        position: 0.0,
        items: ModelRc::new(VecModel::default()),
        style: default_style,
    };

    store_video_editor_global_filter_danmaku_segments!(ui).push(new_segment);
    sync_danmaku_to_project_state(ui);
}

fn video_editor_global_filter_danmaku_remove_segment(ui: &AppWindow, index: i32) {
    if index >= 0
        && index < store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32
    {
        store_video_editor_global_filter_danmaku_segments!(ui).remove(index as usize);
        sync_danmaku_to_project_state(ui);
    }
}

fn video_editor_global_filter_danmaku_insert_segment(ui: &AppWindow, index: i32) {
    let insert_index = (index as usize + 1)
        .min(store_video_editor_global_filter_danmaku_segments!(ui).row_count());

    let default_style = global_store!(ui)
        .get_video_editor_global_filter_config()
        .danmaku
        .default_style;

    let new_segment = UIDanmakuSegment {
        start_offset: 0,
        end_offset: 0,
        scroll_speed: 200.0,
        distribution: UIDanmakuDistributionMode::Uniform,
        track_count: 0,
        track_distribution: UIDanmakuDistributionMode::Uniform,
        position: 0.0,
        items: ModelRc::new(VecModel::default()),
        style: default_style,
    };

    store_video_editor_global_filter_danmaku_segments!(ui).insert(insert_index, new_segment);
    sync_danmaku_to_project_state(ui);
}

fn video_editor_global_filter_danmaku_move_segment(ui: &AppWindow, from_index: i32, to_index: i32) {
    let len = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if from_index >= 0 && from_index < len && to_index >= 0 && to_index < len {
        if let Some(segment) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(from_index as usize)
        {
            store_video_editor_global_filter_danmaku_segments!(ui).remove(from_index as usize);
            store_video_editor_global_filter_danmaku_segments!(ui)
                .insert(to_index as usize, segment);
            sync_danmaku_to_project_state(ui);
        }
    }
}

fn video_editor_global_filter_danmaku_remove_all_segments(ui: &AppWindow) {
    store_video_editor_global_filter_danmaku_segments!(ui).set_vec(vec![]);
    sync_danmaku_to_project_state(ui);
}

fn video_editor_global_filter_danmaku_update_offsets(
    ui: &AppWindow,
    index: i32,
    start_offset: i32,
    end_offset: i32,
) {
    let count = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if index >= 0
        && index < count
        && let Some(item) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(index as usize)
    {
        let updated = UIDanmakuSegment {
            start_offset,
            end_offset,
            scroll_speed: item.scroll_speed,
            distribution: item.distribution,
            track_count: item.track_count,
            track_distribution: item.track_distribution,
            position: item.position,
            items: item.items,
            style: item.style,
        };
        store_video_editor_global_filter_danmaku_segments!(ui)
            .set_row_data(index as usize, updated);
        sync_danmaku_to_project_state(ui);
    }
}

fn video_editor_global_filter_danmaku_update_scroll_speed(ui: &AppWindow, index: i32, speed: f32) {
    let count = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if index >= 0
        && index < count
        && let Some(item) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(index as usize)
    {
        let updated = UIDanmakuSegment {
            start_offset: item.start_offset,
            end_offset: item.end_offset,
            scroll_speed: speed,
            distribution: item.distribution,
            track_count: item.track_count,
            track_distribution: item.track_distribution,
            position: item.position,
            items: item.items,
            style: item.style,
        };
        store_video_editor_global_filter_danmaku_segments!(ui)
            .set_row_data(index as usize, updated);
        sync_danmaku_to_project_state(ui);
    }
}

fn video_editor_global_filter_danmaku_update_style(
    ui: &AppWindow,
    index: i32,
    style: UIDanmakuStyle,
) {
    let count = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if index >= 0
        && index < count
        && let Some(item) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(index as usize)
    {
        let updated = UIDanmakuSegment {
            start_offset: item.start_offset,
            end_offset: item.end_offset,
            scroll_speed: item.scroll_speed,
            distribution: item.distribution,
            track_count: item.track_count,
            track_distribution: item.track_distribution,
            position: item.position,
            items: item.items,
            style: style.clone(),
        };
        store_video_editor_global_filter_danmaku_segments!(ui)
            .set_row_data(index as usize, updated);

        let mut config = global_store!(ui).get_video_editor_global_filter_config();
        config.danmaku.default_style = style;
        global_store!(ui).set_video_editor_global_filter_config(config);

        sync_danmaku_to_project_state(ui);
    }
}

fn video_editor_global_filter_danmaku_update_distribution(
    ui: &AppWindow,
    index: i32,
    distribution: UIDanmakuDistributionMode,
) {
    let count = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if index >= 0
        && index < count
        && let Some(item) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(index as usize)
    {
        let updated = UIDanmakuSegment {
            start_offset: item.start_offset,
            end_offset: item.end_offset,
            scroll_speed: item.scroll_speed,
            distribution,
            track_count: item.track_count,
            track_distribution: item.track_distribution,
            position: item.position,
            items: item.items,
            style: item.style,
        };
        store_video_editor_global_filter_danmaku_segments!(ui)
            .set_row_data(index as usize, updated);
        sync_danmaku_to_project_state(ui);
    }
}

fn video_editor_global_filter_danmaku_update_track_count(
    ui: &AppWindow,
    index: i32,
    count_val: i32,
) {
    let count = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if index >= 0
        && index < count
        && let Some(item) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(index as usize)
    {
        let updated = UIDanmakuSegment {
            start_offset: item.start_offset,
            end_offset: item.end_offset,
            scroll_speed: item.scroll_speed,
            distribution: item.distribution,
            track_count: count_val,
            track_distribution: item.track_distribution,
            position: item.position,
            items: item.items,
            style: item.style,
        };
        store_video_editor_global_filter_danmaku_segments!(ui)
            .set_row_data(index as usize, updated);
        sync_danmaku_to_project_state(ui);
    }
}

fn video_editor_global_filter_danmaku_update_track_distribution(
    ui: &AppWindow,
    index: i32,
    distribution: UIDanmakuDistributionMode,
) {
    let count = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if index >= 0
        && index < count
        && let Some(item) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(index as usize)
    {
        let updated = UIDanmakuSegment {
            start_offset: item.start_offset,
            end_offset: item.end_offset,
            scroll_speed: item.scroll_speed,
            distribution: item.distribution,
            track_count: item.track_count,
            track_distribution: distribution,
            position: item.position,
            items: item.items,
            style: item.style,
        };
        store_video_editor_global_filter_danmaku_segments!(ui)
            .set_row_data(index as usize, updated);
        sync_danmaku_to_project_state(ui);
    }
}

fn video_editor_global_filter_danmaku_update_position(ui: &AppWindow, index: i32, position: f32) {
    let count = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if index >= 0
        && index < count
        && let Some(item) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(index as usize)
    {
        let updated = UIDanmakuSegment {
            start_offset: item.start_offset,
            end_offset: item.end_offset,
            scroll_speed: item.scroll_speed,
            distribution: item.distribution,
            track_count: item.track_count,
            track_distribution: item.track_distribution,
            position,
            items: item.items,
            style: item.style,
        };
        store_video_editor_global_filter_danmaku_segments!(ui)
            .set_row_data(index as usize, updated);
        sync_danmaku_to_project_state(ui);
    }
}

fn video_editor_global_filter_danmaku_is_valid(ui: &AppWindow, index: i32, _flag: bool) -> bool {
    let len = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if index < 0 || index >= len {
        return false;
    }

    if let Some(current_item) =
        store_video_editor_global_filter_danmaku_segments!(ui).row_data(index as usize)
    {
        if current_item.start_offset < 0 {
            return false;
        }

        if current_item.end_offset <= current_item.start_offset {
            return false;
        }

        return true;
    }

    false
}

fn video_editor_global_filter_danmaku_toggle_enable(ui: &AppWindow) {
    sync_danmaku_to_project_state(ui);
}

fn video_editor_global_filter_danmaku_add_items(
    ui: &AppWindow,
    segment_index: i32,
    text: slint::SharedString,
) {
    let count = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if segment_index >= 0
        && segment_index < count
        && let Some(segment) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(segment_index as usize)
    {
        let model = store_video_editor_global_filter_danmaku_segment_items!(segment);
        for line in text.split('\n') {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                model.push(UIDanmakuItem {
                    text: trimmed.into(),
                });
            }
        }
        sync_danmaku_to_project_state(ui);
    }
}

fn video_editor_global_filter_danmaku_remove_item(
    ui: &AppWindow,
    segment_index: i32,
    item_index: i32,
) {
    let count = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if segment_index >= 0
        && segment_index < count
        && let Some(segment) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(segment_index as usize)
    {
        let model = store_video_editor_global_filter_danmaku_segment_items!(segment);
        if item_index >= 0 && item_index < model.row_count() as i32 {
            model.remove(item_index as usize);
            sync_danmaku_to_project_state(ui);
        }
    }
}

fn video_editor_global_filter_danmaku_remove_all_items(ui: &AppWindow, segment_index: i32) {
    let count = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if segment_index >= 0
        && segment_index < count
        && let Some(segment) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(segment_index as usize)
    {
        let model = store_video_editor_global_filter_danmaku_segment_items!(segment);
        model.set_vec(vec![]);
        sync_danmaku_to_project_state(ui);
    }
}

fn video_editor_global_filter_danmaku_copy_all_items(ui: &AppWindow, segment_index: i32) {
    let count = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if segment_index >= 0
        && segment_index < count
        && let Some(segment) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(segment_index as usize)
    {
        let text: String = store_video_editor_global_filter_danmaku_segment_items!(segment)
            .iter()
            .map(|item| item.text.to_string())
            .collect::<Vec<String>>()
            .join("\n");

        crate::global_logic!(ui).invoke_copy_to_clipboard(text.into());
    }
}

fn video_editor_global_filter_danmaku_update_item_text(
    ui: &AppWindow,
    segment_index: i32,
    item_index: i32,
    text: slint::SharedString,
) {
    let count = store_video_editor_global_filter_danmaku_segments!(ui).row_count() as i32;

    if segment_index >= 0
        && segment_index < count
        && let Some(segment) =
            store_video_editor_global_filter_danmaku_segments!(ui).row_data(segment_index as usize)
    {
        let model = store_video_editor_global_filter_danmaku_segment_items!(segment);
        let item_count = model.row_count() as i32;
        if item_index >= 0
            && item_index < item_count
            && let Some(_item) = model.row_data(item_index as usize)
        {
            let updated = UIDanmakuItem { text };
            model.set_row_data(item_index as usize, updated);
            sync_danmaku_to_project_state(ui);
        }
    }
}

fn video_editor_global_filter_danmaku_fetch_from_bilibili(
    ui: &AppWindow,
    url: SharedString,
    counts: i32,
) {
    let Some(bvid) = extract_bvid(url.as_str()) else {
        crate::toast_warn!(
            ui,
            format!("{}: {}", tr("Failed to extract BV ID from URL"), url)
        );
        return;
    };
    let max_count = counts.max(1) as usize;

    global_store!(ui).set_video_editor_global_filter_danmaku_is_fetching(true);

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        match get_all_danmaku_with_limit(&bvid, Some(1), max_count, DEFAULT_TIMEOUT).await {
            Ok(danmaku_list) => {
                let text = danmaku_list
                    .into_iter()
                    .map(|d| d.content)
                    .collect::<Vec<String>>()
                    .join("\n");

                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui)
                        .set_video_editor_global_filter_danmaku_fetch_items_text(text.into());
                    global_store!(ui).set_video_editor_global_filter_danmaku_is_fetching(false);
                });
            }
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak,
                    format!("{}: {:?}", tr("Failed to fetch danmaku"), e),
                );
            }
        }
    });
}

fn extract_bvid(url: &str) -> Option<String> {
    let url = url.trim();
    // Find "BV" followed by base58 chars until delimiter
    let start = url.find("BV")?;
    let rest = &url[start + 2..];
    let end = rest
        .find(|c: char| !c.is_alphanumeric())
        .unwrap_or(rest.len());
    if end > 0 {
        Some(format!("BV{}", &rest[..end]))
    } else {
        None
    }
}

pub fn clear_ui_state(ui: &AppWindow) {
    let mut config = global_store!(ui).get_video_editor_global_filter_config();
    config.danmaku.items = ModelRc::new(VecModel::default());
    config.danmaku.enabled = false;
    global_store!(ui).set_video_editor_global_filter_config(config);
}
