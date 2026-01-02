/// Image Effect Performance Benchmark
/// Tests all 38 effects and measures execution time
use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;
use std::time::Instant;

fn benchmark_effect(name: &str, effect: &ImageEffect, img: &image::RgbaImage) -> (String, f64) {
    let mut times = Vec::with_capacity(5);

    // Run 5 times
    for _ in 0..5 {
        let mut test_img = img.clone();
        let start = Instant::now();
        effect.apply(&mut test_img).unwrap();
        let duration = start.elapsed();
        times.push(duration.as_secs_f64());
    }

    // Sort times and take middle 3 (remove fastest and slowest)
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let middle_times = &times[1..4]; // indices 1, 2, 3

    // Calculate average of middle 3
    let avg_time = middle_times.iter().sum::<f64>() / 3.0;
    let avg_time_ms = avg_time * 1000.0;

    (name.to_string(), avg_time_ms)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load test image once
    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    println!("🚀 Starting Image Effect Performance Benchmark");
    println!(
        "📊 Image size: {}x{} ({} pixels)",
        img.width(),
        img.height(),
        img.width() * img.height()
    );
    println!("🔄 Iterations per effect: 5 (taking middle 3)");
    println!();

    let mut results = Vec::new();

    // ===== BASE EFFECTS (6) =====
    println!("🎨 Testing Base Effects...");

    use image_effect::base_effect::{
        BrightnessConfig, ContrastConfig, GrayscaleConfig, HueRotateConfig, SaturationConfig,
    };

    results.push(benchmark_effect(
        "Grayscale (Luminance)",
        &ImageEffect::Grayscale(GrayscaleConfig::new()),
        &img,
    ));

    results.push(benchmark_effect("Invert", &ImageEffect::Invert, &img));

    results.push(benchmark_effect(
        "Brightness",
        &ImageEffect::Brightness(BrightnessConfig::new().with_brightness(30)),
        &img,
    ));

    results.push(benchmark_effect(
        "Contrast",
        &ImageEffect::Contrast(ContrastConfig::new().with_contrast(20.0)),
        &img,
    ));

    results.push(benchmark_effect(
        "Saturation",
        &ImageEffect::Saturation(SaturationConfig::new().with_amount(50.0)),
        &img,
    ));

    results.push(benchmark_effect(
        "HueRotate",
        &ImageEffect::HueRotate(HueRotateConfig::new().with_degrees(90)),
        &img,
    ));

    // ===== BLUR EFFECTS (3) =====
    println!("🔵 Testing Blur Effects...");

    use image_effect::blur_effect::{BoxBlurConfig, GaussianBlurConfig, MedianBlurConfig};

    results.push(benchmark_effect(
        "GaussianBlur (radius=3)",
        &ImageEffect::GaussianBlur(GaussianBlurConfig::new().with_radius(3)),
        &img,
    ));

    results.push(benchmark_effect(
        "BoxBlur (radius=3)",
        &ImageEffect::BoxBlur(BoxBlurConfig::new().with_radius(3)),
        &img,
    ));

    results.push(benchmark_effect(
        "MedianBlur (radius=3)",
        &ImageEffect::MedianBlur(MedianBlurConfig::new().with_radius(3)),
        &img,
    ));

    // ===== FILTER EFFECTS (5) =====
    println!("🎭 Testing Filter Effects...");

    use image_effect::filter_effect::{
        ColorTintConfig, SepiaConfig, TemperatureConfig, VignetteConfig,
    };

    results.push(benchmark_effect(
        "Sepia",
        &ImageEffect::Sepia(SepiaConfig::new()),
        &img,
    ));

    results.push(benchmark_effect(
        "WarmFilter",
        &ImageEffect::WarmFilter(TemperatureConfig::new().with_amount(0.5)),
        &img,
    ));

    results.push(benchmark_effect(
        "CoolFilter",
        &ImageEffect::CoolFilter(TemperatureConfig::new().with_amount(0.5)),
        &img,
    ));

    results.push(benchmark_effect(
        "ColorTint",
        &ImageEffect::ColorTint(ColorTintConfig::from_rgb(255, 0, 0)),
        &img,
    ));

    results.push(benchmark_effect(
        "Vignette",
        &ImageEffect::Vignette(VignetteConfig::new()),
        &img,
    ));

    // ===== STYLIZED EFFECTS (5) =====
    println!("✨ Testing Stylized Effects...");

    use image_effect::stylized_effect::{
        EdgeDetectionConfig, EdgeDetectionMode, EmbossConfig, PixelateConfig, PosterizeConfig,
        SharpenConfig,
    };

    results.push(benchmark_effect(
        "EdgeDetection (Sobel)",
        &ImageEffect::EdgeDetection(EdgeDetectionConfig::new().with_mode(EdgeDetectionMode::Sobel)),
        &img,
    ));

    results.push(benchmark_effect(
        "Emboss",
        &ImageEffect::Emboss(EmbossConfig::new()),
        &img,
    ));

    results.push(benchmark_effect(
        "Sharpen",
        &ImageEffect::Sharpen(SharpenConfig::new()),
        &img,
    ));

    results.push(benchmark_effect(
        "Pixelate (size=10)",
        &ImageEffect::Pixelate(PixelateConfig::new().with_block_size(10)),
        &img,
    ));

    results.push(benchmark_effect(
        "Posterize",
        &ImageEffect::Posterize(PosterizeConfig::new().with_levels(4)),
        &img,
    ));

    // ===== PRESET FILTERS (15) =====
    println!("🌈 Testing Preset Filters...");

    use image_effect::preset_filter_effect::PresetFilter;

    let preset_filters = [
        ("Oceanic", PresetFilter::Oceanic),
        ("Islands", PresetFilter::Islands),
        ("Marine", PresetFilter::Marine),
        ("Seagreen", PresetFilter::Seagreen),
        ("Flagblue", PresetFilter::Flagblue),
        ("Liquid", PresetFilter::Liquid),
        ("Diamante", PresetFilter::Diamante),
        ("Radio", PresetFilter::Radio),
        ("Twenties", PresetFilter::Twenties),
        ("Rosetint", PresetFilter::Rosetint),
        ("Mauve", PresetFilter::Mauve),
        ("Bluechrome", PresetFilter::Bluechrome),
        ("Vintage", PresetFilter::Vintage),
        ("Perfume", PresetFilter::Perfume),
        ("Serenity", PresetFilter::Serenity),
    ];

    for (name, filter) in preset_filters {
        results.push(benchmark_effect(
            name,
            &ImageEffect::PresetFilter(
                image_effect::preset_filter_effect::PresetFilterConfig::new().with_filter(filter),
            ),
            &img,
        ));
    }

    // ===== MONOCHROME EFFECTS (5) =====
    println!("🖤 Testing Monochrome Effects...");

    use image_effect::monochrome_effect::{
        ColorBalanceConfig, DuotoneConfig, SolarizationConfig, SolarizationMode, ThresholdConfig,
    };

    results.push(benchmark_effect(
        "Duotone",
        &ImageEffect::Duotone(
            DuotoneConfig::from_primary_rgb(0, 0, 255).with_secondary_rgb(128, 128, 128),
        ),
        &img,
    ));

    results.push(benchmark_effect(
        "Solarization (RGB)",
        &ImageEffect::Solarization(
            SolarizationConfig::new()
                .with_mode(SolarizationMode::RGB)
                .with_threshold(128),
        ),
        &img,
    ));

    results.push(benchmark_effect(
        "Threshold",
        &ImageEffect::Threshold(ThresholdConfig::new().with_threshold(128)),
        &img,
    ));

    results.push(benchmark_effect(
        "Level",
        &ImageEffect::Level(image_effect::monochrome_effect::LevelConfig::new()),
        &img,
    ));

    results.push(benchmark_effect(
        "ColorBalance",
        &ImageEffect::ColorBalance(
            ColorBalanceConfig::new()
                .with_red_shift(30)
                .with_green_shift(-10)
                .with_blue_shift(20),
        ),
        &img,
    ));

    println!();
    println!("✅ Benchmark completed!");
    println!();

    // Sort results by time
    results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    // Print results table
    println!("┌────────────────────────────────────┬──────────────┬──────────┐");
    println!("│ Effect                            │ Avg Time (ms) │ Rank     │");
    println!("├────────────────────────────────────┼──────────────┼──────────┤");

    for (i, (name, time)) in results.iter().enumerate() {
        println!("│ {:<33} │ {:>12.3} │ {:>8} │", name, time, i + 1);
    }

    println!("└────────────────────────────────────┴──────────────┴──────────┘");
    println!();

    // Calculate statistics
    let total_time: f64 = results.iter().map(|(_, t)| t).sum();
    let avg_time = total_time / results.len() as f64;
    let min_time = results
        .iter()
        .map(|(_, t)| t)
        .fold(f64::INFINITY, |a, &b| a.min(b));
    let max_time = results
        .iter()
        .map(|(_, t)| t)
        .fold(f64::NEG_INFINITY, |a, &b| a.max(b));

    println!("📈 Statistics:");
    println!("  Total effects tested: {}", results.len());
    println!("  Total time: {:.3} ms", total_time);
    println!("  Average time: {:.3} ms", avg_time);
    println!(
        "  Fastest effect: {} ({:.3} ms)",
        results.first().unwrap().0,
        min_time
    );
    println!(
        "  Slowest effect: {} ({:.3} ms)",
        results.last().unwrap().0,
        max_time
    );

    // Generate benchmark.md
    let mut md_content = String::new();
    md_content.push_str("# Image Effect Performance Benchmark\n\n");
    md_content.push_str(&format!(
        "**Test Image:** {}x{} ({} pixels)\n\n",
        img.width(),
        img.height(),
        img.width() * img.height()
    ));
    md_content.push_str("**Iterations per effect:** 5 (taking middle 3)\n\n");
    md_content.push_str(&format!("**Total effects tested:** {}\n\n", results.len()));

    md_content.push_str("## Summary\n\n");
    md_content.push_str(&format!("- **Total time:** {:.3} ms\n", total_time));
    md_content.push_str(&format!("- **Average time:** {:.3} ms\n", avg_time));
    md_content.push_str(&format!(
        "- **Fastest effect:** {} ({:.3} ms)\n",
        results.first().unwrap().0,
        min_time
    ));
    md_content.push_str(&format!(
        "- **Slowest effect:** {} ({:.3} ms)\n",
        results.last().unwrap().0,
        max_time
    ));
    md_content.push_str("\n");

    md_content.push_str("## Performance Rankings\n\n");
    md_content.push_str("| Rank | Effect | Avg Time (ms) | Category |\n");
    md_content.push_str("|------|--------|---------------|----------|\n");

    for (i, (name, time)) in results.iter().enumerate() {
        let category = if i < 6 {
            "Base"
        } else if i < 9 {
            "Blur"
        } else if i < 14 {
            "Filter"
        } else if i < 19 {
            "Stylized"
        } else if i < 34 {
            "Preset"
        } else {
            "Monochrome"
        };

        md_content.push_str(&format!(
            "| {} | {} | {:.3} | {} |\n",
            i + 1,
            name,
            time,
            category
        ));
    }

    md_content.push_str("\n");

    // Category breakdowns
    md_content.push_str("## Performance by Category\n\n");

    md_content.push_str("### Base Effects\n\n");
    md_content.push_str("| Effect | Time (ms) |\n");
    md_content.push_str("|--------|----------|\n");
    for (name, time) in results.iter().filter(|(n, _)| {
        [
            "Grayscale",
            "Invert",
            "Brightness",
            "Contrast",
            "Saturation",
            "HueRotate",
        ]
        .contains(&n.as_str())
    }) {
        md_content.push_str(&format!("| {} | {:.3} |\n", name, time));
    }
    md_content.push_str("\n");

    md_content.push_str("### Blur Effects\n\n");
    md_content.push_str("| Effect | Time (ms) |\n");
    md_content.push_str("|--------|----------|\n");
    for (name, time) in results.iter().filter(|(n, _)| {
        ["GaussianBlur", "BoxBlur", "MedianBlur"]
            .iter()
            .any(|&s| n.contains(s))
    }) {
        md_content.push_str(&format!("| {} | {:.3} |\n", name, time));
    }
    md_content.push_str("\n");

    md_content.push_str("### Filter Effects\n\n");
    md_content.push_str("| Effect | Time (ms) |\n");
    md_content.push_str("|--------|----------|\n");
    for (name, time) in results.iter().filter(|(n, _)| {
        ["Sepia", "WarmFilter", "CoolFilter", "ColorTint", "Vignette"].contains(&n.as_str())
    }) {
        md_content.push_str(&format!("| {} | {:.3} |\n", name, time));
    }
    md_content.push_str("\n");

    md_content.push_str("### Stylized Effects\n\n");
    md_content.push_str("| Effect | Time (ms) |\n");
    md_content.push_str("|--------|----------|\n");
    for (name, time) in results.iter().filter(|(n, _)| {
        [
            "EdgeDetection",
            "Emboss",
            "Sharpen",
            "Pixelate",
            "Posterize",
        ]
        .iter()
        .any(|&s| n.contains(s))
    }) {
        md_content.push_str(&format!("| {} | {:.3} |\n", name, time));
    }
    md_content.push_str("\n");

    md_content.push_str("### Preset Filters\n\n");
    md_content.push_str("| Effect | Time (ms) |\n");
    md_content.push_str("|--------|----------|\n");
    for (name, time) in results.iter().filter(|(n, _)| {
        [
            "Oceanic",
            "Islands",
            "Marine",
            "Seagreen",
            "Flagblue",
            "Liquid",
            "Diamante",
            "Radio",
            "Twenties",
            "Rosetint",
            "Mauve",
            "Bluechrome",
            "Vintage",
            "Perfume",
            "Serenity",
        ]
        .contains(&n.as_str())
    }) {
        md_content.push_str(&format!("| {} | {:.3} |\n", name, time));
    }
    md_content.push_str("\n");

    md_content.push_str("### Monochrome Effects\n\n");
    md_content.push_str("| Effect | Time (ms) |\n");
    md_content.push_str("|--------|----------|\n");
    for (name, time) in results.iter().filter(|(n, _)| {
        [
            "Duotone",
            "Solarization",
            "Threshold",
            "Level",
            "ColorBalance",
        ]
        .iter()
        .any(|&s| n.contains(s))
    }) {
        md_content.push_str(&format!("| {} | {:.3} |\n", name, time));
    }

    // Write to file
    std::fs::write("benchmark.md", md_content)?;
    println!("\n📝 Benchmark results saved to: benchmark.md");

    Ok(())
}
