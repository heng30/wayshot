use anyhow::{Result, anyhow};
use camera::{CameraClient, CameraConfig, PixelFormat};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::util;

#[derive(clap::Subcommand)]
pub enum Cmd {
    /// 列出可用摄像头
    List,

    /// 拍一张照片
    Snapshot {
        #[arg(short, long)]
        output: PathBuf,
        /// 摄像头序号（见 `camera list`）
        #[arg(long)]
        index: Option<usize>,
        /// 摄像头名称
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        /// 水平镜像
        #[arg(long)]
        mirror: bool,
        /// 启动后等待多少毫秒再取帧
        #[arg(long, default_value_t = 800)]
        warmup_ms: u64,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::List => list(),
        Cmd::Snapshot {
            output,
            index,
            name,
            width,
            height,
            mirror,
            warmup_ms,
        } => snapshot(
            output,
            *index,
            name.as_deref(),
            *width,
            *height,
            *mirror,
            *warmup_ms,
        ),
    }
}

fn list() -> Result<Value> {
    let devices: Vec<Value> = camera::query_available_cameras()
        .iter()
        .map(|c| {
            json!({
                "index": c.index,
                "name": c.name,
                "description": c.description,
            })
        })
        .collect();

    Ok(json!({ "cameras": devices }))
}

#[allow(clippy::too_many_arguments)]
fn snapshot(
    output: &Path,
    index: Option<usize>,
    name: Option<&str>,
    width: Option<u32>,
    height: Option<u32>,
    mirror: bool,
    warmup_ms: u64,
) -> Result<Value> {
    let device = match (name, index) {
        (Some(name), _) => {
            camera::query_camera_id(name).map_err(|e| anyhow!("找不到摄像头: {e}"))?
        }
        (None, Some(index)) => {
            let cameras = camera::query_available_cameras();
            let info = cameras
                .get(index)
                .ok_or_else(|| anyhow!("摄像头序号越界: {index}"))?;
            camera::query_camera_id(&info.name).map_err(|e| anyhow!("找不到摄像头: {e}"))?
        }
        (None, None) => camera::query_first_camera().map_err(|e| anyhow!("找不到摄像头: {e}"))?,
    };

    let mut config = CameraConfig::default()
        .with_mirror_horizontal(mirror)
        .with_pixel_format(PixelFormat::RGBA);
    if let Some(width) = width {
        config = config.with_width(width);
    }
    if let Some(height) = height {
        config = config.with_height(height);
    }

    let mut client =
        CameraClient::new(device, config).map_err(|e| anyhow!("打开摄像头失败: {e}"))?;
    client.start().map_err(|e| anyhow!("启动摄像头失败: {e}"))?;
    std::thread::sleep(Duration::from_millis(warmup_ms.max(50)));

    let image = client
        .last_frame_rgba()
        .map_err(|e| anyhow!("取帧失败: {e}"))?;
    client.stop().ok();

    util::save_rgba(&image, output)?;
    Ok(util::image_json(output, &image))
}
