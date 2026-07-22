use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyframeValue {
    Float(f32),
    Float2(f32, f32), // For position, scale, etc.
    Color(u8, u8, u8, u8),
    Bool(bool),
}

impl KeyframeValue {
    pub fn as_float(&self) -> Option<f32> {
        match self {
            KeyframeValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_float2(&self) -> Option<(f32, f32)> {
        match self {
            KeyframeValue::Float2(x, y) => Some((*x, *y)),
            _ => None,
        }
    }

    pub fn as_color(&self) -> Option<(u8, u8, u8, u8)> {
        match self {
            KeyframeValue::Color(r, g, b, a) => Some((*r, *g, *b, *a)),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            KeyframeValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn float_default(value: f32) -> Self {
        KeyframeValue::Float(value)
    }
}

impl Default for KeyframeValue {
    fn default() -> Self {
        KeyframeValue::Float(0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    pub time_ms: i64,
    pub value: KeyframeValue,
}

impl Keyframe {
    pub fn new(time_ms: i64, value: KeyframeValue) -> Self {
        Self { time_ms, value }
    }

    pub fn float(time_ms: i64, value: f32) -> Self {
        Self::new(time_ms, KeyframeValue::Float(value))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimatableProperty {
    // Property name (e.g., "zoom_level")
    pub name: String,
    // Display name for UI (e.g., "Zoom")
    pub display_name: String,
    // Minimum value for float properties
    pub min_value: f32,
    // Maximum value for float properties
    pub max_value: f32,
    // Default value
    pub default_value: KeyframeValue,
}

impl AnimatableProperty {
    pub fn float(
        name: impl Into<String>,
        display_name: impl Into<String>,
        min_value: f32,
        max_value: f32,
        default: f32,
    ) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
            min_value,
            max_value,
            default_value: KeyframeValue::Float(default),
        }
    }

    pub fn float2(
        name: impl Into<String>,
        display_name: impl Into<String>,
        min_value: f32,
        max_value: f32,
        default_x: f32,
        default_y: f32,
    ) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
            min_value,
            max_value,
            default_value: KeyframeValue::Float2(default_x, default_y),
        }
    }

    pub fn color(
        name: impl Into<String>,
        display_name: impl Into<String>,
        default_r: u8,
        default_g: u8,
        default_b: u8,
        default_a: u8,
    ) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
            min_value: 0.0,
            max_value: 255.0,
            default_value: KeyframeValue::Color(default_r, default_g, default_b, default_a),
        }
    }
}

// Property keyframe track (all keyframes for a single property)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyTrack {
    // Property name this track belongs to
    pub property_name: String,
    // Sorted list of keyframes (by time_ms)
    pub keyframes: Vec<Keyframe>,
}

impl Default for PropertyTrack {
    fn default() -> Self {
        Self::new("")
    }
}

impl PropertyTrack {
    pub fn new(property_name: impl Into<String>) -> Self {
        Self {
            property_name: property_name.into(),
            keyframes: Vec::new(),
        }
    }

    pub fn with_keyframes(property_name: impl Into<String>, keyframes: Vec<Keyframe>) -> Self {
        let mut track = Self::new(property_name);
        track.keyframes = keyframes;
        track.sort_keyframes();
        track
    }

    pub fn add_keyframe(&mut self, keyframe: Keyframe) {
        // Remove existing keyframe at same time if present
        self.keyframes.retain(|k| k.time_ms != keyframe.time_ms);
        self.keyframes.push(keyframe);
        self.sort_keyframes();
    }

    pub fn remove_keyframe(&mut self, time_ms: i64) -> bool {
        let initial_len = self.keyframes.len();
        self.keyframes.retain(|k| k.time_ms != time_ms);
        self.keyframes.len() != initial_len
    }

    pub fn move_keyframe(&mut self, old_time_ms: i64, new_time_ms: i64) -> bool {
        if let Some(keyframe) = self.keyframes.iter_mut().find(|k| k.time_ms == old_time_ms) {
            keyframe.time_ms = new_time_ms;
            self.sort_keyframes();
            true
        } else {
            false
        }
    }

    pub fn update_keyframe_value(&mut self, time_ms: i64, value: KeyframeValue) -> bool {
        if let Some(keyframe) = self.keyframes.iter_mut().find(|k| k.time_ms == time_ms) {
            keyframe.value = value;
            true
        } else {
            false
        }
    }

    fn sort_keyframes(&mut self) {
        self.keyframes.sort_by_key(|k| k.time_ms);
    }

    pub fn len(&self) -> usize {
        self.keyframes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keyframes.is_empty()
    }

    pub fn has_keyframes(&self) -> bool {
        !self.keyframes.is_empty()
    }
}

// Collection of property tracks for a filter
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeyframeTracks {
    pub tracks: Vec<PropertyTrack>,
}

impl KeyframeTracks {
    pub fn new() -> Self {
        Self { tracks: Vec::new() }
    }

    pub fn has_keyframe_at(&self, property_name: &str, time_ms: i64) -> bool {
        self.get_track(property_name)
            .map(|track| track.keyframes.iter().any(|k| k.time_ms == time_ms))
            .unwrap_or(false)
    }

    pub fn get_or_create_track(&mut self, property_name: &str) -> &mut PropertyTrack {
        if !self.tracks.iter().any(|t| t.property_name == property_name) {
            self.tracks.push(PropertyTrack::new(property_name));
        }
        self.tracks
            .iter_mut()
            .find(|t| t.property_name == property_name)
            .unwrap()
    }

    pub fn get_track(&self, property_name: &str) -> Option<&PropertyTrack> {
        self.tracks
            .iter()
            .find(|t| t.property_name == property_name)
    }

    pub fn get_track_mut(&mut self, property_name: &str) -> Option<&mut PropertyTrack> {
        self.tracks
            .iter_mut()
            .find(|t| t.property_name == property_name)
    }

    pub fn add_keyframe(&mut self, property_name: &str, keyframe: Keyframe) {
        self.get_or_create_track(property_name)
            .add_keyframe(keyframe);
    }

    pub fn remove_keyframe(&mut self, property_name: &str, time_ms: i64) -> bool {
        if let Some(track) = self.get_track_mut(property_name) {
            track.remove_keyframe(time_ms)
        } else {
            false
        }
    }

    pub fn move_keyframe(
        &mut self,
        property_name: &str,
        old_time_ms: i64,
        new_time_ms: i64,
    ) -> bool {
        if let Some(track) = self.get_track_mut(property_name) {
            track.move_keyframe(old_time_ms, new_time_ms)
        } else {
            false
        }
    }

    pub fn update_keyframe_value(
        &mut self,
        property_name: &str,
        time_ms: i64,
        value: KeyframeValue,
    ) -> bool {
        if let Some(track) = self.get_track_mut(property_name) {
            track.update_keyframe_value(time_ms, value)
        } else {
            false
        }
    }

    pub fn has_keyframes(&self) -> bool {
        self.tracks.iter().any(|t| t.has_keyframes())
    }

    pub fn is_empty(&self) -> bool {
        !self.has_keyframes()
    }

    pub fn active_track_count(&self) -> usize {
        self.tracks.iter().filter(|t| t.has_keyframes()).count()
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
    }
}
