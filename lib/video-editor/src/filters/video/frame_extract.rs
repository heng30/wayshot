use crate::{
    Result,
    filters::traits::{VideoData, VideoFilter},
    tracks::video_frame_cache::VideoImage,
};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Frame extraction filter (抽帧滤镜)
///
/// Extracts one frame every N frames and fills intermediate frames with
/// the extracted frame, creating a stop-motion / stutter effect without
/// changing playback speed.
///
/// Uses the timeline-relative time offset to determine the current "extract
/// slot", so it works correctly with resize, seek, and mid-playback start.
///
/// # Parameters
/// - `frame_interval`: Extract one frame every N frames (e.g., 5 means keep
///   frame 0, 5, 10, ... and fill frames 1-4, 6-9, ... with the extracted frame).
///   Must be >= 1. A value of 1 means no extraction (every frame is kept).
///
/// # Example
/// With `frame_interval = 3` at 30fps:
/// - t=0ms   → slot 0 (extracted, save copy)
/// - t=33ms  → slot 0 (fill, use copy)
/// - t=66ms  → slot 0 (fill, use copy)
/// - t=100ms → slot 1 (extracted, save copy)
/// - t=133ms → slot 1 (fill, use copy)
/// - ...
#[derive(Debug, Serialize, Deserialize, derivative::Derivative)]
#[derivative(Default)]
pub struct FrameExtractFilter {
    /// Extract one frame every N frames. Must be >= 1.
    /// Default is 10 (no extraction, all frames kept).
    #[derivative(Default(value = "10"))]
    pub frame_interval: u32,

    /// The slot index of the currently saved extracted frame.
    /// `None` means no copy has been saved yet.
    #[serde(skip)]
    saved_slot: Mutex<Option<u64>>,

    /// Saved copy of the last extracted frame.
    #[serde(skip)]
    extracted_frame: Mutex<Option<RgbaImage>>,
}

impl Clone for FrameExtractFilter {
    fn clone(&self) -> Self {
        Self {
            frame_interval: self.frame_interval,
            saved_slot: Mutex::new(None),
            extracted_frame: Mutex::new(None),
        }
    }
}

impl FrameExtractFilter {
    pub const NAME: &'static str = "frame extract";

    pub fn new(frame_interval: u32) -> Self {
        Self {
            frame_interval: frame_interval.max(1),
            saved_slot: Mutex::new(None),
            extracted_frame: Mutex::new(None),
        }
    }

    /// Compute the extract slot index for a given time offset and fps.
    ///
    /// The slot is `floor(time_s * fps / frame_interval)`.
    /// All frames within the same slot share the same extracted frame.
    fn time_to_slot(relative_offset: std::time::Duration, fps: f32, frame_interval: u32) -> u64 {
        let time_s = relative_offset.as_secs_f64();
        let frame_index = (time_s * fps as f64).floor() as u64;
        frame_index / frame_interval as u64
    }

    /// Returns true if the filter actually has an effect (frame_interval > 1).
    pub fn is_active(&self) -> bool {
        self.frame_interval > 1
    }

    /// Reset internal state (saved slot and saved frame).
    /// Should be called when seeking or restarting playback.
    pub fn reset(&self) {
        *self.saved_slot.lock().unwrap() = None;
        *self.extracted_frame.lock().unwrap() = None;
    }
}

impl VideoFilter for FrameExtractFilter {
    crate::impl_default_video_filter!(FrameExtractFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        if self.frame_interval <= 1 {
            return Ok(());
        }

        let current_slot = Self::time_to_slot(
            data.relative_timeline_offset,
            data.config.output_fps,
            self.frame_interval,
        );

        let saved_slot = *self.saved_slot.lock().unwrap();

        // Determine whether we need to save a new extracted frame.
        // This happens when:
        //   - No frame has been saved yet (saved_slot is None)
        //   - We've moved to a new slot (current_slot != saved_slot)
        let need_save = saved_slot != Some(current_slot);

        if need_save {
            // We are on a new slot boundary — this is an extracted frame.
            // Save a copy and update the slot index.
            for frame in data.frames.iter() {
                if let VideoImage::Image { buffer } = frame {
                    *self.extracted_frame.lock().unwrap() = Some(buffer.clone());
                    break;
                }
            }
            *self.saved_slot.lock().unwrap() = Some(current_slot);
        } else {
            // Same slot as the saved frame — this is a fill frame.
            // Replace with the saved extracted frame copy.
            let saved = self.extracted_frame.lock().unwrap();
            if let Some(ref saved_buffer) = *saved {
                for frame in data.frames.iter_mut() {
                    if let VideoImage::Image { buffer } = frame {
                        *buffer = saved_buffer.clone();
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters::traits::VideoFilterConfig;
    use crate::metadata::Metadata;
    use crate::tracks::segment::Segment;
    use std::sync::Arc;
    use std::time::Duration;

    fn make_segment() -> Arc<Segment> {
        Arc::new(Segment::new(
            Duration::ZERO,
            Duration::from_secs(1),
            Arc::new(Metadata::default()),
            1.0,
        ))
    }

    fn make_video_data(buffer: RgbaImage, offset: Duration) -> VideoData {
        VideoData {
            config: VideoFilterConfig::new(buffer.width(), buffer.height(), 30.0),
            frames: vec![VideoImage::image(buffer)],
            from_segment: make_segment(),
            relative_timeline_offset: offset,
        }
    }

    fn make_solid_buffer(r: u8, g: u8, b: u8) -> RgbaImage {
        RgbaImage::from_pixel(4, 4, image::Rgba([r, g, b, 255]))
    }

    #[test]
    fn test_time_to_slot() {
        // At 30fps, frame_interval=3:
        //   slot duration = 3/30 = 0.1s = 100ms
        //   t=0ms   → frame 0   → slot 0
        //   t=33ms  → frame 1   → slot 0
        //   t=66ms  → frame 2   → slot 0
        //   t=100ms → frame 3   → slot 1
        //   t=133ms → frame 4   → slot 1
        //   t=166ms → frame 5   → slot 1
        //   t=200ms → frame 6   → slot 2
        assert_eq!(
            FrameExtractFilter::time_to_slot(Duration::from_millis(0), 30.0, 3),
            0
        );
        assert_eq!(
            FrameExtractFilter::time_to_slot(Duration::from_millis(33), 30.0, 3),
            0
        );
        assert_eq!(
            FrameExtractFilter::time_to_slot(Duration::from_millis(66), 30.0, 3),
            0
        );
        assert_eq!(
            FrameExtractFilter::time_to_slot(Duration::from_millis(100), 30.0, 3),
            1
        );
        assert_eq!(
            FrameExtractFilter::time_to_slot(Duration::from_millis(133), 30.0, 3),
            1
        );
        assert_eq!(
            FrameExtractFilter::time_to_slot(Duration::from_millis(166), 30.0, 3),
            1
        );
        assert_eq!(
            FrameExtractFilter::time_to_slot(Duration::from_millis(200), 30.0, 3),
            2
        );
    }

    #[test]
    fn test_time_to_slot_interval_5() {
        // At 30fps, frame_interval=5:
        //   slot duration = 5/30 ≈ 166.67ms
        //   t=0ms    → slot 0
        //   t=166ms  → slot 0 (frame 4.98)
        //   t=167ms  → slot 1 (frame 5.01)
        assert_eq!(
            FrameExtractFilter::time_to_slot(Duration::from_millis(0), 30.0, 5),
            0
        );
        assert_eq!(
            FrameExtractFilter::time_to_slot(Duration::from_millis(166), 30.0, 5),
            0
        );
        assert_eq!(
            FrameExtractFilter::time_to_slot(Duration::from_millis(167), 30.0, 5),
            1
        );
    }

    #[test]
    fn test_extract_and_fill_by_time() {
        let filter = FrameExtractFilter::new(3);

        // t=0ms (slot 0, extracted): red frame
        let mut data = make_video_data(make_solid_buffer(255, 0, 0), Duration::from_millis(0));
        filter.apply(&mut data).unwrap();
        let frame0 = match &data.frames[0] {
            VideoImage::Image { buffer } => buffer.clone(),
            _ => panic!("expected image"),
        };

        // t=33ms (slot 0, fill): green input → should be replaced by red
        let mut data = make_video_data(make_solid_buffer(0, 255, 0), Duration::from_millis(33));
        filter.apply(&mut data).unwrap();
        let frame1 = match &data.frames[0] {
            VideoImage::Image { buffer } => buffer.clone(),
            _ => panic!("expected image"),
        };
        assert_eq!(
            frame1.as_raw(),
            frame0.as_raw(),
            "fill frame should match extracted frame"
        );

        // t=100ms (slot 1, extracted): green frame — new slot, save copy
        let mut data = make_video_data(make_solid_buffer(0, 255, 0), Duration::from_millis(100));
        filter.apply(&mut data).unwrap();
        let frame3 = match &data.frames[0] {
            VideoImage::Image { buffer } => buffer.clone(),
            _ => panic!("expected image"),
        };
        assert_eq!(frame3.as_raw(), make_solid_buffer(0, 255, 0).as_raw());

        // t=133ms (slot 1, fill): blue input → should be replaced by green
        let mut data = make_video_data(make_solid_buffer(0, 0, 255), Duration::from_millis(133));
        filter.apply(&mut data).unwrap();
        let frame4 = match &data.frames[0] {
            VideoImage::Image { buffer } => buffer.clone(),
            _ => panic!("expected image"),
        };
        assert_eq!(
            frame4.as_raw(),
            frame3.as_raw(),
            "fill frame should match new extracted frame"
        );
    }

    #[test]
    fn test_seek_back_to_same_slot() {
        let filter = FrameExtractFilter::new(3);

        // t=0ms (slot 0, extracted): red
        let mut data = make_video_data(make_solid_buffer(255, 0, 0), Duration::from_millis(0));
        filter.apply(&mut data).unwrap();

        // t=100ms (slot 1, extracted): green
        let mut data = make_video_data(make_solid_buffer(0, 255, 0), Duration::from_millis(100));
        filter.apply(&mut data).unwrap();

        // Seek back to t=33ms (slot 0, fill): should use red from slot 0 — but
        // the saved copy is now green (slot 1). This is expected: the filter only
        // keeps one copy. For slot 0, the input at t=33ms (green) gets saved as
        // the new slot-0 copy since we moved to a different slot.
        // This is the correct behavior for a single-buffer filter.
        let mut data = make_video_data(make_solid_buffer(0, 0, 255), Duration::from_millis(33));
        filter.apply(&mut data).unwrap();
        // Slot 0 is different from saved slot 1, so this becomes a new extracted frame
        assert_eq!(*filter.saved_slot.lock().unwrap(), Some(0));
    }

    #[test]
    fn test_interval_1_no_effect() {
        let filter = FrameExtractFilter::new(1);

        let original = make_solid_buffer(0, 128, 255);
        let mut data = make_video_data(original.clone(), Duration::from_millis(0));
        filter.apply(&mut data).unwrap();

        match &data.frames[0] {
            VideoImage::Image { buffer } => {
                assert_eq!(
                    buffer.as_raw(),
                    original.as_raw(),
                    "interval=1 should not modify frames"
                );
            }
            _ => panic!("expected image"),
        }
    }

    #[test]
    fn test_mid_playback_start() {
        let filter = FrameExtractFilter::new(5);

        // Start playback from t=200ms (30fps, interval=5):
        // frame_index = floor(0.2 * 30) = 6, slot = 6/5 = 1
        // This is an extracted frame (new slot) — should be saved
        let mut data = make_video_data(make_solid_buffer(255, 0, 0), Duration::from_millis(200));
        filter.apply(&mut data).unwrap();
        assert_eq!(*filter.saved_slot.lock().unwrap(), Some(1));
        assert!(filter.extracted_frame.lock().unwrap().is_some());

        // t=233ms (slot 1, fill): should use the saved red frame
        let mut data = make_video_data(make_solid_buffer(0, 255, 0), Duration::from_millis(233));
        filter.apply(&mut data).unwrap();
        let frame = match &data.frames[0] {
            VideoImage::Image { buffer } => buffer.clone(),
            _ => panic!("expected image"),
        };
        assert_eq!(frame.as_raw(), make_solid_buffer(255, 0, 0).as_raw());
    }

    #[test]
    fn test_reset() {
        let filter = FrameExtractFilter::new(3);

        let mut data = make_video_data(make_solid_buffer(255, 0, 0), Duration::from_millis(0));
        filter.apply(&mut data).unwrap();
        assert!(filter.saved_slot.lock().unwrap().is_some());
        assert!(filter.extracted_frame.lock().unwrap().is_some());

        filter.reset();
        assert!(filter.saved_slot.lock().unwrap().is_none());
        assert!(filter.extracted_frame.lock().unwrap().is_none());
    }

    #[test]
    fn test_is_active() {
        assert!(!FrameExtractFilter::new(1).is_active());
        assert!(FrameExtractFilter::new(2).is_active());
    }

    #[test]
    fn test_default() {
        let filter = FrameExtractFilter::default();
        assert_eq!(filter.frame_interval, 1);
        assert!(!filter.is_active());
    }

    #[test]
    fn test_new_clamps_minimum() {
        let filter = FrameExtractFilter::new(0);
        assert_eq!(filter.frame_interval, 1);
    }
}
