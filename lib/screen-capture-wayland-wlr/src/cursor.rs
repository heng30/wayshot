use derive_setters::Setters;
use nix::sys::memfd;
use screen_capture::ScreenInfo;
use std::{
    fs::File,
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

    #[error("Failed to get configurations from Wayland server: {0}")]
    ConfigurationFailed(String),

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
    seat: Option<wl_seat::WlSeat>,
    shm: Option<wl_shm::WlShm>,

    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    layer_surfaces: Vec<(
        zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        wl_output::WlOutput,
    )>,

    pointer: Option<wl_pointer::WlPointer>,

    surfaces: Vec<wl_surface::WlSurface>,
    surface_buffers: Vec<Option<wl_buffer::WlBuffer>>,

    current_position: Option<CursorPosition>,
    target_screen: ScreenInfo,
    outputs: Vec<(wl_output::WlOutput, String)>,
    callback: Option<Box<dyn FnMut(CursorPosition) + Send>>,
    configured_surfaces: usize,

    // Dynamic input region fields
    last_cursor_pos: Option<(i32, i32)>,
    input_regions: Vec<Option<wl_region::WlRegion>>,

    hole_radius: i32, // Size of the transparent square hole around cursor (30px to each side)
    use_transparent_layer_surface: bool,
}

impl CursorTracker {
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

        state.hole_radius = 30;
        state.target_screen = screen_info;
        state.use_transparent_layer_surface = true;

        Ok(Self { queue, state })
    }

    fn wait_for_output_names(&mut self) -> Result<(), CursorError> {
        let mut attempts = 0;
        while self.state.outputs.iter().any(|(_, name)| name.is_empty()) && attempts < 10 {
            self.queue.blocking_dispatch(&mut self.state)?;
            attempts += 1;
        }

        if self.state.outputs.is_empty() {
            return Err(CursorError::ConnectionFailed(
                "No found outputs".to_string(),
            ));
        }
        Ok(())
    }

    fn create_surface_buffer(
        &self,
        physical_width: i32,
        physical_height: i32,
    ) -> Result<Option<wl_buffer::WlBuffer>, CursorError> {
        if let Some(shm) = &self.state.shm {
            let width = physical_width as u32;
            let height = physical_height as u32;
            let stride = width * 4;
            let size = (stride * height) as i32;

            let memfd = memfd::memfd_create(
                c"wayshot_cursor",
                memfd::MFdFlags::MFD_CLOEXEC | memfd::MFdFlags::MFD_ALLOW_SEALING,
            )
            .map_err(|e| CursorError::PointerFailed(format!("Failed to create memfd: {}", e)))?;

            let total_bytes = size as u64;
            unsafe {
                let mut file = File::from_raw_fd(memfd.as_raw_fd());
                file.set_len(total_bytes).map_err(|e| {
                    CursorError::PointerFailed(format!("Failed to set memfd size: {}", e))
                })?;

                file.seek(std::io::SeekFrom::Start(0)).map_err(|e| {
                    CursorError::PointerFailed(format!("Failed to seek memfd: {}", e))
                })?;

                let pixels = [
                    if self.state.use_transparent_layer_surface {
                        0x00u8
                    } else {
                        0xFFu8
                    },
                    0x00,
                    0x00,
                    0x00,
                ];
                for _ in 0..total_bytes / 4 {
                    file.write_all(&pixels).map_err(|e| {
                        CursorError::PointerFailed(format!("Failed to write to memfd: {}", e))
                    })?;
                }

                std::mem::forget(file); // memfd own the raw fd
            }

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

            Ok(Some(buffer)) // own the memfd
        } else {
            Ok(None)
        }
    }

    fn create_layer_surface(
        &mut self,
        output: &wl_output::WlOutput,
    ) -> Result<
        (
            wl_surface::WlSurface,
            zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
            Option<wl_buffer::WlBuffer>,
        ),
        CursorError,
    > {
        let target_screen = &self.state.target_screen;
        let surface = self
            .state
            .compositor
            .as_mut()
            .ok_or_else(|| CursorError::ProtocolNotAvailable("wl_compositor".to_string()))?
            .create_surface(&self.queue.handle(), ());

        let layer_surface = self
            .state
            .layer_shell
            .as_mut()
            .ok_or_else(|| CursorError::ProtocolNotAvailable("zwlr_layer_shell_v1".to_string()))?
            .get_layer_surface(
                &surface,
                Some(output),
                zwlr_layer_shell_v1::Layer::Overlay,
                "wayshot_layer_surface".to_string(),
                &self.queue.handle(),
                (),
            );

        layer_surface.set_anchor(
            zwlr_layer_surface_v1::Anchor::Top
                | zwlr_layer_surface_v1::Anchor::Bottom
                | zwlr_layer_surface_v1::Anchor::Left
                | zwlr_layer_surface_v1::Anchor::Right,
        );

        let physical_width =
            (target_screen.logical_size.width as f32 / target_screen.scale_factor) as i32;
        let physical_height =
            (target_screen.logical_size.height as f32 / target_screen.scale_factor) as i32;

        layer_surface.set_size(
            physical_width.try_into().unwrap(),
            physical_height.try_into().unwrap(),
        );
        layer_surface.set_margin(0, 0, 0, 0);
        layer_surface.set_exclusive_zone(-1);
        layer_surface
            .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);

        let full_region = self
            .state
            .compositor
            .as_mut()
            .ok_or_else(|| CursorError::ProtocolNotAvailable("wl_compositor".to_string()))?
            .create_region(&self.queue.handle(), ());
        full_region.add(0, 0, physical_width, physical_height);
        surface.set_input_region(Some(&full_region));
        full_region.destroy();

        let buffer = self.create_surface_buffer(physical_width, physical_height)?;
        surface.commit();

        Ok((surface, layer_surface, buffer))
    }

    fn wait_for_surface_configuration(&mut self, expected_count: usize) -> Result<(), CursorError> {
        self.queue.dispatch_pending(&mut self.state)?;
        self.queue.roundtrip(&mut self.state)?;
        self.queue.blocking_dispatch(&mut self.state)?;

        let mut timeout_attempts = 0;
        while self.state.configured_surfaces < expected_count && timeout_attempts < 10 {
            self.queue.blocking_dispatch(&mut self.state)?;
            timeout_attempts += 1;
        }

        if self.state.configured_surfaces < expected_count {
            return Err(CursorError::ConfigurationFailed(format!(
                "Timeout waiting for surface configuration - only {}/{} configured",
                self.state.configured_surfaces, expected_count
            )));
        }

        Ok(())
    }

    fn setup_pointer(&mut self) -> Result<(), CursorError> {
        let seat = self
            .state
            .seat
            .as_ref()
            .ok_or_else(|| CursorError::ProtocolNotAvailable("wl_seat".to_string()))?;
        let pointer = seat.get_pointer(&self.queue.handle(), ());
        self.state.pointer = Some(pointer);
        Ok(())
    }

    /// Create a rectangular region with a square hole in the input region
    /// Returns a list of rectangles that form the donut shape (full area minus square)
    /// return: (x, y, w, h)
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

    fn update_surface_input_region(
        &mut self,
        surface_idx: usize,
        surface_x: i32,
        surface_y: i32,
        physical_width: i32,
        physical_height: i32,
    ) -> Result<(), CursorError> {
        if let Some(old_region) = self.state.input_regions[surface_idx].take() {
            old_region.destroy();
        }

        let rectangles = Self::create_donut_rectangles(
            physical_width,
            physical_height,
            surface_x,
            surface_y,
            self.state.hole_radius,
        );

        if !rectangles.is_empty() {
            let region = self
                .state
                .compositor
                .as_mut()
                .ok_or_else(|| CursorError::ProtocolNotAvailable("wl_compositor".to_string()))?
                .create_region(&self.queue.handle(), ());

            for (x, y, width, height) in rectangles {
                region.add(x, y, width, height);
            }

            if let Some(surface) = self.state.surfaces.get(surface_idx) {
                surface.set_input_region(Some(&region));
                surface.commit();
            }

            self.state.input_regions[surface_idx] = Some(region);
        }

        Ok(())
    }

    fn update_input_regions(&mut self, cursor_x: i32, cursor_y: i32) -> Result<(), CursorError> {
        let target_screen_name = self.state.target_screen.name.clone();

        let target_surfaces: Vec<usize> = self
            .state
            .outputs
            .iter()
            .enumerate()
            .filter(|(_, (_, name))| name == &target_screen_name)
            .map(|(idx, _)| idx)
            .collect();

        let target_position = self.state.target_screen.position;
        let target_logical_size = self.state.target_screen.logical_size;
        let target_scale_factor = self.state.target_screen.scale_factor;

        for surface_idx in target_surfaces {
            let surface_x = cursor_x - target_position.x;
            let surface_y = cursor_y - target_position.y;

            let physical_width = (target_logical_size.width as f32 / target_scale_factor) as i32;
            let physical_height = (target_logical_size.height as f32 / target_scale_factor) as i32;

            if surface_x < 0
                || surface_y < 0
                || surface_x >= physical_width
                || surface_y >= physical_height
            {
                continue;
            }

            self.update_surface_input_region(
                surface_idx,
                surface_x,
                surface_y,
                physical_width,
                physical_height,
            )?;
        }

        self.state.last_cursor_pos = Some((cursor_x, cursor_y));
        Ok(())
    }

    pub fn process_pending_input_updates(&mut self) -> Result<(), CursorError> {
        if let Some(position) = self.state.current_position {
            self.update_input_regions(position.x, position.y)?;
        }
        Ok(())
    }

    pub fn start_tracking(&mut self) -> Result<(), CursorError> {
        self.queue.dispatch_pending(&mut self.state)?;
        self.queue.roundtrip(&mut self.state)?;

        self.wait_for_output_names()?;

        let target_screen_name = self.state.target_screen.name.clone();
        let target_outputs: Vec<(wl_output::WlOutput, String)> = self
            .state
            .outputs
            .iter()
            .filter(|(_, name)| name == &target_screen_name)
            .map(|(output, name)| (output.clone(), name.clone()))
            .collect();

        if target_outputs.is_empty() {
            return Err(CursorError::ConnectionFailed(format!(
                "No found output of {}",
                target_screen_name
            )));
        }

        let mut surfaces = Vec::new();
        let mut layer_surfaces = Vec::new();
        let mut surface_buffers = Vec::new();

        for (output, _) in target_outputs {
            let (surface, layer_surface, buffer) = self.create_layer_surface(&output)?;

            surfaces.push(surface.clone());
            layer_surfaces.push((layer_surface, output));
            surface_buffers.push(buffer);

            self.state.input_regions.push(None);
        }

        let surfaces_len = surfaces.len();
        self.state.surfaces.extend(surfaces);
        self.state.layer_surfaces.extend(layer_surfaces);
        self.state.surface_buffers.extend(surface_buffers);

        self.wait_for_surface_configuration(surfaces_len)?;
        self.setup_pointer()?;

        Ok(())
    }
}

// This callback will be called firstly
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
            } => match interface.as_str() {
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
            },
            wl_registry::Event::GlobalRemove { name: _ } => {}
            _ => {}
        }
    }
}

// Get all screens' name of output
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
                }
            }
            _ => {}
        }
    }
}

// Attach surfaces buffer to layer surfaces
impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for CursorTrackerState {
    fn event(
        state: &mut Self,
        layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                handle_surface_configure(state, layer_surface, serial, width, height);
            }
            zwlr_layer_surface_v1::Event::Closed => {}
            _ => {}
        }
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
                if let wayland_client::WEnum::Value(caps) = capabilities {
                    if caps.contains(wl_seat::Capability::Pointer) {
                        let pointer = seat.get_pointer(qh, ());
                        state.pointer = Some(pointer);
                    }
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
        let new_position = match event {
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => handle_motion_event(surface_x, surface_y, &state.target_screen),
            wl_pointer::Event::Enter { .. } => None,
            wl_pointer::Event::Leave { .. } => None,
            wl_pointer::Event::Button { .. } => None,
            wl_pointer::Event::Axis { .. } => None,
            _ => None,
        };

        if let Some(position) = new_position {
            update_cursor_state(state, position);
        }
    }
}

fn handle_motion_event(
    surface_x: f64,
    surface_y: f64,
    target_screen: &ScreenInfo,
) -> Option<CursorPosition> {
    let cursor_x = surface_x as i32;
    let cursor_y = surface_y as i32;

    let physical_width =
        (target_screen.logical_size.width as f32 / target_screen.scale_factor) as i32;
    let physical_height =
        (target_screen.logical_size.height as f32 / target_screen.scale_factor) as i32;

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
}

fn update_cursor_state(state: &mut CursorTrackerState, position: CursorPosition) {
    state.current_position = Some(position);

    if let Some(ref mut callback) = state.callback {
        let scaled_position = CursorPosition {
            x: (position.x as f32 * state.target_screen.scale_factor) as i32,
            y: (position.y as f32 * state.target_screen.scale_factor) as i32,
            output_x: position.output_x,
            output_y: position.output_y,
            output_width: position.output_width,
            output_height: position.output_height,
        };
        callback(scaled_position);
    }

    state.last_cursor_pos = Some((position.x, position.y));
}

fn handle_surface_configure(
    state: &mut CursorTrackerState,
    layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    serial: u32,
    _width: u32,
    _height: u32,
) {
    layer_surface.ack_configure(serial);
    state.configured_surfaces += 1;

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
            let target_screen = &state.target_screen;
            let physical_width =
                (target_screen.logical_size.width as f32 / target_screen.scale_factor) as i32;
            let physical_height =
                (target_screen.logical_size.height as f32 / target_screen.scale_factor) as i32;

            surface.attach(Some(buffer), 0, 0);
            surface.damage(0, 0, physical_width, physical_height);
            surface.commit();
        } else if let Some(surface) = state.surfaces.get(surface_idx) {
            surface.commit();
        }
    }
}

#[derive(Debug, Clone, Setters)]
#[setters(prefix = "with_")]
pub struct MonitorCursorPositionConfig {
    #[setters(skip)]
    pub screen_info: ScreenInfo,

    pub use_transparent_layer_surface: bool,
    pub hole_radius: i32,
}

impl MonitorCursorPositionConfig {
    pub fn new(screen_info: ScreenInfo) -> Self {
        Self {
            screen_info,
            use_transparent_layer_surface: true,
            hole_radius: 30,
        }
    }
}

pub fn monitor_cursor_position<F>(
    config: MonitorCursorPositionConfig,
    stop_sig: Arc<AtomicBool>,
    callback: F,
) -> Result<(), CursorError>
where
    F: FnMut(CursorPosition) + Send + 'static,
{
    let mut tracker = CursorTracker::new(config.screen_info)?;
    tracker.state.callback = Some(Box::new(callback));
    tracker.state.use_transparent_layer_surface = config.use_transparent_layer_surface;
    tracker.state.hole_radius = config.hole_radius;
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
        std::thread::sleep(Duration::from_millis(5));
    }

    Ok(())
}

impl Dispatch<wl_compositor::WlCompositor, ()> for CursorTrackerState {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for CursorTrackerState {
    fn event(
        _: &mut Self,
        _: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        _: zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for CursorTrackerState {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm::WlShm, ()> for CursorTrackerState {
    fn event(
        _: &mut Self,
        _: &wl_shm::WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for CursorTrackerState {
    fn event(
        _: &mut Self,
        _: &wl_shm_pool::WlShmPool,
        _: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for CursorTrackerState {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_region::WlRegion, ()> for CursorTrackerState {
    fn event(
        _: &mut Self,
        _: &wl_region::WlRegion,
        _: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
