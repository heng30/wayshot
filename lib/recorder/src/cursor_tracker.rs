use crate::error::RecorderError;
use crossbeam::channel::{Receiver, Sender};
use derive_setters::Setters;
use screen_capture::{CursorPosition, LogicalSize, Rectangle};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Setters)]
#[setters(prefix = "with_")]
pub struct CursorTrackerConfig {
    /// The frame rate for transition animations (24, 25, 30, or 60 FPS)
    fps: u32,

    /// The total screen dimensions (width and height) in pixels
    screen_size: LogicalSize,

    /// The minimum crop region size that will always be maintained
    /// The region will never be smaller than this size
    target_size: LogicalSize,

    /// Channel for sending calculated crop regions to the recording system
    /// Each region represents the area that should be captured
    crop_region_sender: Sender<Rectangle>,

    /// Channel for receiving cursor position data from the screen capture system
    /// Each message contains a timestamp and cursor coordinates
    cursor_position_receiver: Receiver<(Instant, CursorPosition)>,

    /// The maximum distance in pixels that the cursor can move while still being
    /// considered "stable". Movement within this radius won't reset stability timers
    stable_radius: u32,

    /// The minimum duration a cursor must remain stable before considering
    /// a transition when the current region is at screen size
    fast_moving_duration: Duration,

    /// The time it takes to smoothly transition from target size to screen size
    /// or vice versa when the cursor behavior changes
    linear_transition_duration: Duration,

    /// The maximum duration a cursor can remain stable before the crop region
    /// automatically expands from target size to screen size
    max_stable_region_duration: Duration,

    /// Atomic boolean flag used to signal the cursor tracker to stop running
    /// When set to true, the main tracking loop will exit gracefully
    stop_sig: Arc<AtomicBool>,
}

impl CursorTrackerConfig {
    pub fn new(
        screen_size: LogicalSize,
        target_size: LogicalSize,
        crop_region_sender: Sender<Rectangle>,
        cursor_position_receiver: Receiver<(Instant, CursorPosition)>,
        stop_sig: Arc<AtomicBool>,
    ) -> Result<Self, RecorderError> {
        assert!(target_size.width > 0 && target_size.height > 0);
        assert!(screen_size.width > 0 && screen_size.height > 0);
        assert!(target_size.width <= screen_size.width && target_size.height <= screen_size.height);

        Ok(Self {
            fps: 25,
            screen_size,
            target_size,
            cursor_position_receiver,
            crop_region_sender,
            stable_radius: 100,
            fast_moving_duration: Duration::from_millis(200),
            linear_transition_duration: Duration::from_secs(1),
            max_stable_region_duration: Duration::from_secs(10),
            stop_sig,
        })
    }
}

pub struct CursorTracker {
    config: CursorTrackerConfig,
    current_region: Rectangle,

    last_process_timestamp: Instant,

    last_cursor_position: Option<CursorPosition>,

    /// Tracks when the last cursor movement occurred
    last_cursor_capture_timestamp: Option<Instant>,

    /// The last stable cursor position, used for movement detection
    /// and stability calculations
    stable_cursor_position: Option<CursorPosition>,

    /// Tracks when the cursor started being stable
    stable_start_time: Option<Instant>,
}

impl CursorTracker {
    pub fn new(config: CursorTrackerConfig) -> Result<Self, RecorderError> {
        let current_region =
            Rectangle::new(0, 0, config.screen_size.width, config.screen_size.height);

        Ok(Self {
            config,
            current_region,
            last_process_timestamp: Instant::now(),

            last_cursor_position: None,
            last_cursor_capture_timestamp: None,

            stable_cursor_position: None,
            stable_start_time: None,
        })
    }

    pub fn run(mut self) -> Result<(), RecorderError> {
        // Send initial region
        if let Err(e) = self.config.crop_region_sender.try_send(self.current_region) {
            return Err(RecorderError::CursorTrackerChannelError(format!(
                "Failed to send initial crop region: {}",
                e
            )));
        }

        let process_interval = Duration::from_secs_f32(1.0 / self.config.fps as f32) / 2;

        loop {
            if self.config.stop_sig.load(Ordering::Relaxed) {
                log::info!("Receive a stop signal, exit cursor tracker...");
                break;
            }

            self.last_process_timestamp = Instant::now();

            match self
                .config
                .cursor_position_receiver
                .recv_timeout(process_interval)
            {
                Ok((timestamp, cursor_pos)) => {
                    if self.verify_cursor_position(&cursor_pos) {
                        self.last_cursor_position = Some(cursor_pos);
                        self.last_cursor_capture_timestamp = Some(timestamp);
                        self.current_region = self.handle_cursor_position();
                    }
                }
                Err(crossbeam::channel::RecvTimeoutError::Timeout) => {
                    if self.last_cursor_position.is_some() {
                        self.current_region = self.handle_cursor_position();
                    }
                }
                Err(crossbeam::channel::RecvTimeoutError::Disconnected) => {
                    log::warn!("Exit cursor tracker. Cursor position receiver disconnected");
                    break;
                }
            }
        }

        Ok(())
    }

    fn verify_cursor_position(&self, cursor_pos: &CursorPosition) -> bool {
        !(cursor_pos.x < 0
            || cursor_pos.y < 0
            || cursor_pos.x >= self.config.screen_size.width
            || cursor_pos.y >= self.config.screen_size.height)
    }

    fn is_cursor_within_stable_radius(&self) -> bool {
        let Some(stable_pos) = self.stable_cursor_position else {
            return false;
        };

        let distance = (stable_pos.x as f64 - self.last_cursor_position.unwrap().x as f64).powi(2)
            + (stable_pos.y as f64 - self.last_cursor_position.unwrap().y as f64).powi(2);

        distance <= self.config.stable_radius.pow(2) as f64
    }

    fn should_zoom_out(&self) -> bool {
        let Some(stable_start_time) = self.stable_start_time else {
            return false;
        };

        let current_size: LogicalSize = self.current_region.into();

        current_size == self.config.target_size
            && self
                .last_process_timestamp
                .duration_since(stable_start_time)
                >= self.config.max_stable_region_duration
            && self.is_cursor_within_stable_radius()
    }

    fn should_zoom_in(&self) -> bool {
        let Some(last_timestamp) = self.last_cursor_capture_timestamp else {
            return false;
        };

        let current_size: LogicalSize = self.current_region.into();

        current_size == self.config.screen_size
            && self.last_process_timestamp.duration_since(last_timestamp)
                >= self.config.fast_moving_duration
            && !self.is_cursor_within_stable_radius()
    }

    fn handle_cursor_position(&mut self) -> Rectangle {
        let mut final_region = None;

        if self.should_zoom_out() {
            let transition_regions = self.handle_transition(&self.config.screen_size);
            self.stable_start_time = None;
            self.stable_cursor_position = None;
            self.last_cursor_capture_timestamp = None;
            final_region = transition_regions.last().cloned();

            for region in &transition_regions {
                if let Err(e) = self.config.crop_region_sender.try_send(*region) {
                    log::warn!("Failed to send zoom out transition region: {e}");
                    continue;
                }
            }
        } else if self.should_zoom_in() {
            let transition_regions = self.handle_transition(&self.config.target_size);
            self.stable_start_time = Some(Instant::now());
            self.stable_cursor_position = self.last_cursor_position;
            final_region = transition_regions.last().cloned();

            for region in &transition_regions {
                if let Err(e) = self.config.crop_region_sender.try_send(*region) {
                    log::warn!("Failed to send zoom in transition region: {e}");
                    continue;
                }
            }
        } else {
            // keep the region with target size and move the region
            if self.stable_cursor_position.is_some() && !self.is_cursor_within_stable_radius() {
                self.stable_start_time = Some(Instant::now());
                self.stable_cursor_position = self.last_cursor_position;
            }
        }

        let new_region = if final_region.is_some() {
            final_region.take().unwrap()
        } else {
            self.create_centered_region(&self.current_region.into())
        };

        if new_region != self.current_region
            && let Err(e) = self.config.crop_region_sender.try_send(new_region)
        {
            log::warn!("Failed to send crop region: {e}");
        }

        new_region
    }

    fn handle_transition(&self, to_size: &LogicalSize) -> Vec<Rectangle> {
        let total_frames = (self.config.linear_transition_duration.as_secs_f64()
            * self.config.fps as f64)
            .ceil() as usize;
        let mut regions = Vec::with_capacity(total_frames + 1);
        let from_size: LogicalSize = self.current_region.into();

        // Generate all frames in the transition sequence
        for frame in 1..=total_frames {
            let progress = (frame as f64) / (total_frames as f64);
            let progress = progress.min(1.0); // Ensure we don't exceed 1.0

            // Calculate interpolated size
            let width =
                from_size.width as f64 + (to_size.width as f64 - from_size.width as f64) * progress;
            let height = from_size.height as f64
                + (to_size.height as f64 - from_size.height as f64) * progress;

            let region = self.create_centered_region(&LogicalSize {
                width: width as i32,
                height: height as i32,
            });

            regions.push(region);
        }

        // Ensure final state is exactly the target size
        let final_region = self.create_centered_region(to_size);
        if regions.last().is_none() || regions.last() != Some(&final_region) {
            regions.push(final_region);
        }

        regions
    }

    fn create_centered_region(&self, size: &LogicalSize) -> Rectangle {
        let half_width = size.width as f64 / 2.0;
        let half_height = size.height as f64 / 2.0;

        let x = self.last_cursor_position.unwrap().x as f64 - half_width;
        let x = x.clamp(
            0.0,
            (self.config.screen_size.width as f64 - size.width as f64).max(0.0),
        );

        let y = self.last_cursor_position.unwrap().y as f64 - half_height;
        let y = y.clamp(
            0.0,
            (self.config.screen_size.height as f64 - size.height as f64).max(0.0),
        );

        Rectangle {
            x: x as i32,
            y: y as i32,
            width: size.width,
            height: size.height,
        }
    }
}
