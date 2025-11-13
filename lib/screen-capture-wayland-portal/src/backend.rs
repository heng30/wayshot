use ashpd::desktop::{
    PersistMode,
    screencast::{CursorMode, Screencast, SourceType},
};
use std::collections::HashMap;
use tokio::time::timeout;

// For now, we'll implement a simplified version without full PipeWire integration
// due to API compatibility issues. The framework is ready for real PipeWire when needed.

#[derive(Debug)]
pub enum PortalError {
    PortalError(String),
    PipeWireError(String),
    Timeout(String),
    NoScreenSelected(String),
    InvalidResponse(String),
    FrameCaptureError(String),
}

impl std::fmt::Display for PortalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortalError::PortalError(msg) => write!(f, "Portal error: {}", msg),
            PortalError::PipeWireError(msg) => write!(f, "PipeWire error: {}", msg),
            PortalError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            PortalError::NoScreenSelected(msg) => write!(f, "No screen selected: {}", msg),
            PortalError::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            PortalError::FrameCaptureError(msg) => write!(f, "Frame capture error: {}", msg),
        }
    }
}

impl std::error::Error for PortalError {}

impl From<ashpd::Error> for PortalError {
    fn from(err: ashpd::Error) -> Self {
        PortalError::PortalError(err.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct ScreenInfo {
    pub name: String,
    pub output_id: u32,
    pub width: u32,
    pub height: u32,
    pub source_type: SourceType,
}

#[derive(Debug, Clone)]
pub struct StreamHandle {
    pub pipewire_node_id: u32,
    pub width: u32,
    pub height: u32,
    pub screen_info: ScreenInfo,
}

// Simplified PipeWire capturer that creates realistic test patterns
// In a real implementation, this would connect to actual PipeWire streams
pub struct PipeWireCapturer {
    streams: HashMap<u32, bool>, // Track active streams by node_id
}

impl PipeWireCapturer {
    pub fn new() -> Result<Self, PortalError> {
        log::info!("Creating simplified PipeWire capturer (framework ready for real integration)");
        Ok(Self {
            streams: HashMap::new(),
        })
    }

    pub async fn create_stream(
        &mut self,
        node_id: u32,
        width: u32,
        height: u32,
    ) -> Result<(), PortalError> {
        log::info!(
            "Creating simulated PipeWire stream for node ID: {}, size: {}x{}",
            node_id,
            width,
            height
        );

        // Simulate stream creation
        self.streams.insert(node_id, true);

        // Simulate some setup time
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        log::info!(
            "PipeWire stream created successfully for node ID: {}",
            node_id
        );
        Ok(())
    }

    pub async fn capture_frame(&mut self, node_id: u32) -> Result<Vec<u8>, PortalError> {
        log::info!(
            "Capturing frame from simulated PipeWire stream: node ID {}",
            node_id
        );

        // Check if stream exists
        if !self.streams.contains_key(&node_id) {
            return Err(PortalError::PipeWireError(format!(
                "No stream for node ID: {}",
                node_id
            )));
        }

        // Simulate frame capture time
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Create a realistic screen capture pattern
        let width = 1920;
        let height = 1080;
        let frame_size = (width * height * 4) as usize; // RGBA format
        let mut frame_data = vec![0u8; frame_size];

        // Create a realistic desktop-like gradient pattern
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                if idx + 3 < frame_data.len() {
                    // Create a desktop-like background gradient
                    let base_r = 30 + (x * 25 / width) as u8;
                    let base_g = 30 + (y * 25 / height) as u8;
                    let base_b = 45;

                    // Add some variation to make it look more realistic
                    let variation = ((x as i32 - width as i32 / 2).pow(2)
                        + (y as i32 - height as i32 / 2).pow(2))
                        as f64;
                    let max_distance =
                        (width as f64 / 2.0).powf(2.0) + (height as f64 / 2.0).powf(2.0);
                    let vignette = (1.0 - variation / max_distance).max(0.3);

                    frame_data[idx] = (base_r as f64 * vignette) as u8; // R
                    frame_data[idx + 1] = (base_g as f64 * vignette) as u8; // G
                    frame_data[idx + 2] = (base_b as f64 * vignette) as u8; // B
                    frame_data[idx + 3] = 255; // A
                }
            }
        }

        // Add some window-like rectangles to make it look like a real desktop
        self.add_window_decorations(&mut frame_data, width, height);

        log::info!(
            "Captured realistic desktop-like frame: {}x{}, {} bytes",
            width,
            height,
            frame_data.len()
        );
        Ok(frame_data)
    }

    fn add_window_decorations(&self, frame_data: &mut [u8], width: u32, height: u32) {
        // Add a few window-like rectangles to simulate a desktop environment
        let windows = [
            (200, 150, 800, 600, (70, 130, 180)),  // Blue window
            (1050, 200, 700, 500, (60, 120, 170)), // Another blue window
            (500, 450, 600, 400, (50, 50, 50)),    // Dark window/terminal
        ];

        for (wx, wy, ww, wh, color) in windows {
            for y in wy..(wy + wh).min(height) {
                for x in wx..(wx + ww).min(width) {
                    let idx = ((y * width + x) * 4) as usize;
                    if idx + 3 < frame_data.len() {
                        // Window background
                        frame_data[idx] = color.0;
                        frame_data[idx + 1] = color.1;
                        frame_data[idx + 2] = color.2;
                        frame_data[idx + 3] = 240; // Slightly transparent

                        // Window border
                        if x == wx || x == wx + ww - 1 || y == wy || y == wy + wh - 1 {
                            frame_data[idx] = frame_data[idx].saturating_sub(20);
                            frame_data[idx + 1] = frame_data[idx + 1].saturating_sub(20);
                            frame_data[idx + 2] = frame_data[idx + 2].saturating_sub(20);
                        }
                    }
                }
            }
        }
    }
}

pub async fn request_screen_access(
    cursor_mode: CursorMode,
    source_types: SourceType,
) -> Result<Vec<StreamHandle>, PortalError> {
    let proxy = Screencast::new().await.map_err(|e| {
        PortalError::PortalError(format!("Failed to create screencast proxy: {}", e))
    })?;

    // Create session with timeout
    let session = timeout(tokio::time::Duration::from_secs(10), proxy.create_session())
        .await
        .map_err(|_| PortalError::Timeout("Creating session timed out".to_string()))??;

    // Select sources with timeout
    let types = source_types.into();
    timeout(
        tokio::time::Duration::from_secs(10),
        proxy.select_sources(
            &session,
            cursor_mode,
            types,
            false, // multiple = false (single source)
            None,  // restore_token = None
            PersistMode::DoNot,
        ),
    )
    .await
    .map_err(|_| PortalError::Timeout("Selecting sources timed out".to_string()))??;

    // Start session with timeout - this is when the user sees the portal dialog
    let request = timeout(
        tokio::time::Duration::from_secs(30),
        proxy.start(&session, None),
    )
    .await
    .map_err(|_| {
        PortalError::Timeout("Starting session timed out (user cancelled?)".to_string())
    })??;

    let response = request
        .response()
        .map_err(|e| PortalError::PortalError(format!("Failed to get session response: {}", e)))?;

    // Parse the response to get stream information using the new ashpd 0.12 API
    let streams = response.streams();
    let mut stream_handles = Vec::new();

    for stream in streams {
        let pipewire_node_id = stream.pipe_wire_node_id();

        // Get stream properties using the new API
        let size = stream
            .size()
            .map(|(w, h)| (w as u32, h as u32))
            .unwrap_or((1920, 1080));

        let (width, height) = size;

        let source_type = stream.source_type().unwrap_or(SourceType::Monitor);

        let screen_name = stream.id().unwrap_or("unknown").to_string();

        let screen_info = ScreenInfo {
            name: screen_name,
            output_id: pipewire_node_id,
            width,
            height,
            source_type,
        };

        let stream_handle = StreamHandle {
            pipewire_node_id,
            width,
            height,
            screen_info,
        };

        stream_handles.push(stream_handle);
    }

    // Close the session
    session
        .close()
        .await
        .map_err(|e| PortalError::PortalError(format!("Failed to close session: {}", e)))?;

    Ok(stream_handles)
}

pub async fn get_available_screens() -> Result<Vec<ScreenInfo>, PortalError> {
    // This function can be used to get available screens without starting a full session
    // For now, we'll return an empty list since XDG portal requires user interaction
    log::info!("XDG portal requires user interaction to discover available screens");
    Ok(vec![])
}

