use recorder::SimpleFpsCounter;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let mut fps_counter = SimpleFpsCounter::new();

    for _ in 0..100 {
        let fps = fps_counter.add_frame(Instant::now());
        println!("Current FPS: {:.2}", fps);
        thread::sleep(Duration::from_millis(20));
    }
}
