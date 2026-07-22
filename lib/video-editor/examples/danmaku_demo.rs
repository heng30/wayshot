// Example demonstrating the danmaku (bullet comment) filter
// Generates test images with scrolling danmaku text and saves them to tmp/ directory

use image::{Rgba, RgbaImage};
use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    Result,
    filters::{
        global::{DanmakuDistributionMode, DanmakuFilter, DanmakuItem, DanmakuSegment, DanmakuStyle},
        traits::{GlobalFilter, GlobalFilterData},
    },
};

fn create_test_image(width: u32, height: u32, color: (u8, u8, u8, u8)) -> RgbaImage {
    RgbaImage::from_fn(width, height, |_, _| {
        Rgba([color.0, color.1, color.2, color.3])
    })
}

fn apply_filter_to_image(
    filter: &DanmakuFilter,
    width: u32,
    height: u32,
    frame_time: Duration,
) -> Result<RgbaImage> {
    apply_filter_to_image_with_bg(filter, width, height, frame_time, (30, 30, 40, 255))
}

fn apply_filter_to_image_with_bg(
    filter: &DanmakuFilter,
    width: u32,
    height: u32,
    frame_time: Duration,
    bg_color: (u8, u8, u8, u8),
) -> Result<RgbaImage> {
    let buffer = create_test_image(width, height, bg_color);
    let mut data = GlobalFilterData {
        image: buffer,
        timeline_offset: frame_time,
        total_duration: Duration::from_secs(60),
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
    println!("Danmaku (Bullet Comment) Filter Demo");
    println!("======================================\n");

    // Create tmp directory
    let tmp_dir = "tmp";
    fs::create_dir_all(tmp_dir)?;

    // Check for font file
    let mut font_path: PathBuf = "../../wayshot/ui/fonts/SourceHanSansCN.otf".into();
    if !font_path.exists() {
        let alt_paths = [
            "fonts/SourceHanSansCN.otf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ];
        let found = alt_paths.iter().find(|p| PathBuf::from(p).exists());
        if let Some(p) = found {
            font_path = PathBuf::from(p);
        } else {
            println!("Warning: No font file found. Danmaku text will not render.");
            println!("Please provide a font file or run from the correct directory.");
        }
    }

    let (width, height) = (1280, 720);

    // Example 1: Basic scrolling danmaku with Uniform distribution
    println!("Example 1: Basic scrolling danmaku (Uniform distribution)");
    let style1 = DanmakuStyle {
        font_path: font_path.clone(),
        font_family: String::new(),
        font_size: 36,
        color: (255, 255, 255, 255),
        outline_width: 1,
        outline_color: (0, 0, 0, 255),
        line_spacing: 8,
    };

    let segment1 = DanmakuSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(20),
        scroll_speed: 200.0,
        distribution: DanmakuDistributionMode::Uniform,
        track_count: 0, // auto
        track_distribution: DanmakuDistributionMode::Uniform,
        position: 0.0,
        items: vec![
            DanmakuItem { text: "Hello World!".into() },
            DanmakuItem { text: "Danmaku test".into() },
            DanmakuItem { text: "Third comment".into() },
            DanmakuItem { text: "Another one".into() },
            DanmakuItem { text: "Last one".into() },
        ],
        style: style1,
    };

    let filter1 = DanmakuFilter::new()
        .with_segments(vec![segment1]);

    let anim_dir1 = format!("{}/danmaku_basic", tmp_dir);
    fs::create_dir_all(&anim_dir1)?;

    for i in 0..21 {
        let frame_time = Duration::from_millis(i * 1000);
        let img = apply_filter_to_image(&filter1, width, height, frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir1, i);
        save_image(&img, &frame_path)?;
        println!("  Frame {}: time={:.1}s", i, frame_time.as_secs_f32());
    }
    println!("  Saved 21 frames to: {}/", anim_dir1);

    // Example 2: StartDense distribution - items clustered at the beginning
    println!("\nExample 2: StartDense distribution (items clustered at start)");
    let style2 = DanmakuStyle {
        font_path: font_path.clone(),
        font_family: String::new(),
        font_size: 40,
        color: (255, 200, 50, 255),       // Gold text
        outline_width: 2,
        outline_color: (80, 0, 0, 255),    // Dark red outline
        line_spacing: 10,
    };

    let segment2 = DanmakuSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(15),
        scroll_speed: 150.0,
        distribution: DanmakuDistributionMode::StartDense,
        track_count: 0,
        track_distribution: DanmakuDistributionMode::Uniform,
        position: 0.0,
        items: vec![
            DanmakuItem { text: "Golden text!".into() },
            DanmakuItem { text: "With dark outline".into() },
            DanmakuItem { text: "Clustered at start".into() },
            DanmakuItem { text: "Fourth item".into() },
            DanmakuItem { text: "Fifth item".into() },
        ],
        style: style2,
    };

    let filter2 = DanmakuFilter::new()
        .with_segments(vec![segment2]);

    let anim_dir2 = format!("{}/danmaku_start_dense", tmp_dir);
    fs::create_dir_all(&anim_dir2)?;

    for i in 0..16 {
        let frame_time = Duration::from_millis(i * 1000);
        let img = apply_filter_to_image(&filter2, width, height, frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir2, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 16 frames to: {}/", anim_dir2);

    // Example 3: EndDense distribution - items clustered at the end
    println!("\nExample 3: EndDense distribution (items clustered at end)");
    let style3 = DanmakuStyle {
        font_path: font_path.clone(),
        font_family: String::new(),
        font_size: 32,
        color: (150, 255, 150, 255),      // Green
        outline_width: 1,
        outline_color: (0, 50, 0, 255),
        line_spacing: 6,
    };

    let segment3 = DanmakuSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(20),
        scroll_speed: 200.0,
        distribution: DanmakuDistributionMode::EndDense,
        track_count: 0,
        track_distribution: DanmakuDistributionMode::Uniform,
        position: 0.0,
        items: vec![
            DanmakuItem { text: "First item".into() },
            DanmakuItem { text: "Second item".into() },
            DanmakuItem { text: "Third item".into() },
            DanmakuItem { text: "Fourth item".into() },
            DanmakuItem { text: "Clustered at end!".into() },
        ],
        style: style3,
    };

    let filter3 = DanmakuFilter::new()
        .with_segments(vec![segment3]);

    let anim_dir3 = format!("{}/danmaku_end_dense", tmp_dir);
    fs::create_dir_all(&anim_dir3)?;

    for i in 0..21 {
        let frame_time = Duration::from_millis(i * 1000);
        let img = apply_filter_to_image(&filter3, width, height, frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir3, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 21 frames to: {}/", anim_dir3);

    // Example 4: Multiple segments with different time ranges and distributions
    println!("\nExample 4: Multiple segments with different distributions");
    let style4a = DanmakuStyle {
        font_path: font_path.clone(),
        font_family: String::new(),
        font_size: 32,
        color: (150, 255, 150, 255),      // Green
        outline_width: 1,
        outline_color: (0, 50, 0, 255),
        line_spacing: 6,
    };

    let style4b = DanmakuStyle {
        font_path: font_path.clone(),
        font_family: String::new(),
        font_size: 28,
        color: (150, 150, 255, 255),      // Blue
        outline_width: 1,
        outline_color: (0, 0, 50, 255),
        line_spacing: 6,
    };

    let segment4a = DanmakuSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(10),
        scroll_speed: 200.0,
        distribution: DanmakuDistributionMode::StartDense,
        track_count: 0,
        track_distribution: DanmakuDistributionMode::Uniform,
        position: 0.0,
        items: vec![
            DanmakuItem { text: "Opening comments".into() },
            DanmakuItem { text: "Green style".into() },
            DanmakuItem { text: "First segment".into() },
        ],
        style: style4a,
    };

    let segment4b = DanmakuSegment {
        start_time: Duration::from_secs(10),
        end_time: Duration::from_secs(20),
        scroll_speed: 250.0,
        distribution: DanmakuDistributionMode::EndDense,
        track_count: 0,
        track_distribution: DanmakuDistributionMode::Uniform,
        position: 0.0,
        items: vec![
            DanmakuItem { text: "Later comments".into() },
            DanmakuItem { text: "Blue style".into() },
            DanmakuItem { text: "Faster scroll".into() },
        ],
        style: style4b,
    };

    let filter4 = DanmakuFilter::new()
        .with_segments(vec![segment4a, segment4b]);

    let anim_dir4 = format!("{}/danmaku_multi_segment", tmp_dir);
    fs::create_dir_all(&anim_dir4)?;

    for i in 0..21 {
        let frame_time = Duration::from_millis(i * 1000);
        let img = apply_filter_to_image(&filter4, width, height, frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir4, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 21 frames to: {}/", anim_dir4);

    // Example 5: Fixed track count with different track distributions
    println!("\nExample 5: Fixed track count (4 tracks) with StartDense track distribution");
    let style5 = DanmakuStyle {
        font_path: font_path.clone(),
        font_family: String::new(),
        font_size: 30,
        color: (255, 255, 255, 255),
        outline_width: 1,
        outline_color: (0, 0, 0, 255),
        line_spacing: 8,
    };

    let segment5 = DanmakuSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(15),
        scroll_speed: 180.0,
        distribution: DanmakuDistributionMode::Uniform,
        track_count: 4, // Only 4 tracks
        track_distribution: DanmakuDistributionMode::StartDense, // Prefer lower tracks
        position: 0.0,
        items: vec![
            DanmakuItem { text: "Item 1".into() },
            DanmakuItem { text: "Item 2".into() },
            DanmakuItem { text: "Item 3".into() },
            DanmakuItem { text: "Item 4".into() },
            DanmakuItem { text: "Item 5".into() },
            DanmakuItem { text: "Item 6".into() },
        ],
        style: style5,
    };

    let filter5 = DanmakuFilter::new()
        .with_segments(vec![segment5]);

    let anim_dir5 = format!("{}/danmaku_fixed_tracks", tmp_dir);
    fs::create_dir_all(&anim_dir5)?;

    for i in 0..16 {
        let frame_time = Duration::from_millis(i * 1000);
        let img = apply_filter_to_image(&filter5, width, height, frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir5, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 16 frames to: {}/", anim_dir5);

    // Example 6: Dense danmaku (many items with fast scroll)
    println!("\nExample 6: Dense danmaku with many items");
    let style6 = DanmakuStyle {
        font_path: font_path.clone(),
        font_family: String::new(),
        font_size: 24,
        color: (255, 255, 255, 255),
        outline_width: 1,
        outline_color: (0, 0, 0, 255),
        line_spacing: 4,
    };

    let items6: Vec<DanmakuItem> = (0..20)
        .map(|i| DanmakuItem {
            text: format!("Comment #{}", i + 1),
        })
        .collect();

    let segment6 = DanmakuSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(20),
        scroll_speed: 300.0,
        distribution: DanmakuDistributionMode::Uniform,
        track_count: 0,
        track_distribution: DanmakuDistributionMode::Uniform,
        position: 0.0,
        items: items6,
        style: style6,
    };

    let filter6 = DanmakuFilter::new()
        .with_segments(vec![segment6]);

    let anim_dir6 = format!("{}/danmaku_dense", tmp_dir);
    fs::create_dir_all(&anim_dir6)?;

    for i in 0..21 {
        let frame_time = Duration::from_millis(i * 1000);
        let img = apply_filter_to_image(&filter6, width, height, frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir6, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 21 frames to: {}/", anim_dir6);

    // Example 7: Chinese danmaku
    println!("\nExample 7: Chinese danmaku");
    let style7 = DanmakuStyle {
        font_path: font_path.clone(),
        font_family: String::new(),
        font_size: 34,
        color: (255, 255, 255, 255),
        outline_width: 1,
        outline_color: (0, 0, 0, 255),
        line_spacing: 8,
    };

    let segment7 = DanmakuSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(20),
        scroll_speed: 200.0,
        distribution: DanmakuDistributionMode::Uniform,
        track_count: 0,
        track_distribution: DanmakuDistributionMode::Uniform,
        position: 0.0,
        items: vec![
            DanmakuItem { text: "弹幕测试".into() },
            DanmakuItem { text: "你好世界".into() },
            DanmakuItem { text: "这个滤镜真不错".into() },
            DanmakuItem { text: "前方高能".into() },
            DanmakuItem { text: "哈哈哈哈哈".into() },
        ],
        style: style7,
    };

    let filter7 = DanmakuFilter::new()
        .with_segments(vec![segment7]);

    let anim_dir7 = format!("{}/danmaku_chinese", tmp_dir);
    fs::create_dir_all(&anim_dir7)?;

    for i in 0..21 {
        let frame_time = Duration::from_millis(i * 1000);
        let img = apply_filter_to_image(&filter7, width, height, frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir7, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 21 frames to: {}/", anim_dir7);

    // Example 8: White background, white text with 2px black outline (large font for outline inspection)
    println!("\nExample 8: White background, white text with 2px black outline");
    let style8 = DanmakuStyle {
        font_path: font_path.clone(),
        font_family: String::new(),
        font_size: 72,
        color: (255, 255, 255, 255),       // White text
        outline_width: 2,
        outline_color: (0, 0, 0, 255),     // Black outline
        line_spacing: 16,
    };

    let segment8 = DanmakuSegment {
        start_time: Duration::from_secs(0),
        end_time: Duration::from_secs(20),
        scroll_speed: 200.0,
        distribution: DanmakuDistributionMode::Uniform,
        track_count: 0,
        track_distribution: DanmakuDistributionMode::Uniform,
        position: 0.0,
        items: vec![
            DanmakuItem { text: "White on white!".into() },
            DanmakuItem { text: "Outline keeps it visible".into() },
            DanmakuItem { text: "2px black border".into() },
            DanmakuItem { text: "Clear and readable".into() },
            DanmakuItem { text: "弹幕也能看清".into() },
        ],
        style: style8,
    };

    let filter8 = DanmakuFilter::new()
        .with_segments(vec![segment8]);

    let anim_dir8 = format!("{}/danmaku_white_bg", tmp_dir);
    fs::create_dir_all(&anim_dir8)?;

    for i in 0..21 {
        let frame_time = Duration::from_millis(i * 1000);
        let img = apply_filter_to_image_with_bg(&filter8, width, height, frame_time, (255, 255, 255, 255))?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir8, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 21 frames to: {}/", anim_dir8);

    println!("\n======================================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}/", tmp_dir);
    println!("\nExamples:");
    println!("  - {}/danmaku_basic/ - Uniform distribution (evenly spaced)", tmp_dir);
    println!("  - {}/danmaku_start_dense/ - StartDense distribution (clustered at start)", tmp_dir);
    println!("  - {}/danmaku_end_dense/ - EndDense distribution (clustered at end)", tmp_dir);
    println!("  - {}/danmaku_multi_segment/ - Multiple segments with different distributions", tmp_dir);
    println!("  - {}/danmaku_fixed_tracks/ - Fixed track count with track distribution", tmp_dir);
    println!("  - {}/danmaku_dense/ - Dense danmaku with many items", tmp_dir);
    println!("  - {}/danmaku_chinese/ - Chinese danmaku", tmp_dir);
    println!("  - {}/danmaku_white_bg/ - White background, white text with 2px black outline", tmp_dir);

    if font_path.exists() {
        println!("\nFont used: {}", font_path.display());
    } else {
        println!("\nNote: No font file was found. Danmaku text was not rendered.");
        println!("To see danmaku text, provide a valid font_path.");
    }

    println!("\nDanmaku Filter Features:");
    println!("- Distribution modes: StartDense, Uniform, EndDense");
    println!("  - Uniform: Items evenly spaced across segment duration");
    println!("  - StartDense: Items clustered at the beginning (quadratic spacing)");
    println!("  - EndDense: Items clustered at the end (quadratic spacing)");
    println!("- Track assignment: Auto or fixed count");
    println!("  - track_count=0: Auto-detect from image height");
    println!("  - track_count>0: Use exactly that many tracks");
    println!("- Track distribution: Controls which tracks get priority");
    println!("  - StartDense: Prefer lower-numbered tracks first");
    println!("  - Uniform: Round-robin across tracks");
    println!("  - EndDense: Prefer higher-numbered tracks first");
    println!("- Time segments: Each segment has independent time range and style");
    println!("- Text outline: Customizable outline width and color");
    println!("- Scroll speed: Pixels per second, configurable per segment");

    Ok(())
}
