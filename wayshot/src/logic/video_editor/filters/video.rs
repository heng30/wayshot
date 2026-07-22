use crate::{
    from_filter_json, global_store, global_ve_filter,
    logic::{
        tr::tr,
        video_editor::{
            command::{sync_and_refresh, with_history_manager},
            filters::filter::get_filter_type_and_local_index,
            track::get_selected_segment_indices,
        },
    },
    slint_generatedAppWindow::{
        AppWindow, BackgroundDetail as UIBackgroundDetail, BorderDetail as UIBorderDetail,
        BreathingDetail as UIBreathingDetail, ChromaKeyDetail as UIChromaKeyDetail,
        CircleMaskDetail as UICircleMaskDetail, CropDetail as UICropDetail,
        DeviceFrameDetail as UIDeviceFrameDetail, DirectionalBlurDetail as UIDirectionalBlurDetail,
        DrawCircleDetail as UIDrawCircleDetail, DrawRectangleDetail as UIDrawRectangleDetail,
        EdgeDetectDetail as UIEdgeDetectDetail, FilterType as UIFilterType,
        FisheyeDetail as UIFisheyeDetail, FlipDetail as UIFlipDetail, FlyInDetail as UIFlyInDetail,
        FocusDetail as UIFocusDetail, FrameExtractDetail as UIFrameExtractDetail,
        GaussianBlurDetail as UIGaussianBlurDetail, GenieDetail as UIGenieDetail,
        GrainDetail as UIGrainDetail, GrayscaleDetail as UIGrayscaleDetail,
        GridDetail as UIGridDetail, HighlightRegionDetail as UIHighlightRegionDetail,
        HslAdjustDetail as UIHslAdjustDetail, LinearMaskDetail as UILinearMaskDetail,
        Live2dDetail as UILive2dDetail, LocalMagnifyDetail as UILocalMagnifyDetail,
        MagnifierDetail as UIMagnifierDetail, MirrorMaskDetail as UIMirrorMaskDetail,
        MosaicDetail as UIMosaicDetail, OldFilmDetail as UIOldFilmDetail,
        OpacityDetail as UIOpacityDetail, PageFlipDetail as UIPageFlipDetail,
        RectangleMaskDetail as UIRectangleMaskDetail, ShadowDetail as UIShadowDetail,
        SharpenDetail as UISharpenDetail, SketchDetail as UISketchDetail,
        SlideDetail as UISlideDetail, SpeedDetail as UISpeedDetail, SplitDetail as UISplitDetail,
        TextHighlightDetail as UITextHighlightDetail, TransformDetail as UITransformDetail,
        VideoFadeInDetail as UIVideoFadeInDetail, VideoFadeOutDetail as UIVideoFadeOutDetail,
        VignetteDetail as UIVignetteDetail, WaveDetail as UIWaveDetail, WipeDetail as UIWipeDetail,
        ZoomDetail as UIZoomDetail,
    },
    ve_filter_cb,
};
use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};
use std::time::Duration;
use video_editor::{
    commands::{
        BatchCommand,
        filter::{FilterType, InsertFilterCommand, RemoveFilterCommand},
        segment::SetPlaybackSpeedCommand,
    },
    filters::{
        VideoFilter,
        keyframe::KeyframeTracks,
        subtitle::style::scale_pixel_for_height,
        traits::ImageFilterWrapper,
        video::{
            BackgroundFilter, BorderFilter, BreathingFilter, ChromaKeyFilter, CircleMaskFilter,
            CropFilter, DeviceFrameFilter, DirectionalBlurFilter, DrawCircleFilter,
            DrawRectangleFilter, EdgeDetectFilter, FadeInFilter as VideoFadeInFilter,
            FadeOutFilter as VideoFadeOutFilter, FisheyeFilter, FlipFilter, FlyInFilter,
            FocusFilter, FrameExtractFilter, GaussianBlurFilter, GenieFilter, GrainFilter,
            GrayscaleFilter, GridFilter, HSLAdjustFilter, LinearMaskFilter, Live2dFilter,
            LocalMagnifyFilter, MagnifierFilter, MirrorMaskFilter, MosaicFilter, OldFilmFilter,
            OpacityFilter, PageFlipFilter, RectangleMaskFilter, ShadowFilter, SharpenFilter,
            SketchFilter, SlideFilter, SpeedFilter, SplitFilter, TextHighlightFilter,
            TransformFilter, VignetteFilter, WaveFilter, WipeFilter, ZoomFilter,
            device_frame::available_device_names,
            live2d::{model_expression_names, model_motion_names},
        },
    },
};

pub fn init(ui: &AppWindow) {
    ve_filter_cb!(from_crop_json, ui, json);
    ve_filter_cb!(from_flip_json, ui, json);
    ve_filter_cb!(from_chroma_key_json, ui, json);
    ve_filter_cb!(from_zoom_json, ui, json);
    ve_filter_cb!(from_transform_json, ui, json);
    ve_filter_cb!(from_fly_in_json, ui, json);
    ve_filter_cb!(from_video_fade_in_json, ui, json);
    ve_filter_cb!(from_video_fade_out_json, ui, json);
    ve_filter_cb!(from_slide_json, ui, json);
    ve_filter_cb!(from_wipe_json, ui, json);
    ve_filter_cb!(from_opacity_json, ui, json);
    ve_filter_cb!(from_border_json, ui, json);
    ve_filter_cb!(from_mosaic_json, ui, json);
    ve_filter_cb!(from_draw_circle_json, ui, json);
    ve_filter_cb!(from_draw_rectangle_json, ui, json);
    ve_filter_cb!(from_background_json, ui, json);
    ve_filter_cb!(from_vignette_json, ui, json);
    ve_filter_cb!(from_linear_mask_json, ui, json);
    ve_filter_cb!(from_circle_mask_json, ui, json);
    ve_filter_cb!(from_mirror_mask_json, ui, json);
    ve_filter_cb!(from_rectangle_mask_json, ui, json);
    ve_filter_cb!(from_hsl_adjust_json, ui, json);
    ve_filter_cb!(from_speed_json, ui, json);
    ve_filter_cb!(from_frame_extract_json, ui, json);
    ve_filter_cb!(from_breathing_json, ui, json);
    ve_filter_cb!(from_local_magnify_json, ui, json);
    ve_filter_cb!(from_magnifier_json, ui, json);
    ve_filter_cb!(from_gaussian_blur_json, ui, json);
    ve_filter_cb!(from_directional_blur_json, ui, json);
    ve_filter_cb!(from_sharpen_json, ui, json);
    ve_filter_cb!(from_edge_detect_json, ui, json);
    ve_filter_cb!(from_grain_json, ui, json);
    ve_filter_cb!(from_grid_json, ui, json);
    ve_filter_cb!(from_grayscale_json, ui, json);
    ve_filter_cb!(from_fisheye_json, ui, json);
    ve_filter_cb!(from_focus_json, ui, json);
    ve_filter_cb!(from_old_film_json, ui, json);
    ve_filter_cb!(from_sketch_json, ui, json);
    ve_filter_cb!(from_wave_json, ui, json);
    ve_filter_cb!(from_text_highlight_json, ui, json);
    ve_filter_cb!(from_shadow_json, ui, json);
    ve_filter_cb!(from_device_frame_json, ui, json);
    ve_filter_cb!(from_genie_json, ui, json);
    ve_filter_cb!(from_page_flip_json, ui, json);
    ve_filter_cb!(from_split_json, ui, json);
    ve_filter_cb!(from_live_2d_json, ui, json);

    ve_filter_cb!(modify_crop_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_flip_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_chroma_key_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_zoom_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_transform_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_fly_in_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_video_fade_in_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_video_fade_out_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_slide_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_wipe_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_opacity_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_border_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_mosaic_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_draw_circle_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_draw_rectangle_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_background_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_vignette_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_linear_mask_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_circle_mask_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_mirror_mask_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_rectangle_mask_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_hsl_adjust_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_speed_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_frame_extract_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_breathing_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_local_magnify_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_magnifier_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_gaussian_blur_filter, ui, index, config, filter_type);
    ve_filter_cb!(
        modify_directional_blur_filter,
        ui,
        index,
        config,
        filter_type
    );
    ve_filter_cb!(modify_sharpen_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_edge_detect_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_grain_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_grid_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_grayscale_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_fisheye_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_focus_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_old_film_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_sketch_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_wave_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_text_highlight_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_shadow_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_device_frame_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_genie_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_page_flip_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_split_filter, ui, index, config, filter_type);
    ve_filter_cb!(modify_live_2d_filter, ui, index, config, filter_type);
    ve_filter_cb!(text_highlight_remove_region, ui, filter_index, region_index);
    ve_filter_cb!(text_highlight_add_region, ui, filter_index, region);
    ve_filter_cb!(
        text_highlight_update_region,
        ui,
        filter_index,
        region_index,
        region
    );
    ve_filter_cb!(text_highlight_pick_color, ui, filter_index, image, x, y);
    ve_filter_cb!(chroma_key_pick_color, ui, filter_index, image, x, y);

    global_ve_filter!(ui).on_scale_pixel_for_resolution(|pixel_value, target_height| {
        scale_pixel_for_height(pixel_value as u32, target_height as u32) as i32
    });

    global_ve_filter!(ui).on_device_frame_names(|| {
        let names: Vec<SharedString> = available_device_names()
            .into_iter()
            .map(|s| s.into())
            .collect();
        ModelRc::new(VecModel::from(names)).into()
    });

    global_ve_filter!(ui).on_live_2d_pick_model_dir(|| {
        let dialog =
            native_dialog::DialogBuilder::file().set_title(tr("Select Live2D Model Directory"));

        match dialog.open_single_dir().show() {
            Ok(Some(path)) => path.to_string_lossy().to_string().into(),
            _ => SharedString::new(),
        }
    });

    global_ve_filter!(ui).on_live_2d_motion_names(|model_path| {
        let names: Vec<SharedString> = model_motion_names(model_path.as_str())
            .into_iter()
            .map(|s| s.into())
            .collect();
        ModelRc::new(VecModel::from(names)).into()
    });

    global_ve_filter!(ui).on_live_2d_expression_names(|model_path| {
        let names: Vec<slint::SharedString> = model_expression_names(model_path.as_str())
            .into_iter()
            .map(|s| s.into())
            .collect();
        ModelRc::new(VecModel::from(names)).into()
    });
}

macro_rules! impl_modify_video_filter {
    ($func_name:ident, $filter_type:ty, $ui_type:ty) => {
        fn $func_name(ui: &AppWindow, index: i32, config: $ui_type, filter_type: UIFilterType) {
            let merged_index = index as usize;
            let selected_segments = get_selected_segment_indices(ui);
            if selected_segments.is_empty() {
                log::warn!("No segments selected for filter update");
                return;
            }

            let (track_idx, seg_idx) = selected_segments.last().unwrap();

            // Get playhead time and segment timeline offset for keyframe update
            let playhead_time_ms = global_store!(ui).get_video_editor_timeline_offset();
            let segment_timeline_offset_ms: i32 = with_history_manager(|state| {
                let track = state.tracks_manager.get(*track_idx)?;
                let segment = track.get_segment(*seg_idx).ok()?;
                Some(segment.timeline_offset.as_millis() as i32)
            })
            .unwrap_or(0);
            let relative_time_ms = (playhead_time_ms - segment_timeline_offset_ms) as i64;

            let Some((actual_filter_type, local_index)) =
                get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
            else {
                log::warn!("No filter found at merged index {}", merged_index);
                return;
            };

            let expected_filter_type: FilterType = filter_type.into();
            if actual_filter_type != expected_filter_type {
                log::warn!(
                    "Filter type mismatch: expected {:?}, got {:?}",
                    expected_filter_type,
                    actual_filter_type
                );
                return;
            }

            let existing_info: Option<(usize, bool, KeyframeTracks, Option<(f32, Duration)>)> =
                with_history_manager(|state| {
                    let track = state.tracks_manager.get(*track_idx)?;
                    let segment = track.get_segment(*seg_idx).ok()?;

                    match filter_type {
                        UIFilterType::Video => segment.video_filters.get(local_index).map(|f| {
                            let is_speed_filter = f.inner.name() == SpeedFilter::NAME;
                            let old_segment_state = if is_speed_filter {
                                Some((segment.playback_speed, segment.duration))
                            } else {
                                None
                            };
                            (
                                local_index,
                                f.enabled(),
                                f.inner.get_keyframe_tracks(),
                                old_segment_state,
                            )
                        }),
                        UIFilterType::Image => segment.image_filters.get(local_index).map(|f| {
                            let is_speed_filter = f.inner.name() == SpeedFilter::NAME;
                            let old_segment_state = if is_speed_filter {
                                Some((segment.playback_speed, segment.duration))
                            } else {
                                None
                            };
                            (
                                local_index,
                                f.enabled(),
                                f.inner.get_keyframe_tracks(),
                                old_segment_state,
                            )
                        }),
                        _ => None,
                    }
                });

            let Some((idx, enabled, mut existing_keyframes, old_segment_state)) = existing_info
            else {
                log::warn!("No {} filter found in segment", <$filter_type>::NAME);
                return;
            };

            let mut new_filter: $filter_type = config.into();
            new_filter.update_keyframes_at_time(&mut existing_keyframes, relative_time_ms);
            new_filter.set_keyframe_tracks(existing_keyframes);

            let new_speed: Option<f32> = if old_segment_state.is_some() {
                new_filter
                    .as_any()
                    .downcast_ref::<SpeedFilter>()
                    .map(|f| f.speed)
            } else {
                None
            };

            let mut batch_command =
                BatchCommand::new(format!("Update {} filter", <$filter_type>::NAME));

            match filter_type {
                UIFilterType::Video => {
                    batch_command.add_command(Box::new(RemoveFilterCommand::new_video(
                        *track_idx, *seg_idx, idx,
                    )));
                    batch_command.add_command(Box::new(InsertFilterCommand::new_video(
                        *track_idx,
                        *seg_idx,
                        idx,
                        Box::new(new_filter),
                    )));
                }
                UIFilterType::Image => {
                    batch_command.add_command(Box::new(RemoveFilterCommand::new_image(
                        *track_idx, *seg_idx, idx,
                    )));
                    batch_command.add_command(Box::new(InsertFilterCommand::new_image(
                        *track_idx,
                        *seg_idx,
                        idx,
                        ImageFilterWrapper::new(enabled, Box::new(new_filter)),
                    )));
                }
                _ => {
                    log::warn!("Unsupported filter type: {:?}", filter_type);
                    return;
                }
            }

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

impl_modify_video_filter!(modify_transform_filter, TransformFilter, UITransformDetail);
impl_modify_video_filter!(modify_fly_in_filter, FlyInFilter, UIFlyInDetail);
impl_modify_video_filter!(modify_crop_filter, CropFilter, UICropDetail);
impl_modify_video_filter!(modify_flip_filter, FlipFilter, UIFlipDetail);
impl_modify_video_filter!(modify_chroma_key_filter, ChromaKeyFilter, UIChromaKeyDetail);
impl_modify_video_filter!(modify_zoom_filter, ZoomFilter, UIZoomDetail);
impl_modify_video_filter!(
    modify_video_fade_in_filter,
    VideoFadeInFilter,
    UIVideoFadeInDetail
);
impl_modify_video_filter!(
    modify_video_fade_out_filter,
    VideoFadeOutFilter,
    UIVideoFadeOutDetail
);
impl_modify_video_filter!(modify_slide_filter, SlideFilter, UISlideDetail);
impl_modify_video_filter!(modify_wipe_filter, WipeFilter, UIWipeDetail);
impl_modify_video_filter!(modify_opacity_filter, OpacityFilter, UIOpacityDetail);
impl_modify_video_filter!(modify_border_filter, BorderFilter, UIBorderDetail);
impl_modify_video_filter!(modify_mosaic_filter, MosaicFilter, UIMosaicDetail);
impl_modify_video_filter!(
    modify_draw_circle_filter,
    DrawCircleFilter,
    UIDrawCircleDetail
);
impl_modify_video_filter!(
    modify_draw_rectangle_filter,
    DrawRectangleFilter,
    UIDrawRectangleDetail
);
impl_modify_video_filter!(
    modify_background_filter,
    BackgroundFilter,
    UIBackgroundDetail
);
impl_modify_video_filter!(modify_vignette_filter, VignetteFilter, UIVignetteDetail);
impl_modify_video_filter!(
    modify_linear_mask_filter,
    LinearMaskFilter,
    UILinearMaskDetail
);
impl_modify_video_filter!(
    modify_circle_mask_filter,
    CircleMaskFilter,
    UICircleMaskDetail
);
impl_modify_video_filter!(
    modify_mirror_mask_filter,
    MirrorMaskFilter,
    UIMirrorMaskDetail
);
impl_modify_video_filter!(
    modify_rectangle_mask_filter,
    RectangleMaskFilter,
    UIRectangleMaskDetail
);
impl_modify_video_filter!(modify_hsl_adjust_filter, HSLAdjustFilter, UIHslAdjustDetail);
impl_modify_video_filter!(modify_speed_filter, SpeedFilter, UISpeedDetail);
impl_modify_video_filter!(
    modify_frame_extract_filter,
    FrameExtractFilter,
    UIFrameExtractDetail
);
impl_modify_video_filter!(modify_breathing_filter, BreathingFilter, UIBreathingDetail);
impl_modify_video_filter!(
    modify_local_magnify_filter,
    LocalMagnifyFilter,
    UILocalMagnifyDetail
);
impl_modify_video_filter!(modify_magnifier_filter, MagnifierFilter, UIMagnifierDetail);
impl_modify_video_filter!(
    modify_gaussian_blur_filter,
    GaussianBlurFilter,
    UIGaussianBlurDetail
);
impl_modify_video_filter!(
    modify_directional_blur_filter,
    DirectionalBlurFilter,
    UIDirectionalBlurDetail
);
impl_modify_video_filter!(modify_sharpen_filter, SharpenFilter, UISharpenDetail);
impl_modify_video_filter!(
    modify_edge_detect_filter,
    EdgeDetectFilter,
    UIEdgeDetectDetail
);
impl_modify_video_filter!(modify_grain_filter, GrainFilter, UIGrainDetail);
impl_modify_video_filter!(modify_grayscale_filter, GrayscaleFilter, UIGrayscaleDetail);
impl_modify_video_filter!(modify_fisheye_filter, FisheyeFilter, UIFisheyeDetail);
impl_modify_video_filter!(modify_focus_filter, FocusFilter, UIFocusDetail);
impl_modify_video_filter!(modify_old_film_filter, OldFilmFilter, UIOldFilmDetail);
impl_modify_video_filter!(modify_sketch_filter, SketchFilter, UISketchDetail);
impl_modify_video_filter!(modify_wave_filter, WaveFilter, UIWaveDetail);
impl_modify_video_filter!(
    modify_text_highlight_filter,
    TextHighlightFilter,
    UITextHighlightDetail
);
impl_modify_video_filter!(modify_shadow_filter, ShadowFilter, UIShadowDetail);
impl_modify_video_filter!(
    modify_device_frame_filter,
    DeviceFrameFilter,
    UIDeviceFrameDetail
);
impl_modify_video_filter!(modify_genie_filter, GenieFilter, UIGenieDetail);
impl_modify_video_filter!(modify_page_flip_filter, PageFlipFilter, UIPageFlipDetail);
impl_modify_video_filter!(modify_split_filter, SplitFilter, UISplitDetail);
impl_modify_video_filter!(modify_grid_filter, GridFilter, UIGridDetail);
impl_modify_video_filter!(modify_live_2d_filter, Live2dFilter, UILive2dDetail);
from_filter_json!(from_crop_json, CropFilter, UICropDetail);
from_filter_json!(from_flip_json, FlipFilter, UIFlipDetail);
from_filter_json!(from_chroma_key_json, ChromaKeyFilter, UIChromaKeyDetail);
from_filter_json!(from_zoom_json, ZoomFilter, UIZoomDetail);
from_filter_json!(from_transform_json, TransformFilter, UITransformDetail);
from_filter_json!(from_fly_in_json, FlyInFilter, UIFlyInDetail);
from_filter_json!(
    from_video_fade_in_json,
    VideoFadeInFilter,
    UIVideoFadeInDetail
);
from_filter_json!(
    from_video_fade_out_json,
    VideoFadeOutFilter,
    UIVideoFadeOutDetail
);
from_filter_json!(from_slide_json, SlideFilter, UISlideDetail);
from_filter_json!(from_wipe_json, WipeFilter, UIWipeDetail);
from_filter_json!(from_opacity_json, OpacityFilter, UIOpacityDetail);
from_filter_json!(from_border_json, BorderFilter, UIBorderDetail);
from_filter_json!(from_mosaic_json, MosaicFilter, UIMosaicDetail);
from_filter_json!(from_draw_circle_json, DrawCircleFilter, UIDrawCircleDetail);
from_filter_json!(
    from_draw_rectangle_json,
    DrawRectangleFilter,
    UIDrawRectangleDetail
);
from_filter_json!(from_background_json, BackgroundFilter, UIBackgroundDetail);
from_filter_json!(from_vignette_json, VignetteFilter, UIVignetteDetail);
from_filter_json!(from_linear_mask_json, LinearMaskFilter, UILinearMaskDetail);
from_filter_json!(from_circle_mask_json, CircleMaskFilter, UICircleMaskDetail);
from_filter_json!(from_mirror_mask_json, MirrorMaskFilter, UIMirrorMaskDetail);
from_filter_json!(
    from_rectangle_mask_json,
    RectangleMaskFilter,
    UIRectangleMaskDetail
);
from_filter_json!(from_hsl_adjust_json, HSLAdjustFilter, UIHslAdjustDetail);
from_filter_json!(from_speed_json, SpeedFilter, UISpeedDetail);
from_filter_json!(
    from_frame_extract_json,
    FrameExtractFilter,
    UIFrameExtractDetail
);
from_filter_json!(from_breathing_json, BreathingFilter, UIBreathingDetail);
from_filter_json!(
    from_local_magnify_json,
    LocalMagnifyFilter,
    UILocalMagnifyDetail
);
from_filter_json!(from_magnifier_json, MagnifierFilter, UIMagnifierDetail);
from_filter_json!(
    from_gaussian_blur_json,
    GaussianBlurFilter,
    UIGaussianBlurDetail
);
from_filter_json!(
    from_directional_blur_json,
    DirectionalBlurFilter,
    UIDirectionalBlurDetail
);
from_filter_json!(from_sharpen_json, SharpenFilter, UISharpenDetail);
from_filter_json!(from_edge_detect_json, EdgeDetectFilter, UIEdgeDetectDetail);
from_filter_json!(from_grain_json, GrainFilter, UIGrainDetail);
from_filter_json!(from_grayscale_json, GrayscaleFilter, UIGrayscaleDetail);
from_filter_json!(from_fisheye_json, FisheyeFilter, UIFisheyeDetail);
from_filter_json!(from_focus_json, FocusFilter, UIFocusDetail);
from_filter_json!(from_old_film_json, OldFilmFilter, UIOldFilmDetail);
from_filter_json!(from_sketch_json, SketchFilter, UISketchDetail);
from_filter_json!(from_wave_json, WaveFilter, UIWaveDetail);
from_filter_json!(
    from_text_highlight_json,
    TextHighlightFilter,
    UITextHighlightDetail
);
from_filter_json!(from_shadow_json, ShadowFilter, UIShadowDetail);
from_filter_json!(
    from_device_frame_json,
    DeviceFrameFilter,
    UIDeviceFrameDetail
);
from_filter_json!(from_genie_json, GenieFilter, UIGenieDetail);
from_filter_json!(from_page_flip_json, PageFlipFilter, UIPageFlipDetail);
from_filter_json!(from_split_json, SplitFilter, UISplitDetail);
from_filter_json!(from_grid_json, GridFilter, UIGridDetail);
from_filter_json!(from_live_2d_json, Live2dFilter, UILive2dDetail);

fn modify_text_highlight_regions(
    ui: &AppWindow,
    filter_index: i32,
    modify_regions: impl FnOnce(&mut Vec<video_editor::filters::video::HighlightRegion>),
    command_name: &str,
    error_msg: &str,
) {
    let merged_index = filter_index as usize;
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        log::warn!("No segments selected for region modification");
        return;
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();

    let Some((filter_type, local_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        log::warn!("No filter found at merged index {}", merged_index);
        return;
    };

    let existing_info: Option<(usize, bool, KeyframeTracks, TextHighlightFilter)> =
        with_history_manager(|state| {
            let track = state.tracks_manager.get(*track_idx)?;
            let segment = track.get_segment(*seg_idx).ok()?;

            match filter_type {
                FilterType::Image => segment.image_filters.get(local_index).and_then(|f| {
                    f.inner
                        .as_any()
                        .downcast_ref::<TextHighlightFilter>()
                        .map(|filter| {
                            (
                                local_index,
                                f.enabled(),
                                f.inner.get_keyframe_tracks(),
                                filter.clone(),
                            )
                        })
                }),
                FilterType::Video => segment.video_filters.get(local_index).and_then(|f| {
                    f.inner
                        .as_any()
                        .downcast_ref::<TextHighlightFilter>()
                        .map(|filter| {
                            (
                                local_index,
                                f.enabled(),
                                f.inner.get_keyframe_tracks(),
                                filter.clone(),
                            )
                        })
                }),
                _ => None,
            }
        });

    let Some((idx, enabled, existing_keyframes, mut current_filter)) = existing_info else {
        log::warn!("No text highlight filter found in segment");
        return;
    };

    modify_regions(&mut current_filter.regions);

    current_filter.set_keyframe_tracks(existing_keyframes);

    let mut batch_command = BatchCommand::new(command_name.to_string());

    match filter_type {
        FilterType::Image => {
            batch_command.add_command(Box::new(RemoveFilterCommand::new_image(
                *track_idx, *seg_idx, idx,
            )));
            batch_command.add_command(Box::new(InsertFilterCommand::new_image(
                *track_idx,
                *seg_idx,
                idx,
                ImageFilterWrapper::new(enabled, Box::new(current_filter)),
            )));
        }
        FilterType::Video => {
            batch_command.add_command(Box::new(RemoveFilterCommand::new_video(
                *track_idx, *seg_idx, idx,
            )));
            batch_command.add_command(Box::new(InsertFilterCommand::new_video(
                *track_idx,
                *seg_idx,
                idx,
                Box::new(current_filter),
            )));
        }
        _ => {
            log::warn!("Unsupported filter type: {:?}", filter_type);
            return;
        }
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
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", error_msg, e)),
    }
}

fn text_highlight_remove_region(ui: &AppWindow, filter_index: i32, region_index: i32) {
    modify_text_highlight_regions(
        ui,
        filter_index,
        |regions| {
            let region_idx = region_index as usize;
            if region_idx < regions.len() {
                regions.remove(region_idx);
            } else {
                log::warn!("Region index {} out of bounds", region_idx);
            }
        },
        "Remove text highlight region",
        "Failed to remove region",
    );
}

fn text_highlight_add_region(ui: &AppWindow, filter_index: i32, region: UIHighlightRegionDetail) {
    modify_text_highlight_regions(
        ui,
        filter_index,
        |regions| {
            regions.push(region.into());
            regions.sort_by(|a, b| {
                a.y.partial_cmp(&b.y)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
            });
        },
        "Add text highlight region",
        "Failed to add region",
    );
}

fn text_highlight_update_region(
    ui: &AppWindow,
    filter_index: i32,
    region_index: i32,
    region: UIHighlightRegionDetail,
) {
    modify_text_highlight_regions(
        ui,
        filter_index,
        |regions| {
            let region_idx = region_index as usize;
            if region_idx < regions.len() {
                regions[region_idx] = region.into();
                regions.sort_by(|a, b| {
                    a.x.partial_cmp(&b.x)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
                });
            } else {
                log::warn!("Region index {} out of bounds", region_idx);
            }
        },
        "Update text highlight region",
        "Failed to update region",
    );
}

fn pick_color_from_image(ui: &AppWindow, image: slint::Image, x: f32, y: f32) -> Color {
    if let Some(buffer) = image.to_rgba8() {
        let pixels = buffer.as_slice();
        let bw = buffer.width();
        let bh = buffer.height();
        if bw == 0 || bh == 0 {
            crate::toast_warn!(ui, tr("Image is empty"));
            return Color::from_argb_u8(255, 255, 255, 255);
        }

        let px = (x.clamp(0.0, 1.0) * bw as f32) as i32;
        let py = (y.clamp(0.0, 1.0) * bh as f32) as i32;
        let idx =
            py.clamp(0, bh as i32 - 1) as usize * bw as usize + px.clamp(0, bw as i32 - 1) as usize;
        if idx < pixels.len() {
            let pixel = pixels[idx];
            return Color::from_argb_u8(pixel.a, pixel.r, pixel.g, pixel.b);
        }
    }

    crate::toast_warn!(
        ui,
        format!("{} ({}, {})", tr("Could not get pixel color at"), x, y)
    );
    Color::from_argb_u8(255, 255, 255, 255)
}

fn text_highlight_pick_color(
    ui: &AppWindow,
    _filter_index: i32,
    image: slint::Image,
    x: f32,
    y: f32,
) -> Color {
    pick_color_from_image(ui, image, x, y)
}

fn chroma_key_pick_color(
    ui: &AppWindow,
    _filter_index: i32,
    image: slint::Image,
    x: f32,
    y: f32,
) -> Color {
    pick_color_from_image(ui, image, x, y)
}
