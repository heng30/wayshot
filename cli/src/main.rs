mod ai;
mod animation;
mod audio;
mod camera;
mod capture;
mod clipboard;
mod codeimg;
mod danmaku;
mod dedup;
mod download;
mod erase;
mod font;
mod image_ops;
mod imgsearch;
mod llm;
mod musicdl;
mod musicgen;
mod ocr;
mod project;
mod qrcode_gen;
mod similar;
mod smart_mix;
mod subtitle;
mod transcribe;
mod ttfx;
mod tts;
mod update;
mod util;
mod video;
mod voice_cut;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(
    name = "wayshot-cli",
    version,
    about = "Wayshot 的无 GUI 命令行版本：所有命令输出 JSON，便于 LLM 调用"
)]
struct Cli {
    /// 格式化（缩进）JSON 输出
    #[arg(long, global = true)]
    pretty: bool,

    /// 输出调试日志到 stderr
    #[arg(long, short, global = true)]
    verbose: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 屏幕截图（Wayland wlr-screencopy）
    #[command(subcommand)]
    Screen(capture::Cmd),

    /// 图片处理
    #[command(subcommand)]
    Image(image_ops::Cmd),

    /// 视频处理（ffmpeg）
    #[command(subcommand)]
    Video(video::Cmd),

    /// 音频处理
    #[command(subcommand)]
    Audio(audio::Cmd),

    /// 代码转图片
    Code(codeimg::Args),

    /// 终端文字动画特效
    Ttfx(ttfx::Args),

    /// 视频工程（.wayshot）信息与导出
    #[command(subcommand)]
    Project(project::Cmd),

    /// 照片去重
    #[command(subcommand)]
    Dedup(dedup::Cmd),

    /// 下载
    #[command(subcommand)]
    Download(download::Cmd),

    /// 本地 AI 模型能力（超分 / 去背景 / 去水印 / 人声分离）
    #[command(subcommand)]
    Ai(ai::Cmd),

    /// 在视频中查找与参考图相似的片段
    #[command(subcommand)]
    Similar(similar::Cmd),

    /// 图片动画（长图滚动 / 箭头 / 框选 / 评分标记）
    #[command(subcommand)]
    Animation(animation::Cmd),

    /// 本地剪贴板
    #[command(subcommand)]
    Clipboard(clipboard::Cmd),

    /// 二维码生成
    Qrcode(qrcode_gen::Args),

    /// B 站弹幕下载
    #[command(subcommand)]
    Danmaku(danmaku::Cmd),

    /// 网络图片搜索与下载
    #[command(subcommand)]
    ImageSearch(imgsearch::Cmd),

    /// 摄像头
    #[command(subcommand)]
    Camera(camera::Cmd),

    /// 文本转语音（VoxCPM2）
    Tts(tts::Args),

    /// 字幕文件处理（SRT / LRC）
    #[command(subcommand)]
    Subtitle(subtitle::Cmd),

    /// 图片文字识别（PaddleOCR-VL）
    #[command(subcommand)]
    Ocr(ocr::Cmd),

    /// 语音转文字（FunASR-Nano），可输出 SRT
    Transcribe(transcribe::Args),

    /// 文本生成音乐（MusicGen）
    #[command(subcommand)]
    MusicGen(musicgen::Cmd),

    /// 音乐搜索与下载（网易云/酷我/酷狗/千千）
    #[command(subcommand)]
    Music(musicdl::Cmd),

    /// 大模型对话（OpenAI 兼容接口）
    Llm(llm::Args),

    /// 系统字体
    #[command(subcommand)]
    Font(font::Cmd),

    /// 版本更新检查
    #[command(subcommand)]
    Update(update::Cmd),
}

fn main() {
    let cli = Cli::parse();

    let default_level = if cli.verbose { "debug" } else { "warn" };
    let _ =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_level))
            .target(env_logger::Target::Stderr)
            .try_init();

    let result: Result<Value> = match &cli.cmd {
        Cmd::Screen(cmd) => capture::run(cmd),
        Cmd::Image(cmd) => image_ops::run(cmd),
        Cmd::Video(cmd) => video::run(cmd),
        Cmd::Audio(cmd) => audio::run(cmd),
        Cmd::Code(args) => codeimg::run(args),
        Cmd::Ttfx(args) => ttfx::run(args),
        Cmd::Project(cmd) => project::run(cmd),
        Cmd::Dedup(cmd) => dedup::run(cmd),
        Cmd::Download(cmd) => download::run(cmd),
        Cmd::Ai(cmd) => ai::run(cmd),
        Cmd::Similar(cmd) => similar::run(cmd),
        Cmd::Animation(cmd) => animation::run(cmd),
        Cmd::Clipboard(cmd) => clipboard::run(cmd),
        Cmd::Qrcode(args) => qrcode_gen::run(args),
        Cmd::Danmaku(cmd) => danmaku::run(cmd),
        Cmd::ImageSearch(cmd) => imgsearch::run(cmd),
        Cmd::Camera(cmd) => camera::run(cmd),
        Cmd::Tts(args) => tts::run(args),
        Cmd::Subtitle(cmd) => subtitle::run(cmd),
        Cmd::Ocr(cmd) => ocr::run(cmd),
        Cmd::Transcribe(args) => transcribe::run(args),
        Cmd::MusicGen(cmd) => musicgen::run(cmd),
        Cmd::Music(cmd) => musicdl::run(cmd),
        Cmd::Llm(args) => llm::run(args),
        Cmd::Font(cmd) => font::run(cmd),
        Cmd::Update(cmd) => update::run(cmd),
    };

    let (value, code) = match result {
        Ok(value) => (util::ok(value), 0),
        Err(err) => (json!({ "ok": false, "error": format!("{err:#}") }), 1),
    };

    println!(
        "{}",
        if cli.pretty {
            serde_json::to_string_pretty(&value).expect("json")
        } else {
            serde_json::to_string(&value).expect("json")
        }
    );

    std::process::exit(code);
}
