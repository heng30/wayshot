use anyhow::{Context, Result, anyhow};
use clap::{Subcommand, ValueEnum};
use recorder::{FPS, RecorderConfig, RecordingSession, Resolution};
use screen_capture::{ScreenCapture, ScreenInfo};
use screen_capture_wayland_wlr::ScreenCaptureWaylandWlr;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::util;

#[derive(Subcommand)]
pub enum Cmd {
    /// 列出所有显示器
    List,

    /// 截取一帧屏幕并保存为图片
    Shot {
        /// 输出图片路径（扩展名决定格式）
        #[arg(short, long)]
        output: PathBuf,

        /// 显示器名称，缺省为第一个显示器
        #[arg(long)]
        screen: Option<String>,

        /// 截图中是否包含鼠标指针
        #[arg(long)]
        cursor: bool,
    },

    /// 录制屏幕到 mp4
    Record {
        #[arg(short, long)]
        output: PathBuf,
        /// 录制时长（秒）
        #[arg(long, default_value_t = 10.0)]
        seconds: f64,
        /// 显示器名称，缺省为第一个显示器
        #[arg(long)]
        screen: Option<String>,
        #[arg(long, value_enum, default_value_t = RecordFps::Fps30)]
        fps: RecordFps,
        #[arg(long, value_enum, default_value_t = RecordResolution::P1080)]
        resolution: RecordResolution,
        /// 不包含鼠标指针
        #[arg(long)]
        no_cursor: bool,
        /// 麦克风设备名（见 `audio devices`），不给则不录音
        #[arg(long)]
        audio_device: Option<String>,
        /// 同时录制系统声音（需要 pipewire 扬声器录制支持）
        #[arg(long)]
        speaker: bool,
        /// 录成单声道
        #[arg(long)]
        mono: bool,
        /// 麦克风降噪
        #[arg(long)]
        denoise: bool,
        /// 摄像头画中画（设备名见 `camera list`）
        #[arg(long)]
        camera: Option<String>,
        /// 摄像头抠像模型（ONNX 路径），配合 --camera 使用
        #[arg(long)]
        camera_bg_model: Option<PathBuf>,
        /// 摄像头抠像模型类型
        #[arg(long, value_enum, default_value_t = CameraBgKind::Modnet)]
        camera_bg_kind: CameraBgKind,
        /// 跟随鼠标自动缩放（光标跟踪）
        #[arg(long)]
        cursor_track: bool,
        /// 跟踪区域宽（像素）
        #[arg(long, default_value_t = 1280)]
        region_width: i32,
        /// 跟踪区域高（像素）
        #[arg(long, default_value_t = 720)]
        region_height: i32,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum CameraBgKind {
    Modnet,
    Rmbg14,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum RecordFps {
    Fps24,
    Fps25,
    Fps30,
    Fps60,
}

impl RecordFps {
    fn to_fps(self) -> FPS {
        match self {
            RecordFps::Fps24 => FPS::Fps24,
            RecordFps::Fps25 => FPS::Fps25,
            RecordFps::Fps30 => FPS::Fps30,
            RecordFps::Fps60 => FPS::Fps60,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum RecordResolution {
    P480,
    P720,
    P1080,
    P2K,
    P4K,
}

impl RecordResolution {
    fn to_resolution(self) -> Resolution {
        match self {
            RecordResolution::P480 => Resolution::P480,
            RecordResolution::P720 => Resolution::P720,
            RecordResolution::P1080 => Resolution::P1080,
            RecordResolution::P2K => Resolution::P2K,
            RecordResolution::P4K => Resolution::P4K,
        }
    }
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::List => list(),
        Cmd::Shot {
            output,
            screen,
            cursor,
        } => shot(output, screen.as_deref(), *cursor),
        Cmd::Record {
            output,
            seconds,
            screen,
            fps,
            resolution,
            no_cursor,
            audio_device,
            speaker,
            mono,
            denoise,
            camera,
            camera_bg_model,
            camera_bg_kind,
            cursor_track,
            region_width,
            region_height,
        } => {
            let info = find_screen(screen.as_deref())?;
            let config = base_config(
                &info,
                output.clone(),
                *fps,
                *resolution,
                !no_cursor,
                audio_device.as_deref(),
                *speaker,
            )
            .with_convert_to_mono(*mono)
            .with_enable_denoise(*denoise);

            let mut config = config
                .with_enable_cursor_tracking(*cursor_track)
                .with_region_width(*region_width)
                .with_region_height(*region_height);

            if let Some(name) = camera {
                let mut mix = recorder::CameraMixConfig::default()
                    .with_enable(true)
                    .with_camera_name(Some(name.clone()));
                if let Some(model_path) = camera_bg_model {
                    let model = match camera_bg_kind {
                        CameraBgKind::Modnet => background_remover::Model::Modnet,
                        CameraBgKind::Rmbg14 => background_remover::Model::Rmbg14,
                    };
                    mix = mix
                        .with_background_remover_model(Some(model))
                        .with_background_remover_model_path(Some(model_path.clone()));
                }
                config = config.with_camera_mix_config(mix);
            }

            let state = run_session(config, *seconds)?;
            value_of(&info, output, *seconds, state)
        }
    }
}

fn list() -> Result<Value> {
    let screens = ScreenCaptureWaylandWlr.available_screens().show()?;

    let screens: Vec<Value> = screens
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "x": s.position.x,
                "y": s.position.y,
                "width": s.logical_size.width,
                "height": s.logical_size.height,
                "scale": s.scale_factor,
            })
        })
        .collect();

    Ok(json!({ "screens": screens }))
}

fn shot(output: &Path, screen: Option<&str>, include_cursor: bool) -> Result<Value> {
    let info = find_screen(screen)?;

    let capture = screen_capture_wayland_wlr::capture_output(&info.name, include_cursor)
        .with_context(|| format!("截屏失败: {}", info.name))?;
    let img = image::RgbaImage::from_raw(capture.width, capture.height, capture.pixel_data)
        .ok_or_else(|| anyhow!("截屏数据为空"))?;

    util::save_rgba(&img, output)?;
    Ok(util::image_json(output, &img))
}

fn find_screen(name: Option<&str>) -> Result<ScreenInfo> {
    let screens = ScreenCaptureWaylandWlr.available_screens().show()?;
    match name {
        Some(name) => screens
            .into_iter()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow!("找不到显示器: {name}")),
        None => screens
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("没有可用显示器")),
    }
}

fn base_config(
    info: &ScreenInfo,
    save_path: PathBuf,
    fps: RecordFps,
    resolution: RecordResolution,
    include_cursor: bool,
    audio_device: Option<&str>,
    speaker: bool,
) -> RecorderConfig {
    RecorderConfig::new(info.name.clone(), info.logical_size, save_path)
        .with_fps(fps.to_fps())
        .with_resolution(resolution.to_resolution())
        .with_include_cursor(include_cursor)
        .with_audio_device_name(audio_device.map(str::to_string))
        .with_enable_recording_speaker(speaker)
}

/// 启动录制会话，跑 `seconds` 秒后停止；`wait()` 负责编码与 mp4 封装收尾。
fn run_session(mut config: RecorderConfig, seconds: f64) -> Result<String> {
    // recorder 要求 save_path 有可用父目录，相对路径的父目录是空字符串。
    config.save_path = std::path::absolute(&config.save_path)?;
    let save_path = config.save_path.clone();
    if let Some(parent) = save_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("无法创建目录: {}", parent.display()))?;
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow!("创建 tokio runtime 失败: {e}"))?;

    let mut session = RecordingSession::new(config);
    let stop_sig = session.get_stop_sig();
    session
        .start(
            runtime.handle().clone(),
            recorder::platform_screen_capture(),
        )
        .map_err(|e| anyhow!("启动失败: {e}"))?;

    let stopper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs_f64(seconds.max(0.1)));
        stop_sig.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    let state = session.wait().map_err(|e| anyhow!("运行失败: {e}"))?;
    stopper.join().map_err(|_| anyhow!("停止线程异常退出"))?;

    Ok(format!("{state:?}"))
}

fn value_of(info: &ScreenInfo, output: &Path, seconds: f64, state: String) -> Result<Value> {
    let output = std::path::absolute(output)?;
    let bytes = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);

    Ok(json!({
        "path": output.display().to_string(),
        "screen": info.name,
        "width": info.logical_size.width,
        "height": info.logical_size.height,
        "seconds": seconds,
        "bytes": bytes,
        "state": state,
    }))
}

/// 让捕获相关错误直接转成 anyhow。
trait ShowError<T> {
    fn show(self) -> Result<T>;
}

impl<T, E: std::fmt::Display> ShowError<T> for std::result::Result<T, E> {
    fn show(self) -> Result<T> {
        self.map_err(|e| anyhow!("{e}"))
    }
}
