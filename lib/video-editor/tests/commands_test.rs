//! Comprehensive tests for Command pattern implementation
//!
//! Tests include:
//! - Batch commands
//! - History management
//! - Track commands (Add, Remove, Move)
//! - Segment commands (Add, Insert, Remove, Split, Move, Shrink)
//! - Filter commands structure tests
//! - Visibility commands describe tests

use std::{path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    commands::{
        BatchCommand,
        HistoryManager,
        Command,
        ToggleTrackVisibilityCommand,
        ToggleSegmentVisibilityCommand,
        SetTrackVisibilityCommand,
        SetSegmentVisibilityCommand,
        AddTrackCommand,
        RemoveTrackCommand,
        MoveTrackCommand,
        AddSegmentCommand,
        InsertSegmentCommand,
        RemoveSegmentCommand,
        SplitSegmentCommand,
        MoveSegmentCommand,
        ShrinkSegmentLeftCommand,
        ShrinkSegmentRightCommand,
        StretchSegmentLeftCommand,
        StretchSegmentRightCommand,
        FilterType,
        AddFilterCommand,
        RemoveFilterCommand,
        ClearFiltersCommand,
        MoveFilterCommand,
    },
    metadata::{Id, Metadata, Pixel, VideoMetadata},
    tracks::Manager,
    tracks::Track,
    tracks::segment::Segment,
    tracks::track::InnerTrack,
    tracks::video_track::VideoTrack,
};

/// Helper function to create test metadata
fn create_test_metadata() -> Arc<Metadata> {
    Arc::new(Metadata {
        path: PathBuf::from("/test/video.mp4"),
        size: 1024,
        bitrate: 5000000,
        duration: Duration::from_secs(10),
        format: vec!["mp4".to_string()],
        videos: vec![VideoMetadata {
            index: 0,
            codec_id: Id::H264,
            pix_fmt: Pixel::YUV420P,
            width: 1920,
            height: 1080,
            fps: 24.0,
            language: None,
            duration: Duration::from_secs(10),
        }],
        audios: vec![],
        subtitles: vec![],
    })
}

/// Helper function to create a test segment
fn create_test_segment(offset: Duration, duration: Duration) -> Arc<Segment> {
    Arc::new(Segment::new(offset, duration, create_test_metadata(), 1.0))
}

/// Helper function to create a test video track with a single segment
fn create_test_track() -> Track {
    Track::Video(Arc::new(VideoTrack {
        name: String::default(),
        hiding: false,
        muted: false,
        locked: false,
        track: InnerTrack::new(
            create_test_metadata(),
            Duration::from_secs(10),
            vec![create_test_segment(Duration::ZERO, Duration::from_secs(10))],
        ),
    }))
}

/// Helper function to create a track with two segments
fn create_test_track_with_two_segments() -> Track {
    let seg1 = create_test_segment(Duration::ZERO, Duration::from_secs(5));
    let seg2 = create_test_segment(Duration::from_secs(5), Duration::from_secs(5));
    Track::Video(Arc::new(VideoTrack {
        name: String::default(),
        hiding: false,
        muted: false,
        locked: false,
        track: InnerTrack::new(
            create_test_metadata(),
            Duration::from_secs(10),
            vec![seg1, seg2],
        ),
    }))
}

// ============================================================================
// Batch Command Tests
// ============================================================================

#[test]
fn test_batch_command_creation() {
    let batch = BatchCommand::new("Test batch operation".to_string());
    assert!(batch.is_empty());
    assert_eq!(batch.len(), 0);
    assert_eq!(batch.describe(), "Batch operation: Test batch operation");
}

#[test]
fn test_batch_command_with_commands() {
    let mut batch = BatchCommand::new("Multi operation".to_string());
    let segment = create_test_segment(Duration::from_secs(10), Duration::from_secs(5));
    batch.add_command(Box::new(AddSegmentCommand::new(0, segment)));

    assert_eq!(batch.len(), 1);
    assert!(!batch.is_empty());
}

// ============================================================================
// History Manager Tests
// ============================================================================

#[test]
fn test_history_manager_initial_state() {
    let history = HistoryManager::new();
    assert!(!history.can_undo());
    assert!(!history.can_redo());
    assert_eq!(history.undo_count(), 0);
    assert_eq!(history.redo_count(), 0);
}

#[test]
fn test_history_manager_max_size() {
    let history = HistoryManager::new().with_max_history(10);
    assert_eq!(history.undo_count(), 0);

    let history = HistoryManager::new().with_max_history(0);
    assert_eq!(history.undo_count(), 0);
}

#[test]
fn test_history_manager_clear() {
    let mut history = HistoryManager::new();
    history.clear();
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn test_history_manager_can_undo() {
    let mut manager = Manager::new();
    manager.add_track(create_test_track());

    let mut history = HistoryManager::new();

    let segment = create_test_segment(Duration::from_secs(10), Duration::from_secs(5));
    let cmd = AddSegmentCommand::new(0, segment);

    history.execute(&mut manager, Box::new(cmd)).unwrap();
    assert!(history.can_undo());
    assert_eq!(history.undo_count(), 1);

    // Undo should restore state
    history.undo(&mut manager).unwrap();
    assert!(!history.can_undo());
    assert!(history.can_redo());
    assert_eq!(history.redo_count(), 1);
}

// ============================================================================
// Track Command Tests
// ============================================================================

#[test]
fn test_add_track_command_describe() {
    let track = create_test_track();
    let cmd = AddTrackCommand::new(track.clone());
    assert_eq!(cmd.describe(), "Add track");
}

#[test]
fn test_remove_track_command_describe() {
    let cmd = RemoveTrackCommand::new(0);
    assert_eq!(cmd.describe(), "Remove track 0");
}

#[test]
fn test_move_track_command_describe() {
    let cmd = MoveTrackCommand::new(0, 2);
    assert_eq!(cmd.describe(), "Move track 0 -> 2");
}

#[test]
fn test_add_track_command_execute() {
    let mut manager = Manager::new();
    let track = create_test_track();

    let mut cmd = AddTrackCommand::new(track);
    cmd.execute(&mut manager).unwrap();

    assert_eq!(manager.len(), 1);
}

#[test]
fn test_remove_track_command_execute() {
    let mut manager = Manager::new();
    manager.add_track(create_test_track());

    let mut cmd = RemoveTrackCommand::new(0);
    cmd.execute(&mut manager).unwrap();

    assert_eq!(manager.len(), 0);
}

#[test]
fn test_move_track_command_execute() {
    let mut manager = Manager::new();
    manager.add_track(create_test_track());
    manager.add_track(create_test_track());

    let mut cmd = MoveTrackCommand::new(0, 1);
    cmd.execute(&mut manager).unwrap();

    assert_eq!(manager.len(), 2);
}

// ============================================================================
// Segment Command Tests
// ============================================================================

#[test]
fn test_add_segment_command_describe() {
    let segment = create_test_segment(Duration::ZERO, Duration::from_secs(5));
    let cmd = AddSegmentCommand::new(0, segment);
    assert_eq!(cmd.describe(), "Add segment to track 0");
}

#[test]
fn test_insert_segment_command_describe() {
    let segment = create_test_segment(Duration::ZERO, Duration::from_secs(5));
    let cmd = InsertSegmentCommand::new(0, 1, segment);
    assert_eq!(cmd.describe(), "Insert segment at index 1 in track 0");
}

#[test]
fn test_remove_segment_command_describe() {
    let cmd = RemoveSegmentCommand::new(0, 1, true);
    assert_eq!(cmd.describe(), "Remove segment at index 1 from track 0");
}

#[test]
fn test_split_segment_command_describe() {
    let split_time = Duration::from_secs(4);
    let cmd = SplitSegmentCommand::new(0, 0, split_time);
    assert_eq!(cmd.describe(), "Split track 0 segment 0 at 4s");
}

#[test]
fn test_move_segment_command_describe() {
    let cmd = MoveSegmentCommand::new(0, 0, 1);
    assert_eq!(cmd.describe(), "Move segment 0 -> 1 in track 0");
}

#[test]
fn test_shrink_segment_left_command_describe() {
    let shrink_duration = Duration::from_secs(2);
    let cmd = ShrinkSegmentLeftCommand::new(0, 0, shrink_duration, false);
    assert_eq!(cmd.describe(), "Shrink left side of track 0 segment 0 by 2s");
}

#[test]
fn test_shrink_segment_right_command_describe() {
    let shrink_duration = Duration::from_secs(3);
    let cmd = ShrinkSegmentRightCommand::new(0, 0, shrink_duration, false);
    assert_eq!(cmd.describe(), "Shrink right side of track 0 segment 0 by 3s");
}

#[test]
fn test_add_segment_command_execute() {
    let mut manager = Manager::new();
    manager.add_track(create_test_track());

    let segment = create_test_segment(Duration::from_secs(10), Duration::from_secs(5));
    let mut cmd = AddSegmentCommand::new(0, segment);

    cmd.execute(&mut manager).unwrap();
    let track = manager.get(0).unwrap();
    assert_eq!(track.segments_count(), 2);
}

#[test]
fn test_insert_segment_command_execute() {
    let mut manager = Manager::new();
    manager.add_track(create_test_track());

    let segment = create_test_segment(Duration::from_secs(5), Duration::from_secs(3));
    let mut cmd = InsertSegmentCommand::new(0, 1, segment);

    cmd.execute(&mut manager).unwrap();
    let track = manager.get(0).unwrap();
    assert_eq!(track.segments_count(), 2);
}

#[test]
fn test_remove_segment_command_execute() {
    let mut manager = Manager::new();
    manager.add_track(create_test_track_with_two_segments());

    let mut cmd = RemoveSegmentCommand::new(0, 1, true);
    cmd.execute(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    assert_eq!(track.segments_count(), 1);
}

#[test]
fn test_split_segment_command_execute() {
    let mut manager = Manager::new();
    manager.add_track(create_test_track());

    let split_time = Duration::from_secs(4);
    let mut cmd = SplitSegmentCommand::new(0, 0, split_time);

    cmd.execute(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    assert_eq!(track.segments_count(), 2);

    let seg0 = track.get_segment(0).unwrap();
    let seg1 = track.get_segment(1).unwrap();
    assert_eq!(seg0.duration, Duration::from_secs(4));
    assert_eq!(seg1.duration, Duration::from_secs(6));
}

#[test]
fn test_shrink_segment_left_command_execute() {
    let mut manager = Manager::new();
    manager.add_track(create_test_track());

    let shrink_duration = Duration::from_secs(2);
    let mut cmd = ShrinkSegmentLeftCommand::new(0, 0, shrink_duration, false);

    cmd.execute(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    let segment = track.get_segment(0).unwrap();
    assert_eq!(segment.timeline_offset, Duration::from_secs(2));
    assert_eq!(segment.duration, Duration::from_secs(8));
}

#[test]
fn test_shrink_segment_right_command_execute() {
    let mut manager = Manager::new();
    manager.add_track(create_test_track());

    let shrink_duration = Duration::from_secs(3);
    let mut cmd = ShrinkSegmentRightCommand::new(0, 0, shrink_duration, false);

    cmd.execute(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    let segment = track.get_segment(0).unwrap();
    assert_eq!(segment.duration, Duration::from_secs(7));
}

// ============================================================================
// Visibility Command Tests
// ============================================================================

#[test]
fn test_toggle_track_visibility_command_describe() {
    let cmd = ToggleTrackVisibilityCommand::new(0);
    assert_eq!(cmd.describe(), "Toggle track 0 visibility");
}

#[test]
fn test_toggle_segment_visibility_command_describe() {
    let cmd = ToggleSegmentVisibilityCommand::new(0, 1);
    assert_eq!(cmd.describe(), "Toggle track 0 segment 1 visibility");
}

#[test]
fn test_set_track_visibility_command_describe() {
    let cmd_show = SetTrackVisibilityCommand::new(0, false);
    assert_eq!(cmd_show.describe(), "Show track 0");

    let cmd_hide = SetTrackVisibilityCommand::new(1, true);
    assert_eq!(cmd_hide.describe(), "Hide track 1");
}

#[test]
fn test_set_segment_visibility_command_describe() {
    let cmd_show = SetSegmentVisibilityCommand::new(0, 1, false);
    assert_eq!(cmd_show.describe(), "Show track 0 segment 1");

    let cmd_hide = SetSegmentVisibilityCommand::new(2, 0, true);
    assert_eq!(cmd_hide.describe(), "Hide track 2 segment 0");
}

// ============================================================================
// Filter Command Tests
// ============================================================================

#[test]
fn test_filter_type() {
    let _ = FilterType::Video;
    let _ = FilterType::Audio;
    let _ = FilterType::Subtitle;
}

#[test]
fn test_add_filter_command_structure() {
    struct TestVideoFilter;
    impl video_editor::filters::traits::VideoFilter for TestVideoFilter {
        fn name(&self) -> &str { "test" }

        fn as_any(&self) -> &(dyn std::any::Any + 'static) { self }

        fn clone_box(&self) -> Box<dyn video_editor::filters::traits::VideoFilter + 'static> {
            Box::new(TestVideoFilter)
        }

        fn apply(
            &self,
            _data: &mut video_editor::filters::traits::VideoData,
        ) -> video_editor::Result<()> {
            Ok(())
        }
    }

    let _cmd = AddFilterCommand::new_video(0, 0, Box::new(TestVideoFilter));
}

#[test]
fn test_remove_filter_command_structure() {
    let _cmd = RemoveFilterCommand::new_video(0, 0, 0);
}

#[test]
fn test_clear_filters_command_structure() {
    let _cmd = ClearFiltersCommand::new_video(0, 0);
}

#[test]
fn test_move_filter_command_structure() {
    let _cmd = MoveFilterCommand::new(0, 0, FilterType::Video, 0, 1);
}

// ============================================================================
// Stretch Segment Command Tests
// ============================================================================

#[test]
fn test_stretch_segment_left_command_describe() {
    let cmd = StretchSegmentLeftCommand::new(0, 0, Duration::from_secs(2), false);
    assert!(cmd.describe().contains("Left stretch"));
    assert!(cmd.describe().contains("2s"));
}

#[test]
fn test_stretch_segment_left_command_execute() {
    let mut manager = Manager::new();
    let mut track = create_test_track();

    // Modify first segment to have source_offset=3, enabling left stretch
    track.modify_segment(0, |seg| {
        seg.source_offset = Duration::from_secs(3);
        seg.timeline_offset = Duration::from_secs(3);
    }).unwrap();

    manager.add_track(track);

    let stretch_duration = Duration::from_secs(2);
    let mut cmd = StretchSegmentLeftCommand::new(0, 0, stretch_duration, false);

    cmd.execute(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    let segment = track.get_segment(0).unwrap();
    assert_eq!(segment.duration, Duration::from_secs(12)); // Was 10, now 12
    assert_eq!(segment.source_offset, Duration::from_secs(1)); // Was 3, now 1
    assert_eq!(segment.timeline_offset, Duration::from_secs(1)); // Was 3, now 1 (stretched left)
}

#[test]
fn test_stretch_segment_left_command_undo() {
    let mut manager = Manager::new();
    let mut track = create_test_track();

    // Modify segment to enable left stretch
    track.modify_segment(0, |seg| {
        seg.source_offset = Duration::from_secs(3);
        seg.timeline_offset = Duration::from_secs(3);
    }).unwrap();

    manager.add_track(track);

    let stretch_duration = Duration::from_secs(2);
    let mut cmd = StretchSegmentLeftCommand::new(0, 0, stretch_duration, false);

    // Execute
    cmd.execute(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    let segment = track.get_segment(0).unwrap();
    let stretched_duration = segment.duration;
    assert_ne!(stretched_duration, Duration::from_secs(10));

    // Undo
    cmd.undo(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    let segment = track.get_segment(0).unwrap();
    assert_eq!(segment.duration, Duration::from_secs(10)); // Back to original
    assert_eq!(segment.source_offset, Duration::from_secs(3)); // Back to original
    assert_eq!(segment.timeline_offset, Duration::from_secs(3)); // Back to original
}

#[test]
fn test_stretch_segment_left_command_with_shift() {
    let mut manager = Manager::new();
    let mut track = create_test_track();

    // Modify first segment to have source_offset=3
    track.modify_segment(0, |seg| {
        seg.source_offset = Duration::from_secs(3);
        seg.timeline_offset = Duration::from_secs(3);
    }).unwrap();

    track.split_segment(0, Duration::from_secs(5)).unwrap();
    manager.add_track(track);

    // Use shift_timeline=false so that timeline_offset changes
    let mut cmd = StretchSegmentLeftCommand::new(0, 0, Duration::from_secs(2), false);
    cmd.execute(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    // First segment stretched left (timeline_offset decreased)
    let seg0 = track.get_segment(0).unwrap();
    assert_eq!(seg0.duration, Duration::from_secs(7)); // 5 + 2
    assert_eq!(seg0.timeline_offset, Duration::from_secs(1)); // 3 - 2 = 1 (stretched left)

    // Second segment stays at original position (no shift since shift_timeline=false)
    let seg1 = track.get_segment(1).unwrap();
    assert_eq!(seg1.timeline_offset, Duration::from_secs(8)); // Unchanged

    // Undo
    cmd.undo(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    let seg0 = track.get_segment(0).unwrap();
    assert_eq!(seg0.duration, Duration::from_secs(5)); // Back to original
    assert_eq!(seg0.timeline_offset, Duration::from_secs(3)); // Back to original
}

#[test]
fn test_stretch_segment_right_command_describe() {
    let cmd = StretchSegmentRightCommand::new(0, 0, Duration::from_secs(3), true);
    assert!(cmd.describe().contains("Right stretch"));
    assert!(cmd.describe().contains("3s"));
}

#[test]
fn test_stretch_segment_right_command_execute() {
    let mut manager = Manager::new();
    let mut track = create_test_track();

    // Modify segment to have room for stretching (source_offset=0, duration=8, end at 8)
    track.modify_segment(0, |seg| {
        seg.duration = Duration::from_secs(8);
        seg.original_duration = Duration::from_secs(8);
    }).unwrap();

    manager.add_track(track);

    let stretch_duration = Duration::from_secs(2);
    let mut cmd = StretchSegmentRightCommand::new(0, 0, stretch_duration, false);

    cmd.execute(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    let segment = track.get_segment(0).unwrap();
    assert_eq!(segment.duration, Duration::from_secs(10)); // Was 8, now 10
}

#[test]
fn test_stretch_segment_right_command_undo() {
    let mut manager = Manager::new();
    let mut track = create_test_track();

    // Modify segment to have room for stretching
    track.modify_segment(0, |seg| {
        seg.duration = Duration::from_secs(8);
        seg.original_duration = Duration::from_secs(8);
    }).unwrap();

    manager.add_track(track);

    let stretch_duration = Duration::from_secs(2);
    let mut cmd = StretchSegmentRightCommand::new(0, 0, stretch_duration, false);

    // Execute
    cmd.execute(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    let segment = track.get_segment(0).unwrap();
    let stretched_duration = segment.duration;
    assert_ne!(stretched_duration, Duration::from_secs(8));

    // Undo
    cmd.undo(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    let segment = track.get_segment(0).unwrap();
    assert_eq!(segment.duration, Duration::from_secs(8)); // Back to original
}

#[test]
fn test_stretch_segment_right_command_with_shift() {
    let mut manager = Manager::new();
    let mut track = create_test_track();

    // Modify segment to have room for stretching
    track.modify_segment(0, |seg| {
        seg.duration = Duration::from_secs(8);
        seg.original_duration = Duration::from_secs(8);
    }).unwrap();

    track.split_segment(0, Duration::from_secs(4)).unwrap();
    manager.add_track(track);

    let mut cmd = StretchSegmentRightCommand::new(0, 0, Duration::from_secs(2), true);
    cmd.execute(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    // First segment stretched
    let seg0 = track.get_segment(0).unwrap();
    assert_eq!(seg0.duration, Duration::from_secs(6)); // Was 4, now 6

    // Second segment should shift forward by 2s
    let seg1 = track.get_segment(1).unwrap();
    assert_eq!(seg1.timeline_offset, Duration::from_secs(6)); // Was 4, now 6 (shifted by 2s)

    // Undo
    cmd.undo(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    let seg0 = track.get_segment(0).unwrap();
    assert_eq!(seg0.duration, Duration::from_secs(4)); // Back to original

    let seg1 = track.get_segment(1).unwrap();
    assert_eq!(seg1.timeline_offset, Duration::from_secs(4)); // Back to original
}

#[test]
fn test_stretch_segment_command_clamping() {
    let mut manager = Manager::new();
    let mut track = create_test_track();

    // Modify first segment to have limited room for stretching
    // source_offset=8, duration=2, so end is at 10 (source duration)
    track.modify_segment(0, |seg| {
        seg.source_offset = Duration::from_secs(8);
        seg.duration = Duration::from_secs(2);
    }).unwrap();

    manager.add_track(track);

    // Try to stretch more than available (only 0s available since segment ends at source boundary)
    let mut cmd = StretchSegmentRightCommand::new(0, 0, Duration::from_secs(5), false);
    cmd.execute(&mut manager).unwrap();

    let track = manager.get(0).unwrap();
    let segment = track.get_segment(0).unwrap();

    // Should remain unchanged (already at source duration)
    assert_eq!(segment.duration, Duration::from_secs(2)); // Still 2, can't stretch
    assert_eq!(segment.source_offset, Duration::from_secs(8)); // Still at 8
}

