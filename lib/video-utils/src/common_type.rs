#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn is_point(&self) -> bool {
        self.width == 0 && self.height == 0
    }

    // 检查 other 是否完全包含在当前矩形内
    pub fn contains_rect(&self, other: &Rect) -> bool {
        let self_right = self.x + self.width as i32;
        let self_bottom = self.y + self.height as i32;
        let other_right = other.x + other.width as i32;
        let other_bottom = other.y + other.height as i32;

        other.x >= self.x
            && other.y >= self.y
            && other_right <= self_right
            && other_bottom <= self_bottom
    }

    // 扩展矩形以包含另一个矩形
    pub fn union(&mut self, other: &Rect) {
        if other.width == 0 || other.height == 0 {
            return;
        }

        if self.width == 0 || self.height == 0 {
            *self = *other;
            return;
        }

        let self_right = self.x + self.width as i32;
        let self_bottom = self.y + self.height as i32;
        let other_right = other.x + other.width as i32;
        let other_bottom = other.y + other.height as i32;

        let new_x = self.x.min(other.x);
        let new_y = self.y.min(other.y);
        let new_right = self_right.max(other_right);
        let new_bottom = self_bottom.max(other_bottom);

        self.x = new_x;
        self.y = new_y;
        self.width = (new_right - new_x) as u32;
        self.height = (new_bottom - new_y) as u32;
    }
}
