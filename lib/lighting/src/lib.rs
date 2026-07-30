pub mod animation;
pub mod light;
pub mod physics;
pub mod render;
pub mod scene;

pub use animation::{AnimationConfig, FrameProducer, render_animation};
pub use light::{LightDirection, SpotLightConfig, SpotLightFrame, SpotLightState};
pub use physics::{PendulumConfig, PendulumState};
pub use render::{SpotLightIntensity, apply_spotlight, apply_spotlight_rgba};
pub use scene::SceneGeometry;
