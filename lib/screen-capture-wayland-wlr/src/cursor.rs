use nix::sys::memfd;
use screen_capture::ScreenInfo;
use std::{
    io::{Seek, Write},
    os::fd::{AsFd, AsRawFd, FromRawFd},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use thiserror::Error;
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    protocol::{
        wl_buffer, wl_compositor, wl_output, wl_pointer, wl_region, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_surface,
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

#[derive(Debug, Error)]
pub enum CursorError {
    #[error("Failed to connect to Wayland display: {0}")]
    ConnectionFailed(String),

    #[error("Required Wayland protocol not available: {0}")]
    ProtocolNotAvailable(String),

    #[error("Failed to get pointer: {0}")]
    PointerFailed(String),

    #[error("Dispatch Failed: {0}")]
    DispatchFailed(#[from] wayland_client::DispatchError),
}

#[derive(Debug, Clone, Copy)]
pub struct CursorPosition {
    pub x: i32,
    pub y: i32,
    pub output_x: i32,
    pub output_y: i32,
    pub output_width: i32,
    pub output_height: i32,
}

pub struct CursorTracker {
    queue: EventQueue<CursorTrackerState>,
    state: CursorTrackerState,
}

#[derive(Default)]
struct CursorTrackerState {
    compositor: Option<wl_compositor::WlCompositor>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    layer_surfaces: Vec<(
        zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        wl_output::WlOutput,
    )>,
    surfaces: Vec<wl_surface::WlSurface>,
    surface_buffers: Vec<Option<wl_buffer::WlBuffer>>,
    pointer: Option<wl_pointer::WlPointer>,
    seat: Option<wl_seat::WlSeat>,
    outputs: Vec<(wl_output::WlOutput, String)>,
    current_position: Option<CursorPosition>,
    callback: Option<Box<dyn FnMut(CursorPosition) + Send>>,
    configured_surfaces: usize,
    shm: Option<wl_shm::WlShm>,

    // Target screen information
    target_screen: Option<ScreenInfo>,

    // Dynamic input region fields
    last_cursor_pos: Option<(i32, i32)>,
    input_regions: Vec<Option<wl_region::WlRegion>>,
    hole_radius: i32, // Size of the transparent square hole around cursor (30px to each side)
}

impl CursorTracker {
    /// Create a rectangular region with a square hole in the input region
    /// Returns a list of rectangles that form the donut shape (full area minus square)
    fn create_donut_rectangles(
        surface_width: i32,
        surface_height: i32,
        center_x: i32,
        center_y: i32,
        radius: i32,
    ) -> Vec<(i32, i32, i32, i32)> {
        let mut rectangles = Vec::new();

        let left = (center_x - radius).max(0);
        let right = (center_x + radius).min(surface_width);
        let top = (center_y - radius).max(0);
        let bottom = (center_y + radius).min(surface_height);

        if top > 0 {
            rectangles.push((0, 0, surface_width, top));
        }

        if bottom < surface_height {
            rectangles.push((0, bottom, surface_width, surface_height - bottom));
        }

        if left > 0 {
            rectangles.push((0, top, left, bottom - top));
        }

        if right < surface_width {
            rectangles.push((right, top, surface_width - right, bottom - top));
        }

        rectangles
    }

    /// Update input region for target screen surface to create a hole around cursor position
    fn update_input_regions(&mut self, cursor_x: i32, cursor_y: i32) -> Result<(), CursorError> {
        log::debug!(
            "Updating input regions for global position ({}, {})",
            cursor_x,
            cursor_y
        );

        let target_screen = self.state.target_screen.as_ref().ok_or_else(|| {
            CursorError::ProtocolNotAvailable("Target screen not set".to_string())
        })?;

        // Only update input regions for the target screen
        for (surface_idx, (_output, output_name)) in self.state.outputs.iter().enumerate() {
            // Check if this output matches our target screen by name
            if output_name != &target_screen.name {
                log::debug!(
                    "Skipping surface {} ({}) - not the target screen ({})",
                    surface_idx,
                    output_name,
                    target_screen.name
                );
                continue;
            }

            // Calculate cursor position relative to this surface
            let surface_x = cursor_x - target_screen.position.x;
            let surface_y = cursor_y - target_screen.position.y;

            // Calculate physical dimensions
            let physical_width =
                (target_screen.logical_size.width as f32 / target_screen.scale_factor) as i32;
            let physical_height =
                (target_screen.logical_size.height as f32 / target_screen.scale_factor) as i32;

            log::debug!(
                "Surface {}: surface coords ({}, {}), physical bounds: {}x{}, logical bounds: {}x{} at ({}, {})",
                surface_idx,
                surface_x,
                surface_y,
                physical_width,
                physical_height,
                target_screen.logical_size.width,
                target_screen.logical_size.height,
                target_screen.position.x,
                target_screen.position.y
            );

            // Skip if cursor is not on this output (use physical bounds)
            if surface_x < 0
                || surface_y < 0
                || surface_x >= physical_width
                || surface_y >= physical_height
            {
                log::debug!(
                    "Skipping surface {} - cursor not on this output",
                    surface_idx
                );
                continue;
            }

            // Always update for now - we'll optimize this later if needed
            // The threshold logic was causing issues with responsiveness
            log::debug!(
                "Updating input region at surface coords ({}, {})",
                surface_x,
                surface_y
            );

            // Destroy old region if exists
            if let Some(old_region) = self.state.input_regions[surface_idx].take() {
                old_region.destroy();
            }

            // Use physical size for input region calculation
            let physical_width =
                (target_screen.logical_size.width as f32 / target_screen.scale_factor) as i32;
            let physical_height =
                (target_screen.logical_size.height as f32 / target_screen.scale_factor) as i32;

            // Create new input region with square hole
            let rectangles = Self::create_donut_rectangles(
                physical_width,
                physical_height,
                surface_x,
                surface_y,
                self.state.hole_radius,
            );

            let rect_count = rectangles.len();
            log::debug!("Generated {} rectangles for square hole", rect_count);

            if !rectangles.is_empty() {
                let region = self
                    .state
                    .compositor
                    .as_mut()
                    .ok_or_else(|| CursorError::ProtocolNotAvailable("wl_compositor".to_string()))?
                    .create_region(&self.queue.handle(), ());

                // Add all rectangles to the region
                for (x, y, width, height) in rectangles {
                    region.add(x, y, width, height);
                }

                // Set the new input region
                if let Some(surface) = self.state.surfaces.get(surface_idx) {
                    surface.set_input_region(Some(&region));
                    surface.commit();
                }

                self.state.input_regions[surface_idx] = Some(region);
                log::debug!(
                    "Updated input region for surface {} at ({}, {}) with {} rectangles",
                    surface_idx,
                    surface_x,
                    surface_y,
                    rect_count
                );
            }
        }

        self.state.last_cursor_pos = Some((cursor_x, cursor_y));
        Ok(())
    }

    pub fn new(screen_info: ScreenInfo) -> Result<Self, CursorError> {
        let conn = Connection::connect_to_env()
            .map_err(|e| CursorError::ConnectionFailed(e.to_string()))?;

        let mut queue: EventQueue<CursorTrackerState> = conn.new_event_queue();
        let mut state = CursorTrackerState::default();

        let display = conn.display();
        let _registry = display.get_registry(&queue.handle(), ());

        queue.dispatch_pending(&mut state)?;
        queue.roundtrip(&mut state)?;

        if state.compositor.is_none() {
            return Err(CursorError::ProtocolNotAvailable(
                "wl_compositor".to_string(),
            ));
        }
        if state.layer_shell.is_none() {
            return Err(CursorError::ProtocolNotAvailable(
                "zwlr_layer_shell_v1".to_string(),
            ));
        }

        if state.seat.is_none() {
            return Err(CursorError::ProtocolNotAvailable("wl_seat".to_string()));
        }

        if state.shm.is_none() {
            return Err(CursorError::ProtocolNotAvailable("wl_shm".to_string()));
        }

        log::info!(
            "CursorTracker created successfully with {} outputs",
            state.outputs.len()
        );

        // Set target screen and hole radius while preserving other state
        state.target_screen = Some(screen_info);
        state.hole_radius = 30;

        Ok(Self { queue, state })
    }

    pub fn start_tracking(&mut self) -> Result<(), CursorError> {
        self.queue.dispatch_pending(&mut self.state)?;
        self.queue.roundtrip(&mut self.state)?;

        log::info!(
            "Found {} outputs during start_tracking",
            self.state.outputs.len()
        );
        for (i, (_output, name)) in self.state.outputs.iter().enumerate() {
            log::info!("Output {}: {}", i, name);
        }

        // Wait for output information to be populated (we need screen names)
        let mut attempts = 0;
        while self.state.outputs.iter().any(|(_, name)| name.is_empty()) && attempts < 50 {
            self.queue.blocking_dispatch(&mut self.state)?;
            attempts += 1;
            log::debug!("Waiting for output names, attempt {}", attempts);
        }

        log::info!("After waiting, outputs count: {}", self.state.outputs.len());
        for (i, (_output, name)) in self.state.outputs.iter().enumerate() {
            log::info!("Final Output {}: {}", i, name);
        }

        let mut surfaces = Vec::new();
        let mut layer_surfaces = Vec::new();
        let mut surface_buffers = Vec::new();

        // Only create surfaces for the target screen
        let target_screen = self.state.target_screen.as_ref().ok_or_else(|| {
            CursorError::ProtocolNotAvailable("Target screen not set".to_string())
        })?;

        for (output, output_name) in self.state.outputs.iter() {
            // Check if this output matches our target screen by name
            if output_name != &target_screen.name {
                log::debug!(
                    "Skipping output {} ({}) - not the target screen ({})",
                    output.id(),
                    output_name,
                    target_screen.name
                );
                continue;
            }

            log::info!("Creating surface for target screen: {}", target_screen.name);
            let surface = self
                .state
                .compositor
                .as_mut()
                .unwrap()
                .create_surface(&self.queue.handle(), ());

            // Use Overlay layer but with explicit exclusive zone settings
            let layer_surface = self.state.layer_shell.as_mut().unwrap().get_layer_surface(
                &surface,
                Some(output),
                zwlr_layer_shell_v1::Layer::Overlay,
                "wayshot".to_string(),
                &self.queue.handle(),
                (),
            );

            // Set anchors to cover the entire output
            layer_surface.set_anchor(
                zwlr_layer_surface_v1::Anchor::Top
                    | zwlr_layer_surface_v1::Anchor::Bottom
                    | zwlr_layer_surface_v1::Anchor::Left
                    | zwlr_layer_surface_v1::Anchor::Right,
            );

            log::debug!("Layer surface anchors set to all four directions");
            // Use physical pixel size (logical size divided by scale factor)
            let physical_width =
                (target_screen.logical_size.width as f32 / target_screen.scale_factor) as i32;
            let physical_height =
                (target_screen.logical_size.height as f32 / target_screen.scale_factor) as i32;

            log::debug!(
                "Surface size calculation: logical={}x{}, scale_factor={}, physical={}x{}",
                target_screen.logical_size.width,
                target_screen.logical_size.height,
                target_screen.scale_factor,
                physical_width,
                physical_height
            );

            layer_surface.set_size(
                physical_width.try_into().unwrap(),
                physical_height.try_into().unwrap(),
            );
            layer_surface.set_margin(0, 0, 0, 0); // no margin
            layer_surface.set_exclusive_zone(-1); // -1 means no exclusive zone (don't affect other windows)
            layer_surface
                .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None); // No keyboard interaction needed for cursor tracking

            log::info!(
                "Configured layer surface: {}x{} (physical) at ({}, {}), logical: {}x{}, scale: {}",
                physical_width,
                physical_height,
                target_screen.position.x,
                target_screen.position.y,
                target_screen.logical_size.width,
                target_screen.logical_size.height,
                target_screen.scale_factor
            );

            // Don't set opaque region to allow mouse interaction while staying visually transparent

            // Create full input region initially (will be updated dynamically)
            let full_region = self
                .state
                .compositor
                .as_mut()
                .unwrap()
                .create_region(&self.queue.handle(), ());
            full_region.add(0, 0, physical_width, physical_height);
            surface.set_input_region(Some(&full_region));
            full_region.destroy();

            // Create buffer for this surface using memfd
            let buffer = if let Some(shm) = &self.state.shm {
                let width = physical_width as u32;
                let height = physical_height as u32;
                let stride = width * 4;
                let size = (stride * height) as i32;

                // Create memory file descriptor for pixel data
                let memfd = memfd::memfd_create(
                    c"wayshot_cursor",
                    memfd::MFdFlags::MFD_CLOEXEC | memfd::MFdFlags::MFD_ALLOW_SEALING,
                )
                .map_err(|e| {
                    CursorError::PointerFailed(format!("Failed to create memfd: {}", e))
                })?;

                // Set file size to accommodate the pixel data (width * height * 4 bytes)
                let total_bytes = size as u64;
                unsafe {
                    let mut file = std::fs::File::from_raw_fd(memfd.as_raw_fd());
                    file.set_len(total_bytes).map_err(|e| {
                        CursorError::PointerFailed(format!("Failed to set memfd size: {}", e))
                    })?;

                    file.seek(std::io::SeekFrom::Start(0)).map_err(|e| {
                        CursorError::PointerFailed(format!("Failed to seek memfd: {}", e))
                    })?;

                    let barely_visible_pixel = [0xffu8, 0x00, 0x00, 0x00]; // ARGB: barely visible (almost transparent)
                    for _ in 0..total_bytes / 4 {
                        file.write_all(&barely_visible_pixel).map_err(|e| {
                            CursorError::PointerFailed(format!("Failed to write to memfd: {}", e))
                        })?;
                    }

                    log::debug!("Filled memfd buffer with {}x{} pixels", width, height);

                    // Don't drop the file as it would close the fd.
                    // It will be closed when dropping the memfd.
                    std::mem::forget(file);
                }

                // Create pool and buffer from memfd
                let pool = shm.create_pool(memfd.as_fd(), size, &self.queue.handle(), ());
                let buffer = pool.create_buffer(
                    0,
                    width as i32,
                    height as i32,
                    stride as i32,
                    wl_shm::Format::Argb8888,
                    &self.queue.handle(),
                    (),
                );

                // The memfd will be kept alive by the pool
                Some(buffer)
            } else {
                log::warn!("SHM not available, surface may remain transparent");
                None
            };

            // Initial commit to trigger configure event - don't attach buffer yet
            surface.commit();
            log::debug!("Initial surface commit to trigger configure event");

            surfaces.push(surface.clone());
            layer_surfaces.push((layer_surface, output.clone()));
            surface_buffers.push(buffer);

            // Initialize input region storage
            self.state.input_regions.push(None);
        }

        log::debug!("Created {} layer surfaces for outputs", surfaces.len());

        self.state.surfaces.extend(surfaces.clone());
        self.state.layer_surfaces.extend(layer_surfaces);
        self.state.surface_buffers.extend(surface_buffers);

        // CRITICAL: Process events immediately after creating surfaces
        // This is needed to receive configure events from sway
        self.queue.dispatch_pending(&mut self.state)?;
        self.queue.roundtrip(&mut self.state)?;

        // Also try a blocking dispatch to ensure we process any pending events
        if let Err(e) = self.queue.blocking_dispatch(&mut self.state) {
            log::debug!("Blocking dispatch error (may be expected): {}", e);
        }

        log::info!("Waiting for {} surfaces to configure...", surfaces.len());
        let mut timeout_attempts = 0;
        while self.state.configured_surfaces < surfaces.len() && timeout_attempts < 100 {
            self.queue.blocking_dispatch(&mut self.state)?;
            log::debug!(
                "Configured surfaces: {}/{}, attempt {}",
                self.state.configured_surfaces,
                surfaces.len(),
                timeout_attempts
            );
            timeout_attempts += 1;
        }

        if self.state.configured_surfaces >= surfaces.len() {
            log::info!("All {} surfaces configured", surfaces.len());
        } else {
            log::warn!(
                "Timeout waiting for surface configuration - only {}/{} configured",
                self.state.configured_surfaces,
                surfaces.len()
            );
        }

        let seat = self
            .state
            .seat
            .as_ref()
            .ok_or_else(|| CursorError::ProtocolNotAvailable("wl_seat".to_string()))?;
        let pointer = seat.get_pointer(&self.queue.handle(), ());
        self.state.pointer = Some(pointer);

        Ok(())
    }
}

impl Dispatch<wl_output::WlOutput, ()> for CursorTrackerState {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_output::Event::Name { name } => {
                if let Some((_, output_name)) = state
                    .outputs
                    .iter_mut()
                    .find(|(o, _)| o.id() == output.id())
                {
                    *output_name = name.clone();
                    log::debug!("Output name updated: {}", name);
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for CursorTrackerState {
    fn event(
        state: &mut Self,
        _pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Process the event and return position data
        let new_position = match event {
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                log::info!(
                    "Pointer motion: surface_x={}, surface_y={}",
                    surface_x,
                    surface_y,
                );

                // Find the target screen and check if cursor is on it
                if let Some(target_screen) = &state.target_screen {
                    // Since target_screen.position is (0,0), surface coordinates should already be correct
                    // The issue might be that surface coordinates have an offset
                    let cursor_x = surface_x as i32;
                    let cursor_y = surface_y as i32;

                    // Debug: Log coordinate transformation
                    log::debug!(
                        "Coordinate info: surface=({:.2}, {:.2}) -> final=({}, {}), screen_pos=({}, {}), screen_size={}x{}",
                        surface_x,
                        surface_y,
                        cursor_x,
                        cursor_y,
                        target_screen.position.x,
                        target_screen.position.y,
                        target_screen.logical_size.width,
                        target_screen.logical_size.height
                    );

                    // Check if cursor is within target screen bounds (use physical bounds)
                    let physical_width = (target_screen.logical_size.width as f32
                        / target_screen.scale_factor)
                        as i32;
                    let physical_height = (target_screen.logical_size.height as f32
                        / target_screen.scale_factor)
                        as i32;

                    if surface_x >= 0.0
                        && surface_y >= 0.0
                        && surface_x < physical_width as f64
                        && surface_y < physical_height as f64
                    {
                        Some(CursorPosition {
                            x: cursor_x,
                            y: cursor_y,
                            output_x: target_screen.position.x,
                            output_y: target_screen.position.y,
                            output_width: target_screen.logical_size.width,
                            output_height: target_screen.logical_size.height,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                log::debug!(
                    "Pointer entered surface: id={}, at ({}, {})",
                    surface.id(),
                    surface_x,
                    surface_y
                );

                // Check if this is one of our surfaces
                if state.surfaces.iter().any(|s| s.id() == surface.id()) {
                    log::info!(
                        "Pointer entered OUR surface! Our surface IDs: {:?}",
                        state.surfaces.iter().map(|s| s.id()).collect::<Vec<_>>()
                    );
                } else {
                    log::debug!("Pointer entered OTHER surface (not ours)");
                }
                None
            }
            wl_pointer::Event::Leave { surface, .. } => {
                log::debug!("Pointer left surface: {:?}", surface);

                // Check if this is one of our surfaces
                if state.surfaces.iter().any(|s| s.id() == surface.id()) {
                    log::debug!("Pointer left OUR surface!");
                }
                None
            }
            wl_pointer::Event::Button {
                button,
                state: button_state,
                ..
            } => {
                log::debug!(
                    "Pointer button: button={}, state={:?}",
                    button,
                    button_state
                );
                None
            }
            wl_pointer::Event::Axis { axis, value, .. } => {
                log::debug!("Pointer axis: axis={:?}, value={}", axis, value);
                None
            }
            _ => None,
        };

        // Update state and callback if we have a new position
        if let Some(position) = new_position {
            state.current_position = Some(position);

            if let Some(ref mut callback) = state.callback {
                // Apply scaling transformation before calling callback
                let scaled_position = if let Some(target_screen) = &state.target_screen {
                    CursorPosition {
                        x: (position.x as f32 * target_screen.scale_factor) as i32,
                        y: (position.y as f32 * target_screen.scale_factor) as i32,
                        output_x: position.output_x,
                        output_y: position.output_y,
                        output_width: position.output_width,
                        output_height: position.output_height,
                    }
                } else {
                    position
                };
                
                callback(scaled_position);
            }

            // Set flag to indicate input region needs updating
            // This is a workaround since we can't directly call update_input_regions from here
            state.last_cursor_pos = Some((position.x, position.y));
        }
    }
}

// New struct to hold mutable reference for updating input regions
pub struct CursorTrackerUpdater<'a> {
    tracker: &'a mut CursorTracker,
}

impl<'a> CursorTrackerUpdater<'a> {
    pub fn update_input_regions(
        &mut self,
        global_x: i32,
        global_y: i32,
    ) -> Result<(), CursorError> {
        self.tracker.update_input_regions(global_x, global_y)
    }
}

// Helper function to update input regions from outside the event handler
impl CursorTracker {
    pub fn process_pending_input_updates(&mut self) -> Result<(), CursorError> {
        if let Some(position) = self.state.current_position {
            log::debug!(
                "Processing input region updates for position ({}, {})",
                position.x,
                position.y
            );
            self.update_input_regions(position.x, position.y)?;
        } else {
            log::debug!("No cursor position available for input region updates");
        }
        Ok(())
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for CursorTrackerState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_seat::Event::Capabilities { capabilities } => {
                // Check if pointer capability is available
                if let wayland_client::WEnum::Value(caps) = capabilities {
                    if caps.contains(wl_seat::Capability::Pointer) {
                        let pointer = seat.get_pointer(qh, ());
                        state.pointer = Some(pointer);
                        log::debug!("Pointer capability detected and assigned");
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for CursorTrackerState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                // log::debug!("Registry global: {} {} v{}", name, interface, version);

                match interface.as_str() {
                    "wl_compositor" => {
                        let compositor = registry.bind(name, version, qh, ());
                        state.compositor = Some(compositor);
                    }
                    "wl_seat" => {
                        let seat = registry.bind(name, version, qh, ());
                        state.seat = Some(seat);
                    }
                    "wl_output" => {
                        let output = registry.bind(name, version, qh, ());
                        state.outputs.push((output, String::new()));
                    }
                    "zwlr_layer_shell_v1" => {
                        let layer_shell = registry.bind(name, version, qh, ());
                        state.layer_shell = Some(layer_shell);
                    }
                    "wl_shm" => {
                        let shm = registry.bind(name, version, qh, ());
                        state.shm = Some(shm);
                    }
                    _ => {}
                }
            }
            wl_registry::Event::GlobalRemove { name } => {
                log::debug!("Registry global removed: {}", name);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for CursorTrackerState {
    fn event(
        _state: &mut Self,
        _compositor: &wl_compositor::WlCompositor,
        _event: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Compositor events are not typically used
    }
}

impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for CursorTrackerState {
    fn event(
        _state: &mut Self,
        _layer_shell: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        _event: zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Layer shell events are not typically used
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for CursorTrackerState {
    fn event(
        _state: &mut Self,
        _surface: &wl_surface::WlSurface,
        _event: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Surface events are not typically used
    }
}

impl Dispatch<wl_shm::WlShm, ()> for CursorTrackerState {
    fn event(
        _state: &mut Self,
        _shm: &wl_shm::WlShm,
        _event: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // SHM events are not typically used
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for CursorTrackerState {
    fn event(
        _state: &mut Self,
        _pool: &wl_shm_pool::WlShmPool,
        _event: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // SHM pool events are not typically used
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for CursorTrackerState {
    fn event(
        _state: &mut Self,
        _buffer: &wl_buffer::WlBuffer,
        _event: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Buffer events are not typically used
    }
}

impl Dispatch<wl_region::WlRegion, ()> for CursorTrackerState {
    fn event(
        _state: &mut Self,
        _region: &wl_region::WlRegion,
        _event: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Region events are not typically used
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for CursorTrackerState {
    fn event(
        state: &mut Self,
        layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        log::info!("Layer surface event received: {:?}", event);

        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                layer_surface.ack_configure(serial);
                state.configured_surfaces += 1;
                log::info!(
                    "Layer surface configured by compositor: {}x{} (total configured: {})",
                    width,
                    height,
                    state.configured_surfaces
                );

                // Log the expected vs actual dimensions
                if let Some(target_screen) = &state.target_screen {
                    let expected_width = (target_screen.logical_size.width as f32
                        / target_screen.scale_factor)
                        as i32;
                    let expected_height = (target_screen.logical_size.height as f32
                        / target_screen.scale_factor)
                        as i32;
                    log::info!(
                        "Expected physical size: {}x{}, Actual configured size: {}x{}",
                        expected_width,
                        expected_height,
                        width,
                        height
                    );

                    if width as i32 != expected_width || height as i32 != expected_height {
                        log::warn!(
                            "Surface size mismatch! Expected {}x{} but got {}x{}",
                            expected_width,
                            expected_height,
                            width,
                            height
                        );
                    }
                }

                // Find the surface associated with this layer surface and attach pre-created buffer
                if let Some(surface_idx) = state
                    .layer_surfaces
                    .iter()
                    .position(|(ls, _)| ls.id() == layer_surface.id())
                {
                    if let (Some(surface), Some(buffer)) = (
                        state.surfaces.get(surface_idx),
                        state
                            .surface_buffers
                            .get(surface_idx)
                            .and_then(|b| b.as_ref()),
                    ) {
                        // Attach pre-created buffer to surface and commit
                        let (output_name, target_screen) = if let (Some(name), Some(screen)) = (
                            state.outputs.get(surface_idx).map(|(_, name)| name.clone()),
                            &state.target_screen,
                        ) {
                            (name, screen)
                        } else {
                            log::warn!("Could not get output info for surface {}", surface_idx);
                            return;
                        };

                        // Use physical size for buffer attachment
                        let physical_width = (target_screen.logical_size.width as f32
                            / target_screen.scale_factor)
                            as i32;
                        let physical_height = (target_screen.logical_size.height as f32
                            / target_screen.scale_factor)
                            as i32;
                        let width = physical_width;
                        let height = physical_height;

                        surface.attach(Some(buffer), 0, 0);
                        surface.damage(0, 0, width, height);
                        surface.commit();
                        log::info!(
                            "Attached buffer and committed surface - blue overlay should now be visible! ({}x{}) on {}",
                            width,
                            height,
                            output_name
                        );
                    } else {
                        log::warn!(
                            "Could not find surface or buffer to commit after configuration"
                        );
                        // Still commit the surface even without buffer
                        if let Some(surface) = state.surfaces.get(surface_idx) {
                            surface.commit();
                            log::info!("Committed surface without buffer");
                        }
                    }
                } else {
                    log::warn!("Could not find layer surface in configuration");
                }
            }
            zwlr_layer_surface_v1::Event::Closed => {
                log::debug!("Layer surface closed");
            }
            _ => {}
        }
    }
}

pub fn monitor_cursor_position<F>(
    stop_sig: Arc<AtomicBool>,
    screen_info: ScreenInfo,
    callback: F,
) -> Result<(), CursorError>
where
    F: FnMut(CursorPosition) + Send + 'static,
{
    let mut tracker = CursorTracker::new(screen_info)?;
    tracker.state.callback = Some(Box::new(callback));
    tracker.start_tracking()?;

    loop {
        if stop_sig.load(Ordering::Relaxed) {
            break;
        }

        if let Err(e) = tracker.queue.dispatch_pending(&mut tracker.state) {
            log::warn!("Dispatch pending error: {}", e);
        }

        // Process input region updates immediately after dispatching events
        if let Err(e) = tracker.process_pending_input_updates() {
            log::warn!("Input region update error: {}", e);
        }

        // Try to process any pending events immediately
        if let Err(e) = tracker.queue.blocking_dispatch(&mut tracker.state) {
            log::debug!("Blocking dispatch error (may be expected): {}", e);
        }

        if let Err(e) = tracker.queue.roundtrip(&mut tracker.state) {
            log::warn!("Roundtrip  error: {}", e);
        }

        // Reduced sleep time for better responsiveness
        std::thread::sleep(Duration::from_millis(1));
    }

    Ok(())
}

pub fn get_cursor_position(screen_info: ScreenInfo) -> Result<Option<CursorPosition>, CursorError> {
    let mut tracker = CursorTracker::new(screen_info)?;
    tracker.start_tracking()?;

    let mut attempts = 0;
    while tracker.state.current_position.is_none() && attempts < 10 {
        tracker.queue.dispatch_pending(&mut tracker.state)?;
        tracker.queue.roundtrip(&mut tracker.state)?;
        std::thread::sleep(Duration::from_millis(20));
        attempts += 1;
    }

    log::info!(
        "get_cursor_position completed after {} attempts, position: {:?}",
        attempts,
        tracker.state.current_position
    );
    Ok(tracker.state.current_position)
}
