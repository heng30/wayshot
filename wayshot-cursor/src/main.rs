use anyhow::Result;
use rdev::{Event, EventType, grab};
use std::{
    fs,
    io::Write,
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    sync::atomic::{AtomicU64, Ordering},
    thread,
};

static CURSOR_POSITION: AtomicU64 = AtomicU64::new(u64::MAX);
static LAST_SENT_POSITION: AtomicU64 = AtomicU64::new(u64::MAX);

pub fn main() -> Result<()> {
    env_logger::init();
    log::info!("start long run cursor grap thread...");

    ctrlc::set_handler(move || {
        log::info!("Ctrl-C received, stopping...");
        std::process::exit(0);
    })?;

    thread::spawn(|| {
        if let Err(e) = server() {
            log::warn!("start socket server failed: {e}");
            std::process::exit(-1);
        }
    });

    let callback = move |event: Event| -> Option<Event> {
        if let EventType::MouseMove { x, y } = event.event_type {
            log::debug!("cursor position: (x, y) = ({x}, {y})");
            let cur_pos = (((x as u64) << 32) & 0xffff_ffff_0000_0000) | (y as u64);
            CURSOR_POSITION.store(cur_pos, Ordering::Relaxed);
        }

        Some(event)
    };

    if let Err(e) = grab(callback) {
        log::warn!("cursor monitor failed: {e:?}");
        std::process::exit(-1);
    }

    Ok(())
}

fn server() -> Result<()> {
    let socket_path = "/tmp/wayshot-cursor.sock";
    _ = fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path)?;
    log::info!("Admin process listening on {}", socket_path);

    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o666))?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                log::info!("Client connected");

                thread::spawn(move || {
                    loop {
                        let pos = CURSOR_POSITION.load(Ordering::Relaxed);
                        let last_sent_pos = LAST_SENT_POSITION.load(Ordering::Relaxed);

                        if pos != last_sent_pos && pos != u64::MAX {
                            if let Err(e) = send_position(&mut stream, pos) {
                                log::warn!("send position failed: {e}");
                                break;
                            }

                            LAST_SENT_POSITION.store(pos, Ordering::Relaxed);
                        }

                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                });
            }
            Err(err) => {
                log::warn!("Connection error: {}", err);
            }
        }
    }

    Ok(())
}

fn send_position(stream: &mut UnixStream, value: u64) -> Result<()> {
    let bytes = value.to_ne_bytes();
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}
