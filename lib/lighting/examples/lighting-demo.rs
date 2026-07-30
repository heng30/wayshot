use std::path::PathBuf;

use clap::Parser;
use lighting::{AnimationConfig, LightDirection, SceneGeometry, SpotLightConfig, render_animation};

#[derive(Parser)]
#[command(
    name = "lighting-demo",
    about = "Render a pendulum spotlight animation onto an image"
)]
struct Cli {
    /// Input image path
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    /// Output directory for rendered frames
    #[arg(value_name = "OUTPUT_DIR")]
    output_dir: PathBuf,

    /// Frames per second
    #[arg(short, long, default_value_t = 30)]
    fps: u32,

    /// Animation duration in seconds
    #[arg(short, long, default_value_t = 10.0)]
    duration: f32,

    /// Ambient light level (0.0 - 1.0)
    #[arg(long, default_value_t = 0.06)]
    ambient: f32,

    /// Spotlight beam angle in degrees (1 - 120), larger = wider illumination
    #[arg(long, default_value_t = 34.0)]
    angle: f32,

    /// Spotlight brightness in lumens (300 - 2600)
    #[arg(long, default_value_t = 1450.0)]
    brightness: f32,

    /// Light direction (up, down, left, right)
    #[arg(long, default_value = "down")]
    direction: LightDirection,

    /// Light position X (-1.0 - 2.0)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    pos_x: f32,

    /// Light position Y (-1.0 - 2.0)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    pos_y: f32,

    /// Rope length (0.0 - 1.0), scaled by scene dimension
    #[arg(long, default_value_t = 0.3)]
    rope_length: f32,

    /// Gravity magnitude (0.0 = weightless, default = 9.81)
    #[arg(long, default_value_t = 9.81)]
    gravity: f32,

    /// Swing amplitude (0.0 - 1.0), initial displacement of the lamp
    #[arg(long, default_value_t = 0.3)]
    swing: f32,

    /// Damping (0.9 - 1.0), lower = faster decay, 1.0 = no damping
    #[arg(long, default_value_t = 0.9948)]
    damping: f32,

    /// Scene type: vertical or horizontal
    #[arg(long, default_value = "vertical")]
    scene: String,
}

fn main() {
    let cli = Cli::parse();

    if !cli.input.exists() {
        eprintln!("Error: input image not found: {}", cli.input.display());
        std::process::exit(1);
    }

    let pos_x = cli.pos_x.clamp(-1.0, 2.0);
    let pos_y = cli.pos_y.clamp(-1.0, 2.0);

    let scene = match cli.scene.as_str() {
        "horizontal" => SceneGeometry::default_horizontal(),
        _ => SceneGeometry::default_vertical(),
    };

    eprintln!(
        "Rendering: {} -> {} ({}fps, {:.1}s, {} frames, dir={:?}, pos=({:.2},{:.2}))",
        cli.input.display(),
        cli.output_dir.display(),
        cli.fps,
        cli.duration,
        (cli.fps as f32 * cli.duration) as u32,
        cli.direction,
        pos_x,
        pos_y,
    );

    let anim_config = AnimationConfig {
        fps: cli.fps,
        duration_secs: cli.duration,
        ambient: cli.ambient,
        ..Default::default()
    };

    let light_config = SpotLightConfig {
        angle_deg: cli.angle,
        brightness: cli.brightness,
        direction: cli.direction,
        pos: (pos_x, pos_y),
        rope_length: cli.rope_length.clamp(0.0, 1.0),
        gravity: cli.gravity.max(0.0),
        swing: cli.swing.clamp(0.0, 1.0),
        damping: cli.damping.clamp(0.9, 1.0),
        ..Default::default()
    };

    match render_animation(
        &cli.input,
        &cli.output_dir,
        light_config,
        scene,
        anim_config,
    ) {
        Ok(()) => eprintln!("Done."),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
