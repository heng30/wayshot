use crate::Result;
use screen_capture::{CursorPosition, MonitorCursorPositionConfig};
use std::{
    sync::atomic::Ordering,
    {io::Read, os::unix::net::UnixStream, time::Duration},
};

pub fn monitor_cursor_position(
    config: MonitorCursorPositionConfig,
    mut callback: impl FnMut(CursorPosition) + Send + 'static,
) -> Result<()> {
    let socket_path = "/tmp/wayshot-cursor.sock";

    loop {
        if config.stop_sig.load(Ordering::Relaxed) {
            log::info!("exit monitor cursor thread...");
            break;
        }

        match UnixStream::connect(socket_path) {
            Ok(mut stream) => {
                log::info!("Connected to server process");

                if let Err(e) = process_mouse_positions(&mut stream, &config, &mut callback) {
                    log::warn!("process mouse positions failed: {e}");
                }
            }
            Err(e) => log::warn!("UnixStream connect `{socket_path}` failed: {e}"),
        }

        std::thread::sleep(Duration::from_secs(3));
    }

    Ok(())
}

fn process_mouse_positions(
    stream: &mut UnixStream,
    config: &MonitorCursorPositionConfig,
    callback: &mut (impl FnMut(CursorPosition) + Send + 'static),
) -> Result<()> {
    loop {
        if config.stop_sig.load(Ordering::Relaxed) {
            break;
        }

        let pos = receive_position(stream)?;
        let x = ((pos >> 32) & 0x0000_0000_ffff_ffff) as i32;
        let y = (pos & 0x0000_0000_ffff_ffff) as i32;

        log::debug!("Received mouse position: ({}, {})", x, y);

        let position = CursorPosition {
            x,
            y,
            output_x: config.screen_info.position.x,
            output_y: config.screen_info.position.y,
            output_width: config.screen_info.logical_size.width,
            output_height: config.screen_info.logical_size.height,
        };

        callback(position);

        std::thread::sleep(Duration::from_millis(5));
    }

    Ok(())
}

fn receive_position(stream: &mut UnixStream) -> Result<u64> {
    let mut buffer = [0u8; 8];
    stream.read_exact(&mut buffer)?;
    let value = u64::from_ne_bytes(buffer);
    Ok(value)
}
