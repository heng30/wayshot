use screen_capture_wayland_portal::{PortalCapturer, available_screens};
use std::os::fd::IntoRawFd;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let screens = available_screens()?;
    if screens.is_empty() {
        log::warn!("No found available screens");
        return Ok(());
    }

    let mut backend = PortalCapturer::new(screens[0].clone());
    let (stream, fd) = backend.open_portal().await.expect("failed to open portal");
    let pipewire_node_id = stream.pipe_wire_node_id();

    log::debug!(
        "node id {}, fd {}",
        pipewire_node_id,
        &fd.try_clone().unwrap().into_raw_fd()
    );

    if let Err(e) = backend.start_streaming(pipewire_node_id, fd).await {
        log::warn!("Error: {e}");
    };

    Ok(())
}
