use crate::{
    Result,
    filters::{
        audio::{
            AudioSpeedFilter, CompressorFilter, CopyChannelFilter, DenoiseFilter, FadeInFilter,
            FadeOutFilter, GainFilter, LimiterFilter, MuteFilter, NoiseGateFilter, NormalizeFilter,
            VoiceChangerFilter,
        },
        global::{
            DanmakuFilter, GlobalSpeedFilter, ProgressBarFilter, RotationGlobalFilter, TimerFilter,
        },
        subtitle::style::{
            alignment::AlignmentFilter,
            border::{BorderRadiusFilter, OutlineWidthFilter},
            colors::{BackgroundColorFilter, OutlineColorFilter, PrimaryColorFilter},
            font_path::FontPathFilter,
            font_size::FontSizeFilter,
            margin::{MarginHorizontalFilter, MarginVerticalFilter},
            padding::PaddingFilter,
            text_alignment::TextAlignmentFilter,
        },
        traits::{
            AudioFilter, AudioFilterWrapper, GlobalFilter, GlobalFilterWrapper, ImageFilterWrapper,
            SubtitleFilter, SubtitleFilterWrapper, VideoFilter, VideoFilterWrapper,
        },
        video::{
            BackgroundFilter, BorderFilter, BreathingFilter, ChromaKeyFilter, CircleMaskFilter,
            CropFilter, DeviceFrameFilter, DirectionalBlurFilter, DrawCircleFilter,
            DrawRectangleFilter, EdgeDetectFilter, FadeInFilter as VideoFadeInFilter,
            FadeOutFilter as VideoFadeOutFilter, FisheyeFilter, FlipFilter, FlyInFilter,
            FocusFilter, FrameExtractFilter, GaussianBlurFilter, GenieFilter, GrainFilter,
            GrayscaleFilter, GridFilter, HSLAdjustFilter, LightingFilter, LinearMaskFilter,
            LiquidGlassFilter, Live2dFilter, LocalMagnifyFilter, MagnifierFilter, MirrorMaskFilter,
            MosaicFilter, OldFilmFilter, OpacityFilter, PageFlipFilter, RectangleMaskFilter,
            ShadowFilter, SharpenFilter, SketchFilter, SlideFilter, SpeedFilter, SplitFilter,
            TextHighlightFilter, TransformFilter, VignetteFilter, WaveFilter, WipeFilter,
            ZoomFilter,
        },
    },
};
use serde::{Deserialize, Serialize};

macro_rules! filter_to_data_match {
    ($filter:expr, $enum_type:ty, $($filter_type:ty, $variant:ident), *) => {
        match $filter.name() {
            $(
                <$filter_type>::NAME => {
                    if let Some(f) = $filter.as_any().downcast_ref::<$filter_type>() {
                        return <$enum_type>::$variant(f.clone());
                    }
                }
            )*
            _ => {}
        }
    };
}

macro_rules! data_to_filter_match {
    ($prefix:expr, $data:expr, $unknown:path, $($path:path),*) => {
        match $data {
            $($path(filter) => Ok(Box::new(filter.clone())),)*
            $unknown { type_name } => Err(crate::Error::UnknownFilter {
                filter_type: format!("{}:{}", $prefix, type_name),
            }),
        }
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoFilterData {
    pub enabled: bool,

    #[serde(flatten)]
    pub inner: VideoFilterDataInner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum VideoFilterDataInner {
    #[serde(rename = "chroma_key")]
    ChromaKey(ChromaKeyFilter),

    #[serde(rename = "flip")]
    Flip(FlipFilter),

    #[serde(rename = "crop")]
    Crop(CropFilter),

    #[serde(rename = "transform")]
    Transform(TransformFilter),

    #[serde(rename = "video_fade_in")]
    VideoFadeIn(VideoFadeInFilter),

    #[serde(rename = "video_fade_out")]
    VideoFadeOut(VideoFadeOutFilter),

    #[serde(rename = "fly_in")]
    FlyIn(FlyInFilter),

    #[serde(rename = "slide")]
    Slide(SlideFilter),

    #[serde(rename = "wipe")]
    Wipe(WipeFilter),

    #[serde(rename = "zoom")]
    Zoom(ZoomFilter),

    #[serde(rename = "mosaic")]
    Mosaic(MosaicFilter),

    #[serde(rename = "liquid_glass")]
    LiquidGlass(LiquidGlassFilter),

    #[serde(rename = "border")]
    Border(BorderFilter),

    #[serde(rename = "opacity")]
    Opacity(OpacityFilter),

    #[serde(rename = "vignette")]
    Vignette(VignetteFilter),

    #[serde(rename = "draw_circle")]
    DrawCircle(DrawCircleFilter),

    #[serde(rename = "draw_rectangle")]
    DrawRectangle(DrawRectangleFilter),

    #[serde(rename = "background")]
    Background(BackgroundFilter),

    #[serde(rename = "hsl_adjust")]
    HslAdjust(HSLAdjustFilter),

    #[serde(rename = "speed")]
    Speed(SpeedFilter),

    #[serde(rename = "breathing")]
    Breathing(BreathingFilter),

    #[serde(rename = "local_magnify")]
    LocalMagnify(LocalMagnifyFilter),

    #[serde(rename = "magnifier")]
    Magnifier(MagnifierFilter),

    #[serde(rename = "grain")]
    Grain(GrainFilter),

    #[serde(rename = "fisheye")]
    Fisheye(FisheyeFilter),

    #[serde(rename = "edge_detect")]
    EdgeDetect(EdgeDetectFilter),

    #[serde(rename = "sketch")]
    Sketch(SketchFilter),

    #[serde(rename = "gaussian_blur")]
    GaussianBlur(GaussianBlurFilter),

    #[serde(rename = "directional_blur")]
    DirectionalBlur(DirectionalBlurFilter),

    #[serde(rename = "sharpen")]
    Sharpen(SharpenFilter),

    #[serde(rename = "grayscale")]
    Grayscale(GrayscaleFilter),

    #[serde(rename = "grid")]
    Grid(GridFilter),

    #[serde(rename = "linear_mask")]
    LinearMask(LinearMaskFilter),

    #[serde(rename = "circle_mask")]
    CircleMask(CircleMaskFilter),

    #[serde(rename = "rectangle_mask")]
    RectangleMask(RectangleMaskFilter),

    #[serde(rename = "mirror_mask")]
    MirrorMask(MirrorMaskFilter),

    #[serde(rename = "old_film")]
    OldFilm(OldFilmFilter),

    #[serde(rename = "wave")]
    Wave(WaveFilter),

    #[serde(rename = "text_highlight")]
    TextHighlight(TextHighlightFilter),

    #[serde(rename = "shadow")]
    Shadow(ShadowFilter),

    #[serde(rename = "device_frame")]
    DeviceFrame(DeviceFrameFilter),

    #[serde(rename = "focus")]
    Focus(FocusFilter),

    #[serde(rename = "genie")]
    Genie(GenieFilter),

    #[serde(rename = "page_flip")]
    PageFlip(PageFlipFilter),

    #[serde(rename = "lighting")]
    Lighting(LightingFilter),

    #[serde(rename = "split")]
    Split(SplitFilter),

    #[serde(rename = "frame_extract")]
    FrameExtract(FrameExtractFilter),

    #[serde(rename = "live_2d")]
    Live2d(Live2dFilter),

    // 未知滤镜类型（用于向前兼容）
    #[serde(rename = "unknown")]
    Unknown { type_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFilterData {
    pub enabled: bool,

    #[serde(flatten)]
    pub inner: AudioFilterDataInner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum AudioFilterDataInner {
    #[serde(rename = "gain")]
    Gain(GainFilter),

    #[serde(rename = "compressor")]
    Compressor(CompressorFilter),

    #[serde(rename = "denoise")]
    Denoise(DenoiseFilter),

    #[serde(rename = "limiter")]
    Limiter(LimiterFilter),

    #[serde(rename = "noise_gate")]
    NoiseGate(NoiseGateFilter),

    #[serde(rename = "normalize")]
    Normalize(NormalizeFilter),

    #[serde(rename = "fade_in")]
    FadeIn(FadeInFilter),

    #[serde(rename = "fade_out")]
    FadeOut(FadeOutFilter),

    #[serde(rename = "mute")]
    Mute(MuteFilter),

    #[serde(rename = "copy_channel")]
    CopyChannel(CopyChannelFilter),

    #[serde(rename = "voice_changer")]
    VoiceChanger(VoiceChangerFilter),

    #[serde(rename = "audio_speed")]
    AudioSpeed(AudioSpeedFilter),

    // 未知滤镜类型（用于向前兼容）
    #[serde(rename = "unknown")]
    Unknown { type_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleFilterData {
    pub enabled: bool,
    #[serde(flatten)]
    pub inner: SubtitleFilterDataInner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum SubtitleFilterDataInner {
    #[serde(rename = "primary_color")]
    PrimaryColor(PrimaryColorFilter),

    #[serde(rename = "outline_color")]
    OutlineColor(OutlineColorFilter),

    #[serde(rename = "background_color")]
    BackgroundColor(BackgroundColorFilter),

    #[serde(rename = "font_size")]
    FontSize(FontSizeFilter),

    #[serde(rename = "font_path")]
    FontPath(FontPathFilter),

    #[serde(rename = "alignment")]
    Alignment(AlignmentFilter),

    #[serde(rename = "outline_width")]
    OutlineWidth(OutlineWidthFilter),

    #[serde(rename = "border_radius")]
    BorderRadius(BorderRadiusFilter),

    #[serde(rename = "padding")]
    Padding(PaddingFilter),

    #[serde(rename = "margin_vertical")]
    MarginVertical(MarginVerticalFilter),

    #[serde(rename = "margin_horizontal")]
    MarginHorizontal(MarginHorizontalFilter),

    #[serde(rename = "text_alignment")]
    TextAlignment(TextAlignmentFilter),

    /// 未知滤镜类型（用于向前兼容）
    #[serde(rename = "unknown")]
    Unknown { type_name: String },
}

pub mod color_serde {
    use image::Rgba;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct ColorArray([u8; 4]);

    pub fn serialize<S>(color: &Option<Rgba<u8>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match color {
            Some(c) => {
                let arr = ColorArray(c.0);
                Some(arr).serialize(serializer)
            }
            None => None::<ColorArray>.serialize(serializer),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Rgba<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<ColorArray>::deserialize(deserializer)?;
        Ok(opt.map(|c| Rgba(c.0)))
    }

    // 子模块，用于非可选的 Rgba<u8>
    pub mod required {
        use image::Rgba;
        use serde::{Deserialize, Deserializer, Serialize, Serializer};

        pub fn serialize<S>(color: &Rgba<u8>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            use super::ColorArray;
            ColorArray(color.0).serialize(serializer)
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Rgba<u8>, D::Error>
        where
            D: Deserializer<'de>,
        {
            use super::ColorArray;
            let arr = ColorArray::deserialize(deserializer)?;
            Ok(Rgba(arr.0))
        }
    }
}

pub fn video_filter_to_data(filter: &Box<dyn VideoFilter>) -> VideoFilterDataInner {
    filter_to_data_match!(
        filter,
        VideoFilterDataInner,
        ChromaKeyFilter,
        ChromaKey,
        FlipFilter,
        Flip,
        CropFilter,
        Crop,
        TransformFilter,
        Transform,
        VideoFadeInFilter,
        VideoFadeIn,
        VideoFadeOutFilter,
        VideoFadeOut,
        FlyInFilter,
        FlyIn,
        SlideFilter,
        Slide,
        WipeFilter,
        Wipe,
        ZoomFilter,
        Zoom,
        MosaicFilter,
        Mosaic,
        LiquidGlassFilter,
        LiquidGlass,
        BorderFilter,
        Border,
        OpacityFilter,
        Opacity,
        VignetteFilter,
        Vignette,
        DrawCircleFilter,
        DrawCircle,
        DrawRectangleFilter,
        DrawRectangle,
        BackgroundFilter,
        Background,
        HSLAdjustFilter,
        HslAdjust,
        SpeedFilter,
        Speed,
        BreathingFilter,
        Breathing,
        LocalMagnifyFilter,
        LocalMagnify,
        MagnifierFilter,
        Magnifier,
        GrainFilter,
        Grain,
        FisheyeFilter,
        Fisheye,
        EdgeDetectFilter,
        EdgeDetect,
        SketchFilter,
        Sketch,
        GaussianBlurFilter,
        GaussianBlur,
        DirectionalBlurFilter,
        DirectionalBlur,
        SharpenFilter,
        Sharpen,
        GrayscaleFilter,
        Grayscale,
        GridFilter,
        Grid,
        CircleMaskFilter,
        CircleMask,
        LinearMaskFilter,
        LinearMask,
        MirrorMaskFilter,
        MirrorMask,
        RectangleMaskFilter,
        RectangleMask,
        OldFilmFilter,
        OldFilm,
        WaveFilter,
        Wave,
        TextHighlightFilter,
        TextHighlight,
        ShadowFilter,
        Shadow,
        DeviceFrameFilter,
        DeviceFrame,
        FocusFilter,
        Focus,
        GenieFilter,
        Genie,
        PageFlipFilter,
        PageFlip,
        LightingFilter,
        Lighting,
        SplitFilter,
        Split,
        FrameExtractFilter,
        FrameExtract,
        Live2dFilter,
        Live2d
    );

    let name = filter.name();
    log::warn!("Unknown video filter type: '{}', storing as unknown", name);
    VideoFilterDataInner::Unknown {
        type_name: name.to_string(),
    }
}

pub fn audio_filter_to_data(filter: &Box<dyn AudioFilter>) -> AudioFilterDataInner {
    filter_to_data_match!(
        filter,
        AudioFilterDataInner,
        GainFilter,
        Gain,
        CompressorFilter,
        Compressor,
        DenoiseFilter,
        Denoise,
        LimiterFilter,
        Limiter,
        NoiseGateFilter,
        NoiseGate,
        NormalizeFilter,
        Normalize,
        FadeInFilter,
        FadeIn,
        FadeOutFilter,
        FadeOut,
        MuteFilter,
        Mute,
        CopyChannelFilter,
        CopyChannel,
        VoiceChangerFilter,
        VoiceChanger,
        AudioSpeedFilter,
        AudioSpeed
    );

    let name = filter.name();
    log::warn!("Unknown audio filter type: '{}', storing as unknown", name);
    AudioFilterDataInner::Unknown {
        type_name: name.to_string(),
    }
}

pub fn subtitle_filter_to_data(filter: &Box<dyn SubtitleFilter>) -> SubtitleFilterDataInner {
    filter_to_data_match!(
        filter,
        SubtitleFilterDataInner,
        PrimaryColorFilter,
        PrimaryColor,
        OutlineColorFilter,
        OutlineColor,
        BackgroundColorFilter,
        BackgroundColor,
        FontSizeFilter,
        FontSize,
        FontPathFilter,
        FontPath,
        AlignmentFilter,
        Alignment,
        OutlineWidthFilter,
        OutlineWidth,
        BorderRadiusFilter,
        BorderRadius,
        PaddingFilter,
        Padding,
        MarginVerticalFilter,
        MarginVertical,
        MarginHorizontalFilter,
        MarginHorizontal,
        TextAlignmentFilter,
        TextAlignment
    );

    let name = filter.name();
    log::warn!(
        "Unknown subtitle filter type: '{}', storing as unknown",
        name
    );
    SubtitleFilterDataInner::Unknown {
        type_name: name.to_string(),
    }
}

pub fn data_to_video_filter(data: &VideoFilterData) -> Result<Box<dyn VideoFilter>> {
    data_to_video_filter_inner(&data.inner)
}

pub fn data_to_video_filter_inner(data: &VideoFilterDataInner) -> Result<Box<dyn VideoFilter>> {
    data_to_filter_match!(
        "video",
        data,
        VideoFilterDataInner::Unknown,
        VideoFilterDataInner::ChromaKey,
        VideoFilterDataInner::Flip,
        VideoFilterDataInner::Crop,
        VideoFilterDataInner::Transform,
        VideoFilterDataInner::VideoFadeIn,
        VideoFilterDataInner::VideoFadeOut,
        VideoFilterDataInner::FlyIn,
        VideoFilterDataInner::Slide,
        VideoFilterDataInner::Wipe,
        VideoFilterDataInner::Zoom,
        VideoFilterDataInner::Mosaic,
        VideoFilterDataInner::LiquidGlass,
        VideoFilterDataInner::Border,
        VideoFilterDataInner::Opacity,
        VideoFilterDataInner::Vignette,
        VideoFilterDataInner::DrawCircle,
        VideoFilterDataInner::DrawRectangle,
        VideoFilterDataInner::Background,
        VideoFilterDataInner::HslAdjust,
        VideoFilterDataInner::Speed,
        VideoFilterDataInner::Breathing,
        VideoFilterDataInner::LocalMagnify,
        VideoFilterDataInner::Magnifier,
        VideoFilterDataInner::Grain,
        VideoFilterDataInner::Fisheye,
        VideoFilterDataInner::EdgeDetect,
        VideoFilterDataInner::Sketch,
        VideoFilterDataInner::GaussianBlur,
        VideoFilterDataInner::DirectionalBlur,
        VideoFilterDataInner::Sharpen,
        VideoFilterDataInner::Grayscale,
        VideoFilterDataInner::Grid,
        VideoFilterDataInner::CircleMask,
        VideoFilterDataInner::LinearMask,
        VideoFilterDataInner::MirrorMask,
        VideoFilterDataInner::RectangleMask,
        VideoFilterDataInner::OldFilm,
        VideoFilterDataInner::Wave,
        VideoFilterDataInner::TextHighlight,
        VideoFilterDataInner::Shadow,
        VideoFilterDataInner::DeviceFrame,
        VideoFilterDataInner::Focus,
        VideoFilterDataInner::Genie,
        VideoFilterDataInner::PageFlip,
        VideoFilterDataInner::Lighting,
        VideoFilterDataInner::Split,
        VideoFilterDataInner::FrameExtract,
        VideoFilterDataInner::Live2d
    )
}

pub fn data_to_audio_filter(data: &AudioFilterData) -> Result<Box<dyn AudioFilter>> {
    data_to_audio_filter_inner(&data.inner)
}

pub fn data_to_audio_filter_inner(data: &AudioFilterDataInner) -> Result<Box<dyn AudioFilter>> {
    data_to_filter_match!(
        "audio",
        data,
        AudioFilterDataInner::Unknown,
        AudioFilterDataInner::Gain,
        AudioFilterDataInner::Compressor,
        AudioFilterDataInner::Denoise,
        AudioFilterDataInner::Limiter,
        AudioFilterDataInner::NoiseGate,
        AudioFilterDataInner::Normalize,
        AudioFilterDataInner::FadeIn,
        AudioFilterDataInner::FadeOut,
        AudioFilterDataInner::Mute,
        AudioFilterDataInner::CopyChannel,
        AudioFilterDataInner::VoiceChanger,
        AudioFilterDataInner::AudioSpeed
    )
}

pub fn data_to_subtitle_filter(data: &SubtitleFilterData) -> Result<Box<dyn SubtitleFilter>> {
    data_to_subtitle_filter_inner(&data.inner)
}

pub fn data_to_subtitle_filter_inner(
    data: &SubtitleFilterDataInner,
) -> Result<Box<dyn SubtitleFilter>> {
    data_to_filter_match!(
        "subtitle",
        data,
        SubtitleFilterDataInner::Unknown,
        SubtitleFilterDataInner::PrimaryColor,
        SubtitleFilterDataInner::OutlineColor,
        SubtitleFilterDataInner::BackgroundColor,
        SubtitleFilterDataInner::FontSize,
        SubtitleFilterDataInner::FontPath,
        SubtitleFilterDataInner::Alignment,
        SubtitleFilterDataInner::OutlineWidth,
        SubtitleFilterDataInner::BorderRadius,
        SubtitleFilterDataInner::Padding,
        SubtitleFilterDataInner::MarginVertical,
        SubtitleFilterDataInner::MarginHorizontal,
        SubtitleFilterDataInner::TextAlignment
    )
}

pub fn video_filter_wrapper_to_data(wrapper: &VideoFilterWrapper) -> VideoFilterData {
    VideoFilterData {
        enabled: wrapper.enabled(),
        inner: video_filter_to_data(&wrapper.inner),
    }
}

pub fn audio_filter_wrapper_to_data(wrapper: &AudioFilterWrapper) -> AudioFilterData {
    AudioFilterData {
        enabled: wrapper.enabled(),
        inner: audio_filter_to_data(&wrapper.inner),
    }
}

pub fn subtitle_filter_wrapper_to_data(wrapper: &SubtitleFilterWrapper) -> SubtitleFilterData {
    SubtitleFilterData {
        enabled: wrapper.enabled(),
        inner: subtitle_filter_to_data(&wrapper.inner),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageFilterData {
    pub enabled: bool,

    #[serde(flatten)]
    pub inner: VideoFilterDataInner,
}

pub fn image_filter_wrapper_to_data(wrapper: &ImageFilterWrapper) -> ImageFilterData {
    ImageFilterData {
        enabled: wrapper.enabled(),
        inner: video_filter_to_data(&wrapper.inner),
    }
}

pub fn data_to_image_filter(data: &ImageFilterData) -> Result<ImageFilterWrapper> {
    let filter = data_to_video_filter_inner(&data.inner)?;
    Ok(ImageFilterWrapper::new(data.enabled, filter))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalFilterData {
    pub enabled: bool,
    #[serde(flatten)]
    pub inner: GlobalFilterDataInner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum GlobalFilterDataInner {
    #[serde(rename = "progress_bar")]
    ProgressBar(ProgressBarFilter),

    #[serde(rename = "timer")]
    Timer(TimerFilter),

    #[serde(rename = "global_speed")]
    GlobalSpeed(GlobalSpeedFilter),

    #[serde(rename = "rotation")]
    Rotation(RotationGlobalFilter),

    #[serde(rename = "danmaku")]
    Danmaku(DanmakuFilter),

    #[serde(rename = "unknown")]
    Unknown { type_name: String },
}

pub fn global_filter_wrapper_to_data(wrapper: &GlobalFilterWrapper) -> GlobalFilterData {
    GlobalFilterData {
        enabled: wrapper.enabled(),
        inner: global_filter_to_data(&wrapper.inner),
    }
}

pub fn global_filter_to_data(filter: &Box<dyn GlobalFilter>) -> GlobalFilterDataInner {
    filter_to_data_match!(
        filter,
        GlobalFilterDataInner,
        ProgressBarFilter,
        ProgressBar,
        TimerFilter,
        Timer,
        GlobalSpeedFilter,
        GlobalSpeed,
        RotationGlobalFilter,
        Rotation,
        DanmakuFilter,
        Danmaku
    );

    let name = filter.name();
    log::warn!("Unknown global filter type: '{}', storing as unknown", name);
    GlobalFilterDataInner::Unknown {
        type_name: name.to_string(),
    }
}

pub fn data_to_global_filter(data: &GlobalFilterData) -> Result<GlobalFilterWrapper> {
    let filter = data_to_global_filter_inner(&data.inner)?;
    Ok(GlobalFilterWrapper::new(data.enabled, filter))
}

pub fn data_to_global_filter_inner(data: &GlobalFilterDataInner) -> Result<Box<dyn GlobalFilter>> {
    data_to_filter_match!(
        "global",
        data,
        GlobalFilterDataInner::Unknown,
        GlobalFilterDataInner::ProgressBar,
        GlobalFilterDataInner::Timer,
        GlobalFilterDataInner::GlobalSpeed,
        GlobalFilterDataInner::Rotation,
        GlobalFilterDataInner::Danmaku
    )
}
