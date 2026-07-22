// Example demonstrating the progress bar filter with font rendering
// Generates test images with progress bars and saves them to tmp/ directory

use image::{Rgba, RgbaImage};
use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    Result,
    filters::{
        global::ProgressBarFilter,
        traits::{GlobalFilter, GlobalFilterData},
    },
};

fn create_test_image(width: u32, height: u32, color: (u8, u8, u8, u8)) -> RgbaImage {
    RgbaImage::from_fn(width, height, |_, _| {
        Rgba([color.0, color.1, color.2, color.3])
    })
}

fn apply_filter_to_image(
    filter: &ProgressBarFilter,
    width: u32,
    height: u32,
    frame_time: Duration,
    total_duration: Duration,
) -> Result<RgbaImage> {
    let buffer = create_test_image(width, height, (30, 30, 40, 255));
    let mut data = GlobalFilterData {
        image: buffer,
        timeline_offset: frame_time,
        total_duration,
    };

    filter.apply(&mut data)?;
    Ok(data.image)
}

fn save_image(image: &RgbaImage, path: &str) -> Result<()> {
    image
        .save(path)
        .map_err(|e| video_editor::Error::InvalidConfig(format!("Failed to save image: {}", e)))
}

fn main() -> Result<()> {
    println!("Progress Bar Filter Demo");
    println!("=========================\n");

    // Create tmp directory
    let tmp_dir = "tmp";
    fs::create_dir_all(tmp_dir)?;

    // Check for font file
    let mut font_path: PathBuf = "../../wayshot/ui/fonts/SourceHanSansCN.otf".into();
    if !font_path.exists() {
        // Try alternative font paths
        let alt_paths = [
            "fonts/SourceHanSansCN.otf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ];
        let found = alt_paths.iter().find(|p| PathBuf::from(p).exists());
        if let Some(p) = found {
            font_path = PathBuf::from(p);
        } else {
            println!("Warning: No font file found. Text labels will not render.");
            println!("Please provide a font file or run from the correct directory.");
        }
    }

    let (width, height) = (1280, 720);
    let total_duration = Duration::from_secs(80);

    // Example 1: Simple progress bar with segments
    println!("Example 1: Progress bar with segments and text labels");
    let filter1 = ProgressBarFilter::new()
        .with_padding(4)
        .with_margin_h(40)
        .with_font_size(16)
        .with_font_path(if font_path.exists() {
            Some(font_path.clone())
        } else {
            None
        })
        .with_background_color((60, 60, 60, 200))
        .with_progress_color((100, 180, 100, 230))
        .with_separator_color((255, 255, 255, 180))
        .with_text_color((255, 255, 255, 255));

    // Add segments
    let mut filter1 = filter1;
    filter1.add_segment("Intro".to_string(), Duration::from_secs(10));
    filter1.add_segment("Chapter 1".to_string(), Duration::from_secs(25));
    filter1.add_segment("Chapter 2".to_string(), Duration::from_secs(45));
    filter1.add_segment("End".to_string(), Duration::from_secs(60));

    let anim_dir1 = format!("{}/progress_bar_basic", tmp_dir);
    fs::create_dir_all(&anim_dir1)?;

    // Generate frames at different progress levels (including 100%)
    for i in 0..11 {
        let frame_time = if i == 10 {
            total_duration // Final frame at 100%
        } else {
            Duration::from_secs(i * 6 + 3) // Every 6 seconds, starting at 3s
        };
        let img = apply_filter_to_image(&filter1, width, height, frame_time, total_duration)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir1, i);
        save_image(&img, &frame_path)?;
        println!(
            "  Frame {}: time={:.1}s, progress={:.1}%",
            i,
            frame_time.as_secs_f32(),
            (frame_time.as_secs_f32() / total_duration.as_secs_f32()) * 100.0
        );
    }
    println!("  Saved 11 frames to: {}/", anim_dir1);

    // Example 2: Progress bar at different position
    println!("\nExample 2: Progress bar at top (position_y=0.9)");
    let filter2 = ProgressBarFilter::new()
        .with_position_y(0.9)
        .with_padding(3)
        .with_margin_h(60)
        .with_font_size(14)
        .with_font_path(if font_path.exists() {
            Some(font_path.clone())
        } else {
            None
        })
        .with_background_color((40, 40, 50, 180))
        .with_progress_color((200, 100, 150, 220));

    let mut filter2 = filter2;
    filter2.add_segment("Part A".to_string(), Duration::from_secs(20));
    filter2.add_segment("Part B".to_string(), Duration::from_secs(40));
    filter2.add_segment("Part C".to_string(), Duration::from_secs(60));

    let anim_dir2 = format!("{}/progress_bar_top", tmp_dir);
    fs::create_dir_all(&anim_dir2)?;

    for i in 0..6 {
        let frame_time = Duration::from_secs(i * 10 + 5);
        let img = apply_filter_to_image(&filter2, width, height, frame_time, total_duration)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir2, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 6 frames to: {}/", anim_dir2);

    // Example 3: Progress bar with Chinese text
    println!("\nExample 3: Progress bar with Chinese segment labels");
    let filter3 = ProgressBarFilter::new()
        .with_padding(4)
        .with_margin_h(50)
        .with_font_size(18)
        .with_font_path(if font_path.exists() {
            Some(font_path.clone())
        } else {
            None
        })
        .with_background_color((30, 30, 35, 200))
        .with_progress_color((80, 200, 120, 240))
        .with_separator_color((200, 200, 200, 150))
        .with_text_color((255, 255, 255, 255));

    let mut filter3 = filter3;
    filter3.add_segment("序章".to_string(), Duration::from_secs(15));
    filter3.add_segment("第一章".to_string(), Duration::from_secs(35));
    filter3.add_segment("第二章".to_string(), Duration::from_secs(60));
    filter3.add_segment("尾声".to_string(), Duration::from_secs(90));

    let anim_dir3 = format!("{}/progress_bar_chinese", tmp_dir);
    fs::create_dir_all(&anim_dir3)?;

    for i in 0..8 {
        let frame_time = Duration::from_secs(i * 12 + 5);
        let img =
            apply_filter_to_image(&filter3, width, height, frame_time, Duration::from_secs(90))?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir3, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 8 frames to: {}/", anim_dir3);

    // Example 4: Progress bar without segments (just the bar)
    println!("\nExample 4: Progress bar without segment labels");
    let filter4 = ProgressBarFilter::new()
        .with_padding(2)
        .with_margin_h(30)
        .with_background_color((50, 50, 55, 200))
        .with_progress_color((255, 180, 80, 230));

    let anim_dir4 = format!("{}/progress_bar_no_segments", tmp_dir);
    fs::create_dir_all(&anim_dir4)?;

    for i in 0..5 {
        let frame_time = Duration::from_secs(i * 6);
        let img =
            apply_filter_to_image(&filter4, width, height, frame_time, Duration::from_secs(30))?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir4, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 5 frames to: {}/", anim_dir4);

    println!("\n=========================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}/", tmp_dir);
    println!("\nExamples:");
    println!(
        "  - {}/progress_bar_basic/ - Basic progress bar with segments",
        tmp_dir
    );
    println!(
        "  - {}/progress_bar_top/ - Progress bar at top position",
        tmp_dir
    );
    println!(
        "  - {}/progress_bar_chinese/ - Progress bar with Chinese labels",
        tmp_dir
    );
    println!(
        "  - {}/progress_bar_no_segments/ - Progress bar without labels",
        tmp_dir
    );

    if font_path.exists() {
        println!("\nFont used: {}", font_path.display());
    } else {
        println!("\nNote: No font file was found. Text labels were not rendered.");
        println!("To see text labels, provide a valid font_path.");
    }

    println!("\nProgress Bar Features:");
    println!("- Text is centered between separator lines");
    println!("- Background bar shows total duration");
    println!("- Progress (filled) bar shows current timeline position");
    println!("- Separators mark segment boundaries");

    Ok(())
}

