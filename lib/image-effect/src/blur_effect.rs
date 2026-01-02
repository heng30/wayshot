use crate::Effect;
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;
use photon_rs::conv;

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct GaussianBlurConfig {
    #[derivative(Default(value = "3"))]
    radius: i32,
}

impl GaussianBlurConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for GaussianBlurConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        // Use photon-rs gaussian blur
        let mut photon_img = photon_rs::PhotonImage::new(
            image.to_vec(),
            image.width(),
            image.height(),
        );
        conv::gaussian_blur(&mut photon_img, self.radius);
        RgbaImage::from_raw(
            image.width(),
            image.height(),
            photon_img.get_raw_pixels(),
        )
        
    }
}

/// Box blur configuration using photon-rs
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct BoxBlurConfig {
    #[derivative(Default(value = "3"))]
    radius: u32,
}

impl BoxBlurConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for BoxBlurConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        // Use photon-rs box blur
        let mut photon_img = photon_rs::PhotonImage::new(
            image.to_vec(),
            image.width(),
            image.height(),
        );
        conv::box_blur(&mut photon_img);
        RgbaImage::from_raw(
            image.width(),
            image.height(),
            photon_img.get_raw_pixels(),
        )
        
    }
}

/// Median blur configuration (using imageproc)
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct MedianBlurConfig {
    #[derivative(Default(value = "3"))]
    radius: u32,
}

impl MedianBlurConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for MedianBlurConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        // Use imageproc for median filter since it's more efficient
        let gray_image = image::imageops::grayscale(&image);

        // Apply median filter using imageproc
        let filtered = imageproc::filter::median_filter(&gray_image, self.radius, self.radius);

        // Convert back to RGBA
        let mut result = image.clone();
        for (pixel, gray_pixel) in result.pixels_mut().zip(filtered.pixels()) {
            let gray = gray_pixel.0[0];
            *pixel = image::Rgba([gray, gray, gray, pixel[3]]);
        }

        Some(result)
    }
}
