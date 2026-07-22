use std::{path::PathBuf, sync::Arc, time::Duration};
use video_editor::{metadata::get_metadata, tracks::segment::Segment};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let tmp_dir = PathBuf::from("tmp");
    let video_path = PathBuf::from("data").join("test.mp4");
    log::info!("Loading video from: {}", video_path.display());

    let metadata = Arc::new(get_metadata(&video_path)?);
    let video_meta = metadata.videos.first().ok_or("No video track found")?;

    log::info!("\nVideo info:");
    log::info!("  Resolution: {}x{}", video_meta.width, video_meta.height);
    log::info!("  FPS: {}", video_meta.fps);
    log::info!("  Duration: {:.2}s", metadata.duration.as_secs_f64());

    let segment = Segment::new(Duration::ZERO, metadata.duration, metadata.clone(), 1.0);

    log::info!("\nSegment info:");
    log::info!("  First frame index: {:?}", segment.first_frame_index());
    log::info!("  Last frame index: {:?}", segment.last_frame_index());
    log::info!("  Frame count: {:?}", segment.frame_count());

    // 1. 获取第一帧
    log::info!("\n=== Extracting first frame ===");
    let first_frame = segment.first_frame_image()?;
    log::info!(
        "First frame size: {}x{}",
        first_frame.width(),
        first_frame.height()
    );
    let first_frame_path = tmp_dir.join("first_frame.png");
    first_frame.save(&first_frame_path)?;
    log::info!("✓ Saved to: {}", first_frame_path.display());

    // 2. 获取 25% 位置的帧
    log::info!("\n=== Extracting frame at 25% position ===");
    let quarter_offset = metadata.duration / 4;
    log::info!("Quarter offset: {:.2}s", quarter_offset.as_secs_f64());

    let quarter_frame = segment.frame_image_at_timeline_offset(quarter_offset)?;
    let quarter_frame_path = tmp_dir.join("quarter_frame.png");
    quarter_frame.save(&quarter_frame_path)?;
    log::info!("✓ Saved to: {}", quarter_frame_path.display());

    // 3. 获取中间位置的帧 (50% 处)
    log::info!("\n=== Extracting middle frame (50% position) ===");
    let middle_offset = metadata.duration / 2;
    log::info!("Middle offset: {:.2}s", middle_offset.as_secs_f64());

    let middle_frame = segment.frame_image_at_timeline_offset(middle_offset)?;
    let middle_frame_path = tmp_dir.join("middle_frame.png");
    middle_frame.save(&middle_frame_path)?;
    log::info!("✓ Saved to: {}", middle_frame_path.display());

    // 4. 获取 75% 位置的帧
    log::info!("\n=== Extracting frame at 75% position ===");
    let three_quarter_offset = metadata.duration * 3 / 4;
    log::info!(
        "Three-quarter offset: {:.2}s",
        three_quarter_offset.as_secs_f64()
    );

    let three_quarter_frame = segment.frame_image_at_timeline_offset(three_quarter_offset)?;
    let three_quarter_frame_path = tmp_dir.join("three_quarter_frame.png");
    three_quarter_frame.save(&three_quarter_frame_path)?;
    log::info!("✓ Saved to: {}", three_quarter_frame_path.display());

    // 5. 获取最后一帧
    log::info!("\n=== Extracting last frame ===");
    let last_frame = segment.last_frame_image()?;
    log::info!(
        "Last frame size: {}x{}",
        last_frame.width(),
        last_frame.height()
    );
    let last_frame_path = tmp_dir.join("last_frame.png");
    last_frame.save(&last_frame_path)?;
    log::info!("✓ Saved to: {}", last_frame_path.display());

    log::info!("\n=== Summary ===");
    log::info!("✓ All frames extracted successfully!");
    log::info!("Output directory: {}", tmp_dir.display());
    log::info!(
        "Frame size: {}x{}",
        first_frame.width(),
        first_frame.height()
    );
    log::info!("\nExtracted frames:");
    log::info!("  1. first_frame.png      - First frame (0%)");
    log::info!("  2. quarter_frame.png    - Frame at 25%");
    log::info!("  3. middle_frame.png    - Frame at 50%");
    log::info!("  4. three_quarter_frame.png - Frame at 75%");

    Ok(())
}
