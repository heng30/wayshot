use crate::slint_generatedAppWindow::{
    AnimatableProperty as UIAnimatableProperty, BackgroundColorDetail as UIBackgroundColorDetail,
    BackgroundDetail as UIBackgroundDetail, BorderDetail as UIBorderDetail,
    BorderRadiusDetail as UIBorderRadiusDetail, BreathingDetail as UIBreathingDetail,
    ChromaKeyDetail as UIChromaKeyDetail, CircleMaskDetail as UICircleMaskDetail,
    CompressorDetail as UICompressorDetail, CopyChannelDetail as UICopyChannelDetail,
    CropDetail as UICropDetail, DeviceFrameDetail as UIDeviceFrameDetail,
    DirectionalBlurDetail as UIDirectionalBlurDetail, DrawCircleDetail as UIDrawCircleDetail,
    DrawRectangleDetail as UIDrawRectangleDetail, EdgeDetectDetail as UIEdgeDetectDetail,
    FadeInDetail as UIFadeInDetail, FadeOutDetail as UIFadeOutDetail, FilterEntry as UIFilterEntry,
    FisheyeDetail as UIFisheyeDetail, FlipDetail as UIFlipDetail, FlyInDetail as UIFlyInDetail,
    FocusDetail as UIFocusDetail, FontPathDetail as UIFontPathDetail,
    FontSizeDetail as UIFontSizeDetail, FrameExtractDetail as UIFrameExtractDetail,
    GainDetail as UIGainDetail, GaussianBlurDetail as UIGaussianBlurDetail,
    GenieDetail as UIGenieDetail, GrainDetail as UIGrainDetail,
    GrayscaleDetail as UIGrayscaleDetail, GridDetail as UIGridDetail,
    HighlightRegionDetail as UIHighlightRegionDetail, HslAdjustDetail as UIHslAdjustDetail,
    Keyframe as UIKeyframe, KeyframeValue as UIKeyframeValue,
    KeyframeValueType as UIKeyframeValueType, LightingDetail as UILightingDetail,
    LimiterDetail as UILimiterDetail, LinearMaskDetail as UILinearMaskDetail,
    Live2dDetail as UILive2dDetail, LocalMagnifyDetail as UILocalMagnifyDetail,
    MagnifierDetail as UIMagnifierDetail, MarginHorizontalDetail as UIMarginHorizontalDetail,
    MarginVerticalDetail as UIMarginVerticalDetail, MirrorMaskDetail as UIMirrorMaskDetail,
    MosaicDetail as UIMosaicDetail, MuteDetail as UIMuteDetail,
    NoiseGateDetail as UINoiseGateDetail, NormalizeDetail as UINormalizeDetail,
    OldFilmDetail as UIOldFilmDetail, OpacityDetail as UIOpacityDetail,
    OutlineColorDetail as UIOutlineColorDetail, OutlineWidthDetail as UIOutlineWidthDetail,
    PaddingDetail as UIPaddingDetail, PageFlipDetail as UIPageFlipDetail,
    PrimaryColorDetail as UIPrimaryColorDetail, PropertyTrack as UIPropertyTrack,
    RectangleMaskDetail as UIRectangleMaskDetail, SegmentFilter as UISegmentFilter,
    ShadowDetail as UIShadowDetail, SharpenDetail as UISharpenDetail,
    SketchDetail as UISketchDetail, SlideDetail as UISlideDetail, SpeedDetail as UISpeedDetail,
    SplitDetail as UISplitDetail, TextAlignmentDetail as UITextAlignmentDetail,
    TextHighlightDetail as UITextHighlightDetail, TransformDetail as UITransformDetail,
    VideoFadeInDetail as UIVideoFadeInDetail, VideoFadeOutDetail as UIVideoFadeOutDetail,
    VignetteDetail as UIVignetteDetail, VoiceChangerDetail as UIVoiceChangerDetail,
    WaveDetail as UIWaveDetail, WipeDetail as UIWipeDetail, ZoomDetail as UIZoomDetail,
};
use image::Rgba;
use slint::{Model, ModelRc, SharedString, VecModel};
use std::{path::PathBuf, time::Duration};
use video_editor::filters::{
    EffectPosition,
    audio::{
        AudioSpeedFilter, CompressorFilter, CopyChannelFilter, CopyDirection,
        FadeInFilter as AudioFadeInFilter, FadeOutFilter as AudioFadeOutFilter, GainFilter,
        LimiterFilter, MuteChannel, MuteFilter, NoiseGateFilter, NormalizeFilter,
        VoiceChangerFilter,
    },
    keyframe::{AnimatableProperty, Keyframe, KeyframeTracks, KeyframeValue, PropertyTrack},
    subtitle::style::{
        BackgroundColorFilter, BorderRadiusFilter, FontPathFilter, FontSizeFilter,
        MarginHorizontalFilter, MarginVerticalFilter, OutlineColorFilter, OutlineWidthFilter,
        PaddingFilter, PrimaryColorFilter, TextAlignment, TextAlignmentFilter,
    },
    traits::EasingFunction,
    video::{
        BackgroundFilter, BorderFilter, BreathingCurve, BreathingFilter, ChromaKeyFilter,
        CircleMaskFilter, CropFilter, CropShape, DeviceFrameFilter, DirectionalBlurFilter,
        DrawCircleFilter, DrawRectangleFilter, EdgeDetectFilter, FadeInFilter as VideoFadeInFilter,
        FadeOutFilter as VideoFadeOutFilter, FisheyeFilter, FlipDirection, FlipFilter,
        FlyInDirection, FlyInFilter, FocusFilter, FrameExtractFilter, GaussianBlurFilter,
        GenieAnchor, GenieFilter, GrainFilter, GrayscaleFilter, GridFilter, HSLAdjustFilter,
        HighlightRegion, LightingDirection as VELightingDirection,
        LightingFilter as VELightingFilter, LightingScene as VELightingScene, LinearMaskFilter,
        Live2dFilter, LocalMagnifyFilter, LuminanceStandard, MagnifierFilter, MirrorMaskFilter,
        MosaicFilter, OldFilmFilter, OpacityFilter, PageFlipAxis, PageFlipCorner,
        PageFlipDirection, PageFlipFilter, PageFlipPosition, RectangleMaskFilter, ShadowFilter,
        SharpenFilter, SketchFilter, SlideDirection, SlideFilter, SpeedFilter, SplitDirection,
        SplitFilter, TextHighlightFilter, TransformFilter, VignetteFilter, WaveFilter, WaveType,
        WipeDirection, WipeFilter, ZoomFilter,
    },
};

impl From<UISegmentFilter> for UIFilterEntry {
    fn from(f: UISegmentFilter) -> Self {
        Self {
            ty: f.ty,
            name: f.name,
            is_marked: false,
        }
    }
}

// ==================== Video/Image Filters ====================

impl From<CropFilter> for UICropDetail {
    fn from(f: CropFilter) -> Self {
        let shape: u8 = f.shape.into();
        Self {
            left: f.left,
            top: f.top,
            width: f.width,
            height: f.height,
            shape: shape as i32,
        }
    }
}

impl From<FlipFilter> for UIFlipDetail {
    fn from(f: FlipFilter) -> Self {
        let direction = match f.direction {
            FlipDirection::Horizontal => 0,
            FlipDirection::Vertical => 1,
            FlipDirection::Both => 2,
        };
        Self { direction }
    }
}

impl From<ChromaKeyFilter> for UIChromaKeyDetail {
    fn from(f: ChromaKeyFilter) -> Self {
        Self {
            target_color_r: f.target_color[0] as i32,
            target_color_g: f.target_color[1] as i32,
            target_color_b: f.target_color[2] as i32,
            target_color_a: f.target_color[3] as i32,
            similarity: f.similarity,
            softness: f.softness,
            feather: f.feather,
            spill_reduction: f.spill_reduction,
        }
    }
}

impl From<ZoomFilter> for UIZoomDetail {
    fn from(f: ZoomFilter) -> Self {
        Self {
            relative_start_ms: f.relative_start.as_millis() as i32,
            zoom_in_duration_ms: f.zoom_in_duration.as_millis() as i32,
            hold_duration_ms: f.hold_duration.as_millis() as i32,
            zoom_out_duration_ms: f.zoom_out_duration.as_millis() as i32,
            level: f.level,
            center_x: f.center.0,
            center_y: f.center.1,
        }
    }
}

impl From<TransformFilter> for UITransformDetail {
    fn from(t: TransformFilter) -> Self {
        Self {
            zoom_level: t.zoom_level,
            center_x_percent: t.center_x_percent,
            center_y_percent: t.center_y_percent,
            rotation: t.rotation.to_degrees(),
        }
    }
}

impl From<UITransformDetail> for TransformFilter {
    fn from(t: UITransformDetail) -> Self {
        Self {
            zoom_level: t.zoom_level,
            center_x_percent: t.center_x_percent,
            center_y_percent: t.center_y_percent,
            rotation: t.rotation.to_radians(),
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl From<UICropDetail> for CropFilter {
    fn from(d: UICropDetail) -> Self {
        let shape = CropShape::try_from(d.shape as u8).unwrap_or_default();
        Self {
            left: d.left,
            top: d.top,
            width: d.width,
            height: d.height,
            shape,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl From<UIFlipDetail> for FlipFilter {
    fn from(d: UIFlipDetail) -> Self {
        let direction = match d.direction {
            0 => FlipDirection::Horizontal,
            1 => FlipDirection::Vertical,
            _ => FlipDirection::Both,
        };
        Self { direction }
    }
}

impl From<UIChromaKeyDetail> for ChromaKeyFilter {
    fn from(d: UIChromaKeyDetail) -> Self {
        Self {
            target_color: Rgba([
                d.target_color_r as u8,
                d.target_color_g as u8,
                d.target_color_b as u8,
                d.target_color_a as u8,
            ]),
            similarity: d.similarity,
            softness: d.softness,
            feather: d.feather,
            spill_reduction: d.spill_reduction,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl From<UIZoomDetail> for ZoomFilter {
    fn from(d: UIZoomDetail) -> Self {
        Self::default()
            .with_relative_start(Duration::from_millis(d.relative_start_ms as u64))
            .with_zoom_in_duration(Duration::from_millis(d.zoom_in_duration_ms as u64))
            .with_hold_duration(Duration::from_millis(d.hold_duration_ms as u64))
            .with_zoom_out_duration(Duration::from_millis(d.zoom_out_duration_ms as u64))
            .with_level(d.level)
            .with_center((d.center_x, d.center_y))
    }
}

impl From<OpacityFilter> for UIOpacityDetail {
    fn from(f: OpacityFilter) -> Self {
        Self { opacity: f.opacity }
    }
}

impl From<UIOpacityDetail> for OpacityFilter {
    fn from(d: UIOpacityDetail) -> Self {
        Self {
            opacity: d.opacity.clamp(0.0, 1.0),
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl From<VignetteFilter> for UIVignetteDetail {
    fn from(f: VignetteFilter) -> Self {
        Self {
            intensity: f.intensity,
            inner_radius: f.inner_radius,
            outer_radius: f.outer_radius,
            center_x: f.center_x,
            center_y: f.center_y,
        }
    }
}

impl From<UIVignetteDetail> for VignetteFilter {
    fn from(d: UIVignetteDetail) -> Self {
        Self {
            intensity: d.intensity.clamp(0.0, 1.0),
            inner_radius: d.inner_radius.clamp(0.0, 1.0),
            outer_radius: d.outer_radius.clamp(0.0, 1.0).max(d.inner_radius),
            center_x: d.center_x.clamp(0.0, 1.0),
            center_y: d.center_y.clamp(0.0, 1.0),
            aspect: 1.0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl From<LinearMaskFilter> for UILinearMaskDetail {
    fn from(f: LinearMaskFilter) -> Self {
        Self {
            center_x: f.center_x,
            center_y: f.center_y,
            rotation: f.rotation,
            feather: f.feather,
            opacity: f.opacity,
            flip: f.flip,
        }
    }
}

impl From<UILinearMaskDetail> for LinearMaskFilter {
    fn from(d: UILinearMaskDetail) -> Self {
        LinearMaskFilter::default()
            .with_center_x(d.center_x)
            .with_center_y(d.center_y)
            .with_rotation(d.rotation)
            .with_feather(d.feather)
            .with_opacity(d.opacity)
            .with_flip(d.flip)
    }
}

impl From<CircleMaskFilter> for UICircleMaskDetail {
    fn from(f: CircleMaskFilter) -> Self {
        Self {
            center_x: f.center_x,
            center_y: f.center_y,
            rotation: 0.0,
            feather: f.feather,
            opacity: f.opacity,
            flip: f.flip,
            radius: f.radius as i32,
        }
    }
}

impl From<UICircleMaskDetail> for CircleMaskFilter {
    fn from(d: UICircleMaskDetail) -> Self {
        CircleMaskFilter::default()
            .with_center_x(d.center_x)
            .with_center_y(d.center_y)
            .with_feather(d.feather)
            .with_opacity(d.opacity)
            .with_flip(d.flip)
            .with_radius(d.radius as u32)
    }
}

impl From<MirrorMaskFilter> for UIMirrorMaskDetail {
    fn from(f: MirrorMaskFilter) -> Self {
        Self {
            center_x: f.center_x,
            center_y: f.center_y,
            rotation: f.rotation,
            feather: f.feather,
            opacity: f.opacity,
            flip: f.flip,
            width: f.width,
        }
    }
}

impl From<UIMirrorMaskDetail> for MirrorMaskFilter {
    fn from(d: UIMirrorMaskDetail) -> Self {
        MirrorMaskFilter::default()
            .with_center_x(d.center_x)
            .with_center_y(d.center_y)
            .with_rotation(d.rotation)
            .with_feather(d.feather)
            .with_opacity(d.opacity)
            .with_flip(d.flip)
            .with_width(d.width)
    }
}

impl From<RectangleMaskFilter> for UIRectangleMaskDetail {
    fn from(f: RectangleMaskFilter) -> Self {
        Self {
            center_x: f.center_x,
            center_y: f.center_y,
            rotation: f.rotation,
            feather: f.feather,
            opacity: f.opacity,
            flip: f.flip,
            width: f.width,
            height: f.height,
        }
    }
}

impl From<UIRectangleMaskDetail> for RectangleMaskFilter {
    fn from(d: UIRectangleMaskDetail) -> Self {
        RectangleMaskFilter::default()
            .with_center_x(d.center_x)
            .with_center_y(d.center_y)
            .with_rotation(d.rotation)
            .with_feather(d.feather)
            .with_opacity(d.opacity)
            .with_flip(d.flip)
            .with_width(d.width)
            .with_height(d.height)
    }
}

impl From<BorderFilter> for UIBorderDetail {
    fn from(f: BorderFilter) -> Self {
        Self {
            size: f.size as i32,
            color_r: f.color[0] as i32,
            color_g: f.color[1] as i32,
            color_b: f.color[2] as i32,
            color_a: f.color[3] as i32,
            corner_radius: f.corner_radius as i32,
        }
    }
}

impl From<UIBorderDetail> for BorderFilter {
    fn from(d: UIBorderDetail) -> Self {
        Self {
            size: d.size as u32,
            color: [
                d.color_r as u8,
                d.color_g as u8,
                d.color_b as u8,
                d.color_a as u8,
            ],
            corner_radius: d.corner_radius as u32,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl From<GridFilter> for UIGridDetail {
    fn from(f: GridFilter) -> Self {
        Self {
            rows: f.rows as i32,
            columns: f.columns as i32,
            line_size: f.line_size as i32,
            line_color_r: f.line_color[0] as i32,
            line_color_g: f.line_color[1] as i32,
            line_color_b: f.line_color[2] as i32,
            line_color_a: f.line_color[3] as i32,
        }
    }
}

impl From<UIGridDetail> for GridFilter {
    fn from(d: UIGridDetail) -> Self {
        Self {
            rows: d.rows.max(1) as u32,
            columns: d.columns.max(1) as u32,
            line_size: d.line_size as u32,
            line_color: [
                d.line_color_r as u8,
                d.line_color_g as u8,
                d.line_color_b as u8,
                d.line_color_a as u8,
            ],
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl From<BackgroundFilter> for UIBackgroundDetail {
    fn from(f: BackgroundFilter) -> Self {
        Self {
            color_r: f.color[0] as i32,
            color_g: f.color[1] as i32,
            color_b: f.color[2] as i32,
            color_a: f.color[3] as i32,
        }
    }
}

impl From<UIBackgroundDetail> for BackgroundFilter {
    fn from(d: UIBackgroundDetail) -> Self {
        Self {
            color: [
                d.color_r as u8,
                d.color_g as u8,
                d.color_b as u8,
                d.color_a as u8,
            ],
        }
    }
}

impl From<MosaicFilter> for UIMosaicDetail {
    fn from(f: MosaicFilter) -> Self {
        Self {
            left: f.left,
            top: f.top,
            width: f.width,
            height: f.height,
            block_size: f.block_size as i32,
        }
    }
}

impl From<UIMosaicDetail> for MosaicFilter {
    fn from(d: UIMosaicDetail) -> Self {
        Self {
            left: d.left,
            top: d.top,
            width: d.width,
            height: d.height,
            block_size: d.block_size.max(1) as u32,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl From<FlyInFilter> for UIFlyInDetail {
    fn from(f: FlyInFilter) -> Self {
        let easing: u8 = f.easing.into();
        let direction: u8 = f.direction.into();
        Self {
            duration_ms: f.duration.as_millis() as i32,
            direction: direction as i32,
            move_to_x: f.move_to_position.0,
            move_to_y: f.move_to_position.1,
            easing: easing as i32,
        }
    }
}

impl From<UIFlyInDetail> for FlyInFilter {
    fn from(d: UIFlyInDetail) -> Self {
        let easing: EasingFunction = EasingFunction::try_from(d.easing as u8).unwrap_or_default();
        let direction: FlyInDirection =
            FlyInDirection::try_from(d.direction as u8).unwrap_or_default();
        Self::default()
            .with_duration(Duration::from_millis(d.duration_ms as u64))
            .with_direction(direction)
            .with_move_to_position((d.move_to_x, d.move_to_y))
            .with_easing(easing)
    }
}

impl From<VideoFadeInFilter> for UIVideoFadeInDetail {
    fn from(f: VideoFadeInFilter) -> Self {
        Self {
            duration_ms: f.duration.as_millis() as i32,
        }
    }
}

impl From<UIVideoFadeInDetail> for VideoFadeInFilter {
    fn from(d: UIVideoFadeInDetail) -> Self {
        Self::new(Duration::from_millis(d.duration_ms as u64))
    }
}

impl From<VideoFadeOutFilter> for UIVideoFadeOutDetail {
    fn from(f: VideoFadeOutFilter) -> Self {
        Self {
            duration_ms: f.duration.as_millis() as i32,
        }
    }
}

impl From<UIVideoFadeOutDetail> for VideoFadeOutFilter {
    fn from(d: UIVideoFadeOutDetail) -> Self {
        Self::new(Duration::from_millis(d.duration_ms as u64))
    }
}

impl From<SlideFilter> for UISlideDetail {
    fn from(f: SlideFilter) -> Self {
        let direction: u8 = f.direction.into();
        let position: u8 = f.position.into();

        Self {
            direction: direction as i32,
            position: position as i32,
            duration_ms: f.duration.as_millis() as i32,
        }
    }
}

impl From<UISlideDetail> for SlideFilter {
    fn from(d: UISlideDetail) -> Self {
        let direction: SlideDirection = (d.direction as u8).try_into().unwrap_or_default();
        let position: EffectPosition = (d.position as u8).try_into().unwrap_or_default();

        Self::new(
            direction,
            position,
            Duration::from_millis(d.duration_ms as u64),
        )
    }
}

impl From<WipeFilter> for UIWipeDetail {
    fn from(f: WipeFilter) -> Self {
        let direction: u8 = f.direction.into();
        Self {
            direction: direction as i32,
            duration_ms: f.duration.as_millis() as i32,
        }
    }
}

impl From<UIWipeDetail> for WipeFilter {
    fn from(d: UIWipeDetail) -> Self {
        let direction: WipeDirection = (d.direction as u8).try_into().unwrap_or_default();
        Self::new(direction, Duration::from_millis(d.duration_ms as u64))
    }
}

impl From<UIDrawCircleDetail> for DrawCircleFilter {
    fn from(d: UIDrawCircleDetail) -> Self {
        let fill_color = Some((
            d.fill_color_r as u8,
            d.fill_color_g as u8,
            d.fill_color_b as u8,
            d.fill_color_a as u8,
        ));

        let border_color = Some((
            d.border_color_r as u8,
            d.border_color_g as u8,
            d.border_color_b as u8,
            d.border_color_a as u8,
        ));

        Self::default()
            .with_center_x(d.center_x)
            .with_center_y(d.center_y)
            .with_radius(d.radius as u32)
            .with_fill_color(fill_color)
            .with_border_color(border_color)
            .with_border_width(d.border_width as u32)
    }
}

impl From<UIDrawRectangleDetail> for DrawRectangleFilter {
    fn from(d: UIDrawRectangleDetail) -> Self {
        let fill_color = Some((
            d.fill_color_r as u8,
            d.fill_color_g as u8,
            d.fill_color_b as u8,
            d.fill_color_a as u8,
        ));

        let border_color = Some((
            d.border_color_r as u8,
            d.border_color_g as u8,
            d.border_color_b as u8,
            d.border_color_a as u8,
        ));

        Self::default()
            .with_x(d.x)
            .with_y(d.y)
            .with_width(d.width)
            .with_height(d.height)
            .with_fill_color(fill_color)
            .with_border_color(border_color)
            .with_border_width(d.border_width as u32)
            .with_corner_radius(d.corner_radius as u32)
    }
}

impl From<DrawCircleFilter> for UIDrawCircleDetail {
    fn from(f: DrawCircleFilter) -> Self {
        let (fill_r, fill_g, fill_b, fill_a) = f
            .fill_color
            .map(|c| (c.0 as i32, c.1 as i32, c.2 as i32, c.3 as i32))
            .unwrap_or((0, 0, 0, 0));

        let (border_r, border_g, border_b, border_a) = f
            .border_color
            .map(|c| (c.0 as i32, c.1 as i32, c.2 as i32, c.3 as i32))
            .unwrap_or((0, 0, 0, 0));

        Self {
            center_x: f.center_x,
            center_y: f.center_y,
            radius: f.radius as i32,
            fill_color_r: fill_r,
            fill_color_g: fill_g,
            fill_color_b: fill_b,
            fill_color_a: fill_a,
            border_color_r: border_r,
            border_color_g: border_g,
            border_color_b: border_b,
            border_color_a: border_a,
            border_width: f.border_width as i32,
        }
    }
}

impl From<DrawRectangleFilter> for UIDrawRectangleDetail {
    fn from(f: DrawRectangleFilter) -> Self {
        let (fill_r, fill_g, fill_b, fill_a) = f
            .fill_color
            .map(|c| (c.0 as i32, c.1 as i32, c.2 as i32, c.3 as i32))
            .unwrap_or((0, 0, 0, 0));

        let (border_r, border_g, border_b, border_a) = f
            .border_color
            .map(|c| (c.0 as i32, c.1 as i32, c.2 as i32, c.3 as i32))
            .unwrap_or((0, 0, 0, 0));

        Self {
            x: f.x,
            y: f.y,
            width: f.width,
            height: f.height,
            fill_color_r: fill_r,
            fill_color_g: fill_g,
            fill_color_b: fill_b,
            fill_color_a: fill_a,
            border_color_r: border_r,
            border_color_g: border_g,
            border_color_b: border_b,
            border_color_a: border_a,
            border_width: f.border_width as i32,
            corner_radius: f.corner_radius as i32,
        }
    }
}

// ==================== Audio Filters ====================

impl From<GainFilter> for UIGainDetail {
    fn from(f: GainFilter) -> Self {
        Self {
            amplitude: f.amplitude,
        }
    }
}

impl From<UIGainDetail> for GainFilter {
    fn from(d: UIGainDetail) -> Self {
        Self::default().with_amplitude(d.amplitude)
    }
}

impl From<AudioFadeInFilter> for UIFadeInDetail {
    fn from(f: AudioFadeInFilter) -> Self {
        Self {
            duration_ms: f.duration.as_millis() as i32,
        }
    }
}

impl From<UIFadeInDetail> for AudioFadeInFilter {
    fn from(d: UIFadeInDetail) -> Self {
        Self {
            duration: Duration::from_millis(d.duration_ms as u64),
        }
    }
}

impl From<AudioFadeOutFilter> for UIFadeOutDetail {
    fn from(f: AudioFadeOutFilter) -> Self {
        Self {
            duration_ms: f.duration.as_millis() as i32,
        }
    }
}

impl From<UIFadeOutDetail> for AudioFadeOutFilter {
    fn from(d: UIFadeOutDetail) -> Self {
        Self {
            duration: Duration::from_millis(d.duration_ms as u64),
        }
    }
}

impl From<CompressorFilter> for UICompressorDetail {
    fn from(f: CompressorFilter) -> Self {
        Self {
            threshold: f.threshold,
            ratio: f.ratio,
            attack: f.attack,
            release: f.release,
            makeup_gain: f.makeup_gain,
        }
    }
}

impl From<UICompressorDetail> for CompressorFilter {
    fn from(d: UICompressorDetail) -> Self {
        Self {
            threshold: d.threshold,
            ratio: d.ratio,
            attack: d.attack,
            release: d.release,
            makeup_gain: d.makeup_gain,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl From<LimiterFilter> for UILimiterDetail {
    fn from(f: LimiterFilter) -> Self {
        Self {
            threshold: f.threshold,
        }
    }
}

impl From<UILimiterDetail> for LimiterFilter {
    fn from(d: UILimiterDetail) -> Self {
        Self {
            threshold: d.threshold,
        }
    }
}

impl From<NoiseGateFilter> for UINoiseGateDetail {
    fn from(f: NoiseGateFilter) -> Self {
        Self {
            threshold: f.threshold,
            ratio: f.ratio,
            attack: f.attack,
            hold: f.hold,
            release: f.release,
        }
    }
}

impl From<UINoiseGateDetail> for NoiseGateFilter {
    fn from(d: UINoiseGateDetail) -> Self {
        Self {
            threshold: d.threshold,
            ratio: d.ratio,
            attack: d.attack,
            hold: d.hold,
            release: d.release,
        }
    }
}

impl From<NormalizeFilter> for UINormalizeDetail {
    fn from(f: NormalizeFilter) -> Self {
        Self {
            target_level_db: f.target_level_db,
        }
    }
}

impl From<UINormalizeDetail> for NormalizeFilter {
    fn from(d: UINormalizeDetail) -> Self {
        Self {
            target_level_db: d.target_level_db,
        }
    }
}

impl From<MuteFilter> for UIMuteDetail {
    fn from(f: MuteFilter) -> Self {
        let channel: u8 = f.channel.into();
        Self {
            channel: channel as i32,
        }
    }
}

impl From<UIMuteDetail> for MuteFilter {
    fn from(d: UIMuteDetail) -> Self {
        let channel: MuteChannel = (d.channel as u8).try_into().unwrap_or_default();
        Self { channel }
    }
}

impl From<CopyChannelFilter> for UICopyChannelDetail {
    fn from(f: CopyChannelFilter) -> Self {
        let direction: u8 = f.direction.into();
        Self {
            direction: direction as i32,
        }
    }
}

impl From<UICopyChannelDetail> for CopyChannelFilter {
    fn from(d: UICopyChannelDetail) -> Self {
        let direction: CopyDirection = (d.direction as u8).try_into().unwrap_or_default();
        Self { direction }
    }
}

impl From<VoiceChangerFilter> for UIVoiceChangerDetail {
    fn from(f: VoiceChangerFilter) -> Self {
        Self {
            pitch_semitones: f.pitch_semitones,
            formant_semitones: f.formant_semitones,
        }
    }
}

impl From<UIVoiceChangerDetail> for VoiceChangerFilter {
    fn from(d: UIVoiceChangerDetail) -> Self {
        Self::default()
            .with_pitch_semitones(d.pitch_semitones)
            .with_formant_semitones(d.formant_semitones)
    }
}

// ==================== Subtitle Filters ====================

impl From<FontPathFilter> for UIFontPathDetail {
    fn from(f: FontPathFilter) -> Self {
        Self {
            font_path: f.font_path.to_string_lossy().to_string().into(),
            font_family: f.font_family.into(),
            font_style: f.font_style.into(),
        }
    }
}

impl From<UIFontPathDetail> for FontPathFilter {
    fn from(d: UIFontPathDetail) -> Self {
        Self {
            font_path: PathBuf::from(d.font_path.as_str()),
            font_family: d.font_family.as_str().into(),
            font_style: d.font_style.as_str().into(),
        }
    }
}

impl From<FontSizeFilter> for UIFontSizeDetail {
    fn from(f: FontSizeFilter) -> Self {
        Self {
            font_size: f.font_size as i32,
        }
    }
}

impl From<UIFontSizeDetail> for FontSizeFilter {
    fn from(d: UIFontSizeDetail) -> Self {
        Self {
            font_size: d.font_size as u32,
        }
    }
}

impl From<PaddingFilter> for UIPaddingDetail {
    fn from(f: PaddingFilter) -> Self {
        Self {
            padding: f.padding.unwrap_or(0) as i32,
        }
    }
}

impl From<UIPaddingDetail> for PaddingFilter {
    fn from(d: UIPaddingDetail) -> Self {
        Self::new(d.padding)
    }
}

impl From<MarginVerticalFilter> for UIMarginVerticalDetail {
    fn from(f: MarginVerticalFilter) -> Self {
        Self {
            margin: f.margin.unwrap_or(0) as i32,
        }
    }
}

impl From<UIMarginVerticalDetail> for MarginVerticalFilter {
    fn from(d: UIMarginVerticalDetail) -> Self {
        Self {
            margin: if d.margin > 0 {
                Some(d.margin as u32)
            } else {
                None
            },
        }
    }
}

impl From<MarginHorizontalFilter> for UIMarginHorizontalDetail {
    fn from(f: MarginHorizontalFilter) -> Self {
        Self {
            margin: f.margin.unwrap_or(0) as i32,
        }
    }
}

impl From<UIMarginHorizontalDetail> for MarginHorizontalFilter {
    fn from(d: UIMarginHorizontalDetail) -> Self {
        Self::new(d.margin)
    }
}

impl From<OutlineWidthFilter> for UIOutlineWidthDetail {
    fn from(f: OutlineWidthFilter) -> Self {
        Self {
            width: f.width.unwrap_or(0) as i32,
        }
    }
}

impl From<UIOutlineWidthDetail> for OutlineWidthFilter {
    fn from(d: UIOutlineWidthDetail) -> Self {
        Self::new(d.width)
    }
}

impl From<BorderRadiusFilter> for UIBorderRadiusDetail {
    fn from(f: BorderRadiusFilter) -> Self {
        Self {
            radius: f.radius.unwrap_or(0) as i32,
        }
    }
}

impl From<UIBorderRadiusDetail> for BorderRadiusFilter {
    fn from(d: UIBorderRadiusDetail) -> Self {
        Self::new(d.radius)
    }
}

impl From<PrimaryColorFilter> for UIPrimaryColorDetail {
    fn from(f: PrimaryColorFilter) -> Self {
        f.color
            .map(|c| Self {
                r: c[0] as i32,
                g: c[1] as i32,
                b: c[2] as i32,
                a: c[3] as i32,
            })
            .unwrap_or(Self {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            })
    }
}

impl From<UIPrimaryColorDetail> for PrimaryColorFilter {
    fn from(d: UIPrimaryColorDetail) -> Self {
        Self {
            color: Some(Rgba([d.r as u8, d.g as u8, d.b as u8, d.a as u8])),
        }
    }
}

impl From<OutlineColorFilter> for UIOutlineColorDetail {
    fn from(f: OutlineColorFilter) -> Self {
        f.color
            .map(|c| Self {
                r: c[0] as i32,
                g: c[1] as i32,
                b: c[2] as i32,
                a: c[3] as i32,
            })
            .unwrap_or(Self {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            })
    }
}

impl From<UIOutlineColorDetail> for OutlineColorFilter {
    fn from(d: UIOutlineColorDetail) -> Self {
        Self {
            color: Some(Rgba([d.r as u8, d.g as u8, d.b as u8, d.a as u8])),
        }
    }
}

impl From<BackgroundColorFilter> for UIBackgroundColorDetail {
    fn from(f: BackgroundColorFilter) -> Self {
        f.color
            .map(|c| Self {
                r: c[0] as i32,
                g: c[1] as i32,
                b: c[2] as i32,
                a: c[3] as i32,
            })
            .unwrap_or(Self {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            })
    }
}

impl From<UIBackgroundColorDetail> for BackgroundColorFilter {
    fn from(d: UIBackgroundColorDetail) -> Self {
        Self {
            color: Some(Rgba([d.r as u8, d.g as u8, d.b as u8, d.a as u8])),
        }
    }
}

impl From<TextAlignmentFilter> for UITextAlignmentDetail {
    fn from(f: TextAlignmentFilter) -> Self {
        let alignment: i32 = f
            .alignment
            .map(|a| match a {
                TextAlignment::Left => 0,
                TextAlignment::Center => 1,
                TextAlignment::Right => 2,
            })
            .unwrap_or(1);
        Self { alignment }
    }
}

impl From<UITextAlignmentDetail> for TextAlignmentFilter {
    fn from(d: UITextAlignmentDetail) -> Self {
        let alignment = match d.alignment {
            0 => TextAlignment::Left,
            1 => TextAlignment::Center,
            2 => TextAlignment::Right,
            _ => TextAlignment::Center,
        };
        Self::new(alignment)
    }
}

impl From<AnimatableProperty> for UIAnimatableProperty {
    fn from(p: AnimatableProperty) -> Self {
        Self {
            name: SharedString::from(p.name),
            display_name: SharedString::from(p.display_name),
            min_value: p.min_value,
            max_value: p.max_value,
            default_value: p.default_value.into(),
        }
    }
}

impl From<PropertyTrack> for UIPropertyTrack {
    fn from(t: PropertyTrack) -> Self {
        let keyframes: Vec<UIKeyframe> = t.keyframes.into_iter().map(|k| k.into()).collect();
        Self {
            filter_name: SharedString::new(),
            property_name: SharedString::from(t.property_name),
            keyframes: ModelRc::new(VecModel::from_slice(&keyframes)),
        }
    }
}

impl From<Keyframe> for UIKeyframe {
    fn from(k: Keyframe) -> Self {
        Self {
            time_ms: k.time_ms as i32,
            value: k.value.into(),
        }
    }
}

impl From<KeyframeValue> for UIKeyframeValue {
    fn from(v: KeyframeValue) -> Self {
        match v {
            KeyframeValue::Float(f) => Self {
                float_value: f,
                float2_value_x: 0.0,
                float2_value_y: 0.0,
                color_r: 255,
                color_g: 255,
                color_b: 255,
                color_a: 255,
                bool_value: false,
                value_type: UIKeyframeValueType::Float,
            },
            KeyframeValue::Float2(x, y) => Self {
                float_value: 0.0,
                float2_value_x: x,
                float2_value_y: y,
                color_r: 255,
                color_g: 255,
                color_b: 255,
                color_a: 255,
                bool_value: false,
                value_type: UIKeyframeValueType::Float2,
            },
            KeyframeValue::Color(r, g, b, a) => Self {
                float_value: 0.0,
                float2_value_x: 0.0,
                float2_value_y: 0.0,
                color_r: r as i32,
                color_g: g as i32,
                color_b: b as i32,
                color_a: a as i32,
                bool_value: false,
                value_type: UIKeyframeValueType::Color,
            },
            KeyframeValue::Bool(b) => Self {
                float_value: 0.0,
                float2_value_x: 0.0,
                float2_value_y: 0.0,
                color_r: 255,
                color_g: 255,
                color_b: 255,
                color_a: 255,
                bool_value: b,
                value_type: UIKeyframeValueType::Bool,
            },
        }
    }
}

impl From<UIKeyframeValue> for KeyframeValue {
    fn from(v: UIKeyframeValue) -> Self {
        match v.value_type {
            UIKeyframeValueType::Float => Self::Float(v.float_value),
            UIKeyframeValueType::Float2 => Self::Float2(v.float2_value_x, v.float2_value_y),
            UIKeyframeValueType::Color => Self::Color(
                v.color_r as u8,
                v.color_g as u8,
                v.color_b as u8,
                v.color_a as u8,
            ),
            UIKeyframeValueType::Bool => Self::Bool(v.bool_value),
        }
    }
}

impl From<HSLAdjustFilter> for UIHslAdjustDetail {
    fn from(f: HSLAdjustFilter) -> Self {
        let luminance_standard: i32 = match f.luminance_standard {
            LuminanceStandard::BT709 => 0,
            LuminanceStandard::BT601 => 1,
            LuminanceStandard::BT2020 => 2,
        };
        Self {
            hue_shift: f.hue_shift,
            saturation: f.saturation,
            lightness: f.lightness,
            preserve_luminance: f.preserve_luminance,
            luminance_standard,
        }
    }
}

impl From<UIHslAdjustDetail> for HSLAdjustFilter {
    fn from(d: UIHslAdjustDetail) -> Self {
        let luminance_standard = match d.luminance_standard {
            0 => LuminanceStandard::BT709,
            1 => LuminanceStandard::BT601,
            2 => LuminanceStandard::BT2020,
            _ => LuminanceStandard::BT709,
        };
        Self {
            hue_shift: d.hue_shift.clamp(-180.0, 180.0),
            saturation: d.saturation.clamp(-1.0, 1.0),
            lightness: d.lightness.clamp(-1.0, 1.0),
            preserve_luminance: d.preserve_luminance,
            luminance_standard,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl From<SpeedFilter> for UISpeedDetail {
    fn from(f: SpeedFilter) -> Self {
        Self { speed: f.speed }
    }
}

impl From<UISpeedDetail> for SpeedFilter {
    fn from(d: UISpeedDetail) -> Self {
        Self {
            speed: d.speed.clamp(0.1, 10.0),
        }
    }
}

impl From<AudioSpeedFilter> for UISpeedDetail {
    fn from(f: AudioSpeedFilter) -> Self {
        Self { speed: f.speed }
    }
}

impl From<UISpeedDetail> for AudioSpeedFilter {
    fn from(d: UISpeedDetail) -> Self {
        Self {
            speed: d.speed.clamp(0.1, 10.0),
        }
    }
}

impl From<BreathingFilter> for UIBreathingDetail {
    fn from(f: BreathingFilter) -> Self {
        Self {
            center_x: f.center_x,
            center_y: f.center_y,
            min_scale: f.min_scale,
            breathing_duration_ms: f.breathing_duration.as_millis() as i32,
            curve: match f.curve {
                BreathingCurve::Linear => 1,
                BreathingCurve::EaseInOut => 0,
            },
        }
    }
}

impl From<UIBreathingDetail> for BreathingFilter {
    fn from(d: UIBreathingDetail) -> Self {
        Self::default()
            .with_breathing_duration(Duration::from_millis(d.breathing_duration_ms as u64))
            .with_min_scale(d.min_scale.clamp(0.1, 1.0))
            .with_center(d.center_x.clamp(0.0, 1.0), d.center_y.clamp(0.0, 1.0))
            .with_curve(match d.curve {
                0 => BreathingCurve::EaseInOut,
                1 => BreathingCurve::Linear,
                _ => BreathingCurve::default(),
            })
    }
}

impl From<LocalMagnifyFilter> for UILocalMagnifyDetail {
    fn from(f: LocalMagnifyFilter) -> Self {
        let (border_r, border_g, border_b, border_a) = f
            .border_color
            .map(|c| (c.0 as i32, c.1 as i32, c.2 as i32, c.3 as i32))
            .unwrap_or((0, 0, 0, 0));

        Self {
            center_x: f.center_x,
            center_y: f.center_y,
            selection_radius: f.selection_radius as i32,
            scale: f.scale,
            border_color_r: border_r,
            border_color_g: border_g,
            border_color_b: border_b,
            border_color_a: border_a,
            border_width: f.border_width as i32,
        }
    }
}

impl From<UILocalMagnifyDetail> for LocalMagnifyFilter {
    fn from(d: UILocalMagnifyDetail) -> Self {
        let border_color = Some((
            d.border_color_r as u8,
            d.border_color_g as u8,
            d.border_color_b as u8,
            d.border_color_a as u8,
        ));

        Self::default()
            .with_center_x(d.center_x.clamp(0.0, 1.0))
            .with_center_y(d.center_y.clamp(0.0, 1.0))
            .with_selection_radius(d.selection_radius.max(0) as u32)
            .with_scale(d.scale.clamp(1.0, 10.0))
            .with_border_color(border_color)
            .with_border_width(d.border_width.max(0) as u32)
    }
}

impl From<MagnifierFilter> for UIMagnifierDetail {
    fn from(f: MagnifierFilter) -> Self {
        let (border_r, border_g, border_b, border_a) = f
            .border_color
            .map(|c| (c.0 as i32, c.1 as i32, c.2 as i32, c.3 as i32))
            .unwrap_or((255, 255, 255, 255));

        Self {
            center_x: f.center_x,
            center_y: f.center_y,
            radius: f.radius as i32,
            scale: f.scale,
            border_color_r: border_r,
            border_color_g: border_g,
            border_color_b: border_b,
            border_color_a: border_a,
            border_width: f.border_width as i32,
        }
    }
}

impl From<UIMagnifierDetail> for MagnifierFilter {
    fn from(d: UIMagnifierDetail) -> Self {
        let border_color = Some((
            d.border_color_r as u8,
            d.border_color_g as u8,
            d.border_color_b as u8,
            d.border_color_a as u8,
        ));

        Self::default()
            .with_center_x(d.center_x.clamp(0.0, 1.0))
            .with_center_y(d.center_y.clamp(0.0, 1.0))
            .with_radius(d.radius.max(0) as u32)
            .with_scale(d.scale.clamp(1.0, 10.0))
            .with_border_color(border_color)
            .with_border_width(d.border_width.max(0) as u32)
    }
}

impl From<GaussianBlurFilter> for UIGaussianBlurDetail {
    fn from(f: GaussianBlurFilter) -> Self {
        Self {
            radius: f.radius,
            sigma: f.sigma,
            left: f.left,
            top: f.top,
            width: f.width,
            height: f.height,
        }
    }
}

impl From<UIGaussianBlurDetail> for GaussianBlurFilter {
    fn from(d: UIGaussianBlurDetail) -> Self {
        Self::new(d.radius.clamp(0.0, 50.0))
            .with_sigma(d.sigma.clamp(0.1, 20.0))
            .with_left(d.left.clamp(0.0, 1.0))
            .with_top(d.top.clamp(0.0, 1.0))
            .with_width(d.width.clamp(0.0, 1.0))
            .with_height(d.height.clamp(0.0, 1.0))
            .with_keyframe_tracks(KeyframeTracks::default())
    }
}

impl From<DirectionalBlurFilter> for UIDirectionalBlurDetail {
    fn from(f: DirectionalBlurFilter) -> Self {
        Self {
            angle: f.angle,
            length: f.length,
            spread: f.spread,
        }
    }
}

impl From<UIDirectionalBlurDetail> for DirectionalBlurFilter {
    fn from(d: UIDirectionalBlurDetail) -> Self {
        Self::new(d.angle % 360.0, d.length.clamp(0.0, 100.0))
            .with_spread(d.spread.clamp(0.0, 1.0))
            .with_keyframe_tracks(KeyframeTracks::default())
    }
}

impl From<SharpenFilter> for UISharpenDetail {
    fn from(f: SharpenFilter) -> Self {
        Self {
            strength: f.strength,
            radius: f.radius,
        }
    }
}

impl From<UISharpenDetail> for SharpenFilter {
    fn from(d: UISharpenDetail) -> Self {
        Self::new(d.strength.clamp(0.0, 5.0))
            .with_radius(d.radius.clamp(0.0, 10.0))
            .with_keyframe_tracks(KeyframeTracks::default())
    }
}

impl From<EdgeDetectFilter> for UIEdgeDetectDetail {
    fn from(f: EdgeDetectFilter) -> Self {
        Self {
            threshold: f.threshold,
            strength: f.strength,
            invert: f.invert,
            edge_color_r: f.edge_color[0] as i32,
            edge_color_g: f.edge_color[1] as i32,
            edge_color_b: f.edge_color[2] as i32,
            edge_color_a: f.edge_color[3] as i32,
            background_color_r: f.background_color[0] as i32,
            background_color_g: f.background_color[1] as i32,
            background_color_b: f.background_color[2] as i32,
            background_color_a: f.background_color[3] as i32,
        }
    }
}

impl From<UIEdgeDetectDetail> for EdgeDetectFilter {
    fn from(d: UIEdgeDetectDetail) -> Self {
        Self::new(d.threshold.clamp(0.0, 255.0), d.strength.clamp(0.0, 2.0))
            .with_invert(d.invert)
            .with_edge_color([
                d.edge_color_r as u8,
                d.edge_color_g as u8,
                d.edge_color_b as u8,
                d.edge_color_a as u8,
            ])
            .with_background_color([
                d.background_color_r as u8,
                d.background_color_g as u8,
                d.background_color_b as u8,
                d.background_color_a as u8,
            ])
            .with_keyframe_tracks(KeyframeTracks::default())
    }
}

impl From<GrainFilter> for UIGrainDetail {
    fn from(f: GrainFilter) -> Self {
        Self {
            intensity: f.intensity,
            grain_size: f.grain_size,
            colored: f.colored,
            roughness: f.roughness,
            seed: f.seed as i32,
        }
    }
}

impl From<UIGrainDetail> for GrainFilter {
    fn from(d: UIGrainDetail) -> Self {
        GrainFilter::new(d.intensity.clamp(0.0, 1.0))
            .with_grain_size(d.grain_size.clamp(1.0, 10.0))
            .with_colored(d.colored)
            .with_roughness(d.roughness.clamp(0.0, 1.0))
            .with_seed(d.seed as u32)
            .with_keyframe_tracks(KeyframeTracks::default())
    }
}

impl From<FisheyeFilter> for UIFisheyeDetail {
    fn from(f: FisheyeFilter) -> Self {
        Self {
            center_x: f.center_x,
            center_y: f.center_y,
            strength: f.strength,
            radius: f.radius as i32,
        }
    }
}

impl From<UIFisheyeDetail> for FisheyeFilter {
    fn from(d: UIFisheyeDetail) -> Self {
        Self::new(
            d.center_x.clamp(0.0, 1.0),
            d.center_y.clamp(0.0, 1.0),
            d.strength.clamp(-1.0, 2.0),
            d.radius.max(0) as u32,
        )
    }
}

impl From<FocusFilter> for UIFocusDetail {
    fn from(f: FocusFilter) -> Self {
        Self {
            center_x: f.center_x,
            center_y: f.center_y,
            focus_radius: f.focus_radius as i32,
            feather: f.feather as i32,
            blur_radius: f.blur_radius as i32,
            aperture_blades: f.aperture_blades as i32,
            highlight_boost: f.highlight_boost,
        }
    }
}

impl From<UIFocusDetail> for FocusFilter {
    fn from(d: UIFocusDetail) -> Self {
        Self::new(
            d.center_x.clamp(0.0, 1.0),
            d.center_y.clamp(0.0, 1.0),
            d.focus_radius.max(0) as u32,
            d.blur_radius.max(0) as u32,
        )
        .with_feather(d.feather.max(0) as u32)
        .with_aperture_blades(d.aperture_blades.clamp(3, 12) as u32)
        .with_highlight_boost(d.highlight_boost.clamp(0.0, 2.0))
    }
}

impl From<GrayscaleFilter> for UIGrayscaleDetail {
    fn from(f: GrayscaleFilter) -> Self {
        let luminance_standard: i32 = match f.luminance_standard {
            LuminanceStandard::BT709 => 0,
            LuminanceStandard::BT601 => 1,
            LuminanceStandard::BT2020 => 2,
        };
        Self {
            intensity: f.intensity,
            contrast: f.contrast,
            luminance_standard,
        }
    }
}

impl From<UIGrayscaleDetail> for GrayscaleFilter {
    fn from(d: UIGrayscaleDetail) -> Self {
        let luminance_standard = match d.luminance_standard {
            0 => LuminanceStandard::BT709,
            1 => LuminanceStandard::BT601,
            2 => LuminanceStandard::BT2020,
            _ => LuminanceStandard::BT709,
        };
        Self::new(d.intensity.clamp(0.0, 1.0))
            .with_contrast(d.contrast.clamp(-1.0, 1.0))
            .with_luminance_standard(luminance_standard)
            .with_keyframe_tracks(KeyframeTracks::default())
    }
}

impl From<OldFilmFilter> for UIOldFilmDetail {
    fn from(f: OldFilmFilter) -> Self {
        Self {
            seed: f.seed as i32,
            scratch_intensity: f.scratch_intensity,
            scratch_count: f.scratch_count as i32,
            scratch_width: f.scratch_width as i32,
            dust_intensity: f.dust_intensity,
            dust_count: f.dust_count as i32,
            dust_size_max: f.dust_size_max as i32,
            flicker_intensity: f.flicker_intensity,
            flicker_speed: f.flicker_speed,
            vertical_lines_intensity: f.vertical_lines_intensity,
            vertical_lines_count: f.vertical_lines_count as i32,
            jitter_intensity: f.jitter_intensity,
            sepia_intensity: f.sepia_intensity,
        }
    }
}

impl From<UIOldFilmDetail> for OldFilmFilter {
    fn from(d: UIOldFilmDetail) -> Self {
        Self::default()
            .with_seed(d.seed as u64)
            .with_scratch_intensity(d.scratch_intensity.clamp(0.0, 1.0))
            .with_scratch_count(d.scratch_count as u32)
            .with_scratch_width(d.scratch_width as u32)
            .with_dust_intensity(d.dust_intensity.clamp(0.0, 1.0))
            .with_dust_count(d.dust_count as u32)
            .with_dust_size_max(d.dust_size_max as u32)
            .with_flicker_intensity(d.flicker_intensity.clamp(0.0, 0.3))
            .with_flicker_speed(d.flicker_speed.clamp(1.0, 10.0))
            .with_vertical_lines_intensity(d.vertical_lines_intensity.clamp(0.0, 1.0))
            .with_vertical_lines_count(d.vertical_lines_count as u32)
            .with_jitter_intensity(d.jitter_intensity.clamp(0.0, 10.0))
            .with_sepia_intensity(d.sepia_intensity.clamp(0.0, 1.0))
            .with_keyframe_tracks(KeyframeTracks::default())
    }
}

impl From<SketchFilter> for UISketchDetail {
    fn from(f: SketchFilter) -> Self {
        Self {
            line_intensity: f.line_intensity,
            line_width: f.line_width,
            detail_level: f.detail_level,
            paper_color_r: f.paper_color[0] as i32,
            paper_color_g: f.paper_color[1] as i32,
            paper_color_b: f.paper_color[2] as i32,
            paper_color_a: f.paper_color[3] as i32,
            pencil_color_r: f.pencil_color[0] as i32,
            pencil_color_g: f.pencil_color[1] as i32,
            pencil_color_b: f.pencil_color[2] as i32,
            pencil_color_a: f.pencil_color[3] as i32,
        }
    }
}

impl From<UISketchDetail> for SketchFilter {
    fn from(d: UISketchDetail) -> Self {
        Self::new(
            d.line_intensity.clamp(0.0, 1.0),
            d.line_width.clamp(1.0, 10.0),
        )
        .with_paper_color([
            d.paper_color_r as u8,
            d.paper_color_g as u8,
            d.paper_color_b as u8,
            d.paper_color_a as u8,
        ])
        .with_pencil_color([
            d.pencil_color_r as u8,
            d.pencil_color_g as u8,
            d.pencil_color_b as u8,
            d.pencil_color_a as u8,
        ])
        .with_detail_level(d.detail_level.clamp(0.0, 1.0))
        .with_keyframe_tracks(KeyframeTracks::default())
    }
}

impl From<WaveFilter> for UIWaveDetail {
    fn from(f: WaveFilter) -> Self {
        let wave_type: i32 = match f.wave_type {
            WaveType::Horizontal => 0,
            WaveType::Vertical => 1,
            WaveType::Radial => 2,
            WaveType::Concentric => 3,
        };
        Self {
            amplitude: f.amplitude,
            frequency: f.frequency,
            speed: f.speed,
            phase: f.phase,
            wave_type,
            center_x: f.center_x,
            center_y: f.center_y,
        }
    }
}

impl From<UIWaveDetail> for WaveFilter {
    fn from(d: UIWaveDetail) -> Self {
        let wave_type = match d.wave_type {
            0 => WaveType::Horizontal,
            1 => WaveType::Vertical,
            2 => WaveType::Radial,
            3 => WaveType::Concentric,
            _ => WaveType::Horizontal,
        };
        Self::new(
            d.amplitude.clamp(0.0, 100.0),
            d.frequency.clamp(0.1, 10.0),
            wave_type,
        )
        .with_speed(d.speed.clamp(0.0, 10.0))
        .with_phase(d.phase.clamp(0.0, 360.0))
        .with_center_x(d.center_x.clamp(0.0, 1.0))
        .with_center_y(d.center_y.clamp(0.0, 1.0))
        .with_keyframe_tracks(KeyframeTracks::default())
    }
}

impl From<TextHighlightFilter> for UITextHighlightDetail {
    fn from(f: TextHighlightFilter) -> Self {
        let regions: Vec<UIHighlightRegionDetail> = f
            .regions
            .iter()
            .map(|r| UIHighlightRegionDetail {
                x: r.x,
                y: r.y,
                width: r.width,
                height: r.height,
            })
            .collect();
        Self {
            highlight_color_r: f.highlight_color[0] as i32,
            highlight_color_g: f.highlight_color[1] as i32,
            highlight_color_b: f.highlight_color[2] as i32,
            highlight_color_a: f.highlight_color[3] as i32,
            background_color_r: f.background_color_to_detect[0] as i32,
            background_color_g: f.background_color_to_detect[1] as i32,
            background_color_b: f.background_color_to_detect[2] as i32,
            pixel_per_second: f.pixel_per_second as i32,
            invert: f.invert,
            similarity_threshold: f.similarity_threshold,
            regions: ModelRc::new(VecModel::from_slice(&regions)),
        }
    }
}

impl From<UITextHighlightDetail> for TextHighlightFilter {
    fn from(d: UITextHighlightDetail) -> Self {
        let regions: Vec<HighlightRegion> = d.regions.iter().map(|r| r.into()).collect();
        Self::new(regions)
            .with_highlight_color([
                d.highlight_color_r as u8,
                d.highlight_color_g as u8,
                d.highlight_color_b as u8,
                d.highlight_color_a as u8,
            ])
            .with_background_color_to_detect([
                d.background_color_r as u8,
                d.background_color_g as u8,
                d.background_color_b as u8,
            ])
            .with_pixel_per_second(d.pixel_per_second as u32)
            .with_invert(d.invert)
            .with_similarity_threshold(d.similarity_threshold)
    }
}

impl From<UIHighlightRegionDetail> for HighlightRegion {
    fn from(r: UIHighlightRegionDetail) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

impl From<ShadowFilter> for UIShadowDetail {
    fn from(f: ShadowFilter) -> Self {
        Self {
            color_r: f.color[0] as i32,
            color_g: f.color[1] as i32,
            color_b: f.color[2] as i32,
            color_a: f.color[3] as i32,
            opacity: f.opacity,
            size: f.size,
            blur: f.blur,
            angle: f.angle,
            distance: f.distance,
        }
    }
}

impl From<UIShadowDetail> for ShadowFilter {
    fn from(d: UIShadowDetail) -> Self {
        let mut filter = ShadowFilter::new(
            [
                d.color_r as u8,
                d.color_g as u8,
                d.color_b as u8,
                d.color_a as u8,
            ],
            d.opacity.clamp(0.0, 1.0),
            d.blur.clamp(0.0, 100.0),
            d.angle,
            d.distance.clamp(0.0, 200.0),
        )
        .with_size(d.size.clamp(0.0, 1.0));
        filter.keyframe_tracks = KeyframeTracks::default();
        filter
    }
}

impl From<DeviceFrameFilter> for UIDeviceFrameDetail {
    fn from(f: DeviceFrameFilter) -> Self {
        Self {
            device_name: f.device_name.into(),
            screen_background_r: f.screen_background_color[0] as i32,
            screen_background_g: f.screen_background_color[1] as i32,
            screen_background_b: f.screen_background_color[2] as i32,
            screen_background_a: f.screen_background_color[3] as i32,
        }
    }
}

impl From<UIDeviceFrameDetail> for DeviceFrameFilter {
    fn from(d: UIDeviceFrameDetail) -> Self {
        Self {
            device_name: d.device_name.into(),
            screen_background_color: [
                d.screen_background_r as u8,
                d.screen_background_g as u8,
                d.screen_background_b as u8,
                d.screen_background_a as u8,
            ],
        }
    }
}

impl From<GenieFilter> for UIGenieDetail {
    fn from(f: GenieFilter) -> Self {
        Self {
            position: u8::from(f.position) as i32,
            duration: f.duration.as_millis() as i32,
            anchor: u8::from(f.anchor) as i32,
            funnel_power: f.funnel_power,
            shadow: f.shadow,
            easing: u8::from(f.easing) as i32,
        }
    }
}

impl From<UIGenieDetail> for GenieFilter {
    fn from(d: UIGenieDetail) -> Self {
        let position: EffectPosition = (d.position as u8).try_into().unwrap_or_default();
        let anchor: GenieAnchor = (d.anchor as u8).try_into().unwrap_or_default();
        let easing: EasingFunction = EasingFunction::try_from(d.easing as u8).unwrap_or_default();
        GenieFilter::new(
            position,
            Duration::from_millis(d.duration as u64),
            anchor,
            d.funnel_power.clamp(1.0, 4.0),
        )
        .with_shadow(d.shadow.clamp(0.0, 1.0))
        .with_easing(easing)
    }
}

impl From<PageFlipFilter> for UIPageFlipDetail {
    fn from(f: PageFlipFilter) -> Self {
        Self {
            duration: f.duration.as_millis() as i32,
            position: u8::from(f.position) as i32,
            corner: u8::from(f.corner) as i32,
            direction: u8::from(f.direction) as i32,
            axis: u8::from(f.axis) as i32,
            shadow: f.shadow,
            flip_extent: f.flip_extent as f32,
            keep_base: f.keep_base,
        }
    }
}

impl From<UIPageFlipDetail> for PageFlipFilter {
    fn from(d: UIPageFlipDetail) -> Self {
        let position: PageFlipPosition = (d.position as u8).try_into().unwrap_or_default();
        let corner: PageFlipCorner = (d.corner as u8).try_into().unwrap_or_default();
        let direction: PageFlipDirection = (d.direction as u8).try_into().unwrap_or_default();
        let axis: PageFlipAxis = (d.axis as u8).try_into().unwrap_or_default();
        PageFlipFilter::new(Duration::from_millis(d.duration as u64))
            .with_position(position)
            .with_corner(corner)
            .with_direction(direction)
            .with_axis(axis)
            .with_shadow(d.shadow)
            .with_flip_extent(d.flip_extent as f64)
            .with_keep_base(d.keep_base)
    }
}

impl From<VELightingFilter> for UILightingDetail {
    fn from(f: VELightingFilter) -> Self {
        Self {
            color_r: (f.color[0] * 255.0).round() as i32,
            color_g: (f.color[1] * 255.0).round() as i32,
            color_b: (f.color[2] * 255.0).round() as i32,
            brightness: f.brightness,
            angle_deg: f.angle_deg,
            penumbra: f.penumbra,
            decay: f.decay,
            max_distance: f.max_distance,
            direction: u8::from(f.direction) as i32,
            pos_x: f.pos.0,
            pos_y: f.pos.1,
            rope_length: f.rope_length,
            gravity: f.gravity,
            swing: f.swing,
            damping: f.damping,
            ambient: f.ambient,
            scene: u8::from(f.scene) as i32,
        }
    }
}

impl From<UILightingDetail> for VELightingFilter {
    fn from(d: UILightingDetail) -> Self {
        let direction: VELightingDirection = (d.direction as u8).try_into().unwrap_or_default();
        let scene: VELightingScene = (d.scene as u8).try_into().unwrap_or_default();
        VELightingFilter::default()
            .with_color([
                d.color_r as f32 / 255.0,
                d.color_g as f32 / 255.0,
                d.color_b as f32 / 255.0,
            ])
            .with_brightness(d.brightness)
            .with_angle_deg(d.angle_deg)
            .with_penumbra(d.penumbra)
            .with_decay(d.decay)
            .with_max_distance(d.max_distance)
            .with_direction(direction)
            .with_pos((d.pos_x, d.pos_y))
            .with_rope_length(d.rope_length)
            .with_gravity(d.gravity)
            .with_swing(d.swing)
            .with_damping(d.damping)
            .with_ambient(d.ambient)
            .with_scene(scene)
    }
}

impl From<SplitFilter> for UISplitDetail {
    fn from(f: SplitFilter) -> Self {
        Self {
            position: u8::from(f.position) as i32,
            duration: f.duration.as_millis() as i32,
            direction: u8::from(f.direction) as i32,
            split_position: f.split_position,
            shadow: f.shadow,
            shadow_width: f.shadow_width,
            easing: u8::from(f.easing) as i32,
        }
    }
}

impl From<UISplitDetail> for SplitFilter {
    fn from(d: UISplitDetail) -> Self {
        let position: EffectPosition = (d.position as u8).try_into().unwrap_or_default();
        let direction: SplitDirection = (d.direction as u8).try_into().unwrap_or_default();
        let easing: EasingFunction = EasingFunction::try_from(d.easing as u8).unwrap_or_default();
        SplitFilter::new(
            position,
            Duration::from_millis(d.duration as u64),
            direction,
        )
        .with_split_position(d.split_position.clamp(0.0, 1.0))
        .with_shadow(d.shadow.clamp(0.0, 1.0))
        .with_shadow_width(d.shadow_width.clamp(0.0, 100.0))
        .with_easing(easing)
    }
}

impl From<FrameExtractFilter> for UIFrameExtractDetail {
    fn from(f: FrameExtractFilter) -> Self {
        Self {
            frame_interval: f.frame_interval as i32,
        }
    }
}

impl From<UIFrameExtractDetail> for FrameExtractFilter {
    fn from(d: UIFrameExtractDetail) -> Self {
        Self::new(d.frame_interval.max(1) as u32)
    }
}

impl From<Live2dFilter> for UILive2dDetail {
    fn from(f: Live2dFilter) -> Self {
        Self {
            model_path: f.model_dir.into(),
            motion_index: f.motion_index,
            expression_index: f.expression_index,
            model_view_fill: f.model_view_fill,
            background_r: f.background[0] as i32,
            background_g: f.background[1] as i32,
            background_b: f.background[2] as i32,
            background_a: f.background[3] as i32,
        }
    }
}

impl From<UILive2dDetail> for Live2dFilter {
    fn from(d: UILive2dDetail) -> Self {
        Self {
            model_dir: d.model_path.as_str().to_string(),
            motion_index: d.motion_index,
            expression_index: d.expression_index,
            model_view_fill: d.model_view_fill.clamp(0.5, 5.0),
            background: [
                d.background_r as u8,
                d.background_g as u8,
                d.background_b as u8,
                d.background_a as u8,
            ],
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}
