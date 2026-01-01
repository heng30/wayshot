use crate::{CameraError, CameraResult};
use nokhwa::{
    CallbackCamera, query,
    utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType},
};

#[derive(Debug, Clone)]
pub struct CameraInfo {
    pub index: String,
    pub name: String,
    pub description: String,
}

pub fn query_available_cameras() -> Vec<CameraInfo> {
    let cameras = match query(ApiBackend::Auto) {
        Ok(cameras) => cameras,
        Err(_) => return Vec::new(),
    };

    cameras
        .into_iter()
        .filter_map(|camera| match verify_camera(camera.index().clone()) {
            true => Some(CameraInfo {
                index: camera.index().to_string(),
                name: camera.human_name(),
                description: camera.description().to_string(),
            }),
            false => None,
        })
        .collect()
}

pub fn query_camera_id(name: &str) -> CameraResult<CameraIndex> {
    let cameras = query(ApiBackend::Auto)?;

    cameras
        .into_iter()
        .find(|camera| name == camera.human_name() && verify_camera(camera.index().clone()))
        .map(|camera| camera.index().clone())
        .ok_or(CameraError::QueryError(format!("No found camera: {name}")))
}

fn verify_camera(index: CameraIndex) -> bool {
    let format = RequestedFormat::new::<nokhwa::pixel_format::RgbAFormat>(
        RequestedFormatType::AbsoluteHighestFrameRate,
    );

    match CallbackCamera::new(index, format, |_| {}) {
        Ok(mut camera) => match camera.open_stream() {
            Ok(_) => {
                _ = camera.stop_stream();
                true
            }
            Err(_) => false,
        },
        Err(_) => false,
    }
}
