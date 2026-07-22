//! Demo for UnifiedTextTracksCompositorIterator and text track rendering pipeline.
//!
//! This example tests:
//! - Single text track with one segment
//! - Single text track with multiple segments at different timeline offsets
//! - Text track with keyframe animation (position change over time)
//! - Text-only project (no video) - transparent background frames
//!
//! Output: /tmp/unified_text_demo_*.png

use std::{sync::Arc, time::Duration};
use video_editor::{
    filters::subtitle::style::SubtitleStyle,
    tracks::{
        Manager, Track, TextTrack, TextElement, UnifiedMixerConfig,
        create_text_layer_frame,
    },
    tracks::video_track::composite_frame,
};
use image::{Rgba, RgbaImage};

fn main() -> video_editor::Result<()> {
    env_logger::init();

    // Find a font file for text rendering
    let font_path = find_font();
    println!("Using font: {}", font_path.display());

    // Demo 1: Single text track with one segment
    demo_single_text_segment(&font_path)?;

    // Demo 2: Multiple text segments at different times
    demo_multiple_text_segments(&font_path)?;

    // Demo 3: Text with keyframe animation (position change)
    demo_text_with_keyframe(&font_path)?;

    // Demo 4: Text with opacity animation
    demo_text_opacity_animation(&font_path)?;

    println!("\nAll demos completed! Check output files in /tmp/");
    println!("Expected files:");
    println!("  - /tmp/unified_text_single.png (Hello World at center)");
    println!("  - /tmp/unified_text_multi_*.png (First, Second, Third text segments)");
    println!("  - /tmp/unified_text_anim_*.png (Animated position: left to right)");
    println!("  - /tmp/unified_text_opacity_*.png (Opacity fading: 100% to 0%)");

    Ok(())
}

fn find_font() -> std::path::PathBuf {
    // Try common font locations
    let candidates = [
        // Project fonts
        "/home/blue/Code/rust/wayshot/wayshot/ui/fonts/SourceHanSerifCN.ttf",
        "/home/blue/Code/rust/wayshot/wayshot/ui/fonts/SourceHanSansCN.otf",
        // System fonts
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ];

    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return std::path::PathBuf::from(path);
        }
    }

    // Fallback - user should provide a font path
    println!("Warning: No default font found. Using empty path (will fail if not fixed).");
    println!("Please modify font_path in this example to a valid font file.");
    std::path::PathBuf::new()
}

fn demo_single_text_segment(font_path: &std::path::Path) -> video_editor::Result<()> {
    println!("\n=== Demo 1: Single text Segment ===");

    let mut manager = Manager::new();

    // Create text track with a single segment
    let mut text_track = TextTrack::new();
    let style = SubtitleStyle::default()
        .with_font_size(48)
        .with_font_path(font_path.to_path_buf())
        .with_primary_color(Some(Rgba([255, 255, 255, 255])))
        .with_outline_color(Some(Rgba([0, 0, 0, 255])))
        .with_outline_width(Some(2));

    let element = TextElement::new("Hello World")
        .with_style(style)
        .with_position(0.5, 0.5) // Center of frame
        .with_opacity(1.0);

    text_track.add_segment(element, Duration::ZERO, Duration::from_secs(5), 1.0);
    manager.add_track(Track::Text(Arc::new(text_track)));

    // Create UnifiedMixerIterator
    let config = UnifiedMixerConfig::default()
        .with_timeline_offset(Duration::ZERO)
        .with_output_width(Some(640))
        .with_output_height(Some(480))
        .with_output_fps(Some(25.0))
        .with_duration(Some(Duration::from_secs(1)));

    let mixer_iter = manager.unified_tracks_mixer_iter_with_config(config)?;

    // Render first frame
    for unified_frame in mixer_iter {
        let mut composited = RgbaImage::new(640, 480);

        for text in &unified_frame.text {
            let layer_frame = create_text_layer_frame(
                &text.element,
                text.segment.clone(),
                text.segment_index,
                text.track_index,
                unified_frame.timeline_offset,
                640, 480,
            )?;

            composite_frame(&mut composited, &layer_frame.image);
        }

        // Save first frame
        composited.save("/tmp/unified_text_single.png")?;
        println!("Saved: /tmp/unified_text_single.png");
        println!("  Expected: 'Hello World' centered on transparent background");

        // Verify content was rendered
        let pixel_count = composited.pixels().filter(|p| p[3] > 0).count();
        assert!(pixel_count > 0, "Text should have rendered visible pixels");
        println!("  Pixels with alpha > 0: {}", pixel_count);

        break; // Only save first frame for single segment demo
    }

    Ok(())
}

fn demo_multiple_text_segments(font_path: &std::path::Path) -> video_editor::Result<()> {
    println!("\n=== Demo 2: Multiple Text Segments ===");

    let mut manager = Manager::new();

    // Create text track with multiple segments at different times
    let mut text_track = TextTrack::new();
    let style = SubtitleStyle::default()
        .with_font_size(36)
        .with_font_path(font_path.to_path_buf())
        .with_primary_color(Some(Rgba([255, 200, 100, 255])))
        .with_outline_color(Some(Rgba([50, 50, 50, 255])))
        .with_outline_width(Some(2));

    // Segment 1: 0-1s at top
    text_track.add_segment(
        TextElement::new("First Segment").with_style(style.clone()).with_position(0.5, 0.2),
        Duration::ZERO,
        Duration::from_secs(1),
        1.0,
    );

    // Segment 2: 1-2s at center
    text_track.add_segment(
        TextElement::new("Second Segment").with_style(style.clone()).with_position(0.5, 0.5),
        Duration::from_secs(1),
        Duration::from_secs(1),
        1.0,
    );

    // Segment 3: 2-3s at bottom
    text_track.add_segment(
        TextElement::new("Third Segment").with_style(style.clone()).with_position(0.5, 0.8),
        Duration::from_secs(2),
        Duration::from_secs(1),
        1.0,
    );

    manager.add_track(Track::Text(Arc::new(text_track)));

    // Create UnifiedMixerIterator for 3 seconds
    let config = UnifiedMixerConfig::default()
        .with_timeline_offset(Duration::ZERO)
        .with_output_width(Some(640))
        .with_output_height(Some(480))
        .with_output_fps(Some(10.0))
        .with_duration(Some(Duration::from_secs(3)));

    let mixer_iter = manager.unified_tracks_mixer_iter_with_config(config)?;

    let mut frame_count = 0;
    for unified_frame in mixer_iter {
        let mut composited = RgbaImage::new(640, 480);

        for text in &unified_frame.text {
            let layer_frame = create_text_layer_frame(
                &text.element,
                text.segment.clone(),
                text.segment_index,
                text.track_index,
                unified_frame.timeline_offset,
                640, 480,
            )?;

            composite_frame(&mut composited, &layer_frame.image);
        }

        // Save frame at each second boundary
        let seconds = unified_frame.timeline_offset.as_secs();
        if frame_count % 10 == 0 && seconds < 3 {
            let path = format!("/tmp/unified_text_multi_{}.png", seconds);
            composited.save(&path)?;
            println!("Saved: {} (timeline: {}s)", path, seconds);
        }

        frame_count += 1;
    }

    println!("  Expected: Different text at different timestamps");
    println!("    0s: 'First Segment' at top");
    println!("    1s: 'Second Segment' at center");
    println!("    2s: 'Third Segment' at bottom");

    Ok(())
}

fn demo_text_with_keyframe(font_path: &std::path::Path) -> video_editor::Result<()> {
    println!("\n=== Demo 3: Text with Position Keyframe Animation ===");

    let mut manager = Manager::new();

    // Create text track with keyframe animation (position moves from left to right)
    let mut text_track = TextTrack::new();
    let style = SubtitleStyle::default()
        .with_font_size(32)
        .with_font_path(font_path.to_path_buf())
        .with_primary_color(Some(Rgba([100, 200, 255, 255])))
        .with_outline_color(Some(Rgba([0, 0, 0, 255])))
        .with_outline_width(Some(1));

    use video_editor::filters::keyframe::{Keyframe, KeyframeValue};

    let mut element = TextElement::new("Animated Text")
        .with_style(style)
        .with_position(0.1, 0.5) // Start at left
        .with_opacity(1.0);

    // Add position keyframes: left (0s) -> center (1s) -> right (2s)
    element.keyframe_tracks.add_keyframe("position", Keyframe::new(0, KeyframeValue::Float2(0.1, 0.5)));
    element.keyframe_tracks.add_keyframe("position", Keyframe::new(2000, KeyframeValue::Float2(0.9, 0.5)));

    text_track.add_segment(element, Duration::ZERO, Duration::from_secs(3), 1.0);
    manager.add_track(Track::Text(Arc::new(text_track)));

    // Create UnifiedMixerIterator for 3 seconds
    let config = UnifiedMixerConfig::default()
        .with_timeline_offset(Duration::ZERO)
        .with_output_width(Some(640))
        .with_output_height(Some(480))
        .with_output_fps(Some(10.0))
        .with_duration(Some(Duration::from_secs(3)));

    let mixer_iter = manager.unified_tracks_mixer_iter_with_config(config)?;

    let mut frame_count = 0;
    for unified_frame in mixer_iter {
        let mut composited = RgbaImage::new(640, 480);

        for text in &unified_frame.text {
            let layer_frame = create_text_layer_frame(
                &text.element,
                text.segment.clone(),
                text.segment_index,
                text.track_index,
                unified_frame.timeline_offset,
                640, 480,
            )?;

            composite_frame(&mut composited, &layer_frame.image);
        }

        // Save frames at 0s, 1s, 2s
        let seconds = unified_frame.timeline_offset.as_secs();
        if frame_count % 10 == 0 && seconds < 3 {
            let path = format!("/tmp/unified_text_anim_{}.png", seconds);
            composited.save(&path)?;
            println!("Saved: {} (timeline: {}s)", path, seconds);
        }

        frame_count += 1;
    }

    println!("  Expected: Text position animated from left to right");
    println!("    0s: 'Animated Text' at left (10% position)");
    println!("    1s: 'Animated Text' at center-ish (50% position)");
    println!("    2s: 'Animated Text' at right (90% position)");

    Ok(())
}

fn demo_text_opacity_animation(font_path: &std::path::Path) -> video_editor::Result<()> {
    println!("\n=== Demo 4: Text with Opacity Animation ===");

    let mut manager = Manager::new();

    // Create text track with opacity fade-out animation
    let mut text_track = TextTrack::new();
    let style = SubtitleStyle::default()
        .with_font_size(48)
        .with_font_path(font_path.to_path_buf())
        .with_primary_color(Some(Rgba([255, 100, 100, 255])))
        .with_outline_color(Some(Rgba([0, 0, 0, 255])))
        .with_outline_width(Some(2));

    use video_editor::filters::keyframe::{Keyframe, KeyframeValue};

    let mut element = TextElement::new("Fading Text")
        .with_style(style)
        .with_position(0.5, 0.5)
        .with_opacity(1.0);

    // Add opacity keyframes: full (0s) -> half (1s) -> zero (2s)
    element.keyframe_tracks.add_keyframe("opacity", Keyframe::new(0, KeyframeValue::Float(1.0)));
    element.keyframe_tracks.add_keyframe("opacity", Keyframe::new(1000, KeyframeValue::Float(0.5)));
    element.keyframe_tracks.add_keyframe("opacity", Keyframe::new(2000, KeyframeValue::Float(0.0)));

    text_track.add_segment(element, Duration::ZERO, Duration::from_secs(3), 1.0);
    manager.add_track(Track::Text(Arc::new(text_track)));

    // Create UnifiedMixerIterator for 3 seconds
    let config = UnifiedMixerConfig::default()
        .with_timeline_offset(Duration::ZERO)
        .with_output_width(Some(640))
        .with_output_height(Some(480))
        .with_output_fps(Some(10.0))
        .with_duration(Some(Duration::from_secs(3)));

    let mixer_iter = manager.unified_tracks_mixer_iter_with_config(config)?;

    let mut frame_count = 0;
    for unified_frame in mixer_iter {
        let mut composited = RgbaImage::new(640, 480);

        for text in &unified_frame.text {
            let layer_frame = create_text_layer_frame(
                &text.element,
                text.segment.clone(),
                text.segment_index,
                text.track_index,
                unified_frame.timeline_offset,
                640, 480,
            )?;

            composite_frame(&mut composited, &layer_frame.image);
        }

        // Save frames at 0s, 1s, 2s
        let seconds = unified_frame.timeline_offset.as_secs();
        if frame_count % 10 == 0 && seconds < 3 {
            let path = format!("/tmp/unified_text_opacity_{}.png", seconds);
            composited.save(&path)?;
            println!("Saved: {} (timeline: {}s)", path, seconds);
        }

        frame_count += 1;
    }

    println!("  Expected: Text opacity fading out");
    println!("    0s: 'Fading Text' fully visible (100% opacity)");
    println!("    1s: 'Fading Text' half transparent (50% opacity)");
    println!("    2s: 'Fading Text' nearly invisible (0% opacity)");

    Ok(())
}