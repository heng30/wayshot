//! Performance test for TransformFilter
//!
//! Measures execution time for various transformation operations on 1080p frames.
//! Run with: cargo run --example transform_filter_perf_test

use image::{Rgba, RgbaImage};
use std::time::{Duration, Instant};

/// Simulates the old TransformFilter implementation using imageops::resize
fn transform_old(
    buffer: &mut RgbaImage,
    output_width: u32,
    output_height: u32,
    zoom_level: f32,
    center_x_percent: f32,
    center_y_percent: f32,
    rotation: f32,
) {
    use image::imageops;
    use imageproc::geometric_transformations::{Interpolation, rotate_about_center};

    let src_width = buffer.width();
    let src_height = buffer.height();

    // Skip if no transformation needed
    if zoom_level == 1.0
        && rotation == 0.0
        && center_x_percent == 0.5
        && center_y_percent == 0.5
        && src_width == output_width
        && src_height == output_height
    {
        return;
    }

    // Step 1: Scale the image based on zoom_level
    let scaled_width = (src_width as f32 * zoom_level).round() as u32;
    let scaled_height = (src_height as f32 * zoom_level).round() as u32;

    let scaled_width = scaled_width.max(1);
    let scaled_height = scaled_height.max(1);

    let mut scaled_image = if zoom_level != 1.0 {
        imageops::resize(
            buffer,
            scaled_width,
            scaled_height,
            imageops::FilterType::Lanczos3,
        )
    } else {
        buffer.clone()
    };

    // Step 2: Rotate the image around its center
    if rotation != 0.0 {
        let theta = rotation;
        let w = scaled_image.width() as f32;
        let h = scaled_image.height() as f32;

        let cos_theta = theta.cos().abs();
        let sin_theta = theta.sin().abs();
        let expanded_width = (w * cos_theta + h * sin_theta).ceil() as u32;
        let expanded_height = (w * sin_theta + h * cos_theta).ceil() as u32;

        let mut expanded_canvas =
            RgbaImage::from_pixel(expanded_width, expanded_height, Rgba([0, 0, 0, 0]));

        let offset_x = (expanded_width as i64 - scaled_image.width() as i64) / 2;
        let offset_y = (expanded_height as i64 - scaled_image.height() as i64) / 2;
        imageops::overlay(&mut expanded_canvas, &scaled_image, offset_x, offset_y);

        scaled_image = rotate_about_center::<Rgba<u8>>(
            &expanded_canvas,
            theta,
            Interpolation::Bilinear,
            Rgba([0, 0, 0, 0]),
        );
    }

    // Step 3: Create output canvas and position the image
    let mut canvas = RgbaImage::from_pixel(output_width, output_height, Rgba([0, 0, 0, 0]));

    let canvas_center_x = output_width as f32 * center_x_percent;
    let canvas_center_y = output_height as f32 * center_y_percent;

    let image_width = scaled_image.width();
    let image_height = scaled_image.height();
    let x = (canvas_center_x - image_width as f32 / 2.0).round() as i64;
    let y = (canvas_center_y - image_height as f32 / 2.0).round() as i64;

    imageops::overlay(&mut canvas, &scaled_image, x, y);

    *buffer = canvas;
}

/// Optimized TransformFilter using fast_image_resize for scaling
fn transform_optimized(
    buffer: &mut RgbaImage,
    output_width: u32,
    output_height: u32,
    zoom_level: f32,
    center_x_percent: f32,
    center_y_percent: f32,
    rotation: f32,
) {
    use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer, images::Image as FastImage};
    use image::imageops;
    use imageproc::geometric_transformations::{Interpolation, rotate_about_center};

    let src_width = buffer.width();
    let src_height = buffer.height();

    // Skip if no transformation needed
    if zoom_level == 1.0
        && rotation == 0.0
        && center_x_percent == 0.5
        && center_y_percent == 0.5
        && src_width == output_width
        && src_height == output_height
    {
        return;
    }

    // Step 1: Scale using fast_image_resize (SIMD-optimized)
    let scaled_width = (src_width as f32 * zoom_level).round() as u32;
    let scaled_height = (src_height as f32 * zoom_level).round() as u32;

    let scaled_width = scaled_width.max(1);
    let scaled_height = scaled_height.max(1);

    let mut scaled_image = if zoom_level != 1.0 {
        // Convert to fast_image_resize format
        let src_data = buffer.clone().into_raw();
        let src_image = FastImage::from_vec_u8(src_width, src_height, src_data, PixelType::U8x4)
            .expect("Failed to create source image");

        let mut dst_data = vec![0u8; (scaled_width * scaled_height * 4) as usize];
        let mut dst_image = FastImage::from_slice_u8(
            scaled_width,
            scaled_height,
            &mut dst_data,
            PixelType::U8x4,
        )
        .expect("Failed to create destination image");

        let resize_options = ResizeOptions::new()
            .resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));

        Resizer::new()
            .resize(&src_image, &mut dst_image, &resize_options)
            .expect("Failed to resize image");

        RgbaImage::from_raw(scaled_width, scaled_height, dst_data)
            .expect("Failed to create resized image")
    } else {
        buffer.clone()
    };

    // Step 2: Rotate the image around its center (still uses imageproc)
    if rotation != 0.0 {
        let theta = rotation;
        let w = scaled_image.width() as f32;
        let h = scaled_image.height() as f32;

        let cos_theta = theta.cos().abs();
        let sin_theta = theta.sin().abs();
        let expanded_width = (w * cos_theta + h * sin_theta).ceil() as u32;
        let expanded_height = (w * sin_theta + h * cos_theta).ceil() as u32;

        let mut expanded_canvas =
            RgbaImage::from_pixel(expanded_width, expanded_height, Rgba([0, 0, 0, 0]));

        let offset_x = (expanded_width as i64 - scaled_image.width() as i64) / 2;
        let offset_y = (expanded_height as i64 - scaled_image.height() as i64) / 2;
        imageops::overlay(&mut expanded_canvas, &scaled_image, offset_x, offset_y);

        scaled_image = rotate_about_center::<Rgba<u8>>(
            &expanded_canvas,
            theta,
            Interpolation::Bilinear,
            Rgba([0, 0, 0, 0]),
        );
    }

    // Step 3: Create output canvas and position the image
    let mut canvas = RgbaImage::from_pixel(output_width, output_height, Rgba([0, 0, 0, 0]));

    let canvas_center_x = output_width as f32 * center_x_percent;
    let canvas_center_y = output_height as f32 * center_y_percent;

    let image_width = scaled_image.width();
    let image_height = scaled_image.height();
    let x = (canvas_center_x - image_width as f32 / 2.0).round() as i64;
    let y = (canvas_center_y - image_height as f32 / 2.0).round() as i64;

    imageops::overlay(&mut canvas, &scaled_image, x, y);

    *buffer = canvas;
}

/// Creates a synthetic 1920x1080 RGBA test image
fn create_test_image(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::new(width, height);

    // Create a gradient pattern for visual verification
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let r = (x * 255 / width) as u8;
        let g = (y * 255 / height) as u8;
        let b = ((x + y) * 255 / (width + height)) as u8;
        *pixel = Rgba([r, g, b, 255]);
    }

    img
}

/// Timing statistics
struct TimingStats {
    min: Duration,
    avg: Duration,
    max: Duration,
    total: Duration,
    count: usize,
}

impl TimingStats {
    fn from_measurements(measurements: &[Duration]) -> Self {
        if measurements.is_empty() {
            return Self {
                min: Duration::ZERO,
                avg: Duration::ZERO,
                max: Duration::ZERO,
                total: Duration::ZERO,
                count: 0,
            };
        }

        let min = measurements.iter().min().unwrap();
        let max = measurements.iter().max().unwrap();
        let total = measurements.iter().sum();
        let avg = total / measurements.len() as u32;

        Self {
            min: *min,
            avg,
            max: *max,
            total,
            count: measurements.len(),
        }
    }

    fn print(&self, label: &str) {
        println!(
            "  {:20} min: {:3}ms  avg: {:3}ms  max: {:3}ms  ({} runs)",
            label,
            self.min.as_millis(),
            self.avg.as_millis(),
            self.max.as_millis(),
            self.count
        );
    }
}

/// Run performance test for a specific scenario
fn test_scenario(
    name: &str,
    width: u32,
    height: u32,
    zoom: f32,
    center_x: f32,
    center_y: f32,
    rotation: f32,
    iterations: usize,
) -> (TimingStats, TimingStats) {
    println!("Testing: {} (zoom={}, rotation={}°)", name, zoom, rotation.to_degrees());

    let mut old_times = Vec::with_capacity(iterations);
    let mut new_times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        // Test old implementation
        let mut img_old = create_test_image(width, height);
        let start = Instant::now();
        transform_old(&mut img_old, width, height, zoom, center_x, center_y, rotation);
        old_times.push(start.elapsed());

        // Test optimized implementation
        let mut img_new = create_test_image(width, height);
        let start = Instant::now();
        transform_optimized(&mut img_new, width, height, zoom, center_x, center_y, rotation);
        new_times.push(start.elapsed());
    }

    let old_stats = TimingStats::from_measurements(&old_times);
    let new_stats = TimingStats::from_measurements(&new_times);

    old_stats.print("Old (imageops):");
    new_stats.print("New (fast_resize):");

    let improvement = if old_stats.avg.as_nanos() > 0 {
        let old_ns = old_stats.avg.as_nanos() as f64;
        let new_ns = new_stats.avg.as_nanos() as f64;
        (old_ns - new_ns) / old_ns * 100.0
    } else {
        0.0
    };
    println!("  {:20} Improvement: {:.1}%\n", "", improvement);

    (old_stats, new_stats)
}

fn main() {
    println!("=== TransformFilter Performance Test ===\n");
    println!("Image size: 1920x1080 (1080p)\n");

    const WIDTH: u32 = 1920;
    const HEIGHT: u32 = 1080;
    const ITERATIONS: usize = 10;

    // Test scenarios
    let scenarios = [
        // Zoom only (scale down)
        ("Zoom 50%", 0.5, 0.5, 0.5, 0.0),
        // Zoom only (scale up)
        ("Zoom 150%", 1.5, 0.5, 0.5, 0.0),
        // Rotation only
        ("Rotation 30°", 1.0, 0.5, 0.5, std::f32::consts::FRAC_PI_6),
        // Rotation only (larger angle)
        ("Rotation 90°", 1.0, 0.5, 0.5, std::f32::consts::FRAC_PI_2),
        // Combined zoom + rotation
        ("Zoom 50% + Rot 30°", 0.5, 0.5, 0.5, std::f32::consts::FRAC_PI_6),
        // Combined zoom + rotation + center offset
        ("Full Transform", 0.7, 0.3, 0.7, std::f32::consts::FRAC_PI_4),
    ];

    let mut total_old = Duration::ZERO;
    let mut total_new = Duration::ZERO;

    for (name, zoom, cx, cy, rot) in scenarios {
        let (old, new) = test_scenario(name, WIDTH, HEIGHT, zoom, cx, cy, rot, ITERATIONS);
        total_old += old.total;
        total_new += new.total;
    }

    // Summary
    println!("=== Summary ===");
    println!("Total time (old): {:?}", total_old);
    println!("Total time (new): {:?}", total_new);
    let overall_improvement = (total_old.as_nanos() - total_new.as_nanos()) as f64
        / total_old.as_nanos() as f64 * 100.0;
    println!("Overall improvement: {:.1}%", overall_improvement);
}