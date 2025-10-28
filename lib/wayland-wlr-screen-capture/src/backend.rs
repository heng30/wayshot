use nix::sys::memfd;
use screen_capture::{LogicalSize, Position};
use std::{
    os::fd::{AsFd, AsRawFd},
    os::unix::io::FromRawFd,
};
use wayland_client::{
    self, Connection, Dispatch, QueueHandle,
    protocol::{wl_buffer, wl_callback, wl_output, wl_registry, wl_shm, wl_shm_pool},
};
use wayland_protocols::xdg::xdg_output::zv1::client::{zxdg_output_manager_v1, zxdg_output_v1};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1, zwlr_screencopy_manager_v1,
};

#[derive(Debug)]
pub(crate) struct OutputInfo {
    /// Wayland output object
    pub wl_output: wl_output::WlOutput,

    /// Name of the output
    pub name: Option<String>,

    /// Logical position of the output in compositor space
    pub output_logical_position: Option<Position>,

    /// Logical size of the output
    pub output_logical_size: Option<LogicalSize>,

    /// Output transformation (rotation, flipping)
    pub transform: Option<wl_output::Transform>,

    /// Scale factor of the output
    pub scale_factor: i32,

    /// Memory file descriptor for image data
    pub image_memfd: Option<std::os::fd::OwnedFd>,

    /// Memory mapping for image data
    pub image_mmap: Option<memmap2::MmapMut>,

    /// Size of the memory-mapped image
    pub image_mmap_size: Option<LogicalSize>,

    /// Logical position of the captured image
    pub image_logical_position: Option<Position>,

    /// Logical size of the captured image
    pub image_logical_size: Option<LogicalSize>,

    /// Pixel format of the captured image
    pub image_pixel_format: Option<wl_shm::Format>,

    /// Whether the image capture is complete
    pub image_ready: bool,

    pub wlsh_pool: Option<wl_shm_pool::WlShmPool>,

    pub wl_buffer: Option<wl_buffer::WlBuffer>,
}

impl Drop for OutputInfo {
    fn drop(&mut self) {
        log::debug!("Cleaning up OutputInfo resources");

        // Release Wayland output object
        self.wl_output.release();

        // Clean up memory mapping first
        if let Some(mmap) = self.image_mmap.take() {
            drop(mmap);
        }

        // Clean up memory file descriptor
        if let Some(memfd) = self.image_memfd.take() {
            drop(memfd);
        }

        log::debug!("OutputInfo resources cleaned up");
    }
}

#[derive(Default, Debug)]
pub(crate) struct State {
    /// Whether global enumeration is complete
    pub done: bool,

    /// WLR screencopy manager for capturing screens
    pub wlr_screencopy_manager: Option<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1>,

    /// XDG output manager for output information
    pub xdg_output_manager: Option<zxdg_output_manager_v1::ZxdgOutputManagerV1>,

    /// Shared memory manager for buffer creation
    pub wl_shm: Option<wl_shm::WlShm>,

    /// Information about all available outputs
    pub output_infos: Vec<OutputInfo>,
}

impl Drop for State {
    fn drop(&mut self) {
        self.wlr_screencopy_manager.take();
        self.xdg_output_manager.take();
        self.wl_shm.take();
        self.output_infos.clear();
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut State,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version: _,
        } = event
        {
            match &interface[..] {
                // Get the screencopy manager (used to request capture of an output)
                "zwlr_screencopy_manager_v1" => {
                    let wlr_screencopy_manager =
                        registry.bind::<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, _, _>(
                            name,
                            3,
                            queue_handle,
                            (),
                        );

                    state.wlr_screencopy_manager = Some(wlr_screencopy_manager);
                }
                // Get the xdg output manager (used to obtain information of outputs)
                "zxdg_output_manager_v1" => {
                    let xdg_output_manager = registry
                        .bind::<zxdg_output_manager_v1::ZxdgOutputManagerV1, _, _>(
                            name,
                            3,
                            queue_handle,
                            (),
                        );

                    state.xdg_output_manager = Some(xdg_output_manager);
                }
                // Get the shared memory object (used to create shared memory pools)
                "wl_shm" => {
                    let wl_shm = registry.bind::<wl_shm::WlShm, _, _>(name, 1, queue_handle, ());

                    state.wl_shm = Some(wl_shm);
                }
                // Get the outputs for capture
                "wl_output" => {
                    let wl_output = registry.bind::<wl_output::WlOutput, _, _>(
                        name,
                        4,
                        queue_handle,
                        state.output_infos.len(),
                    );

                    // Create a new output info entry with default values
                    state.output_infos.push(OutputInfo {
                        wl_output,
                        name: None,
                        output_logical_position: None,
                        output_logical_size: None,
                        transform: None,
                        scale_factor: 1,
                        image_memfd: None,
                        image_mmap: None,
                        image_mmap_size: None,
                        image_logical_position: None,
                        image_logical_size: None,
                        image_pixel_format: None,
                        image_ready: false,
                        wlsh_pool: None,
                        wl_buffer: None,
                    });
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for State {
    fn event(
        state: &mut State,
        _wl_callback: &wl_callback::WlCallback,
        event: wl_callback::Event,
        _: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { callback_data: _ } = event {
            state.done = true;
        }
    }
}

impl Dispatch<wl_output::WlOutput, usize> for State {
    fn event(
        state: &mut State,
        _wl_output: &wl_output::WlOutput,
        event: wl_output::Event,
        index: &usize,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        // Store relevant information in the output_info
        match event {
            wl_output::Event::Geometry { transform, .. } => {
                state.output_infos[*index].transform = transform.into_result().ok();
            }
            wl_output::Event::Name { name } => {
                state.output_infos[*index].name = Some(name);
            }
            wl_output::Event::Scale { factor } => {
                // FIXME: factor is not correct in `sway`
                state.output_infos[*index].scale_factor = factor;
            }
            _ => {}
        }
    }
}

impl Dispatch<zxdg_output_v1::ZxdgOutputV1, usize> for State {
    fn event(
        state: &mut State,
        _xdg_output: &zxdg_output_v1::ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        index: &usize,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        match event {
            // Logical position is the position in the compositor space accounting for transforms
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                state.output_infos[*index].output_logical_position = Some(Position::new(x, y));
            }
            // Like logical position but for size
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                state.output_infos[*index].output_logical_size =
                    Some(LogicalSize::new(width, height));
            }
            _ => {}
        }
    }
}

impl Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, usize> for State {
    fn event(
        state: &mut State,
        wlr_screencopy_frame: &zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        index: &usize,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                let format = format.into_result().unwrap();

                // Check for valid pixel format (only support common 32-bit formats)
                state.output_infos[*index].image_pixel_format = match format {
                    wl_shm::Format::Argb8888
                    | wl_shm::Format::Xrgb8888
                    | wl_shm::Format::Xbgr8888 => Some(format),
                    _ => return,
                };

                // Store the dimensions of the capture
                state.output_infos[*index].image_mmap_size =
                    Some(LogicalSize::new(width as i32, height as i32));

                // Reuse existing memfd if possible, otherwise create new one
                let memfd = if let Some(existing_memfd) = &state.output_infos[*index].image_memfd {
                    // Check if existing memfd is the right size
                    let required_size = (width * height * 4) as u64;
                    unsafe {
                        let file = std::fs::File::from_raw_fd(existing_memfd.as_raw_fd());
                        let current_size = file.metadata().unwrap().len();
                        if current_size >= required_size {
                            std::mem::forget(file);

                            // Reuse existing memfd
                            existing_memfd.try_clone().unwrap()
                        } else {
                            // Resize existing memfd to required size
                            file.set_len(required_size).unwrap();

                            std::mem::forget(file);
                            existing_memfd.try_clone().unwrap()
                        }
                    }
                } else {
                    // Allocate new memory file descriptor for pixel data
                    let memfd = memfd::memfd_create(
                        c"wayshot",
                        memfd::MFdFlags::MFD_CLOEXEC | memfd::MFdFlags::MFD_ALLOW_SEALING,
                    )
                    .unwrap();

                    // Set file size to accommodate the pixel data (width * height * 4 bytes)
                    unsafe {
                        let file = std::fs::File::from_raw_fd(memfd.as_raw_fd());
                        file.set_len((width * height * 4) as u64)
                            .expect("Failed to allocate memory for screencopy.");

                        // Don't drop the file as it would close the fd
                        // It will be closed when dropping `image_memfd`
                        std::mem::forget(file);
                    }

                    memfd
                };

                state.output_infos[*index].image_memfd = Some(memfd);

                if state.output_infos[*index].wlsh_pool.is_none() {
                    // Create shared memory pool from the memory file descriptor
                    let wl_shm_pool = state.wl_shm.as_ref().unwrap().create_pool(
                        state.output_infos[*index]
                            .image_memfd
                            .as_ref()
                            .unwrap()
                            .as_fd(),
                        (width * height * 4) as i32,
                        queue_handle,
                        (),
                    );

                    state.output_infos[*index].wlsh_pool = Some(wl_shm_pool);
                }

                // Create Wayland buffer from the shared memory pool
                if state.output_infos[*index].wl_buffer.is_none() {
                    let wl_buffer = state.output_infos[*index]
                        .wlsh_pool
                        .as_ref()
                        .unwrap()
                        .create_buffer(
                            0,
                            width as i32,
                            height as i32,
                            stride as i32,
                            format,
                            queue_handle,
                            (),
                        );
                    state.output_infos[*index].wl_buffer = Some(wl_buffer);
                }

                // Request the compositor to copy screen data into our buffer
                let wl_buffer = state.output_infos[*index].wl_buffer.as_ref().unwrap();
                wlr_screencopy_frame.copy(&wl_buffer);
            }
            // Buffer has been filled with screen data
            zwlr_screencopy_frame_v1::Event::Ready {
                tv_sec_hi: _,
                tv_sec_lo: _,
                tv_nsec: _,
            } => {
                // Reuse existing mmap if possible, otherwise create new one
                if state.output_infos[*index].image_mmap.is_none() {
                    unsafe {
                        let file = std::fs::File::from_raw_fd(
                            state.output_infos[*index]
                                .image_memfd
                                .as_ref()
                                .unwrap()
                                .as_raw_fd(),
                        );
                        state.output_infos[*index].image_mmap = Some(
                            memmap2::MmapMut::map_mut(&file)
                                .expect("Failed to create memory mapping"),
                        );

                        // Don't drop the file as it would close the fd.
                        // It will be closed when dropping `image_memfd`
                        std::mem::forget(file);
                    }
                }

                // Mark image as ready for processing
                state.output_infos[*index].image_ready = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_shm::WlShm, ()> for State {
    fn event(
        _state: &mut State,
        _wl_shm: &wl_shm::WlShm,
        _event: wl_shm::Event,
        _: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for State {
    fn event(
        _state: &mut State,
        _wl_shm_pool: &wl_shm_pool::WlShmPool,
        _event: wl_shm_pool::Event,
        _: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for State {
    fn event(
        _state: &mut State,
        _wl_buffer: &wl_buffer::WlBuffer,
        _event: wl_buffer::Event,
        _: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ()> for State {
    fn event(
        _state: &mut State,
        _wlr_screencopy_manager: &zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
        _event: zwlr_screencopy_manager_v1::Event,
        _: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zxdg_output_manager_v1::ZxdgOutputManagerV1, ()> for State {
    fn event(
        _state: &mut State,
        _xdg_output_manager: &zxdg_output_manager_v1::ZxdgOutputManagerV1,
        _event: zxdg_output_manager_v1::Event,
        _: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

pub fn connect_and_get_output_info()
-> Result<(State, wayland_client::EventQueue<State>), crate::Error> {
    let connection = Connection::connect_to_env()?;

    let mut event_queue = connection.new_event_queue();

    let wl_display = connection.display();
    wl_display.get_registry(&event_queue.handle(), ());
    wl_display.sync(&event_queue.handle(), ());

    let mut state = State::default();

    while !state.done {
        event_queue.blocking_dispatch(&mut state)?;
    }

    for (i, output_info) in state.output_infos.iter_mut().enumerate() {
        state.xdg_output_manager.as_ref().unwrap().get_xdg_output(
            &output_info.wl_output,
            &event_queue.handle(),
            i,
        );
    }

    while state.output_infos.iter().any(|output_info| {
        output_info.output_logical_position.is_none()
            || output_info.output_logical_size.is_none()
            || output_info.transform.is_none()
    }) {
        event_queue.blocking_dispatch(&mut state)?;
    }

    Ok((state, event_queue))
}
