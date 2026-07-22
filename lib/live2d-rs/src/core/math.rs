use std::collections::BTreeMap;

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Vector2 {
    x: f32,
    y: f32,
}

impl Vector2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn x(&self) -> f32 {
        self.x
    }

    pub fn y(&self) -> f32 {
        self.y
    }

    pub fn lerp(self, to: Self, weight: f32) -> Self {
        Self::new(
            self.x + (to.x - self.x) * weight,
            self.y + (to.y - self.y) * weight,
        )
    }

    pub fn affine2(origin: Self, u: Self, u_weight: f32, v: Self, v_weight: f32) -> Self {
        Self::new(
            origin.x + (u.x - origin.x) * u_weight + (v.x - origin.x) * v_weight,
            origin.y + (u.y - origin.y) * u_weight + (v.y - origin.y) * v_weight,
        )
    }
}

pub fn degrees_to_radian(degrees: f32) -> f32 {
    (degrees / 180.0) * std::f32::consts::PI
}

pub fn radian_to_degrees(radian: f32) -> f32 {
    (radian * 180.0) / std::f32::consts::PI
}

pub fn direction_to_radian(from: Vector2, to: Vector2) -> f32 {
    let mut result = to.y.atan2(to.x) - from.y.atan2(from.x);

    while result < -std::f32::consts::PI {
        result += std::f32::consts::PI * 2.0;
    }

    while result > std::f32::consts::PI {
        result -= std::f32::consts::PI * 2.0;
    }

    result
}

pub fn radian_to_direction(radian: f32) -> Vector2 {
    Vector2::new(radian.sin(), radian.cos())
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Matrix44 {
    values: [f32; 16],
}

impl Matrix44 {
    pub fn identity() -> Self {
        Self {
            values: [
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    pub fn as_slice(&self) -> &[f32; 16] {
        &self.values
    }

    pub fn multiply(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
        let mut result = [0.0; 16];

        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    result[j + i * 4] += a[k + i * 4] * b[j + k * 4];
                }
            }
        }

        result
    }

    pub fn translate_relative(&mut self, x: f32, y: f32) {
        let translation = [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            x, y, 0.0, 1.0,
        ];
        self.values = Self::multiply(&translation, &self.values);
    }

    pub fn translate(&mut self, x: f32, y: f32) {
        self.values[12] = x;
        self.values[13] = y;
    }

    pub fn translate_x(&mut self, x: f32) {
        self.values[12] = x;
    }

    pub fn translate_y(&mut self, y: f32) {
        self.values[13] = y;
    }

    pub fn scale_relative(&mut self, x: f32, y: f32) {
        let scale = [
            x, 0.0, 0.0, 0.0, //
            0.0, y, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ];
        self.values = Self::multiply(&scale, &self.values);
    }

    pub fn scale(&mut self, x: f32, y: f32) {
        self.values[0] = x;
        self.values[5] = y;
    }

    pub fn scale_x(&self) -> f32 {
        self.values[0]
    }

    pub fn scale_y(&self) -> f32 {
        self.values[5]
    }

    pub fn transform_x(&self, value: f32) -> f32 {
        self.values[0] * value + self.values[12]
    }

    pub fn transform_y(&self, value: f32) -> f32 {
        self.values[5] * value + self.values[13]
    }

    pub fn invert_transform_x(&self, value: f32) -> f32 {
        (value - self.values[12]) / self.values[0]
    }

    pub fn invert_transform_y(&self, value: f32) -> f32 {
        (value - self.values[13]) / self.values[5]
    }
}

impl Default for Matrix44 {
    fn default() -> Self {
        Self::identity()
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ModelMatrix {
    width: f32,
    height: f32,
    matrix: Matrix44,
}

impl ModelMatrix {
    pub fn new(width: f32, height: f32) -> Self {
        let mut matrix = Self {
            width,
            height,
            matrix: Matrix44::identity(),
        };
        matrix.set_height(2.0);
        matrix
    }

    pub fn setup_from_layout(&mut self, layout: &BTreeMap<String, f32>) {
        for (key, value) in layout {
            match key.as_str() {
                "width" => self.set_width(*value),
                "height" => self.set_height(*value),
                _ => {}
            }
        }

        for (key, value) in layout {
            match key.as_str() {
                "x" => self.set_x(*value),
                "y" => self.set_y(*value),
                "center_x" => self.center_x(*value),
                "center_y" => self.center_y(*value),
                "top" => self.top(*value),
                "bottom" => self.bottom(*value),
                "left" => self.left(*value),
                "right" => self.right(*value),
                _ => {}
            }
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.matrix.translate(x, y);
    }

    pub fn set_center_position(&mut self, x: f32, y: f32) {
        self.center_x(x);
        self.center_y(y);
    }

    pub fn top(&mut self, y: f32) {
        self.set_y(y);
    }

    pub fn bottom(&mut self, y: f32) {
        let height = self.height * self.matrix.scale_y();
        self.matrix.translate_y(y - height);
    }

    pub fn left(&mut self, x: f32) {
        self.set_x(x);
    }

    pub fn right(&mut self, x: f32) {
        let width = self.width * self.matrix.scale_x();
        self.matrix.translate_x(x - width);
    }

    pub fn center_x(&mut self, x: f32) {
        let width = self.width * self.matrix.scale_x();
        self.matrix.translate_x(x - width / 2.0);
    }

    pub fn center_y(&mut self, y: f32) {
        let height = self.height * self.matrix.scale_y();
        self.matrix.translate_y(y - height / 2.0);
    }

    pub fn set_x(&mut self, x: f32) {
        self.matrix.translate_x(x);
    }

    pub fn set_y(&mut self, y: f32) {
        self.matrix.translate_y(y);
    }

    pub fn set_width(&mut self, width: f32) {
        let scale = width / self.width;
        self.matrix.scale(scale, scale);
    }

    pub fn set_height(&mut self, height: f32) {
        let scale = height / self.height;
        self.matrix.scale(scale, scale);
    }

    pub fn transform_x(&self, value: f32) -> f32 {
        self.matrix.transform_x(value)
    }

    pub fn transform_y(&self, value: f32) -> f32 {
        self.matrix.transform_y(value)
    }

    pub fn matrix(&self) -> &Matrix44 {
        &self.matrix
    }
}
