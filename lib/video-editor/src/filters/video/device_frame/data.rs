use serde::Deserialize;
use std::sync::OnceLock;

/// Screen rectangle defining where the video content sits inside a device frame image.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ScreenRect {
    pub top: u32,
    pub left: u32,
    pub bottom: u32,
    pub right: u32,
    pub width: u32,
    pub height: u32,
}

/// A device frame definition, parsed from the embedded frames.json subset.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct DeviceFrameDef {
    pub name: String,
    pub category: String,
    pub device: String,
    pub frame: ScreenRect,
    pub pixel_ratio: Option<f32>,
    #[serde(default)]
    pub shadow: bool,
}

// Embedded subset of frames.json for all supported devices (20 devices).
const FRAME_DATA: &str = r#"[
  {
    "name": "Apple iPhone X Black",
    "category": "Phones",
    "device": "Apple iPhone X",
    "frame": { "top": 39, "left": 46, "bottom": 1472, "right": 708, "width": 662, "height": 1433 },
    "pixel_ratio": 3.0,
    "shadow": false
  },
  {
    "name": "Apple iPhone 7 Jet Black",
    "category": "Phones",
    "device": "Apple iPhone 7",
    "frame": { "top": 228, "left": 61, "bottom": 1562, "right": 811, "width": 750, "height": 1334 },
    "pixel_ratio": 2.0,
    "shadow": false
  },
  {
    "name": "Google Pixel Very Silver",
    "category": "Phones",
    "device": "Google Pixel",
    "frame": { "top": 252, "left": 62, "bottom": 2172, "right": 1142, "width": 1080, "height": 1920 },
    "pixel_ratio": 2.6,
    "shadow": false
  },
  {
    "name": "Samsung Galaxy S5 Black",
    "category": "Phones",
    "device": "Samsung Galaxy S5",
    "frame": { "top": 247, "left": 79, "bottom": 2167, "right": 1159, "width": 1080, "height": 1920 },
    "pixel_ratio": 3.0,
    "shadow": false
  },
  {
    "name": "Nexus 5x",
    "category": "Phones",
    "device": "Nexus 5X",
    "frame": { "top": 231, "left": 53, "bottom": 2151, "right": 1133, "width": 1080, "height": 1920 },
    "pixel_ratio": 2.6,
    "shadow": false
  },
  {
    "name": "Apple iPad Air 2 Silver",
    "category": "Tablets",
    "device": "Apple iPad Air 2",
    "frame": { "top": 224, "left": 112, "bottom": 2272, "right": 1648, "width": 1536, "height": 2048 },
    "pixel_ratio": 2.0,
    "shadow": false
  },
  {
    "name": "Apple iPad Pro Silver",
    "category": "Tablets",
    "device": "Apple iPad Pro",
    "frame": { "top": 216, "left": 119, "bottom": 2948, "right": 2167, "width": 2048, "height": 2732 },
    "pixel_ratio": 2.0,
    "shadow": false
  },
  {
    "name": "Apple iPad Mini 4 Silver",
    "category": "Tablets",
    "device": "Apple iPad Mini 4",
    "frame": { "top": 278, "left": 96, "bottom": 2326, "right": 1632, "width": 1536, "height": 2048 },
    "pixel_ratio": 2.0,
    "shadow": false
  },
  {
    "name": "Microsoft Surface Pro 4",
    "category": "Tablets",
    "device": "Microsoft Surface Pro 4",
    "frame": { "top": 148, "left": 164, "bottom": 1972, "right": 2900, "width": 2736, "height": 1824 },
    "pixel_ratio": 2.0,
    "shadow": false
  },
  {
    "name": "Nexus 9",
    "category": "Tablets",
    "device": "Nexus 9",
    "frame": { "top": 273, "left": 96, "bottom": 2321, "right": 1632, "width": 1536, "height": 2048 },
    "pixel_ratio": 2.0,
    "shadow": false
  },
  {
    "name": "Apple-Macbook-Space-Grey",
    "category": "Computers",
    "device": "Apple Macbook",
    "frame": { "top": 128, "left": 380, "bottom": 1568, "right": 2684, "width": 2304, "height": 1440 },
    "pixel_ratio": 2.0,
    "shadow": false
  },
  {
    "name": "Dell XPS 13\"",
    "category": "Computers",
    "device": "Dell XPS 13",
    "frame": { "top": 62, "left": 317, "bottom": 1862, "right": 3517, "width": 3200, "height": 1800 },
    "pixel_ratio": null,
    "shadow": false
  },
  {
    "name": "Microsoft Surface Book",
    "category": "Computers",
    "device": "Microsoft Surface Book",
    "frame": { "top": 159, "left": 549, "bottom": 2159, "right": 3549, "width": 3000, "height": 2000 },
    "pixel_ratio": 2.0,
    "shadow": false
  },
  {
    "name": "Apple Macbook Air 13\"",
    "category": "Computers",
    "device": "Apple Macbook Air",
    "frame": { "top": 80, "left": 262, "bottom": 980, "right": 1702, "width": 1440, "height": 900 },
    "pixel_ratio": 1.0,
    "shadow": false
  },
  {
    "name": "Apple iMac",
    "category": "Computers",
    "device": "Apple iMac",
    "frame": { "top": 122, "left": 114, "bottom": 1562, "right": 2674, "width": 2560, "height": 1440 },
    "pixel_ratio": 1.0,
    "shadow": false
  },
  {
    "name": "Apple Thunderbolt Display",
    "category": "Displays",
    "device": "Apple Thunderbolt Display",
    "frame": { "top": 114, "left": 114, "bottom": 1554, "right": 2674, "width": 2560, "height": 1440 },
    "pixel_ratio": null,
    "shadow": false
  },
  {
    "name": "Dell UltraSharp 27\"",
    "category": "Displays",
    "device": "Dell UltraSharp Monitor",
    "frame": { "top": 33, "left": 33, "bottom": 1473, "right": 2593, "width": 2560, "height": 1440 },
    "pixel_ratio": null,
    "shadow": false
  },
  {
    "name": "Sony W850C",
    "category": "Displays",
    "device": "Sony W850C",
    "frame": { "top": 14, "left": 12, "bottom": 735, "right": 1292, "width": 1280, "height": 721 },
    "pixel_ratio": null,
    "shadow": false
  },
  {
    "name": "Dell UltraSharp 24\"",
    "category": "Displays",
    "device": "Dell UltraSharp Monitor",
    "frame": { "top": 27, "left": 27, "bottom": 1227, "right": 1947, "width": 1920, "height": 1200 },
    "pixel_ratio": null,
    "shadow": false
  }
]"#;

pub fn all_frames() -> &'static [DeviceFrameDef] {
    static CACHE: OnceLock<Vec<DeviceFrameDef>> = OnceLock::new();
    CACHE.get_or_init(|| serde_json::from_str(FRAME_DATA).expect("embedded frames.json is valid"))
}

pub fn find_frame(name: &str) -> Option<&'static DeviceFrameDef> {
    all_frames().iter().find(|f| f.name == name)
}

#[allow(dead_code)]
pub fn available_device_names() -> Vec<&'static str> {
    all_frames().iter().map(|f| f.name.as_str()).collect()
}
