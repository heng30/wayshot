use clap::Parser;
use std::{path::PathBuf, time::Instant};
use stem_splitter::{AudioData, ModelHandle, split};

#[derive(Parser)]
#[command(
    name = "stem_splitter_cli",
    about = "Split a WAV file into stems using an ONNX model"
)]
struct Cli {
    /// Input WAV file path
    #[arg(default_value = "test.wav")]
    input: PathBuf,

    /// Model directory containing manifest.json and model .ort file
    #[arg(short, long, default_value = "/home/blue/models/htdemucs-ort")]
    model_dir: PathBuf,

    /// Output directory for stem WAV files
    #[arg(short, long, default_value = "output")]
    output: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    let model_path = cli.model_dir.join("htdemucs.ort");
    let manifest_path = cli.model_dir.join("manifest.json");

    println!("Loading model ...");
    let t0 = Instant::now();
    let handle =
        ModelHandle::from_json_file(&manifest_path, model_path).expect("Failed to load model");
    println!(
        "  Model {} loaded in {:.2}s",
        handle.manifest.name,
        t0.elapsed().as_secs_f64()
    );

    let audio = read_wav(&cli.input);
    println!(
        "Input: {} Hz, {} ch, {} frames ({:.1}s)",
        audio.sample_rate,
        audio.channels,
        audio.samples.len() / audio.channels as usize,
        audio.samples.len() as f32 / (audio.sample_rate as f32 * audio.channels as f32)
    );

    println!("Splitting into stems ...");
    let t1 = Instant::now();
    let result = split(&audio, &handle).expect("Split failed");
    println!(
        "  Inference completed in {:.2}s",
        t1.elapsed().as_secs_f64()
    );

    std::fs::create_dir_all(&cli.output).ok();

    for (name, stem_audio) in &result.stems {
        let out_path = cli.output.join(format!("{}.wav", name));
        write_wav(&out_path, stem_audio);
        println!("  Wrote {}", out_path.display());
    }

    println!("Done. Total: {:.2}s", t0.elapsed().as_secs_f64());
}

fn read_wav(path: &std::path::Path) -> AudioData {
    let mut reader = hound::WavReader::open(path).expect("Failed to open WAV file");
    let spec = reader.spec();
    let channels = spec.channels;
    let sample_rate = spec.sample_rate;

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap_or(0) as f32 / max)
                .collect()
        }
    };

    AudioData {
        samples,
        sample_rate,
        channels,
    }
}

fn write_wav(path: &std::path::Path, audio: &AudioData) {
    let spec = hound::WavSpec {
        channels: audio.channels,
        sample_rate: audio.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec).expect("Failed to create WAV file");
    let max_val = 32767.0f32;

    for sample in &audio.samples {
        let s = (sample * max_val).clamp(-max_val, max_val);
        writer
            .write_sample(s as i16)
            .expect("Failed to write sample");
    }

    writer.finalize().expect("Failed to finalize WAV file");
}

