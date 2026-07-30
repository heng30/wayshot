use crate::{light::SpotLightFrame, scene::SceneGeometry};
use glam::Vec3;
use image::{Rgb, RgbImage, Rgba, RgbaImage};

pub struct SpotLightIntensity {
    pub factor: f32,
    pub color: [f32; 3],
}

pub fn compute_spotlight_intensity(
    light: &SpotLightFrame,
    world_point: Vec3,
) -> SpotLightIntensity {
    let to_point = world_point - light.position;
    let distance = to_point.length();

    if distance > light.max_distance || distance < 1e-6 {
        return SpotLightIntensity {
            factor: 0.0,
            color: light.color,
        };
    }

    let dist_atten = 1.0 / distance.powf(light.decay);

    let cos_angle = light.direction.dot(to_point.normalize());
    let cos_angle = cos_angle.clamp(-1.0, 1.0);
    let angle = cos_angle.acos();

    let outer_angle = light.angle_rad;
    let inner_angle = outer_angle * (1.0 - light.penumbra);

    let cone_factor = if angle <= inner_angle {
        1.0
    } else if angle >= outer_angle {
        0.0
    } else {
        let t = (angle - inner_angle) / (outer_angle - inner_angle);
        1.0 - t * t * (3.0 - 2.0 * t)
    };

    let brightness_factor = light.brightness / 1450.0;
    let factor = (cone_factor * dist_atten * brightness_factor).min(1.0);

    SpotLightIntensity {
        factor,
        color: light.color,
    }
}

pub fn apply_spotlight(
    image: &RgbImage,
    light: &SpotLightFrame,
    scene: &SceneGeometry,
    ambient: f32,
) -> RgbImage {
    let (width, height) = image.dimensions();
    let mut output = image.clone();

    for y in 0..height {
        for x in 0..width {
            let world_pos = scene.pixel_to_world(x, y, width, height);
            let intensity = compute_spotlight_intensity(light, world_pos);

            let pixel = image.get_pixel(x, y);
            let r = pixel.0[0] as f32 / 255.0;
            let g = pixel.0[1] as f32 / 255.0;
            let b = pixel.0[2] as f32 / 255.0;

            let lit_r = (r * ambient + intensity.color[0] * intensity.factor).min(1.0);
            let lit_g = (g * ambient + intensity.color[1] * intensity.factor).min(1.0);
            let lit_b = (b * ambient + intensity.color[2] * intensity.factor).min(1.0);

            output.put_pixel(
                x,
                y,
                Rgb([
                    (lit_r * 255.0) as u8,
                    (lit_g * 255.0) as u8,
                    (lit_b * 255.0) as u8,
                ]),
            );
        }
    }

    output
}

/// Apply spotlight effect to an RGBA image, preserving the alpha channel.
/// The lighting calculation only affects RGB channels; alpha is passed through unchanged.
pub fn apply_spotlight_rgba(
    image: &RgbaImage,
    light: &SpotLightFrame,
    scene: &SceneGeometry,
    ambient: f32,
) -> RgbaImage {
    let (width, height) = image.dimensions();
    let mut output = image.clone();

    for y in 0..height {
        for x in 0..width {
            let world_pos = scene.pixel_to_world(x, y, width, height);
            let intensity = compute_spotlight_intensity(light, world_pos);

            let pixel = image.get_pixel(x, y);
            let r = pixel.0[0] as f32 / 255.0;
            let g = pixel.0[1] as f32 / 255.0;
            let b = pixel.0[2] as f32 / 255.0;
            let a = pixel.0[3];

            let lit_r = (r * ambient + intensity.color[0] * intensity.factor).min(1.0);
            let lit_g = (g * ambient + intensity.color[1] * intensity.factor).min(1.0);
            let lit_b = (b * ambient + intensity.color[2] * intensity.factor).min(1.0);

            output.put_pixel(
                x,
                y,
                Rgba([
                    (lit_r * 255.0) as u8,
                    (lit_g * 255.0) as u8,
                    (lit_b * 255.0) as u8,
                    a,
                ]),
            );
        }
    }

    output
}
