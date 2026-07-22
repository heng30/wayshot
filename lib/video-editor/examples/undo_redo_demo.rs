//! Undo/Redo 功能演示示例
//!
//! 这个示例展示了如何使用 video-editor 的命令模式来实现撤销/重做功能。
//!
//! 运行方式：
//! ```sh
//! cargo run --example undo_redo_demo
//! ```

use std::sync::Arc;
use std::time::Duration;
use std::path::PathBuf;
use video_editor::{
    commands::{
        HistoryManager, AddTrackCommand,
        AddSegmentCommand, RemoveSegmentCommand,
        AddFilterCommand, RemoveFilterCommand,
        ToggleTrackVisibilityCommand,
    },
    Result,
    tracks::{Manager, Track, segment::Segment},
    filters::video::{FlipFilter, CropFilter},
    metadata::Metadata,
};

/// Helper to create test metadata
fn create_test_metadata() -> Arc<Metadata> {
    Arc::new(Metadata {
        path: PathBuf::from("/test/video.mp4"),
        size: 1024,
        bitrate: 5000000,
        duration: Duration::from_secs(10),
        format: vec!["mp4".to_string()],
        videos: vec![],
        audios: vec![],
        subtitles: vec![],
    })
}

fn main() -> Result<()> {
    println!("=== Video-Editor Undo/Redo 功能演示 ===\n");

    // 创建管理器和历史记录
    let mut manager = Manager::new();
    let mut history = HistoryManager::new().with_max_history(10);

    println!("✓ 创建管理器和历史记录 (最大历史: 10 条)\n");

    // ============================================
    // 演示 1: 添加和移除轨道
    // ============================================
    println!("\n--- 演示 1: 轨道操作 ---");
    println!("操作: 添加 3 个轨道");

    // 创建模拟轨道数据
    let metadata1 = create_test_metadata();
    let metadata2 = create_test_metadata();
    let metadata3 = create_test_metadata();

    let track1 = Track::Video(Arc::new(video_editor::tracks::video_track::VideoTrack {
        name: "Video Track 1".to_string(),
        hiding: false,
        muted: false,
        locked: false,
        track: video_editor::tracks::track::InnerTrack::new(metadata1.clone(), Duration::from_secs(10), vec![]),
    }));

    let track2 = Track::Audio(Arc::new(video_editor::tracks::audio_track::AudioTrack {
        name: "Audio Track 1".to_string(),
        hiding: false,
        locked: false,
        track: video_editor::tracks::track::InnerTrack::new(metadata2.clone(), Duration::from_secs(15), vec![]),
    }));

    let track3 = Track::Video(Arc::new(video_editor::tracks::video_track::VideoTrack {
        name: "Video Track 2".to_string(),
        hiding: false,
        muted: false,
        locked: false,
        track: video_editor::tracks::track::InnerTrack::new(metadata3.clone(), Duration::from_secs(8), vec![]),
    }));

    // 添加轨道
    history.execute(&mut manager, Box::new(AddTrackCommand::new(track1.clone())))?;
    println!("  → 添加轨道 1 (视频, 10秒)");
    history.execute(&mut manager, Box::new(AddTrackCommand::new(track2.clone())))?;
    println!("  → 添加轨道 2 (音频, 15秒)");
    history.execute(&mut manager, Box::new(AddTrackCommand::new(track3.clone())))?;
    println!("  → 添加轨道 3 (视频, 8秒)");

    println!("\n当前状态:");
    println!("  轨道数: {}", manager.len());
    println!("  可撤销: {}, 可重做: {}", history.can_undo(), history.can_redo());

    // 撤销添加轨道
    println!("\n操作: 撤销 2 次");
    for _i in 0..2 {
        let result = history.undo(&mut manager)?;
        println!("  ↻ 撤销: {}", result.description);
    }

    println!("\n当前状态:");
    println!("  轨道数: {}", manager.len());
    println!("  可撤销: {}, 可重做: {}", history.can_undo(), history.can_redo());

    // 重做
    println!("\n操作: 重做 1 次");
    let result = history.redo(&mut manager)?;
    println!("  ↻ 重做: {}", result.description);

    println!("\n当前状态:");
    println!("  轨道数: {}", manager.len());

    // ============================================
    // 演示 2: 片段操作
    // ============================================
    println!("\n--- 演示 2: 片段操作 ---");

    // 向第一个轨道添加片段
    let segment1 = Arc::new(Segment::new(
        Duration::ZERO,
        Duration::from_secs(3),
        metadata1.clone(),
        1.0,
    ));
    let segment2 = Arc::new(Segment::new(
        Duration::from_secs(3),
        Duration::from_secs(4),
        metadata1.clone(),
        1.0,
    ));

    println!("操作: 向轨道 0 添加 2 个片段");
    history.execute(&mut manager, Box::new(AddSegmentCommand::new(0, segment1.clone())))?;
    println!("  → 添加片段 1 (0-3秒)");
    history.execute(&mut manager, Box::new(AddSegmentCommand::new(0, segment2.clone())))?;
    println!("  → 添加片段 2 (3-7秒)");

    println!("\n当前状态:");
    if let Some(Track::Video(track)) = manager.get(0) {
        println!("  轨道 0 片段数: {}", track.track.segments.len());
    }
    println!("  可撤销: {}, 可重做: {}", history.can_undo(), history.can_redo());

    // 移除一个片段
    println!("\n操作: 移除片段 1");
    history.execute(&mut manager, Box::new(RemoveSegmentCommand::new(0, 0, true)))?;
    println!("  → 移除了索引 0 的片段");

    println!("\n当前状态:");
    if let Some(Track::Video(track)) = manager.get(0) {
        println!("  轨道 0 片段数: {}", track.track.segments.len());
    }
    println!("  可撤销: {}, 可重做: {}", history.can_undo(), history.can_redo());

    // 撤销和重做
    println!("\n操作: 撤销移除");
    let result = history.undo(&mut manager)?;
    println!("  ↻ 撤销: {}", result.description);

    println!("\n当前状态:");
    if let Some(Track::Video(track)) = manager.get(0) {
        println!("  轨道 0 片段数: {}", track.track.segments.len());
    }

    println!("\n操作: 重做");
    let result = history.redo(&mut manager)?;
    println!("  ↻ 重做: {}", result.description);

    println!("\n当前状态:");
    if let Some(Track::Video(track)) = manager.get(0) {
        println!("  轨道 0 片段数: {}", track.track.segments.len());
    }
    println!("  可撤销: {}, 可重做: {}", history.can_undo(), history.can_redo());

    // ============================================
    // 演示 3: 滤镜操作
    // ============================================
    println!("\n--- 演示 3: 滤镜操作 ---");

    // 添加滤镜
    let flip = Box::new(FlipFilter::default());
    let crop = Box::new(CropFilter::default());

    println!("操作: 向片段 0 添加 2 个滤镜");
    history.execute(&mut manager, Box::new(AddFilterCommand::new_video(0, 0, flip)))?;
    println!("  → 添加翻转滤镜");
    history.execute(&mut manager, Box::new(AddFilterCommand::new_video(0, 0, crop)))?;
    println!("  → 添加裁剪滤镜");

    println!("\n当前滤镜数:");
    if let Some(Track::Video(track)) = manager.get(0) {
        if let Some(seg) = track.track.segments.get(0) {
            println!("  片段 0 视频滤镜数: {}", seg.video_filters.len());
        }
    }

    // 移除一个滤镜
    println!("\n操作: 移除翻转滤镜");
    history.execute(&mut manager, Box::new(RemoveFilterCommand::new_video(0, 0, 0)))?;
    println!("  → 移除了索引 0 的滤镜");

    println!("\n当前滤镜数:");
    if let Some(Track::Video(track)) = manager.get(0) {
        if let Some(seg) = track.track.segments.get(0) {
            println!("  片段 0 视频滤镜数: {}", seg.video_filters.len());
        }
    }
    println!("  可撤销: {}, 可重做: {}", history.can_undo(), history.can_redo());

    // ============================================
    // 演示 4: 可见性切换
    // ============================================
    println!("\n--- 演示 4: 可见性操作 ---");

    println!("操作: 隐藏轨道 0");
    history.execute(&mut manager, Box::new(ToggleTrackVisibilityCommand::new(0)))?;
    println!("  → 切换轨道 0 可见性");

    println!("\n当前轨道状态:");
    if let Some(track) = manager.get(0) {
        println!("  轨道 0 隐藏: {}", track.is_hiding());
    }
    println!("  可撤销: {}, 可重做: {}", history.can_undo(), history.can_redo());

    // 撤销
    println!("\n操作: 撤销隐藏");
    let result = history.undo(&mut manager)?;
    println!("  ↻ 撤销: {}", result.description);

    println!("\n当前轨道状态:");
    if let Some(track) = manager.get(0) {
        println!("  轨道 0 隐藏: {}", track.is_hiding());
    }
    println!("  可撤销: {}, 可重做: {}", history.can_undo(), history.can_redo());

    // ============================================
    // 演示 5: 历史记录查看
    // ============================================
    println!("\n--- 演示 5: 历史记录 ---");

    println!("\n撤销历史 (最近到最早):");
    for (i, desc) in history.undo_history().iter().enumerate() {
        println!("  {}: {}", history.undo_count() - i, desc);
    }

    println!("\n重做历史 (最近到最早):");
    for (i, desc) in history.redo_history().iter().enumerate() {
        println!("  {}: {}", i, desc);
    }

    // ============================================
    // 总结
    // ============================================
    println!("\n=== 演示完成 ===");
    println!("总命令数: {}", history.undo_count());
    println!("可撤销操作数: {}", history.undo_count());
    println!("可重做操作数: {}", history.redo_count());

    println!("\n✓ 所有操作都可以正确地撤销和重做");

    Ok(())
}
