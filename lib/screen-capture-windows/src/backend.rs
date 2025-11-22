use std::{
    mem::{self, zeroed},
    ptr,
};
use winapi::shared::dxgi::{
    CreateDXGIFactory1, DXGI_MAP_READ, DXGI_OUTPUT_DESC, DXGI_RESOURCE_PRIORITY_MAXIMUM,
    IDXGIAdapter, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, IDXGISurface1, IID_IDXGIFactory1,
};
use winapi::shared::{
    dxgi1_2::{IDXGIOutput1, IDXGIOutputDuplication},
    dxgitype::{
        DXGI_MODE_ROTATION_IDENTITY, DXGI_MODE_ROTATION_ROTATE90, DXGI_MODE_ROTATION_ROTATE270,
        DXGI_MODE_ROTATION_UNSPECIFIED,
    },
    windef::RECT,
    winerror::{
        DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_NOT_FOUND, DXGI_ERROR_WAIT_TIMEOUT, E_ACCESSDENIED,
        HRESULT,
    },
};
use winapi::um::{
    d3d11::{
        D3D11_CPU_ACCESS_READ, D3D11_SDK_VERSION, D3D11_USAGE_STAGING, D3D11CreateDevice,
        ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
    },
    d3dcommon::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_9_1},
    unknwnbase::IUnknown,
    winuser::{GetMonitorInfoW, MONITORINFO},
};
use wio::com::ComPtr;

#[derive(Debug)]
pub enum CaptureError {
    AccessDenied,
    AccessLost,
    RefreshFailure,
    Timeout,
    Fail(&'static str),
}

pub fn hr_failed(hr: HRESULT) -> bool {
    hr < 0
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

fn output_is_primary(output: &ComPtr<IDXGIOutput1>) -> bool {
    unsafe {
        let mut output_desc = zeroed();
        output.GetDesc(&mut output_desc);
        let mut monitor_info: MONITORINFO = zeroed();
        monitor_info.cbSize = mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(output_desc.Monitor, &mut monitor_info);
        (monitor_info.dwFlags & 1) != 0
    }
}

fn get_capture_source(
    output_dups: Vec<(ComPtr<IDXGIOutputDuplication>, ComPtr<IDXGIOutput1>)>,
    cs_index: usize,
) -> Option<(ComPtr<IDXGIOutputDuplication>, ComPtr<IDXGIOutput1>)> {
    if cs_index == 0 {
        output_dups
            .into_iter()
            .find(|(_, out)| output_is_primary(out))
    } else {
        output_dups
            .into_iter()
            .filter(|(_, out)| !output_is_primary(out))
            .nth(cs_index - 1)
    }
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
            let mut frame_info = zeroed();
            let hr = self.output_duplication.AcquireNextFrame(
                timeout_ms,
                &mut frame_info,
                &mut frame_resource,
            );
            if hr_failed(hr) {
                return Err(hr);
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

pub struct DXGIManager {
    duplicated_output: Option<DuplicatedOutput>,
    capture_source_index: usize,
    timeout_ms: u32,
}

impl DXGIManager {
    pub fn new(timeout_ms: u32) -> Result<DXGIManager, &'static str> {
        let mut manager = DXGIManager {
            duplicated_output: None,
            capture_source_index: 0,
            timeout_ms,
        };

        match manager.acquire_output_duplication() {
            Ok(_) => Ok(manager),
            Err(_) => Err("Failed to acquire output duplication"),
        }
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

    pub fn set_capture_source_index(&mut self, cs: usize) {
        self.capture_source_index = cs;
        self.acquire_output_duplication().unwrap()
    }

    pub fn get_capture_source_index(&self) -> usize {
        self.capture_source_index
    }

    pub fn set_timeout_ms(&mut self, timeout_ms: u32) {
        self.timeout_ms = timeout_ms
    }

    pub fn acquire_output_duplication(&mut self) -> Result<(), &'static str> {
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
                .map_err(|_| "Unable to duplicate output")?;
            if let Some((output_duplication, output)) =
                get_capture_source(output_duplications, self.capture_source_index)
            {
                self.duplicated_output = Some(DuplicatedOutput {
                    device: d3d11_device,
                    device_context,
                    output,
                    output_duplication,
                });
                return Ok(());
            }
        }
        Err("No output could be acquired")
    }

    fn capture_frame_to_surface(&mut self) -> Result<ComPtr<IDXGISurface1>, CaptureError> {
        if self.duplicated_output.is_none() {
            if self.acquire_output_duplication().is_ok() {
                return Err(CaptureError::Fail("No valid duplicated output"));
            } else {
                return Err(CaptureError::RefreshFailure);
            }
        }
        let timeout_ms = self.timeout_ms;
        match self
            .duplicated_output
            .as_mut()
            .unwrap()
            .capture_frame_to_surface(timeout_ms)
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
            Err(DXGI_ERROR_WAIT_TIMEOUT) => Err(CaptureError::Timeout),
            Err(_) => {
                if self.acquire_output_duplication().is_ok() {
                    Err(CaptureError::Fail("Failure when acquiring frame"))
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
                return Err(CaptureError::Fail("Failed to map surface"));
            }
            mapped_surface
        };

        if mapped_surface.pBits.is_null() {
            unsafe { frame_surface.Unmap() };
            return Err(CaptureError::Fail("Surface mapping returned null pointer"));
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
        Ok((rgba_buffer, (output_width, output_height)))
    }
}
