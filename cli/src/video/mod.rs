use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::Subcommand;
use ffmpeg_next as ffmpeg;
use serde_json::Value;

mod args;
mod audio;
mod edit;
mod frames;
mod probe;
mod scenes;

pub use args::SceneAlgorithm;
pub(crate) use audio::decode_audio;
pub(crate) use probe::{
    VideoSource, decode_video, open_video, source_duration, source_fps, source_size,
};

#[derive(Subcommand)]
pub enum Cmd {
    /// 查看视频/音频文件的基本信息
    Info { input: PathBuf },

    /// 按间隔抽帧保存为 PNG
    Frames {
        input: PathBuf,
        /// 帧图片输出目录
        #[arg(short, long)]
        output_dir: PathBuf,

        /// 每隔多少秒抽一帧
        #[arg(long, default_value_t = 1.0, conflicts_with_all = ["fps", "count"])]
        every_secs: f64,

        /// 每秒抽多少帧
        #[arg(long, conflicts_with_all = ["every_secs", "count"])]
        fps: Option<f64>,

        /// 全片均匀抽取 N 帧
        #[arg(long, conflicts_with_all = ["every_secs", "fps"])]
        count: Option<u32>,

        /// 起始时间（秒）
        #[arg(long, default_value_t = 0.0)]
        start: f64,

        /// 截取时长（秒），缺省到结尾
        #[arg(long)]
        duration: Option<f64>,

        /// 最多输出多少帧（防止 LLM 误操作产生大量文件）
        #[arg(long, default_value_t = 200)]
        max_frames: u32,
    },

    /// 抽取指定时间点的一帧作为图片
    Thumbnail {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// 时间点（秒）
        #[arg(long, default_value_t = 0.0)]
        at: f64,
    },

    /// 截取视频片段（重新编码，可换分辨率/帧率）
    Clip {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// 起始时间（秒）
        #[arg(long, default_value_t = 0.0)]
        start: f64,
        /// 片段时长（秒），缺省到结尾
        #[arg(long)]
        duration: Option<f64>,
        /// 去掉音轨
        #[arg(long)]
        no_audio: bool,
    },

    /// 擦除画面中固定区域的文字/水印（LaMa 逐帧修复，保留音轨）
    Erase {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// LaMa ONNX 模型路径（lama_fp32.onnx）
        #[arg(long)]
        model: PathBuf,
        /// 待擦除区域左上角 x
        #[arg(long)]
        x: u32,
        #[arg(long)]
        y: u32,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        /// 起始时间（秒）
        #[arg(long, default_value_t = 0.0)]
        start: f64,
        /// 时长（秒），缺省到结尾
        #[arg(long)]
        duration: Option<f64>,
        /// 调试用：跳过修复，只做解码+重编码（验证管线）
        #[arg(long, hide = true)]
        skip_inpaint: bool,
    },

    /// 场景切换检测，输出切分时间点
    Scenes {
        input: PathBuf,
        /// content / adaptive / histogram / threshold
        #[arg(long, value_enum, default_value_t = SceneAlgorithm::Content)]
        algorithm: SceneAlgorithm,
        /// 检测阈值，0 表示用算法默认值
        #[arg(long, default_value_t = 0.0)]
        threshold: f64,
        /// 两个场景之间的最小时长（秒）
        #[arg(long, default_value_t = 1.0)]
        min_duration: f64,
        /// 起始时间（秒）
        #[arg(long, default_value_t = 0.0)]
        start: f64,
        /// 截取时长（秒），缺省到结尾
        #[arg(long)]
        duration: Option<f64>,
    },

    /// 从视频中提取音轨为 WAV
    Audio {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value_t = 16000)]
        sample_rate: u32,
        #[arg(long, default_value_t = 1)]
        channels: u16,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    ffmpeg::init().map_err(|e| anyhow!("初始化 ffmpeg 失败: {e}"))?;
    match cmd {
        Cmd::Info { input } => probe::info(input),
        Cmd::Frames {
            input,
            output_dir,
            every_secs,
            fps,
            count,
            start,
            duration,
            max_frames,
        } => frames::frames(
            input,
            output_dir,
            *every_secs,
            *fps,
            *count,
            *start,
            *duration,
            *max_frames,
        ),
        Cmd::Thumbnail { input, output, at } => frames::thumbnail(input, output, *at),
        Cmd::Erase {
            input,
            output,
            model,
            x,
            y,
            width,
            height,
            start,
            duration,
            skip_inpaint,
        } => crate::erase::run(
            input,
            output,
            model,
            (*x, *y, *width, *height),
            *start,
            *duration,
            *skip_inpaint,
        ),
        Cmd::Scenes {
            input,
            algorithm,
            threshold,
            min_duration,
            start,
            duration,
        } => scenes::scenes(
            input,
            *algorithm,
            *threshold,
            *min_duration,
            *start,
            *duration,
        ),
        Cmd::Clip {
            input,
            output,
            start,
            duration,
            no_audio,
        } => edit::clip(input, output, *start, *duration, !no_audio),
        Cmd::Audio {
            input,
            output,
            sample_rate,
            channels,
        } => audio::extract_audio(input, output, *sample_rate, *channels),
    }
}
