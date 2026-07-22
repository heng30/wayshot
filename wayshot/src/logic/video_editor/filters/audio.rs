use crate::{
    from_filter_json, global_store,
    logic::{
        tr::tr,
        video_editor::{
            command::{sync_and_refresh, with_history_manager},
            track::get_selected_segment_indices,
        },
    },
    slint_generatedAppWindow::{
        AppWindow, CompressorDetail as UICompressorDetail,
        CopyChannelDetail as UICopyChannelDetail, FadeInDetail as UIFadeInDetail,
        FadeOutDetail as UIFadeOutDetail, GainDetail as UIGainDetail,
        LimiterDetail as UILimiterDetail, MuteDetail as UIMuteDetail,
        NoiseGateDetail as UINoiseGateDetail, NormalizeDetail as UINormalizeDetail,
        SpeedDetail as UISpeedDetail, VoiceChangerDetail as UIVoiceChangerDetail,
    },
    ve_filter_cb,
};
use slint::{ComponentHandle, SharedString};
use std::time::Duration;
use video_editor::{
    commands::{BatchCommand, filter::InsertFilterCommand, segment::SetPlaybackSpeedCommand},
    filters::{
        AudioFilter,
        audio::{
            AudioSpeedFilter, CompressorFilter, CopyChannelFilter, FadeInFilter, FadeOutFilter,
            GainFilter, LimiterFilter, MuteFilter, NoiseGateFilter, NormalizeFilter,
            VoiceChangerFilter,
        },
        keyframe::KeyframeTracks,
    },
};

pub fn init(ui: &AppWindow) {
    ve_filter_cb!(from_gain_json, ui, json);
    ve_filter_cb!(from_fade_in_json, ui, json);
    ve_filter_cb!(from_fade_out_json, ui, json);
    ve_filter_cb!(from_compressor_json, ui, json);
    ve_filter_cb!(from_limiter_json, ui, json);
    ve_filter_cb!(from_noise_gate_json, ui, json);
    ve_filter_cb!(from_normalize_json, ui, json);
    ve_filter_cb!(from_mute_json, ui, json);
    ve_filter_cb!(from_copy_channel_json, ui, json);
    ve_filter_cb!(from_voice_changer_json, ui, json);
    ve_filter_cb!(from_audio_speed_json, ui, json);

    ve_filter_cb!(modify_gain_filter, ui, index, config);
    ve_filter_cb!(modify_fade_in_filter, ui, index, config);
    ve_filter_cb!(modify_fade_out_filter, ui, index, config);
    ve_filter_cb!(modify_compressor_filter, ui, index, config);
    ve_filter_cb!(modify_limiter_filter, ui, index, config);
    ve_filter_cb!(modify_noise_gate_filter, ui, index, config);
    ve_filter_cb!(modify_normalize_filter, ui, index, config);
    ve_filter_cb!(modify_mute_filter, ui, index, config);
    ve_filter_cb!(modify_copy_channel_filter, ui, index, config);
    ve_filter_cb!(modify_voice_changer_filter, ui, index, config);
    ve_filter_cb!(modify_audio_speed_filter, ui, index, config);
}

macro_rules! impl_modify_audio_filter {
    ($func_name:ident, $filter_type:ty, $ui_type:ty) => {
        fn $func_name(ui: &AppWindow, index: i32, config: $ui_type) {
            let filter_index = index as usize;
            let selected_segments = get_selected_segment_indices(ui);
            if selected_segments.is_empty() {
                log::warn!("No segments selected for filter update");
                return;
            }

            let (track_idx, seg_idx) = selected_segments.last().unwrap();

            let playhead_time_ms = global_store!(ui).get_video_editor_timeline_offset();
            let segment_timeline_offset_ms: i32 = with_history_manager(|state| {
                let track = state.tracks_manager.get(*track_idx)?;
                let segment = track.get_segment(*seg_idx).ok()?;
                Some(segment.timeline_offset.as_millis() as i32)
            })
            .unwrap_or(0);
            let relative_time_ms = (playhead_time_ms - segment_timeline_offset_ms) as i64;

            let existing_info: Option<(usize, bool, KeyframeTracks, Option<(f32, Duration)>)> =
                with_history_manager(|state| {
                    let track = state.tracks_manager.get(*track_idx)?;
                    let segment = track.get_segment(*seg_idx).ok()?;
                    segment
                        .audio_filters
                        .get(filter_index)
                        .filter(|f| f.inner.name() == <$filter_type>::NAME)
                        .map(|f| {
                            let is_speed_filter = f.inner.name() == AudioSpeedFilter::NAME;
                            let old_segment_state = if is_speed_filter {
                                Some((segment.playback_speed, segment.duration))
                            } else {
                                None
                            };
                            (
                                filter_index,
                                f.enabled(),
                                f.inner.get_keyframe_tracks(),
                                old_segment_state,
                            )
                        })
                });

            let Some((idx, _enabled, mut existing_keyframes, old_segment_state)) = existing_info
            else {
                log::warn!("No filter found at index {}", filter_index);
                return;
            };

            let mut new_filter: $filter_type = config.into();
            new_filter.update_keyframes_at_time(&mut existing_keyframes, relative_time_ms);
            new_filter.set_keyframe_tracks(existing_keyframes);

            let new_speed: Option<f32> = if old_segment_state.is_some() {
                new_filter
                    .as_any()
                    .downcast_ref::<AudioSpeedFilter>()
                    .map(|f| f.speed)
            } else {
                None
            };

            let mut batch_command =
                BatchCommand::new(format!("Update {} filter", <$filter_type>::NAME));
            batch_command.add_command(Box::new(
                video_editor::commands::filter::RemoveFilterCommand::new_audio(
                    *track_idx, *seg_idx, idx,
                ),
            ));
            batch_command.add_command(Box::new(InsertFilterCommand::new_audio(
                *track_idx,
                *seg_idx,
                idx,
                Box::new(new_filter),
            )));

            if let Some((old_speed, old_duration)) = old_segment_state
                && let Some(speed) = new_speed
            {
                batch_command.add_command(Box::new(SetPlaybackSpeedCommand::new(
                    *track_idx,
                    *seg_idx,
                    speed,
                    old_speed,
                    old_duration,
                )));
            }

            let result = with_history_manager(|state| {
                state
                    .history_manager
                    .execute(&mut state.tracks_manager, Box::new(batch_command))
            });

            match result {
                Ok(execute_result) => {
                    sync_and_refresh(ui, execute_result.affected_segments, Some(true));

                    global_store!(ui).set_video_editor_segment_filter_flag(
                        !global_store!(ui).get_video_editor_segment_filter_flag(),
                    );
                }
                Err(e) => {
                    crate::toast_warn!(ui, format!("{}: {}", tr("Failed to update filter"), e))
                }
            }
        }
    };
}

impl_modify_audio_filter!(modify_gain_filter, GainFilter, UIGainDetail);
impl_modify_audio_filter!(modify_fade_in_filter, FadeInFilter, UIFadeInDetail);
impl_modify_audio_filter!(modify_fade_out_filter, FadeOutFilter, UIFadeOutDetail);
impl_modify_audio_filter!(
    modify_compressor_filter,
    CompressorFilter,
    UICompressorDetail
);
impl_modify_audio_filter!(modify_limiter_filter, LimiterFilter, UILimiterDetail);
impl_modify_audio_filter!(modify_noise_gate_filter, NoiseGateFilter, UINoiseGateDetail);
impl_modify_audio_filter!(modify_normalize_filter, NormalizeFilter, UINormalizeDetail);
impl_modify_audio_filter!(modify_mute_filter, MuteFilter, UIMuteDetail);
impl_modify_audio_filter!(
    modify_copy_channel_filter,
    CopyChannelFilter,
    UICopyChannelDetail
);
impl_modify_audio_filter!(
    modify_voice_changer_filter,
    VoiceChangerFilter,
    UIVoiceChangerDetail
);
impl_modify_audio_filter!(modify_audio_speed_filter, AudioSpeedFilter, UISpeedDetail);

from_filter_json!(from_gain_json, GainFilter, UIGainDetail);
from_filter_json!(from_fade_in_json, FadeInFilter, UIFadeInDetail);
from_filter_json!(from_fade_out_json, FadeOutFilter, UIFadeOutDetail);
from_filter_json!(from_compressor_json, CompressorFilter, UICompressorDetail);
from_filter_json!(from_limiter_json, LimiterFilter, UILimiterDetail);
from_filter_json!(from_noise_gate_json, NoiseGateFilter, UINoiseGateDetail);
from_filter_json!(from_normalize_json, NormalizeFilter, UINormalizeDetail);
from_filter_json!(from_mute_json, MuteFilter, UIMuteDetail);
from_filter_json!(
    from_copy_channel_json,
    CopyChannelFilter,
    UICopyChannelDetail
);
from_filter_json!(
    from_voice_changer_json,
    VoiceChangerFilter,
    UIVoiceChangerDetail
);
from_filter_json!(from_audio_speed_json, AudioSpeedFilter, UISpeedDetail);
