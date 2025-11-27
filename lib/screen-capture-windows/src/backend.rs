use derive_setters::Setters;
use std::{
    mem::{self, zeroed},
    ptr,
};
use winapi::{
    shared::{
        dxgi::{
            CreateDXGIFactory1, DXGI_MAP_READ, DXGI_OUTPUT_DESC, DXGI_RESOURCE_PRIORITY_MAXIMUM,
            IDXGIAdapter, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, IDXGISurface1,
            IID_IDXGIFactory1,
        },
        dxgi1_2::{
            DXGI_OUTDUPL_FRAME_INFO, DXGI_OUTDUPL_POINTER_SHAPE_INFO,
            DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR, DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR,
            DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME, IDXGIOutput1, IDXGIOutputDuplication,
        },
        dxgitype::{
            DXGI_MODE_ROTATION_IDENTITY, DXGI_MODE_ROTATION_ROTATE90, DXGI_MODE_ROTATION_ROTATE180,
            DXGI_MODE_ROTATION_ROTATE270, DXGI_MODE_ROTATION_UNSPECIFIED,
        },
        windef::RECT,
        winerror::{
            DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_MORE_DATA, DXGI_ERROR_NOT_FOUND,
            DXGI_ERROR_WAIT_TIMEOUT, E_ACCESSDENIED, HRESULT,
        },
    },
    um::{
        d3d11::{
            D3D11_CPU_ACCESS_READ, D3D11_SDK_VERSION, D3D11_USAGE_STAGING, D3D11CreateDevice,
            ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
        },
        d3dcommon::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_9_1},
        unknwnbase::IUnknown,
        winuser::{GetMonitorInfoW, MONITORINFO},
    },
};
use wio::com::ComPtr;

#[derive(thiserror::Error, Debug)]
pub enum CaptureError {
    #[error("AccessDenied")]
    AccessDenied,

    #[error("AccessLost")]
    AccessLost,

    #[error("RefreshFailure")]
    RefreshFailure,

    #[error("Timeout {0}ms")]
    Timeout(u32),

    #[error("Fail {0}")]
    Fail(String),
}

pub fn hr_failed(hr: HRESULT) -> bool {
    hr < 0
}

struct MouseCursor {
    visible: bool,
    x: i32,
    y: i32,
    shape_info: DXGI_OUTDUPL_POINTER_SHAPE_INFO,
    buffer: Vec<u8>,
}

impl MouseCursor {
    fn new() -> Self {
        Self {
            visible: false,
            x: 0,
            y: 0,
            shape_info: unsafe { zeroed() },
            buffer: Vec::new(),
        }
    }

    fn update(
        &mut self,
        dup: &ComPtr<IDXGIOutputDuplication>,
        frame_info: &DXGI_OUTDUPL_FRAME_INFO,
    ) -> Result<(), HRESULT> {
        if unsafe { *frame_info.LastMouseUpdateTime.QuadPart() } > 0 {
            self.x = frame_info.PointerPosition.Position.x;
            self.y = frame_info.PointerPosition.Position.y;
            self.visible = frame_info.PointerPosition.Visible != 0;
        }

        if frame_info.PointerShapeBufferSize > 0 {
            let required_size = frame_info.PointerShapeBufferSize as usize;
            if self.buffer.len() < required_size {
                self.buffer.resize(required_size, 0);
            }

            unsafe {
                let mut size_required = 0;
                let hr = dup.GetFramePointerShape(
                    frame_info.PointerShapeBufferSize,
                    self.buffer.as_mut_ptr() as *mut _,
                    &mut size_required,
                    &mut self.shape_info,
                );

                if hr_failed(hr) && hr != DXGI_ERROR_MORE_DATA {
                    return Err(hr);
                }
            }
        }
        Ok(())
    }
}

fn draw_cursor_on_buffer(
    buffer: &mut [u8],
    width: usize,
    height: usize,
    cursor: &MouseCursor,
    _rotation: u32,
) {
    if !cursor.visible || cursor.buffer.is_empty() {
        return;
    }

    let cx = cursor.x;
    let cy = cursor.y;
    let shape_w = cursor.shape_info.Width as i32;
    let shape_h = cursor.shape_info.Height as i32;
    let pitch = cursor.shape_info.Pitch as usize;
    let shape_type = cursor.shape_info.Type;

    match shape_type {
        DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR | DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR => unsafe {
            let cursor_data = cursor.buffer.as_ptr();
            for row in 0..shape_h {
                let screen_y = cy + row;
                if screen_y < 0 || screen_y >= height as i32 {
                    continue;
                }

                for col in 0..shape_w {
                    let screen_x = cx + col;
                    if screen_x < 0 || screen_x >= width as i32 {
                        continue;
                    }

                    let src_idx = (row as usize * pitch) + (col as usize * 4);
                    let b = *cursor_data.add(src_idx);
                    let g = *cursor_data.add(src_idx + 1);
                    let r = *cursor_data.add(src_idx + 2);
                    let a = *cursor_data.add(src_idx + 3);

                    // Skip completely transparent pixels
                    if a == 0 {
                        continue;
                    }

                    let dest_idx = (screen_y as usize * width * 4) + (screen_x as usize * 4);

                    // For MASKED_COLOR type, handle XOR mask for special cursor effects
                    if shape_type == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR && a == 255 {
                        // For opaque pixels in masked mode, use XOR for inverted cursor areas
                        buffer[dest_idx] = buffer[dest_idx] ^ r;
                        buffer[dest_idx + 1] = buffer[dest_idx + 1] ^ g;
                        buffer[dest_idx + 2] = buffer[dest_idx + 2] ^ b;
                    } else {
                        // Alpha blending for transparent pixels
                        let alpha_norm = a as f32 / 255.0;
                        let inv_alpha = 1.0 - alpha_norm;

                        // Correct alpha blending with proper color channel order (RGBA)
                        buffer[dest_idx] =
                            (buffer[dest_idx] as f32 * inv_alpha + r as f32 * alpha_norm) as u8;
                        buffer[dest_idx + 1] =
                            (buffer[dest_idx + 1] as f32 * inv_alpha + g as f32 * alpha_norm) as u8;
                        buffer[dest_idx + 2] =
                            (buffer[dest_idx + 2] as f32 * inv_alpha + b as f32 * alpha_norm) as u8;
                    }
                    // Ensure alpha channel is set to opaque for the final buffer
                    buffer[dest_idx + 3] = 255;
                }
            }
        },
        DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME => {
            let actual_h = shape_h / 2;
            unsafe {
                let cursor_data = cursor.buffer.as_ptr();
                for row in 0..actual_h {
                    let screen_y = cy + row;
                    if screen_y < 0 || screen_y >= height as i32 {
                        continue;
                    }

                    for col in 0..shape_w {
                        let screen_x = cx + col;
                        if screen_x < 0 || screen_x >= width as i32 {
                            continue;
                        }

                        let mask_bit_idx = col;
                        let mask_byte_idx = (row as usize * pitch) + (mask_bit_idx as usize / 8);
                        let mask_bit = 0x80 >> (mask_bit_idx % 8);

                        let and_mask = (*cursor_data.add(mask_byte_idx) & mask_bit) != 0;

                        let xor_byte_idx =
                            ((row + actual_h) as usize * pitch) + (mask_bit_idx as usize / 8);
                        let xor_mask = (*cursor_data.add(xor_byte_idx) & mask_bit) != 0;

                        let dest_idx = (screen_y as usize * width * 4) + (screen_x as usize * 4);

                        // Monochrome cursor logic:
                        // - AND mask determines whether to keep background (1) or clear to black (0)
                        // - XOR mask determines whether to invert the pixel (1) or leave as is (0)
                        if !and_mask {
                            // Clear to black (transparent background for cursor)
                            buffer[dest_idx] = 0;
                            buffer[dest_idx + 1] = 0;
                            buffer[dest_idx + 2] = 0;
                        }
                        if xor_mask {
                            // Invert the pixel for cursor outline
                            buffer[dest_idx] = !buffer[dest_idx];
                            buffer[dest_idx + 1] = !buffer[dest_idx + 1];
                            buffer[dest_idx + 2] = !buffer[dest_idx + 2];
                        }
                        buffer[dest_idx + 3] = 255; // Fully opaque
                    }
                }
            }
        }
        _ => {}
    }
}

fn create_dxgi_factory_1() -> ComPtr<IDXGIFactory1> {
    unsafe {
        let mut factory = ptr::null_mut();
        let hr = CreateDXGIFactory1(&IID_IDXGIFactory1, &mut factory);
        if hr_failed(hr) {
            panic!("Failed to create DXGIFactory1, {:x}", hr)
        } else {
            ComPtr::from_raw(factory as *mut IDXGIFactory1)
        }
    }
}

fn d3d11_create_device(
    adapter: *mut IDXGIAdapter,
) -> (ComPtr<ID3D11Device>, ComPtr<ID3D11DeviceContext>) {
    unsafe {
        let (mut d3d11_device, mut device_context) = (ptr::null_mut(), ptr::null_mut());
        let hr = D3D11CreateDevice(
            adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            0,
            D3D11_SDK_VERSION,
            &mut d3d11_device,
            #[allow(const_item_mutation)]
            &mut D3D_FEATURE_LEVEL_9_1,
            &mut device_context,
        );
        if hr_failed(hr) {
            panic!("Failed to create d3d11 device and device context, {:x}", hr)
        } else {
            (
                ComPtr::from_raw(d3d11_device as *mut ID3D11Device),
                ComPtr::from_raw(device_context),
            )
        }
    }
}

fn get_adapter_outputs(adapter: &IDXGIAdapter1) -> Vec<ComPtr<IDXGIOutput>> {
    let mut outputs = Vec::new();
    for i in 0.. {
        unsafe {
            let mut output = ptr::null_mut();
            if hr_failed(adapter.EnumOutputs(i, &mut output)) {
                break;
            } else {
                let mut out_desc = zeroed();
                (*output).GetDesc(&mut out_desc);
                if out_desc.AttachedToDesktop != 0 {
                    outputs.push(ComPtr::from_raw(output))
                } else {
                    break;
                }
            }
        }
    }
    outputs
}

fn get_capture_source(
    output_dups: Vec<(ComPtr<IDXGIOutputDuplication>, ComPtr<IDXGIOutput1>)>,
    screen_name: &str,
) -> Option<(ComPtr<IDXGIOutputDuplication>, ComPtr<IDXGIOutput1>)> {
    for (dup, output) in &output_dups {
        unsafe {
            let mut output_desc = zeroed();
            output.GetDesc(&mut output_desc);

            let device_name = String::from_utf16_lossy(std::slice::from_raw_parts(
                output_desc.DeviceName.as_ptr(),
                output_desc
                    .DeviceName
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(32),
            ))
            .trim_end_matches('\0')
            .to_string();

            if device_name == screen_name || device_name.contains(screen_name) {
                return Some((dup.clone(), output.clone()));
            }
        }
    }

    None
}

#[allow(clippy::type_complexity)]
fn duplicate_outputs(
    mut device: ComPtr<ID3D11Device>,
    outputs: Vec<ComPtr<IDXGIOutput>>,
) -> Result<
    (
        ComPtr<ID3D11Device>,
        Vec<(ComPtr<IDXGIOutputDuplication>, ComPtr<IDXGIOutput1>)>,
    ),
    HRESULT,
> {
    let mut out_dups = Vec::new();
    for output in outputs
        .into_iter()
        .map(|out| out.cast::<IDXGIOutput1>().unwrap())
    {
        let dxgi_device = device.up::<IUnknown>();
        let output_duplication = unsafe {
            let mut output_duplication = ptr::null_mut();
            let hr = output.DuplicateOutput(dxgi_device.as_raw(), &mut output_duplication);
            if hr_failed(hr) {
                return Err(hr);
            }
            ComPtr::from_raw(output_duplication)
        };
        device = dxgi_device.cast().unwrap();
        out_dups.push((output_duplication, output));
    }
    Ok((device, out_dups))
}

struct DuplicatedOutput {
    device: ComPtr<ID3D11Device>,
    device_context: ComPtr<ID3D11DeviceContext>,
    output: ComPtr<IDXGIOutput1>,
    output_duplication: ComPtr<IDXGIOutputDuplication>,
    cursor: MouseCursor,
}
impl DuplicatedOutput {
    fn get_desc(&self) -> DXGI_OUTPUT_DESC {
        unsafe {
            let mut desc = zeroed();
            self.output.GetDesc(&mut desc);
            desc
        }
    }

    fn capture_frame_to_surface(
        &mut self,
        timeout_ms: u32,
    ) -> Result<ComPtr<IDXGISurface1>, HRESULT> {
        let frame_resource = unsafe {
            let mut frame_resource = ptr::null_mut();
            let mut frame_info: DXGI_OUTDUPL_FRAME_INFO = zeroed();
            let hr = self.output_duplication.AcquireNextFrame(
                timeout_ms,
                &mut frame_info,
                &mut frame_resource,
            );

            if hr_failed(hr) {
                return Err(hr);
            }

            // Update cursor data before releasing the frame
            if let Err(e) = self.cursor.update(&self.output_duplication, &frame_info) {
                self.output_duplication.ReleaseFrame();
                return Err(e);
            }

            ComPtr::from_raw(frame_resource)
        };
        let frame_texture = frame_resource.cast::<ID3D11Texture2D>().unwrap();
        let mut texture_desc = unsafe {
            let mut texture_desc = zeroed();
            frame_texture.GetDesc(&mut texture_desc);
            texture_desc
        };

        // Configure the description to make the texture readable
        texture_desc.Usage = D3D11_USAGE_STAGING;
        texture_desc.BindFlags = 0;
        texture_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
        texture_desc.MiscFlags = 0;
        let readable_texture = unsafe {
            let mut readable_texture = ptr::null_mut();
            let hr = self
                .device
                .CreateTexture2D(&texture_desc, ptr::null(), &mut readable_texture);

            if hr_failed(hr) {
                self.output_duplication.ReleaseFrame();
                return Err(hr);
            }

            ComPtr::from_raw(readable_texture)
        };

        // Lower priorities causes stuff to be needlessly copied from gpu to ram,
        // causing huge ram usage on some systems.
        unsafe { readable_texture.SetEvictionPriority(DXGI_RESOURCE_PRIORITY_MAXIMUM) };
        let readable_surface = readable_texture.up::<ID3D11Resource>();
        unsafe {
            self.device_context.CopyResource(
                readable_surface.as_raw(),
                frame_texture.up::<ID3D11Resource>().as_raw(),
            );
            self.output_duplication.ReleaseFrame();
        }
        readable_surface.cast()
    }
}

#[derive(Setters)]
#[setters(prefix = "with_")]
pub struct DXGIManager {
    #[setters(skip)]
    screen_name: String,

    #[setters(skip)]
    duplicated_output: Option<DuplicatedOutput>,

    include_cursor: bool,
    timeout_ms: u32,
}

impl DXGIManager {
    pub fn new(screen_name: String) -> Result<DXGIManager, CaptureError> {
        let mut manager = DXGIManager {
            screen_name,
            include_cursor: true,
            duplicated_output: None,
            timeout_ms: 300,
        };

        match manager.acquire_output_duplication() {
            Ok(_) => Ok(manager),
            Err(_) => Err(CaptureError::Fail(
                "Failed to acquire output duplication".to_string(),
            )),
        }
    }

    pub fn list_available_screens() -> Result<Vec<String>, CaptureError> {
        let factory = create_dxgi_factory_1();
        let mut screen_names = Vec::new();

        for (outputs, _adapter) in (0..)
            .map(|i| {
                let mut adapter = ptr::null_mut();
                unsafe {
                    if factory.EnumAdapters1(i, &mut adapter) != DXGI_ERROR_NOT_FOUND {
                        Some(ComPtr::from_raw(adapter))
                    } else {
                        None
                    }
                }
            })
            .take_while(Option::is_some)
            .map(Option::unwrap)
            .map(|adapter| (get_adapter_outputs(&adapter), adapter))
            .filter(|(outs, _)| !outs.is_empty())
        {
            for output in outputs
                .into_iter()
                .map(|out| out.cast::<IDXGIOutput1>().unwrap())
            {
                unsafe {
                    let mut output_desc = zeroed();
                    output.GetDesc(&mut output_desc);

                    // Convert device name to string
                    let device_name = String::from_utf16_lossy(std::slice::from_raw_parts(
                        output_desc.DeviceName.as_ptr(),
                        output_desc
                            .DeviceName
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(32),
                    ))
                    .trim_end_matches('\0')
                    .to_string();

                    let is_primary = {
                        let mut monitor_info: MONITORINFO = zeroed();
                        monitor_info.cbSize = mem::size_of::<MONITORINFO>() as u32;
                        GetMonitorInfoW(output_desc.Monitor, &mut monitor_info);
                        (monitor_info.dwFlags & 1) != 0
                    };

                    let display_name = if is_primary {
                        format!("{} (Primary)", device_name)
                    } else {
                        device_name
                    };

                    screen_names.push(display_name);
                }
            }
        }

        Ok(screen_names)
    }

    pub fn geometry(&self) -> (usize, usize) {
        let output_desc = self.duplicated_output.as_ref().unwrap().get_desc();
        let RECT {
            left,
            top,
            right,
            bottom,
        } = output_desc.DesktopCoordinates;
        ((right - left) as usize, (bottom - top) as usize)
    }

    fn acquire_output_duplication(&mut self) -> Result<(), CaptureError> {
        self.duplicated_output = None;
        let factory = create_dxgi_factory_1();
        for (outputs, adapter) in (0..)
            .map(|i| {
                let mut adapter = ptr::null_mut();
                unsafe {
                    if factory.EnumAdapters1(i, &mut adapter) != DXGI_ERROR_NOT_FOUND {
                        Some(ComPtr::from_raw(adapter))
                    } else {
                        None
                    }
                }
            })
            .take_while(Option::is_some)
            .map(Option::unwrap)
            .map(|adapter| (get_adapter_outputs(&adapter), adapter))
            .filter(|(outs, _)| !outs.is_empty())
        {
            // Creating device for each adapter that has the output
            let (d3d11_device, device_context) = d3d11_create_device(adapter.up().as_raw());
            let (d3d11_device, output_duplications) = duplicate_outputs(d3d11_device, outputs)
                .map_err(|_| CaptureError::Fail("Unable to duplicate output".to_string()))?;
            if let Some((output_duplication, output)) =
                get_capture_source(output_duplications, &self.screen_name)
            {
                self.duplicated_output = Some(DuplicatedOutput {
                    device: d3d11_device,
                    device_context,
                    output,
                    output_duplication,
                    cursor: MouseCursor::new(),
                });
                return Ok(());
            }
        }
        Err(CaptureError::Fail(format!(
            "No output could be acquired for screen: {}",
            self.screen_name
        )))
    }

    fn capture_frame_to_surface(&mut self) -> Result<ComPtr<IDXGISurface1>, CaptureError> {
        if self.duplicated_output.is_none() {
            if self.acquire_output_duplication().is_ok() {
                return Err(CaptureError::Fail("No valid duplicated output".to_string()));
            } else {
                return Err(CaptureError::RefreshFailure);
            }
        }

        match self
            .duplicated_output
            .as_mut()
            .unwrap()
            .capture_frame_to_surface(self.timeout_ms)
        {
            Ok(surface) => Ok(surface),
            Err(DXGI_ERROR_ACCESS_LOST) => {
                if self.acquire_output_duplication().is_ok() {
                    Err(CaptureError::AccessLost)
                } else {
                    Err(CaptureError::RefreshFailure)
                }
            }
            Err(E_ACCESSDENIED) => Err(CaptureError::AccessDenied),
            Err(DXGI_ERROR_WAIT_TIMEOUT) => Err(CaptureError::Timeout(self.timeout_ms)),
            Err(_) => {
                if self.acquire_output_duplication().is_ok() {
                    Err(CaptureError::Fail(
                        "Failure when acquiring frame".to_string(),
                    ))
                } else {
                    Err(CaptureError::RefreshFailure)
                }
            }
        }
    }

    pub fn capture_frame_rgba(&mut self) -> Result<(Vec<u8>, (usize, usize)), CaptureError> {
        let frame_surface = match self.capture_frame_to_surface() {
            Ok(surface) => surface,
            Err(e) => return Err(e),
        };

        let mapped_surface = unsafe {
            let mut mapped_surface = zeroed();
            if hr_failed(frame_surface.Map(&mut mapped_surface, DXGI_MAP_READ)) {
                frame_surface.Release();
                return Err(CaptureError::Fail("Failed to map surface".to_string()));
            }
            mapped_surface
        };

        if mapped_surface.pBits.is_null() {
            unsafe { frame_surface.Unmap() };
            return Err(CaptureError::Fail(
                "Surface mapping returned null pointer".to_string(),
            ));
        }

        let output_desc = self.duplicated_output.as_mut().unwrap().get_desc();
        let (output_width, output_height) = {
            let RECT {
                left,
                top,
                right,
                bottom,
            } = output_desc.DesktopCoordinates;
            ((right - left) as usize, (bottom - top) as usize)
        };

        let stride = mapped_surface.Pitch as usize;
        let scan_lines = match output_desc.Rotation {
            DXGI_MODE_ROTATION_ROTATE90 | DXGI_MODE_ROTATION_ROTATE270 => output_width,
            _ => output_height,
        };

        let expected_size = output_width * output_height * 4;
        let mut rgba_buffer = vec![0u8; expected_size];

        unsafe {
            let src_data = mapped_surface.pBits as *const u8;

            match output_desc.Rotation {
                DXGI_MODE_ROTATION_IDENTITY | DXGI_MODE_ROTATION_UNSPECIFIED => {
                    for y in 0..output_height.min(scan_lines) {
                        for x in 0..output_width {
                            let src_offset = y * stride + x * 4;
                            let dst_offset = y * output_width * 4 + x * 4;

                            if src_offset + 3 < stride * scan_lines {
                                // Copy BGRA and convert to RGBA by swapping R and B
                                rgba_buffer[dst_offset] = *src_data.add(src_offset + 2); // R
                                rgba_buffer[dst_offset + 1] = *src_data.add(src_offset + 1); // G
                                rgba_buffer[dst_offset + 2] = *src_data.add(src_offset); // B
                                rgba_buffer[dst_offset + 3] = *src_data.add(src_offset + 3); // A
                            }
                        }
                    }
                }
                DXGI_MODE_ROTATION_ROTATE90 => {
                    // Rotate 90 degrees clockwise
                    for y in 0..output_height {
                        for x in 0..output_width {
                            let src_x = output_width - 1 - y;
                            let src_y = x;
                            if src_x < output_width && src_y < scan_lines {
                                let src_offset = src_y * stride + src_x * 4;
                                let dst_offset = y * output_width * 4 + x * 4;

                                if src_offset + 3 < stride * scan_lines {
                                    rgba_buffer[dst_offset] = *src_data.add(src_offset + 2); // R
                                    rgba_buffer[dst_offset + 1] = *src_data.add(src_offset + 1); // G
                                    rgba_buffer[dst_offset + 2] = *src_data.add(src_offset); // B
                                    rgba_buffer[dst_offset + 3] = *src_data.add(src_offset + 3); // A
                                }
                            }
                        }
                    }
                }
                DXGI_MODE_ROTATION_ROTATE180 => {
                    // Rotate 180 degrees
                    for y in 0..output_height {
                        for x in 0..output_width {
                            let src_x = output_width - 1 - x;
                            let src_y = output_height - 1 - y;
                            if src_x < output_width && src_y < scan_lines {
                                let src_offset = src_y * stride + src_x * 4;
                                let dst_offset = y * output_width * 4 + x * 4;

                                if src_offset + 3 < stride * scan_lines {
                                    rgba_buffer[dst_offset] = *src_data.add(src_offset + 2); // R
                                    rgba_buffer[dst_offset + 1] = *src_data.add(src_offset + 1); // G
                                    rgba_buffer[dst_offset + 2] = *src_data.add(src_offset); // B
                                    rgba_buffer[dst_offset + 3] = *src_data.add(src_offset + 3); // A
                                }
                            }
                        }
                    }
                }
                DXGI_MODE_ROTATION_ROTATE270 => {
                    // Rotate 270 degrees clockwise (90 degrees counter-clockwise)
                    for y in 0..output_height {
                        for x in 0..output_width {
                            let src_x = y;
                            let src_y = output_height - 1 - x;
                            if src_x < output_width && src_y < scan_lines {
                                let src_offset = src_y * stride + src_x * 4;
                                let dst_offset = y * output_width * 4 + x * 4;

                                if src_offset + 3 < stride * scan_lines {
                                    rgba_buffer[dst_offset] = *src_data.add(src_offset + 2); // R
                                    rgba_buffer[dst_offset + 1] = *src_data.add(src_offset + 1); // G
                                    rgba_buffer[dst_offset + 2] = *src_data.add(src_offset); // B
                                    rgba_buffer[dst_offset + 3] = *src_data.add(src_offset + 3); // A
                                }
                            }
                        }
                    }
                }
                _ => {
                    // For other rotations, just fill with a default color for now
                    for pixel in rgba_buffer.chunks_exact_mut(4) {
                        pixel[0] = 0; // R
                        pixel[1] = 0; // G
                        pixel[2] = 0; // B
                        pixel[3] = 255; // A
                    }
                }
            }
        };

        unsafe { frame_surface.Unmap() };

        if self.include_cursor
            && let Some(dup_out) = &self.duplicated_output
        {
            draw_cursor_on_buffer(
                &mut rgba_buffer,
                output_width,
                output_height,
                &dup_out.cursor,
                output_desc.Rotation,
            );
        }

        Ok((rgba_buffer, (output_width, output_height)))
    }
}
