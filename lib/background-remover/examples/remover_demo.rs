use anyhow::{Context, Result};
use background_remover::{BackgroundRemover, Model};
use std::{fs, path::PathBuf, time::Instant};

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let input_file = "./examples/test-rgb.png";
    let output_dir = PathBuf::from("./output");
    if !output_dir.exists() {
        fs::create_dir(&output_dir)?;
    }

    let img = image::open(&input_file)?;
    let (img_width, img_height) = (img.width(), img.height());
    log::info!("Image size: {}x{}", img_width, img_height);

    for model in Model::all_models().into_iter() {
        let model_name = model.to_str().trim_end_matches(".onnx");
        let input_name = input_file
            .trim_start_matches("./examples/")
            .trim_end_matches(".png");

        let model_path = PathBuf::from("./models").join(model.to_str());
        if !model_path.exists() {
            log::warn!("Model file not found: {}", model_path.display());
            continue;
        }

        log::info!("Loading model from: {}", model_path.display());

        let mut remover = BackgroundRemover::new(model, &model_path)?;

        let rgb = img.to_rgb8();
        let inference_start = Instant::now();
        let (result, mask) = remover.remove_with_mask(&rgb)?;
        log::info!("Remove background spent: {:?}", inference_start.elapsed());

        let output_path = output_dir.join(format!("{}_{}.png", input_name, model_name));
        result
            .save(&output_path)
            .with_context(|| output_path.to_string_lossy().to_string())?;
        log::info!("Saving result to: {:?}", output_path);

        let mask_path = output_dir.join(format!("mask_{}_{}.png", input_name, model_name));
        let binary_mask = BackgroundRemover::create_binary_mask(&mask, 128);
        binary_mask.save(&mask_path)?;
        log::info!("Saving mask to: {:?}", mask_path);

        log::info!("Background removal completed successfully!");
        log::info!("=========================================\n");
    }

    Ok(())
}
