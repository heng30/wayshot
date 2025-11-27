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

#[derive(Debug, PartialEq, Clone, Copy)]
enum EdgeState {
    None,        // Not touching any edge
    Left,        // Touching left edge
    Right,       // Touching right edge
    Top,         // Touching top edge
    Bottom,      // Touching bottom edge
    TopLeft,     // Touching top-left corner
    TopRight,    // Touching top-right corner
    BottomLeft,  // Touching bottom-left corner
    BottomRight, // Touching bottom-right corner
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum EdgeTouch {
    Left,        // Touching left edge area
    Right,       // Touching right edge area
    Top,         // Touching top edge area
    Bottom,      // Touching bottom edge area
    TopLeft,     // Touching top-left corner area
    TopRight,    // Touching top-right corner area
    BottomLeft,  // Touching bottom-left corner area
    BottomRight, // Touching bottom-right corner area
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TransitionType {
    #[allow(dead_code)]
    Linear,
    EaseIn,
    EaseOut,
}

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
    /// considered "stable". Movement within this radius won't trigger zoom in
    debounce_radius: u32,

    /// The maximum distance in pixels that the cursor can move while still being
    /// considered "stable". Movement within this radius won't reset stability timers
    stable_radius: u32,

    /// The minimum duration a cursor must remain stable before considering
    /// a transition when the current region is at screen size
    fast_moving_duration: Duration,

    /// The time it takes to smoothly transition from target size to screen size
    /// or vice versa when the cursor behavior changes
    zoom_transition_duration: Duration,

    /// The distance from crop region edge as a percentage (0.0-0.5)
    /// When cursor enters this edge area in target_size mode, the region will be repositioned
    /// to keep the cursor centered. This prevents jitter from small movements in the center area.
    reposition_edge_threshold: f32,

    /// The time it takes to smoothly transition from current position to centered-region position
    /// or vice versa when the cursor behavior changes
    reposition_transition_duration: Duration,

    /// The maximum duration a cursor can remain stable before the crop region
    /// automatically expands from target size to screen size
    max_stable_region_duration: Duration,

    zoom_in_transition_type: TransitionType,
    zoom_out_transition_type: TransitionType,

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
            debounce_radius: 30,
            stable_radius: 30,
            fast_moving_duration: Duration::from_millis(200),
            zoom_transition_duration: Duration::from_millis(1000),
            reposition_edge_threshold: 0.15,
            reposition_transition_duration: Duration::from_millis(100),
            max_stable_region_duration: Duration::from_secs(10),
            zoom_in_transition_type: TransitionType::EaseIn,
            zoom_out_transition_type: TransitionType::EaseOut,
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

    /// Records the edge state from the last reposition to prevent
    /// repeated repositioning due to the same edge
    last_edge_state: Option<EdgeState>,

    debounce_reference_position: Option<CursorPosition>,
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
            last_edge_state: None,

            debounce_reference_position: None,
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
                        if self.debounce_reference_position.is_none() {
                            self.debounce_reference_position = Some(cursor_pos);
                        }

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
                    log::info!("Exit cursor tracker. Cursor position receiver disconnected");
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

    fn is_cursor_within_debounce_radius(&self) -> bool {
        let Some(debounce_pos) = self.debounce_reference_position else {
            return true;
        };

        let Some(current_pos) = self.last_cursor_position else {
            return true;
        };

        let distance = (debounce_pos.x as f64 - current_pos.x as f64).powi(2)
            + (debounce_pos.y as f64 - current_pos.y as f64).powi(2);

        distance <= self.config.debounce_radius.pow(2) as f64
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
            && !self.is_cursor_within_debounce_radius()
    }

    fn should_reposition_target_region(&self) -> Option<EdgeTouch> {
        let current_size: LogicalSize = self.current_region.into();
        if current_size != self.config.target_size {
            return None;
        }

        let Some(cursor) = self.last_cursor_position else {
            return None;
        };

        let region = &self.current_region;

        let margin_x = region.width as f32 * self.config.reposition_edge_threshold.clamp(0.0, 0.5);
        let margin_y = region.height as f32 * self.config.reposition_edge_threshold.clamp(0.0, 0.5);

        let left_edge = region.x as f32 + margin_x;
        let right_edge = region.x as f32 + region.width as f32 - margin_x;
        let top_edge = region.y as f32 + margin_y;
        let bottom_edge = region.y as f32 + region.height as f32 - margin_y;

        let cursor_x = cursor.x as f32;
        let cursor_y = cursor.y as f32;

        // Return the specific edge being touched
        let touches_left = cursor_x < left_edge;
        let touches_right = cursor_x > right_edge;
        let touches_top = cursor_y < top_edge;
        let touches_bottom = cursor_y > bottom_edge;

        // Corner touches - return accurate corner information
        if touches_left && touches_top {
            return Some(EdgeTouch::TopLeft);
        }
        if touches_left && touches_bottom {
            return Some(EdgeTouch::BottomLeft);
        }
        if touches_right && touches_top {
            return Some(EdgeTouch::TopRight);
        }
        if touches_right && touches_bottom {
            return Some(EdgeTouch::BottomRight);
        }

        // Single edge touches
        if touches_left {
            return Some(EdgeTouch::Left);
        }
        if touches_right {
            return Some(EdgeTouch::Right);
        }
        if touches_top {
            return Some(EdgeTouch::Top);
        }
        if touches_bottom {
            return Some(EdgeTouch::Bottom);
        }

        None
    }

    fn should_allow_reposition(&self, edge_touch: EdgeTouch, last_edge_state: EdgeState) -> bool {
        // If there's no previous edge state, allow repositioning
        if last_edge_state == EdgeState::None {
            return true;
        }

        match (last_edge_state, edge_touch) {
            // Corner states: allow touches that lead away from the corner
            (EdgeState::BottomRight, EdgeTouch::Left) => true,
            (EdgeState::BottomRight, EdgeTouch::Top) => true,
            (EdgeState::BottomRight, EdgeTouch::TopLeft) => true,
            (EdgeState::BottomRight, EdgeTouch::BottomLeft) => true,
            (EdgeState::BottomRight, EdgeTouch::TopRight) => true,

            (EdgeState::TopLeft, EdgeTouch::Right) => true,
            (EdgeState::TopLeft, EdgeTouch::Bottom) => true,
            (EdgeState::TopLeft, EdgeTouch::TopRight) => true,
            (EdgeState::TopLeft, EdgeTouch::BottomLeft) => true,
            (EdgeState::TopLeft, EdgeTouch::BottomRight) => true,

            (EdgeState::TopRight, EdgeTouch::Left) => true,
            (EdgeState::TopRight, EdgeTouch::Bottom) => true,
            (EdgeState::TopRight, EdgeTouch::TopLeft) => true,
            (EdgeState::TopRight, EdgeTouch::BottomRight) => true,
            (EdgeState::TopRight, EdgeTouch::BottomLeft) => true,

            (EdgeState::BottomLeft, EdgeTouch::Right) => true,
            (EdgeState::BottomLeft, EdgeTouch::Top) => true,
            (EdgeState::BottomLeft, EdgeTouch::TopLeft) => true,
            (EdgeState::BottomLeft, EdgeTouch::BottomRight) => true,
            (EdgeState::BottomLeft, EdgeTouch::TopRight) => true,

            // Single edge states: allow most touches except same edge
            (EdgeState::Left, EdgeTouch::Right) => true,
            (EdgeState::Left, EdgeTouch::Top) => true,
            (EdgeState::Left, EdgeTouch::Bottom) => true,
            (EdgeState::Left, EdgeTouch::TopLeft) => true,
            (EdgeState::Left, EdgeTouch::TopRight) => true,
            (EdgeState::Left, EdgeTouch::BottomLeft) => true,
            (EdgeState::Left, EdgeTouch::BottomRight) => true,

            (EdgeState::Right, EdgeTouch::Left) => true,
            (EdgeState::Right, EdgeTouch::Top) => true,
            (EdgeState::Right, EdgeTouch::Bottom) => true,
            (EdgeState::Right, EdgeTouch::TopLeft) => true,
            (EdgeState::Right, EdgeTouch::TopRight) => true,
            (EdgeState::Right, EdgeTouch::BottomLeft) => true,
            (EdgeState::Right, EdgeTouch::BottomRight) => true,

            (EdgeState::Top, EdgeTouch::Left) => true,
            (EdgeState::Top, EdgeTouch::Right) => true,
            (EdgeState::Top, EdgeTouch::Bottom) => true,
            (EdgeState::Top, EdgeTouch::TopLeft) => true,
            (EdgeState::Top, EdgeTouch::TopRight) => true,
            (EdgeState::Top, EdgeTouch::BottomLeft) => true,
            (EdgeState::Top, EdgeTouch::BottomRight) => true,

            (EdgeState::Bottom, EdgeTouch::Left) => true,
            (EdgeState::Bottom, EdgeTouch::Right) => true,
            (EdgeState::Bottom, EdgeTouch::Top) => true,
            (EdgeState::Bottom, EdgeTouch::TopLeft) => true,
            (EdgeState::Bottom, EdgeTouch::TopRight) => true,
            (EdgeState::Bottom, EdgeTouch::BottomLeft) => true,
            (EdgeState::Bottom, EdgeTouch::BottomRight) => true,

            _ => false,
        }
    }

    fn detect_edge_state(&self, region: &Rectangle) -> EdgeState {
        let screen = &self.config.screen_size;

        let touches_left = region.x <= 0;
        let touches_right = region.x + region.width >= screen.width;
        let touches_top = region.y <= 0;
        let touches_bottom = region.y + region.height >= screen.height;

        match (touches_left, touches_right, touches_top, touches_bottom) {
            (true, false, false, false) => EdgeState::Left,
            (false, true, false, false) => EdgeState::Right,
            (false, false, true, false) => EdgeState::Top,
            (false, false, false, true) => EdgeState::Bottom,
            (true, false, true, false) => EdgeState::TopLeft,
            (true, false, false, true) => EdgeState::BottomLeft,
            (false, true, true, false) => EdgeState::TopRight,
            (false, true, false, true) => EdgeState::BottomRight,
            _ => EdgeState::None,
        }
    }

    fn handle_cursor_position(&mut self) -> Rectangle {
        let mut final_region = None;

        if self.should_zoom_out() {
            let transition_regions = self.handle_transition(
                &self.config.screen_size,
                self.config.zoom_out_transition_type,
            );
            self.stable_start_time = None;
            self.stable_cursor_position = None;
            self.last_cursor_capture_timestamp = None;
            self.last_edge_state = None;
            self.debounce_reference_position = None;
            final_region = transition_regions.last().cloned();

            for region in &transition_regions {
                if let Err(e) = self.config.crop_region_sender.try_send(*region) {
                    log::warn!("Failed to send zoom out transition region: {e}");
                    continue;
                }
            }
        } else if self.should_zoom_in() {
            let transition_regions = self.handle_transition(
                &self.config.target_size,
                self.config.zoom_in_transition_type,
            );
            self.stable_start_time = Some(Instant::now());
            self.stable_cursor_position = self.last_cursor_position;
            self.last_edge_state = None;
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

            // Check if we need to reposition the target region due to edge proximity
            if let Some(edge_touch) = self.should_reposition_target_region() {
                let new_region = self.create_centered_region(&self.config.target_size);
                if new_region != self.current_region {
                    let last_edge_state = self.last_edge_state.unwrap_or(EdgeState::None);

                    // Only reposition if the edge touch is meaningful based on last edge state
                    if self.should_allow_reposition(edge_touch, last_edge_state) {
                        let transition_regions = self.handle_reposition_transition();
                        final_region = transition_regions.last().cloned();

                        for region in &transition_regions {
                            if let Err(e) = self.config.crop_region_sender.try_send(*region) {
                                log::warn!("Failed to send repositioned region: {e}");
                                continue;
                            }
                        }

                        if let Some(ref region) = final_region {
                            let current_edge_state = self.detect_edge_state(region);
                            self.last_edge_state = Some(current_edge_state);
                        }
                    }
                }
            }
        }

        let new_region = if final_region.is_some() {
            final_region.take().unwrap()
        } else {
            self.current_region
        };

        new_region
    }

    fn handle_transition(
        &self,
        to_size: &LogicalSize,
        transition_type: TransitionType,
    ) -> Vec<Rectangle> {
        let total_frames = (self.config.zoom_transition_duration.as_secs_f64()
            * self.config.fps as f64)
            .ceil() as usize;
        let mut regions = Vec::with_capacity(total_frames + 1);
        let from_size: LogicalSize = self.current_region.into();

        // Generate all frames in the transition sequence
        for frame in 1..=total_frames {
            let progress = (frame as f64) / (total_frames as f64);
            let progress = progress.min(1.0); // Ensure we don't exceed 1.0

            // Apply easing function based on transition type
            let eased_progress = match transition_type {
                TransitionType::Linear => progress,
                TransitionType::EaseIn => progress * progress,
                TransitionType::EaseOut => 1.0 - (1.0 - progress).powf(2.0),
            };

            // Calculate interpolated size with eased progress
            let width = from_size.width as f64
                + (to_size.width as f64 - from_size.width as f64) * eased_progress;
            let height = from_size.height as f64
                + (to_size.height as f64 - from_size.height as f64) * eased_progress;

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

    fn handle_reposition_transition(&self) -> Vec<Rectangle> {
        let total_frames = (self.config.reposition_transition_duration.as_secs_f64()
            * self.config.fps as f64)
            .ceil() as usize;
        let mut regions = Vec::with_capacity(total_frames + 1);

        let from_region = self.current_region;
        let to_region = self.create_centered_region(&self.config.target_size);

        // Generate all frames in the position transition sequence
        for frame in 1..=total_frames {
            let progress = (frame as f64) / (total_frames as f64);
            let progress = progress.min(1.0); // Ensure we don't exceed 1.0

            // Apply easing function for smoother animation (ease-in-out)
            let eased_progress = if progress < 0.5 {
                2.0 * progress * progress
            } else {
                1.0 - 2.0 * (1.0 - progress) * (1.0 - progress)
            };

            // Calculate interpolated position
            let x =
                from_region.x as f64 + (to_region.x as f64 - from_region.x as f64) * eased_progress;
            let y =
                from_region.y as f64 + (to_region.y as f64 - from_region.y as f64) * eased_progress;

            let region = Rectangle {
                x: x as i32,
                y: y as i32,
                width: to_region.width,
                height: to_region.height,
            };

            regions.push(region);
        }

        // Ensure final state is exactly the target region
        if regions.last().is_none() || regions.last() != Some(&to_region) {
            regions.push(to_region);
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
