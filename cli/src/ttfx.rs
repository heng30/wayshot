use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::path::PathBuf;
use ttfx_rs::{
    effects::EffectCommand,
    render::{RenderConfig, SequenceRenderer},
};

use crate::util;

#[derive(clap::Args)]
pub struct Args {
    /// 要渲染的文本（--list-effects 时可省略）
    #[arg(long)]
    pub text: Option<String>,

    /// 特效名（matrix / decrypt / beams / print / rain / waves / smoke ...）
    #[arg(long)]
    pub effect: Option<String>,

    /// 最终帧输出 PNG（--list-effects 时可省略）
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// 同时把每一帧保存到该目录
    #[arg(long)]
    pub frames_dir: Option<PathBuf>,

    #[arg(long, default_value_t = 24.0)]
    pub font_size: f32,

    /// 固定画布宽度（配合 --height 使用，缺省按文本自动尺寸）
    #[arg(long)]
    pub width: Option<u32>,

    #[arg(long)]
    pub height: Option<u32>,

    #[arg(long, default_value_t = 2)]
    pub padding: u32,

    #[arg(long, default_value_t = 60)]
    pub fps: u32,

    /// 最多渲染多少帧
    #[arg(long, default_value_t = 600)]
    pub max_frames: u32,

    #[arg(long)]
    pub seed: Option<u64>,

    /// ASCII 字体文件（必填，可用 `font list` 查找）
    #[arg(long)]
    pub ascii_font: PathBuf,

    /// 非 ASCII 字体文件（必填，可用 `font list` 查找）
    #[arg(long)]
    pub non_ascii_font: PathBuf,

    /// 列出所有可用特效
    #[arg(long)]
    pub list_effects: bool,
}

pub fn run(args: &Args) -> Result<Value> {
    let (text, effect_name, output) = match (&args.text, &args.effect, &args.output) {
        (Some(text), Some(effect), Some(output)) => (text, effect, output),
        _ => {
            if args.list_effects {
                return Ok(json!({ "effects": EFFECTS }));
            }
            return Err(anyhow!(
                "需要 --text / --effect / --output，或用 --list-effects 查看特效列表"
            ));
        }
    };

    let mut effect = EffectCommand::from_name(effect_name)
        .map(|c| c.build_effect())
        .ok_or_else(|| anyhow!("未知特效: {effect_name}，可用特效见 --list-effects"))?;

    let font = ttfx_rs::render::Font::from_files(&args.ascii_font, &args.non_ascii_font)
        .map_err(|e| anyhow!("加载字体失败: {e}"))?;

    let mut render = match (args.width, args.height) {
        (Some(width), Some(height)) => RenderConfig::new(width, height, font),
        _ => RenderConfig::auto(args.font_size, args.padding, args.padding, font),
    };
    render.fps = args.fps;
    render.seed = args.seed;

    let mut renderer = SequenceRenderer::new(text, render.clone())
        .map_err(|e| anyhow!("初始化渲染器失败: {e}"))?;
    renderer
        .build_effect(effect.as_mut())
        .map_err(|e| anyhow!("构建特效失败: {e}"))?;

    if let Some(dir) = &args.frames_dir {
        std::fs::create_dir_all(dir).with_context(|| format!("无法创建目录: {}", dir.display()))?;
    }

    let mut frames = 0u32;
    let mut last = None;
    while let Some(frame) = renderer.next_frame(effect.as_mut()) {
        if let Some(dir) = &args.frames_dir {
            frame.save_png(dir.join(format!("frame_{:04}.png", frame.index)))?;
        }
        last = Some(frame.image);
        frames += 1;
        if frames >= args.max_frames {
            break;
        }
    }

    let last = last.ok_or_else(|| anyhow!("特效没有渲染出任何帧"))?;
    util::save_rgba(&last, output)?;

    Ok(json!({
        "path": output.display().to_string(),
        "frames": frames,
        "width": last.width(),
        "height": last.height(),
        "effect": effect_name,
    }))
}

const EFFECTS: &[&str] = &[
    "beams",
    "binarypath",
    "blackhole",
    "bouncyballs",
    "bubbles",
    "burn",
    "colorshift",
    "crumble",
    "decrypt",
    "errorcorrect",
    "expand",
    "fireworks",
    "highlight",
    "laseretch",
    "matrix",
    "middleout",
    "orbittingvolley",
    "overflow",
    "pour",
    "print",
    "rain",
    "randomsequence",
    "rings",
    "scattered",
    "slice",
    "slide",
    "smoke",
    "spotlights",
    "spray",
    "swarm",
    "sweep",
    "synthgrid",
    "thunderstorm",
    "unstable",
    "vhstape",
    "waves",
    "wipe",
];
