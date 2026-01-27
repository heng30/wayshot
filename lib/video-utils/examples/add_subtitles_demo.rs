use std::path::PathBuf;
use video_utils::subtitle::{Subtitle, save_as_srt};
use video_utils::subtitle_burn::{add_subtitles, SubtitleBurnConfig, SubtitleStyle, rgb_to_ass_color};

fn main() {
    env_logger::init();

    // Get the data and tmp directory paths
    let data_dir = PathBuf::from("data");
    let tmp_dir = PathBuf::from("tmp");
    let test_video = data_dir.join("test.mp4");

    // Create test subtitles
    let subtitles = vec![
        Subtitle {
            index: 1,
            start_timestamp: 0,
            end_timestamp: 1000,
            text: "Welcome to Wayshot!".to_string(),
        },
        Subtitle {
            index: 2,
            start_timestamp: 1000,
            end_timestamp: 2000,
            text: "This is a subtitle demo".to_string(),
        },
        Subtitle {
            index: 3,
            start_timestamp: 2000,
            end_timestamp: 3000,
            text: "Supporting Chinese: 欢迎使用 Wayshot!".to_string(),
        },
        Subtitle {
            index: 4,
            start_timestamp: 3000,
            end_timestamp: 4000,
            text: "日本語もサポートしています".to_string(),
        },
        Subtitle {
            index: 5,
            start_timestamp: 4000,
            end_timestamp: 5000,
            text: "한국어 지원도 가능합니다".to_string(),
        },
    ];

    let subtitle_path = tmp_dir.join("test_subtitles.srt");
    save_as_srt(&subtitles, &subtitle_path).expect("Failed to save subtitles");
    println!("Created subtitle file: {:?}", subtitle_path);

    // Test different subtitle styles
    test_style1(&test_video, &subtitle_path, &tmp_dir);
    test_style2(&test_video, &subtitle_path, &tmp_dir);
    test_style3(&test_video, &subtitle_path, &tmp_dir);
    test_style4(&test_video, &subtitle_path, &tmp_dir);
    test_style5_rounded(&test_video, &subtitle_path, &tmp_dir);
}

/// Style 1: Classic white text with black outline (bottom center)
fn test_style1(video: &PathBuf, subtitle: &PathBuf, output_dir: &PathBuf) {
    println!("\n=== Test Style 1: Classic White with Black Outline ===");

    let style = SubtitleStyle::new()
        .with_font_size(28)
        .with_primary_color(Some("&H00FFFFFF".to_string())) // White
        .with_outline_color(Some("&H00000000".to_string())) // Black
        .with_outline_width(Some(2))
        .with_alignment(Some(2)) // Bottom-center
        .with_margin_vertical(Some(30));

    let output = output_dir.join("output_style1_classic.mp4");

    let config = SubtitleBurnConfig::new()
        .with_input(video.to_str().unwrap().to_string())
        .with_subtitle(subtitle.to_str().unwrap().to_string())
        .with_output(output.to_str().unwrap().to_string())
        .with_style(style);

    match add_subtitles(&config) {
        Ok(_) => println!("✓ Style 1 output saved to: {:?}", output),
        Err(e) => eprintln!("✗ Style 1 failed: {}", e),
    }
}

/// Style 2: YouTube style with semi-transparent background
fn test_style2(video: &PathBuf, subtitle: &PathBuf, output_dir: &PathBuf) {
    println!("\n=== Test Style 2: YouTube with Background ===");

    let style = SubtitleStyle::new()
        .with_font_size(24)
        .with_primary_color(Some("&H00FFFFFF".to_string())) // White
        .with_outline_color(Some("&H00000000".to_string())) // Black
        .with_background_color(Some("&H80000000".to_string())) // Semi-transparent black
        .with_border_style(Some(3)) // Opaque box
        .with_alignment(Some(2)) // Bottom-center
        .with_margin_vertical(Some(20))
        .with_padding(Some(8));

    let output = output_dir.join("output_style2_youtube.mp4");

    let config = SubtitleBurnConfig::new()
        .with_input(video.to_str().unwrap().to_string())
        .with_subtitle(subtitle.to_str().unwrap().to_string())
        .with_output(output.to_str().unwrap().to_string())
        .with_style(style);

    match add_subtitles(&config) {
        Ok(_) => println!("✓ Style 2 output saved to: {:?}", output),
        Err(e) => eprintln!("✗ Style 2 failed: {}", e),
    }
}

/// Style 3: Anime style at top with colored text
fn test_style3(video: &PathBuf, subtitle: &PathBuf, output_dir: &PathBuf) {
    println!("\n=== Test Style 3: Anime Style Top ===");

    let style = SubtitleStyle::new()
        .with_font_size(26)
        .with_font_name(Some("Arial".to_string()))
        .with_primary_color(Some("&H00FFFF00".to_string())) // Yellow
        .with_outline_color(Some("&H00000000".to_string())) // Black
        .with_bold(Some(-1))
        .with_alignment(Some(8)) // Top-center
        .with_margin_vertical(Some(20));

    let output = output_dir.join("output_style3_anime.mp4");

    let config = SubtitleBurnConfig::new()
        .with_input(video.to_str().unwrap().to_string())
        .with_subtitle(subtitle.to_str().unwrap().to_string())
        .with_output(output.to_str().unwrap().to_string())
        .with_style(style);

    match add_subtitles(&config) {
        Ok(_) => println!("✓ Style 3 output saved to: {:?}", output),
        Err(e) => eprintln!("✗ Style 3 failed: {}", e),
    }
}

/// Style 4: Netflix style with custom positioning
fn test_style4(video: &PathBuf, subtitle: &PathBuf, output_dir: &PathBuf) {
    println!("\n=== Test Style 4: Netflix Style ===");

    // Custom color: Netflix yellow with red tint
    let primary_color = rgb_to_ass_color(255, 220, 100, 0);

    let style = SubtitleStyle::new()
        .with_font_size(30)
        .with_font_name(Some("Arial".to_string()))
        .with_primary_color(Some(primary_color))
        .with_outline_color(Some("&H00000000".to_string())) // Black
        .with_outline_width(Some(3))
        .with_bold(Some(-1))
        .with_alignment(Some(2)) // Bottom-center
        .with_margin_vertical(Some(40));

    let output = output_dir.join("output_style4_netflix.mp4");

    let config = SubtitleBurnConfig::new()
        .with_input(video.to_str().unwrap().to_string())
        .with_subtitle(subtitle.to_str().unwrap().to_string())
        .with_output(output.to_str().unwrap().to_string())
        .with_style(style);

    match add_subtitles(&config) {
        Ok(_) => println!("✓ Style 4 output saved to: {:?}", output),
        Err(e) => eprintln!("✗ Style 4 failed: {}", e),
    }
}

/// Style 5: Modern rounded background with padding
fn test_style5_rounded(video: &PathBuf, subtitle: &PathBuf, output_dir: &PathBuf) {
    println!("\n=== Test Style 5: Rounded Background with Padding ===");

    let style = SubtitleStyle::new()
        .with_font_size(26)
        .with_primary_color(Some("&H00FFFFFF".to_string())) // White
        .with_outline_color(Some("&H00000000".to_string())) // Black
        .with_background_color(Some("&HDD000000".to_string())) // Semi-transparent dark
        .with_border_style(Some(3)) // Opaque box
        .with_border_radius(Some(12)) // Rounded corners
        .with_padding(Some(12)) // Padding inside background
        .with_alignment(Some(2)) // Bottom-center
        .with_margin_vertical(Some(40));

    let output = output_dir.join("output_style5_rounded.mp4");

    let config = SubtitleBurnConfig::new()
        .with_input(video.to_str().unwrap().to_string())
        .with_subtitle(subtitle.to_str().unwrap().to_string())
        .with_output(output.to_str().unwrap().to_string())
        .with_style(style);

    match add_subtitles(&config) {
        Ok(_) => println!("✓ Style 5 output saved to: {:?}", output),
        Err(e) => eprintln!("✗ Style 5 failed: {}", e),
    }
}
