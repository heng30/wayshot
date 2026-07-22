use super::data;
use crate::{
    Result,
    filters::traits::{VideoData, VideoFilter},
    tracks::video_frame_cache::VideoImage,
};
use fast_image_resize::{
    FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer,
    images::{Image, ImageRef},
};
use image::RgbaImage;
use rayon::prelude::*;
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex},
};

// ── Embedded device frame PNG images ──
// Phones
const IPHONE_X_BLACK: &[u8] = include_bytes!("frames/iphone_x_black.png");
const IPHONE_7_JET_BLACK: &[u8] = include_bytes!("frames/iphone_7_jet_black.png");
const PIXEL_VERY_SILVER: &[u8] = include_bytes!("frames/pixel_very_silver.png");
const GALAXY_S5_BLACK: &[u8] = include_bytes!("frames/galaxy_s5_black.png");
const NEXUS_5X: &[u8] = include_bytes!("frames/nexus_5x.png");
// Tablets
const IPAD_AIR_2_SILVER: &[u8] = include_bytes!("frames/ipad_air_2_silver.png");
const IPAD_PRO_SILVER: &[u8] = include_bytes!("frames/ipad_pro_silver.png");
const IPAD_MINI_4_SILVER: &[u8] = include_bytes!("frames/ipad_mini_4_silver.png");
const SURFACE_PRO_4: &[u8] = include_bytes!("frames/surface_pro_4.png");
const NEXUS_9: &[u8] = include_bytes!("frames/nexus_9.png");
// Computers
const MACBOOK_SPACE_GREY: &[u8] = include_bytes!("frames/macbook_space_grey.png");
const DELL_XPS_13: &[u8] = include_bytes!("frames/dell_xps_13.png");
const SURFACE_BOOK: &[u8] = include_bytes!("frames/surface_book.png");
const MACBOOK_AIR_13: &[u8] = include_bytes!("frames/macbook_air_13.png");
const IMAC: &[u8] = include_bytes!("frames/imac.png");
// Displays
const THUNDERBOLT_DISPLAY: &[u8] = include_bytes!("frames/thunderbolt_display.png");
const DELL_ULTRASHARP_27: &[u8] = include_bytes!("frames/dell_ultrasharp_27.png");
const SONY_W850C: &[u8] = include_bytes!("frames/sony_w850c.png");
const DELL_ULTRASHARP_24: &[u8] = include_bytes!("frames/dell_ultrasharp_24.png");

// Cache decoded RGBA images so each device frame is decoded only once.
static FRAME_CACHE: LazyLock<Mutex<HashMap<String, Arc<RgbaImage>>>> =
    LazyLock::new(|| Mutex::new(HashMap::with_capacity(19)));

fn raw_bytes(name: &str) -> Option<&'static [u8]> {
    Some(match name {
        // Phones
        "Apple iPhone X Black" => IPHONE_X_BLACK,
        "Apple iPhone 7 Jet Black" => IPHONE_7_JET_BLACK,
        "Google Pixel Very Silver" => PIXEL_VERY_SILVER,
        "Samsung Galaxy S5 Black" => GALAXY_S5_BLACK,
        "Nexus 5x" => NEXUS_5X,
        // Tablets
        "Apple iPad Air 2 Silver" => IPAD_AIR_2_SILVER,
        "Apple iPad Pro Silver" => IPAD_PRO_SILVER,
        "Apple iPad Mini 4 Silver" => IPAD_MINI_4_SILVER,
        "Microsoft Surface Pro 4" => SURFACE_PRO_4,
        "Nexus 9" => NEXUS_9,
        // Computers
        "Apple-Macbook-Space-Grey" => MACBOOK_SPACE_GREY,
        "Dell XPS 13\"" => DELL_XPS_13,
        "Microsoft Surface Book" => SURFACE_BOOK,
        "Apple Macbook Air 13\"" => MACBOOK_AIR_13,
        "Apple iMac" => IMAC,
        // Displays
        "Apple Thunderbolt Display" => THUNDERBOLT_DISPLAY,
        "Dell UltraSharp 27\"" => DELL_ULTRASHARP_27,
        "Sony W850C" => SONY_W850C,
        "Dell UltraSharp 24\"" => DELL_ULTRASHARP_24,
        _ => return None,
    })
}

/// Load a device frame image by device name.
/// The decoded image is cached globally so repeated calls reuse the buffer.
#[allow(dead_code)]
pub fn load_frame_image(name: &str) -> Option<RgbaImage> {
    let mut cache = FRAME_CACHE.lock().unwrap();
    if let Some(cached) = cache.get(name) {
        return Some(RgbaImage::clone(cached));
    }
    let bytes = raw_bytes(name)?;
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    cache.insert(name.to_owned(), Arc::new(img.clone()));
    Some(img)
}

fn cached_frame(name: &str) -> Option<Arc<RgbaImage>> {
    let mut cache = FRAME_CACHE.lock().unwrap();
    if let Some(cached) = cache.get(name) {
        return Some(Arc::clone(cached));
    }
    let bytes = raw_bytes(name)?;
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let arc = Arc::new(img);
    cache.insert(name.to_owned(), Arc::clone(&arc));
    Some(arc)
}

/// SIMD-accelerated resize using `fast_image_resize`.
/// Uses `ImageRef` for the source to avoid cloning the pixel data.
fn fast_resize(img: &RgbaImage, target_w: u32, target_h: u32) -> RgbaImage {
    let (w, h) = (img.width(), img.height());
    if w == target_w && h == target_h {
        return img.clone();
    }

    let src =
        ImageRef::new(w, h, img.as_raw(), PixelType::U8x4).expect("valid rgba source for resize");
    let mut dst = Image::new(target_w, target_h, PixelType::U8x4);

    Resizer::new()
        .resize(
            &src,
            &mut dst,
            &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3)),
        )
        .expect("resize failed");

    RgbaImage::from_raw(target_w, target_h, dst.into_vec()).expect("valid rgba buffer after resize")
}

/// Filter that draws a device frame around the video content.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
pub struct DeviceFrameFilter {
    #[derivative(Default(value = "\"Apple iPhone X Black\".to_string()"))]
    pub device_name: String,
    #[derivative(Default(value = "[0, 0, 0, 255]"))]
    pub screen_background_color: [u8; 4],
}

impl DeviceFrameFilter {
    pub const NAME: &'static str = "device frame";

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_device_name(mut self, name: impl Into<String>) -> Self {
        self.device_name = name.into();
        self
    }

    /// Apply device frame compositing directly at the target output size.
    ///
    /// Instead of compositing at the native frame resolution and then resizing
    /// the result down, we calculate the combined scale factor upfront and
    /// composite directly into an output-sized canvas. This eliminates the
    /// large intermediate buffer and the extra resize step.
    fn apply_image_frame(
        buffer: &mut RgbaImage,
        frame_img: &RgbaImage,
        screen: &data::ScreenRect,
        target_w: u32,
        target_h: u32,
        bg_color: [u8; 4],
    ) {
        let src_w = buffer.width() as f64;
        let src_h = buffer.height() as f64;
        let screen_w = screen.width as f64;
        let screen_h = screen.height as f64;
        let frame_w = frame_img.width() as f64;
        let frame_h = frame_img.height() as f64;

        // How does source fit into the device screen area?
        let fit_scale = (screen_w / src_w).min(screen_h / src_h);

        // Calculate composited dimensions and per-component scales
        let (comp_w, comp_h, frame_scale, content_scale) = if fit_scale >= 1.0 {
            let fs = 1.0 / fit_scale;
            (frame_w * fs, frame_h * fs, fs, 1.0)
        } else {
            (frame_w, frame_h, 1.0, fit_scale)
        };

        // Scale composited image to fit within output dimensions
        let out_scale = (target_w as f64 / comp_w).min(target_h as f64 / comp_h);

        // Combined scale factors applied to each image
        let total_frame_scale = frame_scale * out_scale;
        let total_content_scale = content_scale * out_scale;

        // Final pixel dimensions (clamped ≥1, frame clamped ≤ target)
        let final_frame_w = ((frame_w * total_frame_scale).round() as u32)
            .max(1)
            .min(target_w);
        let final_frame_h = ((frame_h * total_frame_scale).round() as u32)
            .max(1)
            .min(target_h);
        let final_content_w = ((src_w * total_content_scale).round() as u32).max(1);
        let final_content_h = ((src_h * total_content_scale).round() as u32).max(1);

        // Scale images to final sizes using SIMD-accelerated resize
        let scaled_frame = fast_resize(frame_img, final_frame_w, final_frame_h);
        let scaled_content = fast_resize(buffer, final_content_w, final_content_h);

        // Screen rect in final frame coordinates
        let final_sl = (screen.left as f64 * total_frame_scale).round() as u32;
        let final_st = (screen.top as f64 * total_frame_scale).round() as u32;
        let final_sw = (screen.width as f64 * total_frame_scale).round() as u32;
        let final_sh = (screen.height as f64 * total_frame_scale).round() as u32;

        // Create transparent output canvas
        let mut canvas = RgbaImage::new(target_w, target_h);

        // Center the frame in the output canvas
        let frame_ox = (target_w - final_frame_w) / 2;
        let frame_oy = (target_h - final_frame_h) / 2;

        // Fill screen rect area with background color (for letterbox/pillarbox gaps)
        let screen_ox = frame_ox + final_sl;
        let screen_oy = frame_oy + final_st;
        for y in screen_oy..(screen_oy + final_sh).min(target_h) {
            for x in screen_ox..(screen_ox + final_sw).min(target_w) {
                canvas.get_pixel_mut(x, y).0 = bg_color;
            }
        }

        // Content position: frame offset + screen offset + centering in screen
        let content_ox = frame_ox + final_sl + final_sw.saturating_sub(final_content_w) / 2;
        let content_oy = frame_oy + final_st + final_sh.saturating_sub(final_content_h) / 2;

        Self::composite(
            &mut canvas,
            &scaled_frame,
            &scaled_content,
            frame_ox,
            frame_oy,
            content_ox,
            content_oy,
        );

        *buffer = canvas;
    }

    /// Two-step compositing: fast row copy for content, parallel blend for frame.
    #[inline]
    fn composite(
        canvas: &mut RgbaImage,
        frame: &RgbaImage,
        content: &RgbaImage,
        frame_ox: u32,
        frame_oy: u32,
        content_ox: u32,
        content_oy: u32,
    ) {
        let (canvas_w, canvas_h) = (canvas.width(), canvas.height());
        let (content_w, content_h) = (content.width(), content.height());
        let (frame_w, frame_h) = (frame.width(), frame.height());

        // Step 1: Fast row copy for content placement (sequential, cache-friendly)
        {
            let (w, h) = (canvas_w, canvas_h);
            let mut raw = std::mem::take(canvas).into_raw();
            let canvas_stride = w as usize * 4;
            let content_raw = content.as_raw();
            let content_stride = content_w as usize * 4;

            let copy_rows = content_h.min(canvas_h.saturating_sub(content_oy)) as usize;
            let copy_cols = (content_w as usize * 4)
                .min((canvas_w as usize).saturating_sub(content_ox as usize) * 4);

            for y in 0..copy_rows {
                let src_off = y * content_stride;
                let dst_off = (content_oy as usize + y) * canvas_stride + content_ox as usize * 4;
                raw[dst_off..dst_off + copy_cols]
                    .copy_from_slice(&content_raw[src_off..src_off + copy_cols]);
            }

            // Step 2: Parallel frame blending (only frame region rows)
            let frame_raw = frame.as_raw();
            let frame_stride = frame_w as usize * 4;
            let frame_oy_usize = frame_oy as usize;
            let frame_ox_usize = frame_ox as usize;
            let frame_h_usize = frame_h as usize;
            let frame_w_usize = frame_w as usize;

            // Calculate the frame region in the canvas buffer
            let region_start = frame_oy_usize * canvas_stride;
            let region_end = (frame_oy_usize + frame_h_usize).min(h as usize) * canvas_stride;

            if region_start < region_end && region_end <= raw.len() {
                let region = &mut raw[region_start..region_end];

                region
                    .par_chunks_exact_mut(canvas_stride)
                    .enumerate()
                    .for_each(|(fy, row)| {
                        let frame_row_off = fy * frame_stride;
                        if frame_row_off + frame_stride > frame_raw.len() {
                            return;
                        }
                        let frame_row = &frame_raw[frame_row_off..frame_row_off + frame_stride];
                        let row_offset = frame_ox_usize * 4;

                        for fx in 0..frame_w_usize {
                            let si = fx * 4;
                            let di = row_offset + si;
                            if di + 4 > row.len() {
                                break;
                            }

                            let a = frame_row[si + 3];
                            if a == 0 {
                                continue;
                            }
                            if a == 255 {
                                row[di..di + 4].copy_from_slice(&frame_row[si..si + 4]);
                                continue;
                            }

                            // Alpha blend (inline for hot path)
                            let sa = a as f32 / 255.0;
                            let da = row[di + 3] as f32 / 255.0;
                            let inv_sa = 1.0 - sa;
                            let oa = sa + da * inv_sa;
                            if oa > 0.0 {
                                row[di] = ((frame_row[si] as f32 * sa
                                    + row[di] as f32 * da * inv_sa)
                                    / oa) as u8;
                                row[di + 1] = ((frame_row[si + 1] as f32 * sa
                                    + row[di + 1] as f32 * da * inv_sa)
                                    / oa) as u8;
                                row[di + 2] = ((frame_row[si + 2] as f32 * sa
                                    + row[di + 2] as f32 * da * inv_sa)
                                    / oa) as u8;
                                row[di + 3] = (oa * 255.0) as u8;
                            }
                        }
                    });
            }

            // Put the modified buffer back
            *canvas = RgbaImage::from_raw(w, h, raw).unwrap();
        }
    }
}

impl VideoFilter for DeviceFrameFilter {
    crate::impl_default_video_filter!(DeviceFrameFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let frame_def = data::find_frame(&self.device_name);
        let frame_img = frame_def.and_then(|_| cached_frame(&self.device_name));

        let (Some(def), Some(frame_img)) = (frame_def, frame_img) else {
            return Ok(());
        };

        let target_w = data.config.output_width;
        let target_h = data.config.output_height;

        for frame in &mut data.frames {
            if let VideoImage::Image { buffer, .. } = frame {
                Self::apply_image_frame(
                    buffer,
                    &frame_img,
                    &def.frame,
                    target_w,
                    target_h,
                    self.screen_background_color,
                );
            }
        }
        Ok(())
    }
}
