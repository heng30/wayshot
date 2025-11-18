use crate::Error;
use rdev::{Event, EventType, grab};
use screen_capture::{CursorPosition, MonitorCursorPositionConfig};
use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Duration,
};

static CURSOR_GRAP_THREAD_RUNNING: AtomicBool = AtomicBool::new(false);
static CURSOR_POSITION: AtomicU64 = AtomicU64::new(u64::MAX);

pub fn monitor_cursor_position(
    config: MonitorCursorPositionConfig,
    mut callback: impl FnMut(CursorPosition) + Send + 'static,
) -> Result<(), Error> {
    if !CURSOR_GRAP_THREAD_RUNNING.load(Ordering::Relaxed) {
        std::thread::spawn(move || {
            CURSOR_GRAP_THREAD_RUNNING.store(true, Ordering::Relaxed);
            log::info!("start long run cursor grap thread...");

            let callback = move |event: Event| -> Option<Event> {
                if let EventType::MouseMove { x, y } = event.event_type {
                    // log::debug!("cursor position: (x, y) = ({x}, {y})");
                    let cur_pos = (((x as u64) << 32) & 0xffff_ffff_0000_0000) | (y as u64);
                    CURSOR_POSITION.store(cur_pos, Ordering::Relaxed);
                }

                Some(event)
            };

            // The grab function use the evdev library to intercept events,
            // so they will work with both X11 and Wayland In order for this to work,
            // the process running the listen or grab loop needs to either run as root (not recommended),
            // or run as a user who's a member of the input group (recommended) Note: on some distros,
            // the group name for evdev access is called plugdev, and on some systems, both groups can exist.
            // When in doubt, add your user to both groups if they exist.
            // commands: `sudo usermod -aG input $USER` or `sudo usermod -aG plugdev $USER`
            if let Err(e) = grab(callback) {
                log::warn!("cursor monitor failed: {e:?}");
            }

            CURSOR_GRAP_THREAD_RUNNING.store(false, Ordering::Relaxed);
            log::info!("exit long run cursor grap thread...");
        });
    }

    let mut last_pos = CURSOR_POSITION.load(Ordering::Relaxed);
    loop {
        if config.stop_sig.load(Ordering::Relaxed) {
            log::info!("exit monitor cursor thread...");
            break;
        }

        let cur_pos = CURSOR_POSITION.load(Ordering::Relaxed);
        if last_pos == cur_pos || cur_pos == u64::MAX {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };

        last_pos = cur_pos;

        let position = CursorPosition {
            x: ((cur_pos >> 32) & 0x0000_0000_ffff_ffff) as i32,
            y: (cur_pos & 0x0000_0000_ffff_ffff) as i32,
            output_x: config.screen_info.position.x,
            output_y: config.screen_info.position.y,
            output_width: config.screen_info.logical_size.width,
            output_height: config.screen_info.logical_size.height,
        };

        callback(position);
    }

    Ok(())
}

