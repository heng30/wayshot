use std::{path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    export::{Mp4ExportConfig, Mp4Exporter},
    filters::subtitle::style::{
        font_path::FontPathFilter, font_size::FontSizeFilter, margin::MarginVerticalFilter,
    },
    metadata::{self, Metadata, VideoMetadata},
    tracks::{
        image_track::ImageTrack,
        manager::Manager,
        segment::Segment,
        subtitle_track::SubtitleTrack,
        track::{InnerTrack, Track},
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let mut manager = Manager::new();

    // --- 创建测试图片 ---
    let img_path = PathBuf::from("data/test_solid.png");
    std::fs::create_dir_all("data")?;
    if !img_path.exists() {
        image::RgbaImage::from_pixel(1920, 1080, image::Rgba([30, 60, 120, 255]))
            .save(&img_path)?;
    }

    // --- 构造图片 Metadata（duration=0 标识为图片） ---
    let img_metadata = Arc::new(Metadata {
        path: img_path.clone(),
        duration: Duration::ZERO,
        videos: vec![VideoMetadata {
            index: 0,
            codec_id: metadata::Id::None,
            pix_fmt: metadata::Pixel::RGBA,
            width: 1920,
            height: 1080,
            fps: 25.0,
            language: None,
            duration: Duration::ZERO,
        }],
        ..Default::default()
    });

    let total_duration = Duration::from_secs(5);
    let img_segment = Arc::new(Segment::new(
        Duration::ZERO,
        total_duration,
        img_metadata.clone(),
        1.0,
    ));
    let img_inner = InnerTrack::new(img_metadata, total_duration, vec![img_segment]);
    manager.add_track(Track::Image(Arc::new(ImageTrack::new_with_inner(img_inner))));

    log::info!("=== Image track added ({}s) ===", total_duration.as_secs());

    // --- 创建字幕轨道 ---
    let sub_meta = Arc::new(Metadata::new_subtitle());

    // 字幕1: 开头短字幕 0~0.5s（预期会丢失的）
    let sub1 = Arc::new(
        Segment::new_with_source_offset(
            Duration::ZERO,
            Duration::ZERO,
            Duration::from_millis(500),
            1.0,
            1.0,
            sub_meta.clone(),
        )
        .with_subtitle_text("FIRST (0-0.5s)"),
    );

    // 字幕2: 中间短字幕 2~2.5s（预期正常）
    let sub2 = Arc::new(
        Segment::new_with_source_offset(
            Duration::from_secs(2),
            Duration::ZERO,
            Duration::from_millis(500),
            1.0,
            1.0,
            sub_meta.clone(),
        )
        .with_subtitle_text("SECOND (2-2.5s)"),
    );

    let sub_inner = InnerTrack::new(sub_meta, Duration::from_secs(3), vec![sub1, sub2]);

    // 应用字幕滤镜
    let mut sub_track = SubtitleTrack::new(sub_inner);
    for seg in &mut sub_track.track.segments {
        let s = Arc::make_mut(seg);
        let font_path = PathBuf::from("../../wayshot/ui/fonts/SourceHanSansCN.otf");
        if font_path.exists() {
            s.add_subtitle_filter(Box::new(FontPathFilter::new(
                font_path,
                "SourceHanSansCN".to_string(),
                String::new(),
            )));
        }
        s.add_subtitle_filter(Box::new(FontSizeFilter::new(72)));
        s.add_subtitle_filter(Box::new(MarginVerticalFilter::new(Some(100))));
    }

    let entries = sub_track.get_subtitle_entries();
    log::info!("=== Subtitle track: {} entries ===", entries.len());
    for e in &entries {
        log::info!(
            "  [{:.3}s - {:.3}s] {}",
            e.start.as_secs_f64(),
            e.end.as_secs_f64(),
            e.text
        );
    }

    manager.add_track(Track::Subtitle(Arc::new(sub_track)));

    // --- 导出 ---
    log::info!("=== Starting export ===");
    std::fs::create_dir_all("tmp")?;
    let output_path = PathBuf::from("tmp/subtitle_burn_test.mp4");

    let config = Mp4ExportConfig::default()
        .with_output_path(output_path.clone())
        .with_burn_subtitles(true)
        .with_fps(Some(25))
        .with_width(Some(1920))
        .with_height(Some(1080));

    let exporter = Mp4Exporter::new(manager, config);
    let result = exporter.export_with_progress(|p| {
        log::info!("[{:?}] {:.1}%", p.phase, p.progress() * 100.0);
    })?;

    log::info!("=== Export complete: {} frames ===", result.total_frames);
    log::info!("Output: {}", output_path.display());
    Ok(())
}
