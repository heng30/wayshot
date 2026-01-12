use crate::{
    CursorTracker, CursorTrackerConfig, Frame, FrameUser, RecorderError, RecordingSession,
    ResizedImageBuffer, Resolution, SimpleFpsCounter, StatsUser,
    process_mode::SHARE_SCREEN_CONNECTIONS_COUNT,
    recorder::{CURSOR_CHANNEL_SIZE, CameraImage, ENCODER_WORKER_CHANNEL_SIZE, EncoderChannelData},
};
use background_remover::BackgroundRemover;
use camera::mix_images_rgb;
use crossbeam::channel::{Receiver, Sender, bounded};
use fast_image_resize::images::Image;
use image::{GrayImage, ImageBuffer, Rgb, Rgba, buffer::ConvertBuffer};
use image_effect::realtime::RealtimeImageEffect;
use once_cell::sync::Lazy;
use screen_capture::{
    Capture, LogicalSize, MonitorCursorPositionConfig, Position, Rectangle, ScreenCapture,
    ScreenInfoError,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

static CURSOR_POSITION: AtomicU64 = AtomicU64::new(u64::MAX);
static LAST_CROP_REGION: Lazy<Mutex<Option<Rectangle>>> = Lazy::new(|| Mutex::new(None));

impl RecordingSession {
    pub(crate) fn process_frame_workers(
        session: &RecordingSession,
        encoder_sender: Sender<EncoderChannelData>,
    ) -> Vec<JoinHandle<()>> {
        let mut handles = vec![];

        let (frame_sender, frame_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);
        let (collect_sender, collect_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);

        handles.push(Self::process_forward_worker(session, frame_sender));

        // Base worker count + camera mix workers + image effect workers
        let mut worker_count = 3;
        if session.config.camera_mix_config.enable {
            if session
                .config
                .camera_mix_config
                .background_remover_model_path
                .is_some()
            {
                worker_count += 2;
            } else {
                worker_count += 1;
            }
        }

        if let Ok(effect) = RealtimeImageEffect::try_from(
            session.config.realtime_image_effect.load(Ordering::Relaxed),
        ) && !matches!(effect, RealtimeImageEffect::None)
        {
            worker_count += 2;
        }

        for i in 0..worker_count {
            handles.push(Self::process_frame_worker(
                session,
                collect_sender.clone(),
                frame_receiver.clone(),
                i,
            ));
        }

        handles.push(Self::process_collect_worker(
            session,
            encoder_sender,
            collect_receiver,
        ));
        handles
    }

    fn process_forward_worker(
        session: &RecordingSession,
        sender: Sender<(u64, Frame, Option<CameraImage>)>,
    ) -> JoinHandle<()> {
        let start_time = session.start_time;
        let receiver = session.frame_receiver.clone();
        let loss_frame_count = session.loss_frame_count.clone();
        let total_frame_count = session.total_frame_count.clone();
        let enable_camera_mix = session.config.camera_mix_config.enable;
        let camera_image_receiver = session.camera_image_receiver.clone();
        let mut last_camera_image: Option<CameraImage> = None;

        thread::spawn(move || {
            while let Ok(frame) = receiver.recv() {
                let total_frame_count = total_frame_count.fetch_add(1, Ordering::Relaxed) + 1;

                log::debug!(
                    "total frame[{}] thread[{}] thread_frame[{}] capture time: {:.2?}. thread_fps: {:.2}. timestamp: {:.2?}. capture channel remained: {}",
                    total_frame_count,
                    frame.thread_id,
                    frame.cb_data.frame_index,
                    frame.cb_data.capture_time,
                    (frame.cb_data.frame_index + 1) as f64 / frame.cb_data.elapse.as_secs_f64(),
                    frame.timestamp.duration_since(start_time),
                    receiver.capacity().unwrap_or_default() - receiver.len()
                );

                let camera_img = if enable_camera_mix {
                    if let Some(ref receiver) = camera_image_receiver
                        && let Ok(img) = receiver.try_recv()
                    {
                        last_camera_image = Some(img);
                    }
                    last_camera_image.clone()
                } else {
                    None
                };

                if let Err(e) = sender.try_send((total_frame_count, frame, camera_img)) {
                    loss_frame_count.fetch_add(1, Ordering::Relaxed);
                    log::warn!("process worker try send failed: {e}");
                }
            }

            log::info!("process forward thread exit");
        })
    }

    fn process_collect_worker(
        session: &RecordingSession,
        sender: Sender<EncoderChannelData>,
        receiver: Receiver<(usize, Instant, EncoderChannelData)>,
    ) -> JoinHandle<()> {
        let total_frame_count = session.total_frame_count.clone();
        let loss_frame_count = session.loss_frame_count.clone();
        let frame_sender_user = session.frame_sender_user.clone();

        thread::spawn(move || {
            let mut expect_total_frame_index = 1;
            let mut disorder_frame_counts = 0;
            let mut frame_cache: HashMap<u64, (u64, ResizedImageBuffer, Option<CameraImage>)> =
                HashMap::new();
            let mut fps_counter = SimpleFpsCounter::new();

            while let Ok((thread_index, frame_timestamp, (total_frame_index, img, _camera_img))) =
                receiver.recv()
            {
                // FIXME: no accuracy. because frame_timestamp may be disorder
                let fps = fps_counter.add_frame(frame_timestamp);

                if expect_total_frame_index == total_frame_index {
                    disorder_frame_counts = 0;

                    Self::send_frame_to_encoder(
                        img,
                        &sender,
                        &frame_sender_user,
                        expect_total_frame_index,
                        total_frame_count.clone(),
                        loss_frame_count.clone(),
                        fps,
                    );

                    loop {
                        expect_total_frame_index += 1;
                        match frame_cache.remove(&expect_total_frame_index) {
                            Some(frame) => {
                                Self::send_frame_to_encoder(
                                    frame.1,
                                    &sender,
                                    &frame_sender_user,
                                    expect_total_frame_index,
                                    total_frame_count.clone(),
                                    loss_frame_count.clone(),
                                    fps,
                                );
                            }
                            _ => break,
                        }
                    }
                } else if expect_total_frame_index > total_frame_index {
                    loss_frame_count.fetch_add(1, Ordering::Relaxed);
                    log::warn!(
                        "too late thread[{thread_index}] frame, frame index: {total_frame_index}, expected index: {expect_total_frame_index}"
                    );
                } else {
                    frame_cache.insert(total_frame_index, (total_frame_index, img, _camera_img));
                    disorder_frame_counts += 1;

                    if disorder_frame_counts > 5 {
                        disorder_frame_counts = 0;

                        log::warn!(
                            "disorder frame counts > 5. So moving to next expected frame index: {expect_total_frame_index} -> {}",
                            expect_total_frame_index + 1
                        );

                        loop {
                            expect_total_frame_index += 1;
                            match frame_cache.remove(&expect_total_frame_index) {
                                Some(frame) => {
                                    Self::send_frame_to_encoder(
                                        frame.1,
                                        &sender,
                                        &frame_sender_user,
                                        expect_total_frame_index,
                                        total_frame_count.clone(),
                                        loss_frame_count.clone(),
                                        fps,
                                    );
                                }
                                _ => break,
                            }
                        }
                    }
                }
            }

            log::info!("precess collected thread exit");
        })
    }

    fn process_frame_worker(
        session: &RecordingSession,
        sender: Sender<(usize, Instant, EncoderChannelData)>,
        receiver: Receiver<(u64, Frame, Option<CameraImage>)>,
        thread_index: usize,
    ) -> JoinHandle<()> {
        let resolution = session.config.resolution.clone();
        let loss_frame_count = session.loss_frame_count.clone();
        let enable_cursor_tracking = session.config.enable_cursor_tracking;
        let crop_region_receiver = session.crop_region_receiver.clone();
        let enable_camera_mix = session.config.camera_mix_config.enable;
        let camera_shape = session.config.camera_mix_config.shape.clone();
        let realtime_image_effect = session.config.realtime_image_effect.clone();
        let camera_background_mask = session.camera_background_mask.clone();

        thread::spawn(move || {
            while let Ok((total_frame_count, frame, camera_img)) = receiver.recv() {
                let now = Instant::now();
                let frame_timestamp = frame.timestamp;

                let img = if enable_cursor_tracking {
                    match Self::crop_and_resize_frame(
                        frame,
                        resolution,
                        crop_region_receiver.clone().unwrap(),
                    ) {
                        Ok(img) => img,
                        Err(e) => {
                            log::warn!("crop and resize frame failed: {e}");
                            continue;
                        }
                    }
                } else {
                    match Self::resize_frame(frame, resolution) {
                        Ok(img) => img,
                        Err(e) => {
                            log::warn!("resize frame failed: {e}");
                            continue;
                        }
                    }
                };

                let img = if let Ok(effect) =
                    RealtimeImageEffect::try_from(realtime_image_effect.load(Ordering::Relaxed))
                    && !matches!(effect, RealtimeImageEffect::None)
                {
                    Self::apply_realtime_image_effect(img, effect)
                } else {
                    img
                };

                let img = if enable_camera_mix {
                    let mask = camera_background_mask.lock().unwrap().clone();
                    Self::mix_screen_and_camera(img, camera_img, &camera_shape, mask)
                } else {
                    img
                };

                log::debug!("process frame spent: {:.2?}", now.elapsed());

                if let Err(e) = sender.try_send((
                    thread_index,
                    frame_timestamp,
                    (total_frame_count, img, None),
                )) {
                    loss_frame_count.fetch_add(1, Ordering::Relaxed);
                    log::warn!("process worker try send failed: {e}");
                }
            }
            log::info!("process worker thread exit");
        })
    }

    pub(crate) fn cursor_tracker_worker(
        &mut self,
        mut screen_capturer: impl ScreenCapture + Clone + Send + 'static,
        crop_region_sender: Sender<Rectangle>,
    ) -> Result<(), RecorderError> {
        let stop_sig = self.stop_sig.clone();
        let screen_name = self.config.screen_name.clone();

        let screen_info = screen_capturer
            .available_screens()?
            .iter()
            .find(|item| item.name == screen_name)
            .ok_or(RecorderError::ScreenInfoFailed(ScreenInfoError::Other(
                format!("No found screen in cursor monitor thread {screen_name}"),
            )))?
            .clone();

        let (cursor_sender, cursor_receiver) = bounded(CURSOR_CHANNEL_SIZE);
        let target_size = LogicalSize::new(
            self.config.region_width.max(1),
            self.config.region_height.max(1),
        );

        let cursor_tracker_config = CursorTrackerConfig::new(
            screen_info.logical_size,
            target_size,
            crop_region_sender,
            cursor_receiver,
            stop_sig.clone(),
        )?
        .with_fps(self.config.fps.to_u32())
        .with_debounce_radius(self.config.debounce_radius)
        .with_stable_radius(self.config.stable_radius)
        .with_zoom_in_transition_type(self.config.zoom_in_transition_type)
        .with_zoom_out_transition_type(self.config.zoom_out_transition_type)
        .with_fast_moving_duration(Duration::from_millis(self.config.fast_moving_duration))
        .with_zoom_transition_duration(Duration::from_millis(self.config.zoom_transition_duration))
        .with_reposition_edge_threshold(self.config.reposition_edge_threshold)
        .with_reposition_transition_duration(Duration::from_millis(
            self.config.reposition_transition_duration,
        ))
        .with_max_stable_region_duration(Duration::from_secs(
            self.config.max_stable_region_duration,
        ));

        thread::spawn(move || {
            let cursor_tracker = match CursorTracker::new(cursor_tracker_config) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("New cursor tracker faild: {e}");
                    return;
                }
            };

            if let Err(e) = cursor_tracker.run() {
                log::error!("Run Cursor tracker failed: {e}");
            }

            log::info!("Exit cursor tracker thread");
        });

        let stable_radius = self.config.stable_radius;
        let cursor_monitor_stop_sig = stop_sig.clone();
        thread::spawn(move || {
            CURSOR_POSITION.store(u64::MAX, Ordering::SeqCst);

            {
                *LAST_CROP_REGION.lock().unwrap() = Some(Rectangle::new(
                    0,
                    0,
                    screen_info.logical_size.width,
                    screen_info.logical_size.height,
                ));
            }

            let config = MonitorCursorPositionConfig::new(screen_info, cursor_monitor_stop_sig)
                .with_use_transparent_layer_surface(true)
                .with_hole_radius((stable_radius as i32 / 2).max(30));

            if let Err(e) = screen_capturer.monitor_cursor_position(config, move |position| {
                let current_position =
                    (((position.x as u64) << 32) & 0xffff_ffff_0000_0000) | (position.y as u64);
                CURSOR_POSITION.store(current_position, Ordering::Relaxed);

                log::debug!(
                    "dimensions: {}x{} at ({}, {}). (x, y) = ({}, {})",
                    position.output_width,
                    position.output_height,
                    position.output_x,
                    position.output_y,
                    position.x,
                    position.y
                );

                if let Err(e) = cursor_sender.try_send((Instant::now(), position)) {
                    log::warn!("cursor sender failed: {e}");
                }
            }) {
                log::error!("monitor cursor position faield: {e}");
            }

            log::info!("Exit monitor cursor position thread");
        });

        Ok(())
    }

    pub(crate) fn background_remover_worker(
        &mut self,
        model_path: PathBuf,
    ) -> Result<(), RecorderError> {
        let model = self
            .config
            .camera_mix_config
            .background_remover_model
            .ok_or(RecorderError::Other(
                "Camera background remover model is None".to_string(),
            ))?;

        let mut remover = BackgroundRemover::new(model, model_path).map_err(|e| {
            RecorderError::Other(format!("Failed to create background remover: {}", e))
        })?;

        let camera_image_receiver =
            self.camera_background_remover_receiver
                .clone()
                .ok_or_else(|| {
                    RecorderError::Other(
                        "Camera background remover receiver not initialized".to_string(),
                    )
                })?;

        let stop_sig = self.stop_sig.clone();
        let mask_cache = self.camera_background_mask.clone();

        thread::spawn(move || {
            while !stop_sig.load(Ordering::Relaxed) {
                if let Ok(camera_img) =
                    camera_image_receiver.recv_timeout(Duration::from_millis(100))
                {
                    match remover.get_mask(&camera_img) {
                        Ok(mask) => *mask_cache.lock().unwrap() = Some(mask),
                        Err(e) => log::warn!("Failed to generate background mask: {e}"),
                    }
                }
            }

            log::info!("Background remover worker exit");
        });

        Ok(())
    }

    fn crop_and_resize_frame(
        frame: Frame,
        resolution: Resolution,
        crop_region_receiver: Receiver<Rectangle>,
    ) -> Result<ResizedImageBuffer, RecorderError> {
        let region = Self::get_matched_crop_region(crop_region_receiver);

        log::debug!("crop region: {:?}", region);

        if matches!(resolution, Resolution::Original(_))
            && region.width as u32 == frame.cb_data.data.width
            && region.height as u32 == frame.cb_data.data.height
        {
            let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
                frame.cb_data.data.width,
                frame.cb_data.data.height,
                frame.cb_data.data.pixel_data,
            )
            .ok_or_else(|| {
                RecorderError::ImageProcessingFailed("Failed to create image buffer".to_string())
            })?;

            let img: ImageBuffer<Rgb<u8>, Vec<u8>> = img.convert();
            return Ok(img);
        }

        let (original_width, original_height) =
            (frame.cb_data.data.width, frame.cb_data.data.height);
        let target_size = resolution.dimensions(original_width, original_height);
        Self::resize_image(frame.cb_data.data, target_size, Some(region))
    }

    fn resize_frame(
        frame: Frame,
        resolution: Resolution,
    ) -> Result<ResizedImageBuffer, RecorderError> {
        let img = if matches!(resolution, Resolution::Original(_)) {
            let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
                frame.cb_data.data.width,
                frame.cb_data.data.height,
                frame.cb_data.data.pixel_data,
            )
            .ok_or_else(|| {
                RecorderError::ImageProcessingFailed("Failed to create image buffer".to_string())
            })?;

            let img: ImageBuffer<Rgb<u8>, Vec<u8>> = img.convert();
            img
        } else {
            let (original_width, original_height) =
                (frame.cb_data.data.width, frame.cb_data.data.height);

            let img = Self::resize_image(
                frame.cb_data.data,
                resolution.dimensions(original_width, original_height),
                None,
            )?;

            img
        };

        Ok(img)
    }

    pub fn resize_image(
        mut capture: Capture,
        target_size: (u32, u32),
        region: Option<Rectangle>,
    ) -> Result<ResizedImageBuffer, RecorderError> {
        let (src_width, src_height) = (capture.width as u32, capture.height as u32);
        let (dst_width, dst_height) = target_size;

        // Use fast_image_resize for high-performance resizing
        let mut dst = vec![0u8; (dst_width * dst_height * 4) as usize];

        let src_image = Image::from_slice_u8(
            src_width,
            src_height,
            &mut capture.pixel_data,
            fast_image_resize::PixelType::U8x4,
        )
        .map_err(|e| {
            RecorderError::ImageProcessingFailed(format!("Failed to create source image: {}", e))
        })?;

        let mut dst_image = Image::from_slice_u8(
            dst_width,
            dst_height,
            &mut dst,
            fast_image_resize::PixelType::U8x4,
        )
        .map_err(|e| {
            RecorderError::ImageProcessingFailed(format!(
                "Failed to create destination image: {}",
                e
            ))
        })?;

        let resize_options = fast_image_resize::ResizeOptions::new().resize_alg(
            fast_image_resize::ResizeAlg::SuperSampling(fast_image_resize::FilterType::Lanczos3, 2),
        );

        let resize_options = if let Some(region) = region {
            resize_options.crop(
                region.x as f64,
                region.y as f64,
                region.width as f64,
                region.height as f64,
            )
        } else {
            resize_options
        };

        fast_image_resize::Resizer::new()
            .resize(&src_image, &mut dst_image, &resize_options)
            .map_err(|e| RecorderError::ImageProcessingFailed(format!("Resize failed: {}", e)))?;

        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(dst_width, dst_height, dst)
            .ok_or_else(|| {
                RecorderError::ImageProcessingFailed(
                    "Failed to create resized image buffer".to_string(),
                )
            })?;

        Ok(img.convert())
    }

    #[inline]
    fn send_frame_to_encoder(
        img: ResizedImageBuffer,
        encoder_sender: &Sender<EncoderChannelData>,
        frame_sender_user: &Option<Sender<FrameUser>>,
        expect_total_frame_index: u64,
        total_frame_count: Arc<AtomicU64>,
        loss_frame_count: Arc<AtomicU64>,
        fps: f32,
    ) {
        if let Some(sender) = frame_sender_user {
            let frame_user = FrameUser {
                stats: StatsUser {
                    fps,
                    total_frames: total_frame_count.load(Ordering::Relaxed),
                    loss_frames: loss_frame_count.load(Ordering::Relaxed),
                    share_screen_connections: SHARE_SCREEN_CONNECTIONS_COUNT
                        .load(Ordering::Relaxed),
                },
                buffer: img.clone(),
            };

            if let Err(e) = sender.try_send(frame_user) {
                log::warn!("try send frame to user frame channel failed: {e}");
            }
        }

        if let Err(e) = encoder_sender.try_send((expect_total_frame_index, img, None)) {
            loss_frame_count.fetch_add(1, Ordering::Relaxed);
            log::warn!("collected thread try send to encoder reciever failed: {e}");
        }
    }

    fn apply_realtime_image_effect(
        rgb_image: ResizedImageBuffer,
        effect: RealtimeImageEffect,
    ) -> ResizedImageBuffer {
        let (width, height) = rgb_image.dimensions();
        let raw_data = rgb_image.into_raw();

        let rgb_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_raw(width, height, raw_data)
                .expect("Failed to reconstruct RGB buffer");

        let rgba_image: ImageBuffer<Rgba<u8>, Vec<u8>> = rgb_buffer.convert();
        let result = effect.apply(rgba_image);

        match result {
            Some(processed_rgba) => processed_rgba.convert(),
            None => {
                log::warn!("Image effect returned None, using original image");
                ImageBuffer::new(width, height)
            }
        }
    }

    fn mix_screen_and_camera(
        screen_image: ResizedImageBuffer,
        camera_img: Option<CameraImage>,
        camera_shape: &camera::Shape,
        camera_background_mask: Option<GrayImage>,
    ) -> ResizedImageBuffer {
        if let Some(camera_img) = camera_img {
            match mix_images_rgb(
                screen_image.clone(),
                camera_img,
                camera_background_mask,
                camera_shape.clone(),
            ) {
                Ok(mixed_img) => mixed_img,
                Err(e) => {
                    log::warn!("Failed to mix camera image: {e}");
                    screen_image
                }
            }
        } else {
            screen_image
        }
    }

    fn get_matched_crop_region(crop_region_receiver: Receiver<Rectangle>) -> Rectangle {
        loop {
            match crop_region_receiver.try_recv() {
                Ok(v) => {
                    let cursor_position = CURSOR_POSITION.load(Ordering::Relaxed);
                    if cursor_position == u64::MAX {
                        return LAST_CROP_REGION.lock().unwrap().clone().unwrap();
                    };

                    let cursor_position = Position::new(
                        ((cursor_position >> 32) & 0x0000_0000_ffff_ffff) as i32,
                        (cursor_position & 0x0000_0000_ffff_ffff) as i32,
                    );

                    if v.contain_position(&cursor_position) {
                        *LAST_CROP_REGION.lock().unwrap() = Some(v);
                        return v;
                    }
                }
                _ => return LAST_CROP_REGION.lock().unwrap().clone().unwrap(),
            };
        }
    }
}
