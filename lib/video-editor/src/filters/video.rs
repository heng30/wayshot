pub mod background;
pub mod border;
pub mod breathing;
pub mod chroma;
pub mod circle_mask;
pub mod crop;
pub mod device_frame;
pub mod directional_blur;
pub mod draw_circle;
pub mod draw_rectangle;
pub mod edge_detect;
pub mod fade_in;
pub mod fade_out;
pub mod fisheye;
pub mod flip;
pub mod fly_in;
pub mod focus;
pub mod frame_extract;
pub mod gaussian_blur;
pub mod genie;
pub mod grain;
pub mod grayscale;
pub mod grid;
pub mod hsl_adjust;
pub mod lighting;
pub mod linear_mask;
pub mod liquid_glass;
pub mod live2d;
pub mod local_magnify;
pub mod magnifier;
pub mod mirror_mask;
pub mod mosaic;
pub mod old_film;
pub mod opacity;
pub mod page_flip;
pub mod rectangle_mask;
pub mod shadow;
pub mod sharpen;
pub mod sketch;
pub mod slide;
pub mod speed;
pub mod split;
pub mod text_highlight;
pub mod transform;
pub mod vignette;
pub mod wave;
pub mod wind_scatter;
pub mod wipe;
pub mod zoom;

pub use background::BackgroundFilter;
pub use border::BorderFilter;
pub use breathing::{BreathingCurve, BreathingFilter};
pub use chroma::ChromaKeyFilter;
pub use circle_mask::CircleMaskFilter;
pub use crop::{CropFilter, CropShape};
pub use device_frame::DeviceFrameFilter;
pub use directional_blur::DirectionalBlurFilter;
pub use draw_circle::DrawCircleFilter;
pub use draw_rectangle::DrawRectangleFilter;
pub use edge_detect::EdgeDetectFilter;
pub use fade_in::FadeInFilter;
pub use fade_out::FadeOutFilter;
pub use fisheye::FisheyeFilter;
pub use flip::{FlipDirection, FlipFilter};
pub use fly_in::{FlyInDirection, FlyInFilter};
pub use focus::FocusFilter;
pub use frame_extract::FrameExtractFilter;
pub use gaussian_blur::GaussianBlurFilter;
pub use genie::{GenieAnchor, GenieFilter};
pub use grain::GrainFilter;
pub use grayscale::GrayscaleFilter;
pub use grid::GridFilter;
pub use hsl_adjust::{HSLAdjustFilter, LuminanceStandard};
pub use lighting::{LightingDirection, LightingFilter, LightingScene};
pub use linear_mask::LinearMaskFilter;
pub use liquid_glass::LiquidGlassFilter;
pub use live2d::{Live2dFilter, model_expression_names, model_motion_names, resolve_model_dir};
pub use local_magnify::LocalMagnifyFilter;
pub use magnifier::MagnifierFilter;
pub use mirror_mask::MirrorMaskFilter;
pub use mosaic::MosaicFilter;
pub use old_film::OldFilmFilter;
pub use opacity::OpacityFilter;
pub use page_flip::{
    PageFlipAxis, PageFlipCorner, PageFlipDirection, PageFlipFilter, PageFlipPosition,
};
pub use rectangle_mask::RectangleMaskFilter;
pub use shadow::ShadowFilter;
pub use sharpen::SharpenFilter;
pub use sketch::SketchFilter;
pub use slide::{SlideDirection, SlideFilter};
pub use speed::SpeedFilter;
pub use split::{SplitDirection, SplitFilter};
pub use text_highlight::{HighlightRegion, TextHighlightFilter};
pub use transform::TransformFilter;
pub use vignette::VignetteFilter;
pub use wave::{WaveFilter, WaveType};
pub use wind_scatter::WindScatterFilter;
pub use wipe::{WipeDirection, WipeFilter};
pub use zoom::ZoomFilter;

pub fn all_filter_names() -> &'static [&'static str] {
    &[
        TransformFilter::NAME,
        BreathingFilter::NAME,
        ZoomFilter::NAME,
        MagnifierFilter::NAME,
        LocalMagnifyFilter::NAME,
        LinearMaskFilter::NAME,
        CircleMaskFilter::NAME,
        RectangleMaskFilter::NAME,
        MirrorMaskFilter::NAME,
        FlyInFilter::NAME,
        WipeFilter::NAME,
        SlideFilter::NAME,
        GenieFilter::NAME,
        PageFlipFilter::NAME,
        LightingFilter::NAME,
        SplitFilter::NAME,
        CropFilter::NAME,
        FlipFilter::NAME,
        FadeInFilter::NAME,
        FadeOutFilter::NAME,
        MosaicFilter::NAME,
        LiquidGlassFilter::NAME,
        OpacityFilter::NAME,
        BorderFilter::NAME,
        ShadowFilter::NAME,
        BackgroundFilter::NAME,
        HSLAdjustFilter::NAME,
        VignetteFilter::NAME,
        ChromaKeyFilter::NAME,
        DrawCircleFilter::NAME,
        DrawRectangleFilter::NAME,
        TextHighlightFilter::NAME,
        SpeedFilter::NAME,
        FrameExtractFilter::NAME,
        GrainFilter::NAME,
        FisheyeFilter::NAME,
        FocusFilter::NAME,
        OldFilmFilter::NAME,
        GaussianBlurFilter::NAME,
        DirectionalBlurFilter::NAME,
        SharpenFilter::NAME,
        WaveFilter::NAME,
        WindScatterFilter::NAME,
        EdgeDetectFilter::NAME,
        SketchFilter::NAME,
        GrayscaleFilter::NAME,
        GridFilter::NAME,
        DeviceFrameFilter::NAME,
        Live2dFilter::NAME,
    ]
}

pub fn rgba_to_photon(
    img: &image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
) -> crate::Result<photon_rs::PhotonImage> {
    Ok(photon_rs::PhotonImage::new(
        img.as_raw().to_vec(),
        img.width(),
        img.height(),
    ))
}

pub fn photon_to_rgba(
    img: photon_rs::PhotonImage,
) -> crate::Result<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>> {
    let width = img.get_width();
    let height = img.get_height();
    let bytes = img.get_raw_pixels();
    let expected_len = width as usize * height as usize * 4;

    if width == 0 || height == 0 {
        return Err(crate::Error::InvalidConfig(format!(
            "Photon image has zero dimensions: {}x{}",
            width, height
        )));
    }

    if bytes.len() != expected_len {
        return Err(crate::Error::InvalidConfig(format!(
            "Photon image buffer size mismatch: expected {} ({}x{}x4), got {}",
            expected_len,
            width,
            height,
            bytes.len()
        )));
    }

    image::ImageBuffer::from_raw(width, height, bytes.to_vec())
        .ok_or_else(|| crate::Error::InvalidConfig("Photon image conversion failed".into()))
}

pub fn apply_photon_effect<F>(
    buffer: &mut image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    effect: F,
) -> crate::Result<()>
where
    F: FnOnce(&mut photon_rs::PhotonImage),
{
    let mut photon_img = rgba_to_photon(buffer)?;
    effect(&mut photon_img);
    *buffer = photon_to_rgba(photon_img)?;
    Ok(())
}
