// Example demonstrating the timer/countdown filter with font rendering
// Generates test images with timers and saves them to tmp/ directory

use image::{Rgba, RgbaImage};
use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    Result,
    filters::{
        global::{TimerFilter, TimerMode, TimerSegment},
        traits::{GlobalFilter, GlobalFilterData},
    },
};

fn create_test_image(width: u32, height: u32, color: (u8, u8, u8, u8)) -> RgbaImage {
    RgbaImage::from_fn(width, height, |_, _| {
        Rgba([color.0, color.1, color.2, color.3])
    })
}

fn apply_filter_to_image(
    filter: &TimerFilter,
    width: u32,
    height: u32,
    frame_time: Duration,
) -> Result<RgbaImage> {
    let buffer = create_test_image(width, height, (30, 30, 40, 255));
    let mut data = GlobalFilterData {
        image: buffer,
        timeline_offset: frame_time,
        total_duration: Duration::from_secs(120), // Not used by timer filter
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
    println!("Timer/Countdown Filter Demo");
    println!("============================\n");

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
            println!("Warning: No font file found. Timer text will not render.");
            println!("Please provide a font file or run from the correct directory.");
        }
    }

    let (width, height) = (1280, 720);

    // Example 1: Simple count-up timer
    println!("Example 1: Count-up timer (00:00 to 01:00)");
    let mut filter1 = TimerFilter::new();

    let segment1 = TimerSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(60),
        mode: TimerMode::CountUp,
        position_x: 0.5,      // Center horizontally
        position_y: 0.9,      // Near top
        font_size: 32,
        font_path: if font_path.exists() {
            Some(font_path.clone())
        } else {
            None
        },
        font_family: None,
        text_color: (255, 255, 255, 255),
        background_color: (40, 40, 60, 180),
        padding: 12,
        border_radius: 8,
    };
    filter1.add_segment(segment1);

    let anim_dir1 = format!("{}/timer_count_up", tmp_dir);
    fs::create_dir_all(&anim_dir1)?;

    // Generate frames showing timer progression
    for i in 0..13 {
        let frame_time = Duration::from_secs(i * 5);
        let img = apply_filter_to_image(&filter1, width, height, frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir1, i);
        save_image(&img, &frame_path)?;
        let elapsed_secs = frame_time.as_secs();
        let mins = elapsed_secs / 60;
        let secs = elapsed_secs % 60;
        println!(
            "  Frame {}: time={}s, display={:02}:{:02}",
            i, elapsed_secs, mins, secs
        );
    }
    println!("  Saved 13 frames to: {}/", anim_dir1);

    // Example 2: Countdown timer (01:30 countdown to 00:00)
    println!("\nExample 2: Countdown timer (90 seconds to 00:00)");
    let mut filter2 = TimerFilter::new();

    let segment2 = TimerSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(90),
        mode: TimerMode::CountDown,
        position_x: 0.5,      // Center horizontally
        position_y: 0.5,      // Center vertically
        font_size: 48,
        font_path: if font_path.exists() {
            Some(font_path.clone())
        } else {
            None
        },
        font_family: None,
        text_color: (255, 200, 100, 255),  // Golden color for countdown
        background_color: (50, 30, 30, 200),  // Dark reddish background
        padding: 20,
        border_radius: 12,
    };
    filter2.add_segment(segment2);

    let anim_dir2 = format!("{}/timer_count_down", tmp_dir);
    fs::create_dir_all(&anim_dir2)?;

    for i in 0..10 {
        let frame_time = Duration::from_secs(i * 10);
        let img = apply_filter_to_image(&filter2, width, height, frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir2, i);
        save_image(&img, &frame_path)?;
        let remaining_secs = (90 - frame_time.as_secs()).max(0);
        let mins = remaining_secs / 60;
        let secs = remaining_secs % 60;
        println!(
            "  Frame {}: elapsed={}s, remaining={:02}:{:02}",
            i, frame_time.as_secs(), mins, secs
        );
    }
    println!("  Saved 10 frames to: {}/", anim_dir2);

    // Example 3: Multiple timers at different positions
    println!("\nExample 3: Multiple timers (count-up at bottom, countdown at top)");
    let mut filter3 = TimerFilter::new();

    // Count-up timer at bottom
    let segment3a = TimerSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(120),
        mode: TimerMode::CountUp,
        position_x: 0.15,     // Left side
        position_y: 0.0,      // Bottom
        font_size: 24,
        font_path: if font_path.exists() {
            Some(font_path.clone())
        } else {
            None
        },
        font_family: None,
        text_color: (150, 255, 150, 255),  // Green
        background_color: (30, 50, 30, 180),
        padding: 10,
        border_radius: 6,
    };

    // Countdown timer at top
    let segment3b = TimerSegment {
        start_time: Duration::from_secs(30),
        end_time: Duration::from_secs(120),
        mode: TimerMode::CountDown,
        position_x: 0.85,     // Right side
        position_y: 0.9,      // Near top
        font_size: 28,
        font_path: if font_path.exists() {
            Some(font_path.clone())
        } else {
            None
        },
        font_family: None,
        text_color: (255, 150, 150, 255),  // Reddish
        background_color: (50, 30, 30, 180),
        padding: 12,
        border_radius: 8,
    };

    filter3.add_segment(segment3a);
    filter3.add_segment(segment3b);

    let anim_dir3 = format!("{}/timer_multiple", tmp_dir);
    fs::create_dir_all(&anim_dir3)?;

    for i in 0..9 {
        let frame_time = Duration::from_secs(i * 15);
        let img = apply_filter_to_image(&filter3, width, height, frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir3, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 9 frames to: {}/", anim_dir3);

    // Example 4: Long duration timer (showing HH:MM:SS format)
    println!("\nExample 4: Long timer showing HH:MM:SS format (>1 hour)");
    let mut filter4 = TimerFilter::new();

    let segment4 = TimerSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(5400), // 1.5 hours = 90 minutes
        mode: TimerMode::CountUp,
        position_x: 0.5,
        position_y: 0.5,
        font_size: 36,
        font_path: if font_path.exists() {
            Some(font_path.clone())
        } else {
            None
        },
        font_family: None,
        text_color: (200, 200, 255, 255),
        background_color: (40, 40, 70, 200),
        padding: 16,
        border_radius: 10,
    };
    filter4.add_segment(segment4);

    let anim_dir4 = format!("{}/timer_hhmmss", tmp_dir);
    fs::create_dir_all(&anim_dir4)?;

    // Show progression from 0 to over 1 hour
    let test_times = [
        Duration::from_secs(0),
        Duration::from_secs(1800),   // 30 minutes -> 00:30:00
        Duration::from_secs(3600),   // 60 minutes -> 01:00:00
        Duration::from_secs(3661),   // 61:01 -> 01:01:01
        Duration::from_secs(4500),   // 75 minutes -> 01:15:00
        Duration::from_secs(5400),   // 90 minutes -> 01:30:00
    ];

    for (i, frame_time) in test_times.iter().enumerate() {
        let img = apply_filter_to_image(&filter4, width, height, *frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir4, i);
        save_image(&img, &frame_path)?;
        let total_secs = frame_time.as_secs();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        println!(
            "  Frame {}: elapsed={}s, display={:02}:{:02}:{:02}",
            i, total_secs, hours, mins, secs
        );
    }
    println!("  Saved 6 frames to: {}/", anim_dir4);

    // Example 5: Timer with transparent background
    println!("\nExample 5: Timer with transparent background");
    let mut filter5 = TimerFilter::new();

    let segment5 = TimerSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(45),
        mode: TimerMode::CountDown,
        position_x: 0.5,
        position_y: 0.5,
        font_size: 56,
        font_path: if font_path.exists() {
            Some(font_path.clone())
        } else {
            None
        },
        font_family: None,
        text_color: (255, 255, 255, 255),
        background_color: (0, 0, 0, 0),  // Transparent background
        padding: 0,
        border_radius: 0,
    };
    filter5.add_segment(segment5);

    let anim_dir5 = format!("{}/timer_transparent", tmp_dir);
    fs::create_dir_all(&anim_dir5)?;

    for i in 0..10 {
        let frame_time = Duration::from_secs(i * 5);
        let img = apply_filter_to_image(&filter5, width, height, frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir5, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 10 frames to: {}/", anim_dir5);

    // Example 6: Small rounded timer button style
    println!("\nExample 6: Small timer (button style)");
    let mut filter6 = TimerFilter::new();

    let segment6 = TimerSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(30),
        mode: TimerMode::CountDown,
        position_x: 0.95,     // Right edge
        position_y: 0.0,      // Bottom
        font_size: 16,
        font_path: if font_path.exists() {
            Some(font_path.clone())
        } else {
            None
        },
        font_family: None,
        text_color: (255, 255, 255, 255),
        background_color: (100, 100, 150, 220),
        padding: 6,
        border_radius: 20,    // Fully rounded (pill shape)
    };
    filter6.add_segment(segment6);

    let anim_dir6 = format!("{}/timer_button_style", tmp_dir);
    fs::create_dir_all(&anim_dir6)?;

    for i in 0..7 {
        let frame_time = Duration::from_secs(i * 5);
        let img = apply_filter_to_image(&filter6, width, height, frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir6, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 7 frames to: {}/", anim_dir6);

    println!("\n============================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}/", tmp_dir);
    println!("\nExamples:");
    println!("  - {}/timer_count_up/ - Count-up timer from 00:00", tmp_dir);
    println!("  - {}/timer_count_down/ - Countdown timer to 00:00", tmp_dir);
    println!("  - {}/timer_multiple/ - Multiple timers at different positions", tmp_dir);
    println!("  - {}/timer_hhmmss/ - Long duration timer showing HH:MM:SS", tmp_dir);
    println!("  - {}/timer_transparent/ - Timer with transparent background", tmp_dir);
    println!("  - {}/timer_button_style/ - Small pill-shaped timer", tmp_dir);

    if font_path.exists() {
        println!("\nFont used: {}", font_path.display());
    } else {
        println!("\nNote: No font file was found. Timer text was not rendered.");
        println!("To see timer text, provide a valid font_path.");
    }

    println!("\nTimer Filter Features:");
    println!("- Count-up: Starts from 00:00, counts elapsed time");
    println!("- Countdown: Starts from duration, counts down to 00:00");
    println!("- Auto format: MM:SS for <1 hour, HH:MM:SS for >=1 hour");
    println!("- Multiple segments: Each with independent position and style");
    println!("- Rounded background: With customizable border_radius");
    println!("- Flexible positioning: Normalized X/Y coordinates (0-1)");

    Ok(())
}