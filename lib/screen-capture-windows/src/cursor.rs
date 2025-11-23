use screen_capture::{
    CursorError,
    CursorPosition,
    MonitorCursorPositionConfig,
    // Rectangle, ScreenInfo,
};

pub fn monitor_cursor_position(
    _config: MonitorCursorPositionConfig,
    _callback: impl FnMut(CursorPosition) + Send + 'static,
) -> Result<(), CursorError> {
    // let mut tracker = CursorTracker::new(config.screen_info)?;
    // tracker.state.callback = Some(Box::new(callback));
    // tracker.state.use_transparent_layer_surface = config.use_transparent_layer_surface;
    // tracker.state.hole_radius = config.hole_radius;
    // tracker.start_tracking()?;
    //
    // loop {
    //     if config.stop_sig.load(Ordering::Relaxed) {
    //         break;
    //     }
    //
    //     // Process input region updates immediately after dispatching events
    //     if let Err(e) = tracker.process_pending_input_updates() {
    //         log::warn!("Input region update error: {}", e);
    //     }
    //
    //     if let Err(e) = tracker.queue.dispatch_pending(&mut tracker.state) {
    //         log::warn!("Dispatch pending error: {}", e);
    //     }
    //
    //     if let Err(e) = tracker.queue.roundtrip(&mut tracker.state) {
    //         log::warn!("Roundtrip  error: {}", e);
    //     }
    //
    //     // Reduced sleep time for better responsiveness
    //     std::thread::sleep(Duration::from_millis(5));
    // }

    Ok(())
}
