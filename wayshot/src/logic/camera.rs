use crate::{
    config, global_store, logic_cb,
    slint_generatedAppWindow::{
        AppWindow, MixPositionWithPadding as UIMixPositionWithPadding,
        MixPositionWithPaddingTag as UIMixPositionWithPaddingTag, Resolution as UIResolution,
        SettingCamera as UISettingCamera, Source as UISource, SourceType,
    },
    store_sources, toast_warn,
};
use camera::{
    self, CameraClient, CameraConfig, CameraError, CameraResult, MixPositionWithPadding,
    PixelFormat, Rgba, Shape as CroppingSharpe, ShapeBase, ShapeCircle, ShapeRectangle,
    query_available_cameras, query_camera_id,
};
use crossbeam::channel::{Sender, bounded};
use fast_image_resize::{PixelType, ResizeAlg, Resizer, images::Image as FastImage};
use image::{RgbImage, imageops};
use once_cell::sync::Lazy;
use recorder::{CameraMixConfig, Resolution};
use slint::{ComponentHandle, Model, SharedPixelBuffer, SharedString, VecModel};
use std::{sync::Mutex, thread, time::Duration};

static CAMERA_CACHE: Lazy<Mutex<CameraCache>> = Lazy::new(|| Mutex::new(CameraCache::default()));

#[derive(Default)]
struct CameraCache {
    stop_sender: Option<Sender<()>>,
    camera_setting: UISettingCamera,
}

pub fn init(ui: &AppWindow) {
    inner_init(&ui);

    logic_cb!(camera_setting_dialog_start_playing, ui, camera);
    logic_cb!(camera_setting_dialog_stop_playing, ui);
}

pub fn inner_init(ui: &AppWindow) {
    let control = config::all().control;
    if control.enable_camera {
        store_sources!(ui).push(UISource {
            ty: SourceType::Camera,
            name: control.camera.into(),
        });
    }
}

pub fn available_cameras() -> Vec<SharedString> {
    camera::init();

    query_available_cameras()
        .into_iter()
        .map(|c| c.name.into())
        .collect::<Vec<SharedString>>()
}

fn camera_setting_dialog_start_playing(ui: &AppWindow, camera: SharedString) {
    camera::init();

    let setting = global_store!(ui).get_camera_setting_cache();
    let config = CameraConfig::default()
        .with_pixel_format(PixelFormat::RGB)
        .with_fps(setting.fps as u32);

    let config = match setting.resolution {
        UIResolution::Original => config,
        _ => {
            let resolution: Resolution = setting.resolution.into();
            let (w, h) = resolution.to_dimension();
            config.with_width(w).with_height(h)
        }
    };

    let camera_id = match query_camera_id(&camera) {
        Ok(id) => id,
        Err(e) => {
            toast_warn!(ui, format!("No found {camera} failed. {e}"));
            return;
        }
    };

    let mut client = match CameraClient::new(camera_id, config) {
        Ok(c) => c,
        Err(e) => {
            toast_warn!(ui, format!("New camera client failed. {e}"));
            return;
        }
    };

    if let Err(e) = client.start() {
        toast_warn!(ui, format!("Start camera client failed. {e}"));
        return;
    }

    let (tx, rx) = bounded(1);
    {
        let mut cache = CAMERA_CACHE.lock().unwrap();
        cache.stop_sender = Some(tx);
        cache.camera_setting = setting;
    }

    let ui_weak = ui.as_weak();

    thread::spawn(move || {
        let mut total_frames = 0;
        let mut setting = Default::default();

        loop {
            if rx.try_recv().is_ok() {
                log::info!("camera thread exit...");
                break;
            }

            if total_frames % 10 == 0 {
                setting = CAMERA_CACHE.lock().unwrap().camera_setting.clone();
            }

            let UISettingCamera {
                fps,
                zoom,
                mirror_horizontal,
                ..
            } = setting;

            // no accurate, but enough
            thread::sleep(Duration::from_millis(1000 / fps.max(24) as u64));

            match client.last_frame_rgb() {
                Ok(mut frame) if !frame.is_empty() => {
                    total_frames += 1;

                    if mirror_horizontal {
                        imageops::flip_horizontal_in_place(&mut frame);
                    }

                    if zoom != 1.0 {
                        match zoom_image(frame.clone(), zoom) {
                            Ok(img) => frame = img,
                            Err(e) => log::warn!("resize with zoom = {zoom} faield. {e}"),
                        }
                    }

                    _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                        let buffer = SharedPixelBuffer::<slint::Rgb8Pixel>::clone_from_slice(
                            &frame.as_raw(),
                            frame.width(),
                            frame.height(),
                        );
                        let img = slint::Image::from_rgb8(buffer);
                        global_store!(ui).set_camera_setting_dialog_image(img);

                        if total_frames % 10 == 0 {
                            CAMERA_CACHE.lock().unwrap().camera_setting =
                                global_store!(ui).get_camera_setting_cache();
                        }
                    });
                }
                Err(e) => log::warn!("{e}"),
                _ => (),
            }
        }
    });
}

fn camera_setting_dialog_stop_playing(_ui: &AppWindow) {
    if let Some(client) = CAMERA_CACHE.lock().unwrap().stop_sender.take()
        && let Err(e) = client.try_send(())
    {
        log::warn!("Stop camera client failed. {e}");
    }
}

fn zoom_image(frame: RgbImage, zoom: f32) -> CameraResult<RgbImage> {
    let (img_width, img_height) = frame.dimensions();
    let scaled_width = ((img_width as f32) * zoom).round() as u32;
    let scaled_height = ((img_height as f32) * zoom).round() as u32;

    let mut resized_src = vec![0u8; (scaled_width * scaled_height * 3) as usize];

    let fast_image =
        FastImage::from_vec_u8(img_width, img_height, frame.into_raw(), PixelType::U8x3)?;

    let mut resized_image = FastImage::from_slice_u8(
        scaled_width,
        scaled_height,
        &mut resized_src,
        PixelType::U8x3,
    )?;

    let resize_options = fast_image_resize::ResizeOptions::new().resize_alg(
        ResizeAlg::Convolution(fast_image_resize::FilterType::Lanczos3),
    );

    Resizer::new().resize(&fast_image, &mut resized_image, &resize_options)?;
    let resized_image = RgbImage::from_raw(scaled_width, scaled_height, resized_image.into_vec())
        .ok_or(CameraError::ImageError("to RgbImage failed".to_string()))?;

    Ok(resized_image)
}

impl From<config::Control> for CameraMixConfig {
    fn from(c: config::Control) -> CameraMixConfig {
        let config = CameraMixConfig::default()
            .with_enable(c.enable_camera)
            .with_camera_name(Some(c.camera))
            .with_fps(c.camera_setting.fps as u32)
            .with_mirror_horizontal(c.camera_setting.mirror_horizontal)
            .with_pixel_format(PixelFormat::RGB);

        let config = match c.camera_setting.resolution {
            UIResolution::Original => config,
            _ => {
                let resolution: Resolution = c.camera_setting.resolution.into();
                let (w, h) = resolution.to_dimension();
                config.with_width(w).with_height(h)
            }
        };

        let base_shape = ShapeBase::default()
            .with_zoom(c.camera_setting.zoom)
            .with_pos(c.camera_setting.pos.into())
            .with_clip_pos((c.camera_setting.cropping_x, c.camera_setting.cropping_y))
            .with_border_width(c.camera_setting.border_size as u32)
            .with_border_color(Rgba(
                parse_hex_color(&c.camera_setting.border_color).unwrap_or([255, 255, 255, 255]),
            ));

        let shape = if c.camera_setting.is_circle_shape {
            CroppingSharpe::Circle(
                ShapeCircle::default()
                    .with_base(base_shape)
                    .with_radius(c.camera_setting.circle_cropping_radius as u32),
            )
        } else {
            CroppingSharpe::Rectangle(ShapeRectangle::default().with_base(base_shape).with_size((
                c.camera_setting.rect_cropping_width as u32,
                c.camera_setting.rect_cropping_height as u32,
            )))
        };

        config.with_shape(shape)
    }
}

fn parse_hex_color(hex: &str) -> Result<[u8; 4], String> {
    let hex = hex.trim_start_matches('#');

    if hex.len() != 8 {
        return Err(format!("Invalid: expected 8 characters, got {}", hex.len()));
    }

    let parse_channel = |start: usize| -> Result<u8, String> {
        u8::from_str_radix(&hex[start..start + 2], 16)
            .map_err(|e| format!("Failed to parse hex: {e}"))
    };

    let r = parse_channel(0)?;
    let g = parse_channel(2)?;
    let b = parse_channel(4)?;
    let a = parse_channel(6)?;

    Ok([r, g, b, a])
}

impl From<UIMixPositionWithPadding> for MixPositionWithPadding {
    fn from(pos: UIMixPositionWithPadding) -> Self {
        match pos.tag {
            UIMixPositionWithPaddingTag::TopLeft => {
                MixPositionWithPadding::TopLeft((pos.padding1 as u32, pos.padding2 as u32))
            }
            UIMixPositionWithPaddingTag::TopRight => {
                MixPositionWithPadding::TopRight((pos.padding1 as u32, pos.padding2 as u32))
            }
            UIMixPositionWithPaddingTag::BottomLeft => {
                MixPositionWithPadding::BottomLeft((pos.padding1 as u32, pos.padding2 as u32))
            }
            UIMixPositionWithPaddingTag::BottomRight => {
                MixPositionWithPadding::BottomRight((pos.padding1 as u32, pos.padding2 as u32))
            }
        }
    }
}

impl serde::Serialize for UIMixPositionWithPadding {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("MixPositionWithPadding", 3)?;
        state.serialize_field("tag", &self.tag)?;
        state.serialize_field("padding1", &self.padding1)?;
        state.serialize_field("padding2", &self.padding2)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for UIMixPositionWithPadding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Field {
            Tag,
            Padding1,
            Padding2,
        }

        impl<'de> serde::Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> serde::de::Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("`tag`, `padding1`, or `padding2`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "tag" => Ok(Field::Tag),
                            "padding1" => Ok(Field::Padding1),
                            "padding2" => Ok(Field::Padding2),
                            _ => Err(E::unknown_field(value, &["tag", "padding1", "padding2"])),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct MixPositionWithPaddingVisitor;

        impl<'de> serde::de::Visitor<'de> for MixPositionWithPaddingVisitor {
            type Value = UIMixPositionWithPadding;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct MixPositionWithPadding")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut tag = None;
                let mut padding1 = None;
                let mut padding2 = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Tag => {
                            if tag.is_some() {
                                return Err(serde::de::Error::duplicate_field("tag"));
                            }
                            tag = Some(map.next_value()?);
                        }
                        Field::Padding1 => {
                            if padding1.is_some() {
                                return Err(serde::de::Error::duplicate_field("padding1"));
                            }
                            padding1 = Some(map.next_value()?);
                        }
                        Field::Padding2 => {
                            if padding2.is_some() {
                                return Err(serde::de::Error::duplicate_field("padding2"));
                            }
                            padding2 = Some(map.next_value()?);
                        }
                    }
                }

                let tag = tag.ok_or_else(|| serde::de::Error::missing_field("tag"))?;
                let padding1 = padding1.unwrap_or(0);
                let padding2 = padding2.unwrap_or(0);

                Ok(UIMixPositionWithPadding {
                    tag,
                    padding1,
                    padding2,
                })
            }
        }

        deserializer.deserialize_struct(
            "MixPositionWithPadding",
            &["tag", "padding1", "padding2"],
            MixPositionWithPaddingVisitor,
        )
    }
}
