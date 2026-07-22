use crate::{
    Result,
    filters::traits::{GlobalFilter, GlobalFilterData},
};
use image::{Rgba, RgbaImage, imageops};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RotationGlobalFilter {
    pub rotation: f32,
}

impl RotationGlobalFilter {
    pub const NAME: &'static str = "rotation";

    pub fn new(rotation: f32) -> Self {
        Self { rotation }
    }
}

impl GlobalFilter for RotationGlobalFilter {
    crate::impl_default_global_filter!(RotationGlobalFilter);

    fn apply(&self, data: &mut GlobalFilterData) -> Result<()> {
        if self.rotation == 0.0 {
            return Ok(());
        }

        let w = data.image.width();
        let h = data.image.height();

        // Use exact pixel rotation for discrete angles
        let rotated = if (self.rotation - 90.0).abs() < 0.5 {
            imageops::rotate90(&data.image)
        } else if (self.rotation - 180.0).abs() < 0.5 || (self.rotation + 180.0).abs() < 0.5 {
            imageops::rotate180(&data.image)
        } else if (self.rotation + 90.0).abs() < 0.5 {
            imageops::rotate270(&data.image)
        } else {
            let theta = self.rotation.to_radians();
            imageproc::geometric_transformations::rotate_about_center::<Rgba<u8>>(
                &data.image,
                theta,
                imageproc::geometric_transformations::Interpolation::Bilinear,
                imageproc::geometric_transformations::Border::Constant(Rgba([0, 0, 0, 0])),
            )
        };

        let rw = rotated.width() as f32;
        let rh = rotated.height() as f32;
        let wf = w as f32;
        let hf = h as f32;

        // Scale rotated image to fit within original dimensions (contain mode)
        let scale = (wf / rw).min(hf / rh);
        let new_w = (rw * scale).round() as u32;
        let new_h = (rh * scale).round() as u32;
        let scaled = if new_w != rotated.width() || new_h != rotated.height() {
            imageops::resize(&rotated, new_w, new_h, imageops::FilterType::Lanczos3)
        } else {
            rotated
        };

        // Center the scaled result on original-sized canvas
        let mut result = RgbaImage::from_pixel(w, h, Rgba([0, 0, 0, 0]));
        let x = (w as i64 - new_w as i64) / 2;
        let y = (h as i64 - new_h as i64) / 2;
        imageops::overlay(&mut result, &scaled, x, y);

        data.image = result;
        Ok(())
    }

    fn apply_post_composite(&self) -> bool {
        true
    }
}
