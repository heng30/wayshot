//! PaddleOCR-RS CLI tool for OCR recognition

use clap::{Parser, Subcommand, ValueEnum};
use paddle_ocr_rs::{PaddleOCR, OcrTask, OcrResult, TextBlock};
use anyhow::Result;
use serde::Serialize;

/// PaddleOCR-RS: OCR recognition using PaddleOCR-VL1.5
#[derive(Parser)]
#[command(name = "paddle-ocr")]
#[command(about = "OCR recognition using PaddleOCR-VL1.5", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// OCR task type for CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliTask {
    /// Pure text recognition (no position information)
    Text,
    /// Text spotting with bounding box position information
    Spotting,
}

impl From<CliTask> for OcrTask {
    fn from(task: CliTask) -> Self {
        match task {
            CliTask::Text => OcrTask::Text,
            CliTask::Spotting => OcrTask::Spotting,
        }
    }
}

/// Output format type
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Plain text output
    Text,
    /// JSON output (includes position info for spotting)
    Json,
}

#[derive(Subcommand)]
enum Commands {
    /// Perform OCR on an image
    Ocr {
        /// Image file path
        #[arg(short, long)]
        image: String,

        /// Model directory path
        #[arg(short, long)]
        model: String,

        /// OCR task type: text (default) or spotting (with positions)
        #[arg(short, long, default_value = "text")]
        task: CliTask,

        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        format: OutputFormat,

        /// Custom prompt for OCR (default depends on task)
        #[arg(short, long)]
        prompt: Option<String>,

        /// Output file path (prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<String>,
    },
}

/// JSON-serializable text block for output
#[derive(Debug, Serialize)]
struct JsonTextBlock {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bbox: Option<JsonBBox>,
}

#[derive(Debug, Serialize)]
struct JsonBBox {
    /// X1 in thousandths (0-999)
    x1: u32,
    /// Y1 in thousandths (0-999)
    y1: u32,
    /// X2 in thousandths (0-999)
    x2: u32,
    /// Y2 in thousandths (0-999)
    y2: u32,
    /// Normalized coordinates (0.0-1.0)
    normalized: (f64, f64, f64, f64),
}

impl From<TextBlock> for JsonTextBlock {
    fn from(block: TextBlock) -> Self {
        JsonTextBlock {
            text: block.text,
            bbox: block.bbox.map(|b| JsonBBox {
                x1: b.x1,
                y1: b.y1,
                x2: b.x2,
                y2: b.y2,
                normalized: b.to_normalized(),
            }),
        }
    }
}

#[derive(Debug, Serialize)]
struct JsonOcrResult {
    text: String,
    blocks: Vec<JsonTextBlock>,
}

impl From<OcrResult> for JsonOcrResult {
    fn from(result: OcrResult) -> Self {
        JsonOcrResult {
            text: result.text,
            blocks: result.blocks.into_iter().map(JsonTextBlock::from).collect(),
        }
    }
}

fn format_output(result: OcrResult, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            // Plain text output
            if result.blocks.len() == 1 && result.blocks[0].bbox.is_none() {
                // Simple text output
                result.text
            } else {
                // Output with position info
                let mut output = String::new();
                for block in &result.blocks {
                    if let Some(bbox) = block.bbox {
                        let norm = bbox.to_normalized();
                        output.push_str(&format!(
                            "[{:3}% {:3}% {:3}% {:3}%] {}\n",
                            (norm.0 * 100.0).round() as i32,
                            (norm.1 * 100.0).round() as i32,
                            (norm.2 * 100.0).round() as i32,
                            (norm.3 * 100.0).round() as i32,
                            block.text
                        ));
                    } else {
                        output.push_str(&format!("{}\n", block.text));
                    }
                }
                output.trim_end().to_string()
            }
        }
        OutputFormat::Json => {
            // JSON output
            serde_json::to_string_pretty(&JsonOcrResult::from(result))
                .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ocr {
            image,
            model,
            task,
            format,
            prompt,
            output,
        } => {
            // Load model
            println!("Loading model from: {}", model);
            let mut ocr = PaddleOCR::new(&model)?;

            // Determine task type
            let ocr_task: OcrTask = task.into();

            // Perform OCR based on task type
            println!("Processing image: {}", image);
            let result = match ocr_task {
                OcrTask::Text => {
                    // Use provided prompt or default
                    let prompt_text = prompt.unwrap_or_else(|| "OCR:".to_string());
                    let text = ocr.ocr_with_prompt(&image, &prompt_text)?;
                    OcrResult {
                        text: text.clone(),
                        blocks: vec![TextBlock::text_only(text)],
                    }
                }
                OcrTask::Spotting => {
                    // Use spotting task
                    ocr.ocr_with_positions(&image)?
                }
            };

            // Format output
            let output_text = format_output(result, format);

            // Write output
            if let Some(output_path) = output {
                std::fs::write(&output_path, &output_text)?;
                println!("OCR result saved to: {}", output_path);
            } else {
                println!("\nOCR Result:");
                println!("{}", output_text);
            }
        }
    }

    Ok(())
}