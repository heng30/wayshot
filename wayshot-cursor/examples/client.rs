use anyhow::Result;
use std::{io::Read, os::unix::net::UnixStream, time::Duration};

fn main() -> Result<()> {
    env_logger::init();

    let socket_path = "/tmp/wayshot-cursor.sock";

    loop {
        match UnixStream::connect(socket_path) {
            Ok(mut stream) => {
                log::info!("Connected to admin process");

                if let Err(e) = process_mouse_positions(&mut stream) {
                    log::warn!("process mouse positions failed: {e}");
                }
            }
            Err(e) => log::warn!("UnixStream connect `{socket_path}` failed: {e}"),
        }

        std::thread::sleep(Duration::from_secs(3));
    }
}

fn process_mouse_positions(stream: &mut UnixStream) -> Result<()> {
    loop {
        let pos = receive_position(stream)?;
        let x = ((pos >> 32) & 0x0000_0000_ffff_ffff) as i32;
        let y = (pos & 0x0000_0000_ffff_ffff) as i32;
        log::debug!("Received mouse position: ({}, {})", x, y);

        std::thread::sleep(Duration::from_millis(5));
    }
}

fn receive_position(stream: &mut UnixStream) -> Result<u64> {
    let mut buffer = [0u8; 8];
    stream.read_exact(&mut buffer)?;
    let value = u64::from_ne_bytes(buffer);
    Ok(value)
}
