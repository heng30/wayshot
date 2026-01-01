use camera::camera_info::{query_available_cameras, query_camera_id};

fn main() {
    let cameras = query_available_cameras();
    println!("Found {} working cameras", cameras.len());

    for cam in &cameras {
        println!("  - {:?}", cam);
    }

    if !cameras.is_empty() {
        let id = query_camera_id(&cameras[0].name);
        println!("{} -> {:?}", cameras[0].name, id);
    }
}
