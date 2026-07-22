use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, derivative::Derivative)]
#[derivative(Default)]
pub struct Theme {
    #[derivative(Default(value = "\"dark\".to_string()"))]
    pub name: String,
    pub colors: ColorPalette,
}

impl Theme {
    pub fn new(name: &str, colors: ColorPalette) -> Self {
        Self {
            name: name.to_string(),
            colors,
        }
    }

    pub fn dark() -> Self {
        Self {
            name: "dark".to_string(),
            colors: ColorPalette::default(),
        }
    }

    pub fn light() -> Self {
        Self {
            name: "light".to_string(),
            colors: ColorPalette::light(),
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "light" => Self::light(),
            _ => Self::dark(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColorPalette {
    pub background: [u8; 3],
    pub foreground: [u8; 3],
    pub cursor: [u8; 3],
    pub selection: [u8; 3],
    #[serde(default)]
    pub ansi: [[u8; 3]; 16], // 16 ANSI colors (0-7 normal, 8-15 bright)
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self::dark()
    }
}

impl ColorPalette {
    pub fn dark() -> Self {
        Self {
            background: [30, 30, 30],
            foreground: [230, 230, 230],
            cursor: [255, 255, 255],
            selection: [160, 190, 230],
            ansi: [
                // Normal colors (0-7)
                [1, 1, 1],       // Black
                [222, 56, 43],   // Red
                [57, 181, 74],   // Green
                [255, 199, 6],   // Yellow
                [0, 111, 184],   // Blue
                [118, 38, 113],  // Magenta
                [44, 181, 233],  // Cyan
                [255, 255, 255], // White
                // Bright colors (8-15)
                [128, 128, 128], // Bright Black (Gray)
                [255, 0, 0],     // Bright Red
                [0, 255, 0],     // Bright Green
                [255, 255, 0],   // Bright Yellow
                [0, 0, 255],     // Bright Blue
                [255, 0, 255],   // Bright Magenta
                [0, 255, 255],   // Bright Cyan
                [255, 255, 255], // Bright White
            ],
        }
    }

    pub fn light() -> Self {
        Self {
            background: [250, 250, 250],
            foreground: [40, 40, 40],
            cursor: [40, 40, 40],
            selection: [160, 190, 230],
            ansi: [
                // Normal colors (0-7)
                [40, 40, 40],    // Black
                [204, 36, 29],   // Red
                [40, 140, 50],   // Green
                [180, 140, 0],   // Yellow
                [30, 80, 160],   // Blue
                [130, 40, 110],  // Magenta
                [20, 130, 160],  // Cyan
                [220, 220, 220], // White
                // Bright colors (8-15)
                [120, 120, 120], // Bright Black (Gray)
                [230, 60, 50],   // Bright Red
                [50, 170, 60],   // Bright Green
                [220, 180, 30],  // Bright Yellow
                [50, 110, 200],  // Bright Blue
                [160, 60, 140],  // Bright Magenta
                [40, 170, 200],  // Bright Cyan
                [250, 250, 250], // Bright White
            ],
        }
    }
}
