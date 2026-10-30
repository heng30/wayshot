use anyhow::{Result, anyhow};
use clap::{Subcommand, ValueEnum};
use image_animation::{
    ArrowDrawConfig, GradeMarkConfig, GradeMarkType, ImageScrollConfig, RectDrawConfig,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, ValueEnum)]
pub enum Mark {
    Circle,
    Checkmark,
    Cross,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum BackgroundKind {
    Grid,
    MovingGrid,
    Glitch,
    NoiseFlow,
    Bokeh,
    MatrixRain,
    Fluid,
    Kaleidoscope,
    LightEffects,
    Ink,
    ParticleLife,
    ParticleNetwork,
    FlowField,
    Shape,
    BlackHole,
    Galaxy,
    Triangle,
    Wave,
    CrossLine,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// 长图滚动动画（长截图 → mp4）
    Scroll {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// 视口高度
        #[arg(long, default_value_t = 1080)]
        height: u32,
        #[arg(long, default_value_t = 25)]
        fps: u32,
        /// 每秒滚动的视口高度比例
        #[arg(long, default_value_t = 0.2)]
        speed: f32,
        #[arg(long, default_value_t = 1.0)]
        start_pause: f32,
        #[arg(long, default_value_t = 1.0)]
        end_pause: f32,
    },

    /// 箭头指示动画
    Arrow {
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value_t = 1920)]
        width: u32,
        #[arg(long, default_value_t = 1080)]
        height: u32,
        /// 绘制时长（毫秒）
        #[arg(long, default_value_t = 800)]
        duration_ms: u32,
        #[arg(long, default_value_t = 1.0)]
        end_pause: f32,
        #[arg(long, default_value_t = 0.5)]
        x: f32,
        #[arg(long, default_value_t = 0.5)]
        y: f32,
        /// 颜色，格式 rrggbb
        #[arg(long, default_value = "ff3355")]
        color: String,
        #[arg(long, default_value_t = 6.0)]
        line_width: f32,
    },

    /// 矩形框选动画
    Rect {
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value_t = 1920)]
        width: u32,
        #[arg(long, default_value_t = 1080)]
        height: u32,
        #[arg(long, default_value_t = 800)]
        duration_ms: u32,
        #[arg(long, default_value_t = 1.0)]
        end_pause: f32,
        #[arg(long, default_value_t = 0.5)]
        x: f32,
        #[arg(long, default_value_t = 0.5)]
        y: f32,
        #[arg(long, default_value = "ff3355")]
        color: String,
        #[arg(long, default_value_t = 6.0)]
        line_width: f32,
        /// 矩形宽（相对输出宽度比例）
        #[arg(long, default_value_t = 0.3)]
        rect_width: f32,
        /// 矩形高（相对输出高度比例）
        #[arg(long, default_value_t = 0.2)]
        rect_height: f32,
        #[arg(long, default_value_t = 8.0)]
        corner_radius: f32,
    },

    /// 背景动画（19 种程序化动态背景 → mp4）
    Background {
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, value_enum, default_value_t = BackgroundKind::Bokeh)]
        kind: BackgroundKind,
        #[arg(long, default_value_t = 1920)]
        width: u32,
        #[arg(long, default_value_t = 1080)]
        height: u32,
        #[arg(long, default_value_t = 30)]
        fps: u32,
        /// 时长（秒）
        #[arg(long, default_value_t = 10.0)]
        duration: f64,
    },

    /// 评分标记动画（对号 / 错号 / 圆圈）
    Grade {
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, value_enum, default_value_t = Mark::Checkmark)]
        mark: Mark,
        #[arg(long, default_value_t = 1920)]
        width: u32,
        #[arg(long, default_value_t = 1080)]
        height: u32,
        /// 标记大小
        #[arg(long, default_value_t = 100.0)]
        size: f32,
        #[arg(long, default_value_t = 10.0)]
        line_width: f32,
        #[arg(long, default_value_t = 500)]
        duration_ms: u32,
        #[arg(long, default_value_t = 1.0)]
        end_pause: f32,
        #[arg(long, default_value_t = 0.5)]
        x: f32,
        #[arg(long, default_value_t = 0.5)]
        y: f32,
        #[arg(long, default_value = "ff5050")]
        color: String,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Scroll { .. } => scroll(cmd),
        Cmd::Arrow { .. } => arrow(cmd),
        Cmd::Rect { .. } => rect(cmd),
        Cmd::Grade { .. } => grade(cmd),
        Cmd::Background { .. } => background(cmd),
    }
}
fn scroll(cmd: &Cmd) -> Result<Value> {
    let Cmd::Scroll {
        input,
        output,
        height,
        fps,
        speed,
        start_pause,
        end_pause,
    } = cmd
    else {
        unreachable!()
    };

    let mut config = ImageScrollConfig::new(input.to_path_buf())
        .with_output_height(*height)
        .with_fps(*fps)
        .with_scroll_speed(*speed)
        .with_start_pause(*start_pause)
        .with_end_pause(*end_pause);
    config
        .record(output.to_path_buf())
        .map_err(|e| anyhow!("渲染失败: {e}"))?;
    Ok(done(output))
}

fn arrow(cmd: &Cmd) -> Result<Value> {
    let Cmd::Arrow {
        output,
        width,
        height,
        duration_ms,
        end_pause,
        x,
        y,
        color,
        line_width,
    } = cmd
    else {
        unreachable!()
    };

    let rgba = parse_color(color)?;
    let mut config = ArrowDrawConfig::new()
        .with_width(*width)
        .with_height(*height)
        .with_duration_ms(*duration_ms)
        .with_end_pause(*end_pause)
        .with_position((*x, *y))
        .with_line_style(image_animation::ArrowLineStyle {
            color: rgba,
            width: *line_width,
            ..Default::default()
        });
    config
        .record(output.to_path_buf())
        .map_err(|e| anyhow!("渲染失败: {e}"))?;
    Ok(done(output))
}

fn rect(cmd: &Cmd) -> Result<Value> {
    let Cmd::Rect {
        output,
        width,
        height,
        duration_ms,
        end_pause,
        x,
        y,
        color,
        line_width,
        rect_width,
        rect_height,
        corner_radius,
    } = cmd
    else {
        unreachable!()
    };

    let rgba = parse_color(color)?;
    let mut config = RectDrawConfig::new()
        .with_width(*width)
        .with_height(*height)
        .with_duration_ms(*duration_ms)
        .with_end_pause(*end_pause)
        .with_position((*x, *y))
        .with_line_style(image_animation::LineStyle {
            color: rgba,
            width: *line_width,
            ..Default::default()
        })
        .with_rect_style(image_animation::RectStyle {
            width: *rect_width * *width as f32,
            height: *rect_height * *height as f32,
            corner_radius: *corner_radius,
        });
    config
        .record(output.to_path_buf())
        .map_err(|e| anyhow!("渲染失败: {e}"))?;
    Ok(done(output))
}

fn grade(cmd: &Cmd) -> Result<Value> {
    let Cmd::Grade {
        output,
        mark,
        width,
        height,
        size,
        line_width,
        duration_ms,
        end_pause,
        x,
        y,
        color,
    } = cmd
    else {
        unreachable!()
    };

    let rgba = parse_color(color)?;
    let mut config = GradeMarkConfig::new(match mark {
        Mark::Circle => GradeMarkType::Circle,
        Mark::Checkmark => GradeMarkType::Checkmark,
        Mark::Cross => GradeMarkType::Cross,
    })
    .with_width(*width)
    .with_height(*height)
    .with_size(*size)
    .with_line_width(*line_width)
    .with_duration_ms(*duration_ms)
    .with_end_pause(*end_pause)
    .with_position((*x, *y))
    .with_color(rgba);
    config
        .record(output.to_path_buf())
        .map_err(|e| anyhow!("渲染失败: {e}"))?;
    Ok(done(output))
}

fn background(cmd: &Cmd) -> Result<Value> {
    let Cmd::Background {
        output,
        kind,
        width,
        height,
        fps,
        duration,
    } = cmd
    else {
        unreachable!()
    };
    let (kind, width, height, fps, duration) = (*kind, *width, *height, *fps, *duration);

    use background_animation::{
        Animation, AnimationRecordConfig, black_hole::BlackHoleConfig, bokeh::BokehConfig,
        cross_line::CrossLineConfig, flow_field::FlowFieldConfig, fluid::FluidConfig,
        galaxy::GalaxyConfig, glitch::GlitchConfig, grid::GridConfig, ink::InkDissipationConfig,
        kaleidoscope::KaleidoscopeConfig, light_effects::LightEffectsConfig,
        matrix_rain::MatrixRainConfig, moving_grid::MovingGridConfig, noise_flow::NoiseFlowConfig,
        particle_life::ParticleLifeConfig, particle_network::ParticleNetworkConfig,
        shape::ShapeConfig, triangle::TriangleConfig, wave::WaveConfig,
    };

    let record = AnimationRecordConfig::new(
        (width - width % 2).max(2),
        (height - height % 2).max(2),
        fps.max(1),
        std::time::Duration::from_secs_f64(duration.max(0.1)),
        output,
    );

    let mut animation: Box<dyn Animation> = match kind {
        BackgroundKind::Grid => Box::new(GridConfig::new()),
        BackgroundKind::MovingGrid => Box::new(MovingGridConfig::new()),
        BackgroundKind::Glitch => Box::new(GlitchConfig::new()),
        BackgroundKind::NoiseFlow => Box::new(NoiseFlowConfig::new()),
        BackgroundKind::Bokeh => Box::new(BokehConfig::new()),
        BackgroundKind::MatrixRain => Box::new(MatrixRainConfig::new()),
        BackgroundKind::Fluid => Box::new(FluidConfig::new()),
        BackgroundKind::Kaleidoscope => Box::new(KaleidoscopeConfig::new()),
        BackgroundKind::LightEffects => Box::new(LightEffectsConfig::new()),
        BackgroundKind::Ink => Box::new(InkDissipationConfig::new()),
        BackgroundKind::ParticleLife => Box::new(ParticleLifeConfig::new()),
        BackgroundKind::ParticleNetwork => Box::new(ParticleNetworkConfig::new()),
        BackgroundKind::FlowField => Box::new(FlowFieldConfig::new()),
        BackgroundKind::Shape => Box::new(ShapeConfig::new()),
        BackgroundKind::BlackHole => Box::new(BlackHoleConfig::new()),
        BackgroundKind::Galaxy => Box::new(GalaxyConfig::new()),
        BackgroundKind::Triangle => Box::new(TriangleConfig::new()),
        BackgroundKind::Wave => Box::new(WaveConfig::new()),
        BackgroundKind::CrossLine => Box::new(CrossLineConfig::new()),
    };

    animation
        .animate_record(record)
        .map_err(|e| anyhow!("渲染失败: {e}"))?;

    Ok(done(output))
}

fn done(output: &Path) -> Value {
    json!({ "path": output.display().to_string() })
}

fn parse_color(hex: &str) -> Result<(u8, u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 && hex.len() != 8 {
        return Err(anyhow!("颜色格式应为 rrggbb 或 rrggbbaa: {hex}"));
    }
    let value = u32::from_str_radix(hex, 16)
        .map_err(|_| anyhow!("颜色格式应为 rrggbb 或 rrggbbaa: {hex}"))?;

    Ok(if hex.len() == 6 {
        ((value >> 16) as u8, (value >> 8) as u8, value as u8, 255)
    } else {
        (
            (value >> 24) as u8,
            (value >> 16) as u8,
            (value >> 8) as u8,
            value as u8,
        )
    })
}
