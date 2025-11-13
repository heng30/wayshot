use screen_capture_wayland_portal::{open_portal, start_streaming};
use std::os::fd::IntoRawFd;

#[tokio::main]
async fn main() {
    let (stream, fd) = open_portal().await.expect("failed to open portal");
    let pipewire_node_id = stream.pipe_wire_node_id();

    println!(
        "node id {}, fd {}",
        pipewire_node_id,
        &fd.try_clone().unwrap().into_raw_fd()
    );

    if let Err(e) = start_streaming(pipewire_node_id, fd).await {
        eprintln!("Error: {}", e);
    };
}

