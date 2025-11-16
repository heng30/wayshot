use screen_capture::CaptureStreamConfig;
use screen_capture_wayland_wlr as capture;

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let screen_infos = capture::available_screens()?;
    assert!(!screen_infos.is_empty());

    let (tx, rx) = mpsc::channel();
    let sig = Arc::new(AtomicBool::new(false));
    let mut ctrlc_count = 0;

    let config = CaptureStreamConfig {
        name: screen_infos[0].name.clone(),
        include_cursor: true,
        fps: Some(25.0),
        cancel_sig: sig.clone(),
    };

    ctrlc::set_handler(move || {
        println!("\nCtrl-C received, stopping recording...");
        sig.store(true, Ordering::Relaxed);
        ctrlc_count += 1;

        if ctrlc_count >= 2 {
            std::process::exit(-1);
        }
    })?;

    thread::spawn(move || {
        match capture::capture_output_stream(config, move |data| {
            println!(
                "capture time [{}]: {:.2?}. fps: {:.2}",
                data.frame_index,
                data.capture_time,
                (data.frame_index + 1) as f64 / data.elapse.as_secs_f64()
            );
            tx.send((data.frame_index, data.data)).unwrap();
        }) {
            Ok(status) => println!("{status:?}"),
            Err(e) => eprintln!("{e}"),
        }
    });

    while let Ok((index, output)) = rx.recv() {
        let temp_file = format!("/tmp/screenshot-one-{index}.png");
        let img = image::RgbaImage::from_raw(
            output.width as u32,
            output.height as u32,
            output.pixel_data,
        )
        .unwrap();
        img.save(&temp_file)?;
        println!("Screenshot saved to: {}", temp_file);
    }

    Ok(())
}
