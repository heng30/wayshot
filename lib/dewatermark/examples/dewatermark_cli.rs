use anyhow::{bail, Context, Result};
use clap::Parser;
use dewatermark::{MaskInput, WatermarkRegion, load_session, process, MODEL_FILENAME};
use image::GenericImageView;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Position logic — moved from the library; only the CLI needs this
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum WatermarkPosition {
    BottomRight,
    BottomLeft,
    TopRight,
    TopLeft,
}

impl WatermarkPosition {
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "bottom-right" => Ok(Self::BottomRight),
            "bottom-left" => Ok(Self::BottomLeft),
            "top-right" => Ok(Self::TopRight),
            "top-left" => Ok(Self::TopLeft),
            _ => bail!(
                "Unknown position '{}'. Use: bottom-right, bottom-left, top-right, top-left",
                s
            ),
        }
    }

    fn to_region(
        self,
        img_w: u32,
        img_h: u32,
        width_ratio: f64,
        height_ratio: f64,
    ) -> WatermarkRegion {
        let w = (img_w as f64 * width_ratio).floor() as u32;
        let h = (img_h as f64 * height_ratio).floor() as u32;
        let (x, y) = match self {
            Self::BottomRight => (img_w.saturating_sub(w), img_h.saturating_sub(h)),
            Self::BottomLeft => (0, img_h.saturating_sub(h)),
            Self::TopRight => (img_w.saturating_sub(w), 0),
            Self::TopLeft => (0, 0),
        };
        WatermarkRegion {
            x,
            y,
            width: w,
            height: h,
        }
    }
}

/// Expand the region by a ratio (>1.0 enlarges, 1.0 unchanged), centered on the original.
fn expand_region(region: &WatermarkRegion, ratio: f64, img_w: u32, img_h: u32) -> WatermarkRegion {
    let extra_w = (region.width as f64 * (ratio - 1.0)).round() as u32;
    let extra_h = (region.height as f64 * (ratio - 1.0)).round() as u32;
    let new_w = region.width + extra_w;
    let new_h = region.height + extra_h;
    let new_x = region.x.saturating_sub(extra_w / 2);
    let new_y = region.y.saturating_sub(extra_h / 2);
    WatermarkRegion {
        x: new_x,
        y: new_y,
        width: new_w.min(img_w.saturating_sub(new_x)),
        height: new_h.min(img_h.saturating_sub(new_y)),
    }
}

fn parse_region(spec: &str, img_w: u32, img_h: u32) -> Result<WatermarkRegion> {
    if let Some(pct) = spec.strip_prefix("pct:") {
        let parts: Vec<&str> = pct.split(',').collect();
        if parts.len() != 4 {
            bail!("Region format: pct:x%,y%,w%,h% (got {} parts)", parts.len());
        }
        let parse_pct = |s: &str| -> Result<f64> {
            let s = s.trim_end_matches('%');
            let v: f64 = s.parse().context("invalid percentage")?;
            if !(0.0..=100.0).contains(&v) {
                bail!("percentage must be 0-100, got {}", v);
            }
            Ok(v / 100.0)
        };
        let xp = parse_pct(parts[0])?;
        let yp = parse_pct(parts[1])?;
        let wp = parse_pct(parts[2])?;
        let hp = parse_pct(parts[3])?;
        return Ok(WatermarkRegion {
            x: (img_w as f64 * xp).round() as u32,
            y: (img_h as f64 * yp).round() as u32,
            width: (img_w as f64 * wp).round() as u32,
            height: (img_h as f64 * hp).round() as u32,
        });
    }

    let parts: Vec<&str> = spec.split(',').collect();
    if parts.len() != 4 {
        bail!("Region format: x,y,width,height or pct:x%,y%,w%,h%");
    }
    Ok(WatermarkRegion {
        x: parts[0].parse().context("invalid x")?,
        y: parts[1].parse().context("invalid y")?,
        width: parts[2].parse().context("invalid width")?,
        height: parts[3].parse().context("invalid height")?,
    })
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "dewatermark")]
#[command(about = "Remove watermarks from images using LaMa inpainting")]
struct Cli {
    /// Input image path(s)
    input: Vec<PathBuf>,

    /// Output image path (only valid with a single input) [default: <input>_clean.png]
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Path to lama_fp32.onnx model file (required)
    #[arg(short, long)]
    model: PathBuf,

    /// Watermark position: bottom-right, bottom-left, top-right, top-left
    #[arg(long, default_value = "bottom-right")]
    position: String,

    /// Watermark height ratio (fraction of image height, used with --position)
    #[arg(long, default_value_t = 0.15)]
    height_ratio: f64,

    /// Watermark width ratio (fraction of image width, used with --position)
    #[arg(long, default_value_t = 0.15)]
    width_ratio: f64,

    /// Extended region ratio for blending overlap (>1.0 enlarges, 1.0 unchanged)
    #[arg(long, default_value_t = 1.16)]
    extended_ratio: f64,

    /// Watermark region as "x,y,width,height" in pixels or "pct:x%,y%,w%,h%" as percentages.
    /// Overrides --position, --height-ratio, --width-ratio, --extended-ratio.
    #[arg(long, value_name = "REGION")]
    region: Option<String>,

    /// Path to a grayscale mask image (white = inpaint, black = keep).
    /// Overrides --region, --position, --height-ratio, --width-ratio, --extended-ratio.
    #[arg(long, value_name = "MASK_PNG")]
    mask: Option<PathBuf>,
}

fn default_output(input: &std::path::Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    let parent = input.parent().unwrap_or_else(|| std::path::Path::new("."));
    parent.join(format!("{}_clean.png", stem))
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    if cli.input.is_empty() {
        bail!(
            "Missing required argument: <INPUT>\n\n\
             Usage: dewatermark <INPUT>... -m <MODEL> [OPTIONS]\n\
             Use --help for more information."
        );
    }

    if cli.output.is_some() && cli.input.len() > 1 {
        bail!("--output cannot be used with multiple input files");
    }

    for input in &cli.input {
        if !input.exists() {
            bail!("Input file not found: {}", input.display());
        }
    }

    if !cli.model.exists() {
        bail!(
            "Model file not found: {}\n\n\
             You can download the model (~208 MB) from:\n  \
               https://huggingface.co/Carve/LaMa-ONNX/resolve/main/{}\n\n\
             Then use --model to specify the path.",
            cli.model.display(),
            MODEL_FILENAME
        );
    }

    eprintln!("Loading model...");
    let mut session = load_session(&cli.model)?;

    for (i, input) in cli.input.iter().enumerate() {
        let output = cli.output.clone().unwrap_or_else(|| default_output(input));

        let img = image::open(input)
            .with_context(|| format!("Failed to open {}", input.display()))?;
        let (img_w, img_h) = img.dimensions();

        let region = if let Some(mask_path) = &cli.mask {
            if !mask_path.exists() {
                bail!("Mask file not found: {}", mask_path.display());
            }
            let gray = image::open(mask_path)
                .with_context(|| format!("Failed to open mask {}", mask_path.display()))?
                .to_luma8();
            MaskInput::Pixels(gray)
        } else if let Some(region_str) = &cli.region {
            let r = parse_region(region_str, img_w, img_h)?;
            MaskInput::Rect(r)
        } else {
            let position = WatermarkPosition::from_str(&cli.position)?;
            let base = position.to_region(img_w, img_h, cli.width_ratio, cli.height_ratio);
            let expanded = expand_region(&base, cli.extended_ratio, img_w, img_h);
            MaskInput::Rect(expanded)
        };

        if cli.input.len() > 1 {
            eprintln!(
                "[{}/{}] Processing {}...",
                i + 1,
                cli.input.len(),
                input.display()
            );
        }

        let pb = ProgressBar::new(4);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .context("Invalid progress bar template")?
                .progress_chars("=> "),
        );

        let mut step = 0u64;
        let final_image = process(&img, &mut session, &region, |msg| {
            pb.set_message(msg.to_string());
            if step < 4 {
                step += 1;
                pb.set_position(step);
            }
        })?;

        final_image
            .save(&output)
            .with_context(|| format!("Failed to save {}", output.display()))?;

        pb.finish_with_message(format!("Saved to {}", output.display()));
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}