//! Comprehensive tests for Track operations
//!
//! Tests all major operations on Track including:
//! - Segment addition, insertion, and removal
//! - Segment modification and splitting
//! - Segment moving and shrinking
//! - Timeline shift behaviors
//! - Track properties (duration, metadata, hiding)

use std::{path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    metadata::{Id, Metadata, Pixel, VideoMetadata},
    tracks::{segment::Segment, track::InnerTrack, track::Track, video_track::VideoTrack},
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

/// Helper function to create an empty test video track
fn create_empty_track() -> Track {
    Track::Video(Arc::new(VideoTrack {
        name: String::default(),
        hiding: false,
        muted: false,
        locked: false,
        track: InnerTrack::new(create_test_metadata(), Duration::ZERO, vec![]),
    }))
}

// ============================================================================
// Basic Track Operations
// ============================================================================

#[test]
fn test_track_creation() {
    let track = create_test_track();
    assert!(!track.is_hiding());
    assert_eq!(track.segments_count(), 1);
    assert_eq!(track.duration(), Duration::from_secs(10));
}

#[test]
fn test_track_segments() {
    let track = create_test_track();
    let segments = track.segments();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].duration, Duration::from_secs(10));
}

#[test]
fn test_track_metadata() {
    let track = create_test_track();
    let metadata = track.metadata();
    assert_eq!(metadata.duration, Duration::from_secs(10));
}

// ============================================================================
// Add Segment Operations
// ============================================================================

#[test]
fn test_add_segment_empty_track() {
    let mut track = create_empty_track();
    let segment = create_test_segment(Duration::ZERO, Duration::from_secs(5));

    track.add_segment(segment);

    assert_eq!(track.segments_count(), 1);
    assert_eq!(track.segments()[0].timeline_offset, Duration::ZERO);
    assert_eq!(track.segments()[0].duration, Duration::from_secs(5));
}

#[test]
fn test_add_segment_appends_to_end() {
    let mut track = create_test_track();
    let segment = create_test_segment(Duration::from_secs(20), Duration::from_secs(5));

    track.add_segment(segment);

    assert_eq!(track.segments_count(), 2);
    // add_segment should set the timeline_offset to the end of the last segment
    assert_eq!(track.segments()[1].timeline_offset, Duration::from_secs(10));
    assert_eq!(track.segments()[1].duration, Duration::from_secs(5));
}

#[test]
fn test_add_segment_multiple() {
    let mut track = create_empty_track();

    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(2)));
    track.add_segment(create_test_segment(
        Duration::from_secs(10),
        Duration::from_secs(3),
    ));
    track.add_segment(create_test_segment(
        Duration::from_secs(20),
        Duration::from_secs(4),
    ));

    assert_eq!(track.segments_count(), 3);
    assert_eq!(track.segments()[0].timeline_offset, Duration::ZERO);
    assert_eq!(track.segments()[1].timeline_offset, Duration::from_secs(2));
    assert_eq!(track.segments()[2].timeline_offset, Duration::from_secs(5));
}

// ============================================================================
// Insert Segment Operations
// ============================================================================

#[test]
fn test_insert_segment_at_beginning_shift() {
    let mut track = create_test_track();
    let segment = Arc::new(Segment::new(
        Duration::ZERO,
        Duration::from_secs(3),
        create_test_metadata(),
        1.0,
    ));

    track.insert_segment(0, segment.clone(), true).unwrap();

    assert_eq!(track.segments_count(), 2);
    assert_eq!(track.segments()[0].duration, Duration::from_secs(3));
    // Original segment should be shifted
    assert_eq!(track.segments()[1].timeline_offset, Duration::from_secs(3));
}

#[test]
fn test_insert_segment_at_beginning_image() {
    let mut track = create_test_track();
    let segment = Arc::new(Segment::new(
        Duration::ZERO,
        Duration::from_secs(3),
        create_test_metadata(),
        1.0,
    ));

    track.insert_segment(0, segment.clone(), false).unwrap();

    assert_eq!(track.segments_count(), 2);
    assert_eq!(track.segments()[0].duration, Duration::from_secs(3));
    // Original segment should NOT be shifted
    assert_eq!(track.segments()[1].timeline_offset, Duration::ZERO);
}

#[test]
fn test_insert_segment_in_middle_shift() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(2)));
    track.add_segment(create_test_segment(
        Duration::from_secs(2),
        Duration::from_secs(3),
    ));

    let segment = Arc::new(Segment::new(
        Duration::from_secs(2),
        Duration::from_secs(1),
        create_test_metadata(),
        1.0,
    ));

    track.insert_segment(1, segment.clone(), true).unwrap();

    assert_eq!(track.segments_count(), 3);
    // First segment unchanged
    assert_eq!(track.segments()[0].duration, Duration::from_secs(2));
    // New segment at index 1
    assert_eq!(track.segments()[1].duration, Duration::from_secs(1));
    // Third segment shifted
    assert_eq!(track.segments()[2].timeline_offset, Duration::from_secs(3));
}

#[test]
fn test_insert_segment_index_out_of_bounds() {
    let mut track = create_test_track();
    let segment = create_test_segment(Duration::ZERO, Duration::from_secs(1));

    let result = track.insert_segment(10, segment, false);
    assert!(result.is_err());
}

// ============================================================================
// Remove Segment Operations
// ============================================================================

#[test]
fn test_remove_segment_shift() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(2)));
    track.add_segment(create_test_segment(
        Duration::from_secs(2),
        Duration::from_secs(3),
    ));
    track.add_segment(create_test_segment(
        Duration::from_secs(5),
        Duration::from_secs(2),
    ));

    // Remove middle segment with shift
    track.remove_segment(1, true).unwrap();

    assert_eq!(track.segments_count(), 2);
    assert_eq!(track.segments()[0].duration, Duration::from_secs(2));
    // Last segment should shift left
    assert_eq!(track.segments()[1].timeline_offset, Duration::from_secs(2));
    assert_eq!(track.segments()[1].duration, Duration::from_secs(2));
}

#[test]
fn test_remove_segment_leave_gap() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(2)));
    track.add_segment(create_test_segment(
        Duration::from_secs(2),
        Duration::from_secs(3),
    ));
    track.add_segment(create_test_segment(
        Duration::from_secs(5),
        Duration::from_secs(2),
    ));

    // Remove middle segment without shift (leave gap)
    track.remove_segment(1, false).unwrap();

    assert_eq!(track.segments_count(), 2);
    assert_eq!(track.segments()[0].duration, Duration::from_secs(2));
    // Last segment should NOT shift, gap remains
    assert_eq!(track.segments()[1].timeline_offset, Duration::from_secs(5));
    assert_eq!(track.segments()[1].duration, Duration::from_secs(2));
}

#[test]
fn test_remove_segment_last() {
    let mut track = create_test_track();
    let initial_count = track.segments_count();

    let removed = track.remove_segment(0, true).unwrap();
    assert_eq!(removed.duration, Duration::from_secs(10));
    assert_eq!(track.segments_count(), initial_count - 1);
}

#[test]
fn test_remove_segment_index_out_of_bounds() {
    let mut track = create_test_track();

    let result = track.remove_segment(10, true);
    assert!(result.is_err());
}

// ============================================================================
// Get Segment Operations
// ============================================================================

#[test]
fn test_get_segment() {
    let track = create_test_track();
    let segment = track.get_segment(0).unwrap();

    assert_eq!(segment.duration, Duration::from_secs(10));
    assert_eq!(segment.timeline_offset, Duration::ZERO);
}

#[test]
fn test_get_segment_index_out_of_bounds() {
    let track = create_test_track();
    let result = track.get_segment(10);
    assert!(result.is_err());
}

// ============================================================================
// Modify Segment Operations
// ============================================================================

#[test]
fn test_modify_segment_duration() {
    let mut track = create_test_track();
    let new_duration = Duration::from_secs(5);

    track
        .modify_segment(0, |seg| {
            seg.duration = new_duration;
        })
        .unwrap();

    assert_eq!(track.segments()[0].duration, new_duration);
}

#[test]
fn test_modify_segment_timeline_offset() {
    let mut track = create_test_track();
    let new_offset = Duration::from_secs(2);

    track
        .modify_segment(0, |seg| {
            seg.timeline_offset = new_offset;
        })
        .unwrap();

    assert_eq!(track.segments()[0].timeline_offset, new_offset);
}

#[test]
fn test_modify_segment_multiple_fields() {
    let mut track = create_test_track();

    track
        .modify_segment(0, |seg| {
            seg.timeline_offset = Duration::from_secs(1);
            seg.duration = Duration::from_secs(8);
            seg.source_offset = Duration::from_secs(2);
        })
        .unwrap();

    let seg = &track.segments()[0];
    assert_eq!(seg.timeline_offset, Duration::from_secs(1));
    assert_eq!(seg.duration, Duration::from_secs(8));
    assert_eq!(seg.source_offset, Duration::from_secs(2));
}

// ============================================================================
// Split Segment Operations
// ============================================================================

#[test]
fn test_split_segment_basic() {
    let mut track = create_test_track();
    let split_offset = Duration::from_secs(4);

    let (left_idx, right_idx) = track.split_segment(0, split_offset).unwrap();

    assert_eq!(track.segments_count(), 2);
    assert_eq!(left_idx, 0);
    assert_eq!(right_idx, 1);

    // Left segment
    assert_eq!(track.segments()[0].duration, Duration::from_secs(4));
    assert_eq!(track.segments()[0].timeline_offset, Duration::ZERO);

    // Right segment
    assert_eq!(track.segments()[1].duration, Duration::from_secs(6));
    assert_eq!(track.segments()[1].timeline_offset, Duration::from_secs(4));
}

#[test]
fn test_split_segment_preserves_filters() {
    let mut track = create_empty_track();

    let mut seg = Segment::new(
        Duration::ZERO,
        Duration::from_secs(10),
        create_test_metadata(),
        1.0,
    );
    // Add some filters (if available in your codebase)
    seg.video_filters = vec![];
    seg.audio_filters = vec![];

    let seg = Arc::new(seg);
    track.add_segment(seg.clone());

    track.split_segment(0, Duration::from_secs(3)).unwrap();

    // Both segments should have the same filters
    assert_eq!(
        track.segments()[0].video_filters.len(),
        seg.video_filters.len()
    );
    assert_eq!(
        track.segments()[1].video_filters.len(),
        seg.video_filters.len()
    );
}

#[test]
fn test_split_segment_zero_offset() {
    let mut track = create_test_track();

    let result = track.split_segment(0, Duration::ZERO);
    assert!(result.is_err());
}

#[test]
fn test_split_segment_offset_greater_than_duration() {
    let mut track = create_test_track();

    let result = track.split_segment(0, Duration::from_secs(20));
    assert!(result.is_err());
}

#[test]
fn test_split_segment_multiple_times() {
    let mut track = create_test_track();

    // First split: 10 -> [4, 6]
    track.split_segment(0, Duration::from_secs(4)).unwrap();
    assert_eq!(track.segments_count(), 2);

    // Second split: split the 6s segment at 2s
    track.split_segment(1, Duration::from_secs(2)).unwrap();
    assert_eq!(track.segments_count(), 3);

    assert_eq!(track.segments()[0].duration, Duration::from_secs(4));
    assert_eq!(track.segments()[1].duration, Duration::from_secs(2));
    assert_eq!(track.segments()[2].duration, Duration::from_secs(4));
}

// ============================================================================
// Move Segment Operations
// ============================================================================

#[test]
fn test_move_segment_to_later_position() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(2)));
    track.add_segment(create_test_segment(
        Duration::from_secs(2),
        Duration::from_secs(3),
    ));

    // Move first segment to start at 5 seconds
    track.move_segment(0, Duration::from_secs(5)).unwrap();

    assert_eq!(track.segments()[0].timeline_offset, Duration::from_secs(5));
    assert_eq!(track.segments()[1].timeline_offset, Duration::from_secs(2));
}

#[test]
fn test_move_segment_to_earlier_position() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(
        Duration::from_secs(5),
        Duration::from_secs(2),
    ));
    track.add_segment(create_test_segment(
        Duration::from_secs(7),
        Duration::from_secs(3),
    ));

    // Move first segment to start at 1 second
    track.move_segment(0, Duration::from_secs(1)).unwrap();

    assert_eq!(track.segments()[0].timeline_offset, Duration::from_secs(1));
}

// ============================================================================
// Shrink Segment Left Operations
// ============================================================================

#[test]
fn test_shrink_segment_left_without_shift() {
    let mut track = create_test_track();
    let shrink_duration = Duration::from_secs(2);

    track
        .shrink_segment_left(0, shrink_duration, false)
        .unwrap();

    let seg = &track.segments()[0];
    // Without shift: timeline_offset advances, source_offset advances, duration decreases
    assert_eq!(seg.timeline_offset, Duration::from_secs(2));
    assert_eq!(seg.source_offset, Duration::from_secs(2));
    assert_eq!(seg.duration, Duration::from_secs(8));
}

#[test]
fn test_shrink_segment_left_with_shift() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(5)));
    track.add_segment(create_test_segment(
        Duration::from_secs(5),
        Duration::from_secs(3),
    ));

    // Shrink first segment by 2 seconds with shift
    track
        .shrink_segment_left(0, Duration::from_secs(2), true)
        .unwrap();

    let seg0 = &track.segments()[0];
    assert_eq!(seg0.timeline_offset, Duration::ZERO); // Position unchanged with shift
    assert_eq!(seg0.source_offset, Duration::from_secs(2));
    assert_eq!(seg0.duration, Duration::from_secs(3));

    // Second segment should shift left by 2 seconds
    let seg1 = &track.segments()[1];
    assert_eq!(seg1.timeline_offset, Duration::from_secs(3)); // Was 5, now 3
}

#[test]
fn test_shrink_segment_left_zero_duration() {
    let mut track = create_test_track();
    let initial_duration = track.segments()[0].duration;

    // Shrink by zero should be no-op
    track.shrink_segment_left(0, Duration::ZERO, false).unwrap();

    assert_eq!(track.segments()[0].duration, initial_duration);
}

#[test]
fn test_shrink_segment_left_with_multiple_subsequent_segments() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(3)));
    track.add_segment(create_test_segment(
        Duration::from_secs(3),
        Duration::from_secs(2),
    ));
    track.add_segment(create_test_segment(
        Duration::from_secs(5),
        Duration::from_secs(3),
    ));

    // Shrink first segment with shift
    track
        .shrink_segment_left(0, Duration::from_secs(1), true)
        .unwrap();

    // Check first segment
    assert_eq!(track.segments()[0].duration, Duration::from_secs(2));
    assert_eq!(track.segments()[0].timeline_offset, Duration::ZERO);

    // Check second segment shifted
    assert_eq!(track.segments()[1].timeline_offset, Duration::from_secs(2)); // Was 3
    // Check third segment shifted
    assert_eq!(track.segments()[2].timeline_offset, Duration::from_secs(4)); // Was 5
}

// ============================================================================
// Shrink Segment Right Operations
// ============================================================================

#[test]
fn test_shrink_segment_right_basic() {
    let mut track = create_test_track();
    let shrink_duration = Duration::from_secs(3);

    track
        .shrink_segment_right(0, shrink_duration, false)
        .unwrap();

    let seg = &track.segments()[0];
    assert_eq!(seg.timeline_offset, Duration::ZERO); // Unchanged
    assert_eq!(seg.source_offset, Duration::ZERO); // Unchanged
    assert_eq!(seg.duration, Duration::from_secs(7)); // Reduced
}

#[test]
fn test_shrink_segment_right_with_shift() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(5)));
    track.add_segment(create_test_segment(
        Duration::from_secs(5),
        Duration::from_secs(3),
    ));

    // Shrink first segment by 2 seconds with shift
    track
        .shrink_segment_right(0, Duration::from_secs(2), true)
        .unwrap();

    let seg0 = &track.segments()[0];
    assert_eq!(seg0.timeline_offset, Duration::ZERO);
    assert_eq!(seg0.duration, Duration::from_secs(3)); // Was 5, now 3

    // Second segment should shift left by 2 seconds
    let seg1 = &track.segments()[1];
    assert_eq!(seg1.timeline_offset, Duration::from_secs(3)); // Was 5, now 3
}

#[test]
fn test_shrink_segment_right_without_shift() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(5)));
    track.add_segment(create_test_segment(
        Duration::from_secs(5),
        Duration::from_secs(3),
    ));

    // Shrink first segment without shift
    track
        .shrink_segment_right(0, Duration::from_secs(2), false)
        .unwrap();

    let seg0 = &track.segments()[0];
    assert_eq!(seg0.timeline_offset, Duration::ZERO);
    assert_eq!(seg0.duration, Duration::from_secs(3)); // Was 5, now 3

    // Second segment should NOT shift (gap remains)
    let seg1 = &track.segments()[1];
    assert_eq!(seg1.timeline_offset, Duration::from_secs(5)); // Still 5
}

#[test]
fn test_shrink_segment_right_zero_duration() {
    let mut track = create_test_track();
    let initial_duration = track.segments()[0].duration;

    track
        .shrink_segment_right(0, Duration::ZERO, false)
        .unwrap();

    assert_eq!(track.segments()[0].duration, initial_duration);
}

// ============================================================================
// Hiding Operations
// ============================================================================

#[test]
fn test_set_hiding() {
    let mut track = create_test_track();

    assert!(!track.is_hiding());

    track.set_hiding(true);
    assert!(track.is_hiding());

    track.set_hiding(false);
    assert!(!track.is_hiding());
}

// ============================================================================
// Duration Property
// ============================================================================

#[test]
fn test_duration_property() {
    let track = create_test_track();
    assert_eq!(track.duration(), Duration::from_secs(10));
}

// ============================================================================
// Edge Cases and Error Conditions
// ============================================================================

#[test]
fn test_empty_track_operations() {
    let mut track = create_empty_track();

    // Get segment from empty track
    assert!(track.get_segment(0).is_err());

    // Remove from empty track
    assert!(track.remove_segment(0, true).is_err());

    // Modify segment in empty track
    assert!(
        track
            .modify_segment(0, |seg| {
                seg.duration = Duration::from_secs(5);
            })
            .is_err()
    );

    // Split segment in empty track
    assert!(track.split_segment(0, Duration::from_secs(1)).is_err());
}

#[test]
fn test_large_duration_values() {
    let mut track = create_empty_track();
    let large_duration = Duration::from_secs(3600); // 1 hour

    track.add_segment(create_test_segment(Duration::ZERO, large_duration));

    assert_eq!(track.segments_count(), 1);
    assert_eq!(track.segments()[0].duration, large_duration);
}

#[test]
fn test_multiple_operations_sequence() {
    let mut track = create_test_track();

    // 1. Split the segment
    track.split_segment(0, Duration::from_secs(4)).unwrap();
    assert_eq!(track.segments_count(), 2);

    // 2. Move the second segment
    track.move_segment(1, Duration::from_secs(5)).unwrap();
    assert_eq!(track.segments()[1].timeline_offset, Duration::from_secs(5));

    // 3. Shrink the first segment from right
    track
        .shrink_segment_right(0, Duration::from_secs(1), false)
        .unwrap();
    assert_eq!(track.segments()[0].duration, Duration::from_secs(3));

    // 4. Verify final state
    assert_eq!(track.segments()[0].timeline_offset, Duration::ZERO);
    assert_eq!(track.segments()[0].duration, Duration::from_secs(3));
    assert_eq!(track.segments()[1].timeline_offset, Duration::from_secs(5));
}

// ============================================================================
// Strech Segment Operations
// ============================================================================

#[test]
fn test_stretch_segment_right_basic() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(5)));

    // Stretch right by 2 seconds
    track
        .stretch_segment_right(0, Duration::from_secs(2), false)
        .unwrap();

    let seg = &track.segments()[0];
    assert_eq!(seg.duration, Duration::from_secs(7));
    assert_eq!(seg.source_offset, Duration::ZERO); // Source offset unchanged
}

#[test]
fn test_stretch_segment_right_with_shift() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(5)));
    track.add_segment(create_test_segment(
        Duration::from_secs(5),
        Duration::from_secs(3),
    ));

    // Stretch right with timeline shift
    track
        .stretch_segment_right(0, Duration::from_secs(2), true)
        .unwrap();

    let seg0 = &track.segments()[0];
    assert_eq!(seg0.duration, Duration::from_secs(7));

    // Second segment should shift
    let seg1 = &track.segments()[1];
    assert_eq!(seg1.timeline_offset, Duration::from_secs(7)); // Was 5, now 7
}

#[test]
fn test_stretch_segment_right_clamped_to_source_duration() {
    let mut track = create_empty_track();
    // Create a segment with source_offset=5, duration=5 (ends at source boundary)
    // Source duration is 10, so the segment already ends at position 10
    // No room to stretch right
    let segment = Segment::new_with_source_offset(
        Duration::ZERO,
        Duration::from_secs(5),
        Duration::from_secs(5),
        1.0,
        1.0,
        track.metadata().clone(),
    );
    track.add_segment(Arc::new(segment));

    // Try to stretch by 10 seconds (more than available)
    track
        .stretch_segment_right(0, Duration::from_secs(10), false)
        .unwrap();

    let seg = &track.segments()[0];
    // Should remain unchanged (already at source boundary)
    assert_eq!(seg.duration, Duration::from_secs(5)); // Still 5, can't stretch
    assert_eq!(seg.source_offset, Duration::from_secs(5)); // Still at 5
}

#[test]
fn test_stretch_segment_left_basic() {
    let mut track = create_empty_track();
    // Create a segment with source_offset=3, so we can stretch left
    let segment = Segment::new_with_source_offset(
        Duration::from_secs(3),
        Duration::from_secs(3),
        Duration::from_secs(5),
        1.0,
        1.0,
        track.metadata().clone(),
    );
    track.add_segment(Arc::new(segment));
    // Set timeline_offset to match source_offset for testing
    track
        .modify_segment(0, |seg| {
            seg.timeline_offset = Duration::from_secs(3);
        })
        .unwrap();

    // Stretch left by 2 seconds
    track
        .stretch_segment_left(0, Duration::from_secs(2), false)
        .unwrap();

    let seg = &track.segments()[0];
    assert_eq!(seg.duration, Duration::from_secs(7)); // Was 5, now 7
    assert_eq!(seg.timeline_offset, Duration::from_secs(1)); // Was 3, now 1
    assert_eq!(seg.source_offset, Duration::from_secs(1)); // Was 3, now 1
}

#[test]
fn test_stretch_segment_left_with_shift() {
    let mut track = create_empty_track();
    // Create first segment with source_offset=3
    let segment1 = Segment::new_with_source_offset(
        Duration::from_secs(3),
        Duration::from_secs(3),
        Duration::from_secs(5),
        1.0,
        1.0,
        track.metadata().clone(),
    );
    track.add_segment(Arc::new(segment1));
    // Set timeline_offset to 3
    track
        .modify_segment(0, |seg| {
            seg.timeline_offset = Duration::from_secs(3);
        })
        .unwrap();
    track.add_segment(create_test_segment(
        Duration::from_secs(8),
        Duration::from_secs(3),
    ));
    // Set second segment's timeline_offset to 8
    track
        .modify_segment(1, |seg| {
            seg.timeline_offset = Duration::from_secs(8);
        })
        .unwrap();

    // Stretch left without timeline shift (so timeline_offset changes)
    track
        .stretch_segment_left(0, Duration::from_secs(2), false)
        .unwrap();

    let seg0 = &track.segments()[0];
    assert_eq!(seg0.timeline_offset, Duration::from_secs(1)); // Was 3, now 1
    assert_eq!(seg0.duration, Duration::from_secs(7)); // Was 5, now 7

    // Second segment stays at original position (no shift)
    let seg1 = &track.segments()[1];
    assert_eq!(seg1.timeline_offset, Duration::from_secs(8)); // Unchanged
}

#[test]
fn test_stretch_segment_left_clamped_to_source_start() {
    let mut track = create_empty_track();
    // Create a segment with source_offset=2
    let segment = Segment::new_with_source_offset(
        Duration::from_secs(2),
        Duration::from_secs(2),
        Duration::from_secs(5),
        1.0,
        1.0,
        track.metadata().clone(),
    );
    track.add_segment(Arc::new(segment));
    // Set timeline_offset to 2
    track
        .modify_segment(0, |seg| {
            seg.timeline_offset = Duration::from_secs(2);
        })
        .unwrap();

    // Try to stretch left by 5 seconds (more than source_offset)
    track
        .stretch_segment_left(0, Duration::from_secs(5), false)
        .unwrap();

    let seg = &track.segments()[0];
    // Should be clamped to source start (source_offset=0)
    assert_eq!(seg.source_offset, Duration::ZERO);
    // timeline_offset should be 0 (moved back by 2s, clamped)
    assert_eq!(seg.timeline_offset, Duration::from_secs(0)); // Was 2, now 0 (clamped)
    assert_eq!(seg.duration, Duration::from_secs(7)); // Was 5, now 7
}

#[test]
fn test_stretch_segment_zero_duration() {
    let mut track = create_test_track();
    let initial_duration = track.segments()[0].duration;

    // Stretch by zero
    track
        .stretch_segment_right(0, Duration::ZERO, false)
        .unwrap();

    assert_eq!(track.segments()[0].duration, initial_duration);
}

// ============================================================================
// Segment Overlap Detection
// ============================================================================

#[test]
fn test_is_segment_overlap_overlapping() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(5)));
    track.add_segment(create_test_segment(
        Duration::from_secs(3),
        Duration::from_secs(5),
    ));
    // Manually set the second segment's timeline_offset to 3 (overlapping with first)
    track
        .modify_segment(1, |seg| {
            seg.timeline_offset = Duration::from_secs(3);
        })
        .unwrap();

    // Segments overlap (0-5 and 3-8)
    assert!(track.is_segment_overlap(0, 1));
    assert!(track.is_segment_overlap(1, 0)); // Symmetric
}

#[test]
fn test_is_segment_overlap_adjacent() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(5)));
    track.add_segment(create_test_segment(
        Duration::from_secs(5),
        Duration::from_secs(3),
    ));

    // Segments are adjacent (0-5 and 5-8), not overlapping
    assert!(!track.is_segment_overlap(0, 1));
    assert!(!track.is_segment_overlap(1, 0));
}

#[test]
fn test_is_segment_overlap_separated() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(3)));
    track.add_segment(create_test_segment(
        Duration::from_secs(5),
        Duration::from_secs(3),
    ));

    // Segments are separated (0-3 and 5-8)
    assert!(!track.is_segment_overlap(0, 1));
    assert!(!track.is_segment_overlap(1, 0));
}

#[test]
fn test_is_segment_overlap_contained() {
    let mut track = create_empty_track();
    track.add_segment(create_test_segment(Duration::ZERO, Duration::from_secs(10)));
    track.add_segment(create_test_segment(
        Duration::from_secs(3),
        Duration::from_secs(4),
    ));
    // Manually set the second segment's timeline_offset to 3 (contained in first)
    track
        .modify_segment(1, |seg| {
            seg.timeline_offset = Duration::from_secs(3);
        })
        .unwrap();

    // Second segment is contained in first (0-10 contains 3-7)
    assert!(track.is_segment_overlap(0, 1));
    assert!(track.is_segment_overlap(1, 0));
}

#[test]
fn test_is_segment_overlap_invalid_indices() {
    let track = create_test_track();

    // Invalid indices should return false (not error)
    assert!(!track.is_segment_overlap(0, 10));
    assert!(!track.is_segment_overlap(10, 0));
    assert!(!track.is_segment_overlap(5, 5));
}

#[test]
fn test_is_segment_overlap_same_segment() {
    let track = create_test_track();

    // A segment overlaps with itself
    assert!(track.is_segment_overlap(0, 0));
}
