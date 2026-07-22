use serde::Deserialize;

use crate::{Error, Result};

const FORMAT: &str = "physics3.json";
const SUPPORTED_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct Physics3 {
    version: u32,
    meta: PhysicsMeta,
    settings: Vec<PhysicsSetting>,
}

impl Physics3 {
    pub fn from_json_str(source: &str) -> Result<Self> {
        let raw: RawPhysics3 =
            serde_json::from_str(source).map_err(|error| Error::InvalidJson {
                format: FORMAT,
                message: error.to_string(),
            })?;

        if raw.version != SUPPORTED_VERSION {
            return Err(Error::UnsupportedVersion {
                format: FORMAT,
                version: raw.version,
            });
        }

        Ok(Self {
            version: raw.version,
            meta: raw.meta,
            settings: raw.settings,
        })
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn meta(&self) -> &PhysicsMeta {
        &self.meta
    }

    pub fn settings(&self) -> &[PhysicsSetting] {
        &self.settings
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct RawPhysics3 {
    #[serde(rename = "Version")]
    version: u32,
    #[serde(rename = "Meta")]
    meta: PhysicsMeta,
    #[serde(rename = "PhysicsSettings", default)]
    settings: Vec<PhysicsSetting>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PhysicsMeta {
    #[serde(rename = "PhysicsSettingCount")]
    physics_setting_count: u32,
    #[serde(rename = "TotalInputCount")]
    total_input_count: u32,
    #[serde(rename = "TotalOutputCount")]
    total_output_count: u32,
    #[serde(rename = "VertexCount")]
    vertex_count: u32,
    #[serde(rename = "EffectiveForces")]
    effective_forces: EffectiveForces,
    #[serde(rename = "PhysicsDictionary", default)]
    physics_dictionary: Vec<PhysicsDictionaryEntry>,
}

impl PhysicsMeta {
    pub fn physics_setting_count(&self) -> u32 {
        self.physics_setting_count
    }

    pub fn total_input_count(&self) -> u32 {
        self.total_input_count
    }

    pub fn total_output_count(&self) -> u32 {
        self.total_output_count
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    pub fn effective_forces(&self) -> &EffectiveForces {
        &self.effective_forces
    }

    pub fn physics_dictionary(&self) -> &[PhysicsDictionaryEntry] {
        &self.physics_dictionary
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EffectiveForces {
    #[serde(rename = "Gravity")]
    gravity: Vector2,
    #[serde(rename = "Wind")]
    wind: Vector2,
}

impl EffectiveForces {
    pub fn gravity(&self) -> &Vector2 {
        &self.gravity
    }

    pub fn wind(&self) -> &Vector2 {
        &self.wind
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PhysicsDictionaryEntry {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
}

impl PhysicsDictionaryEntry {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PhysicsSetting {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Input", default)]
    inputs: Vec<PhysicsInput>,
    #[serde(rename = "Output", default)]
    outputs: Vec<PhysicsOutput>,
    #[serde(rename = "Vertices", default)]
    vertices: Vec<PhysicsVertex>,
    #[serde(rename = "Normalization")]
    normalization: PhysicsNormalization,
}

impl PhysicsSetting {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn inputs(&self) -> &[PhysicsInput] {
        &self.inputs
    }

    pub fn outputs(&self) -> &[PhysicsOutput] {
        &self.outputs
    }

    pub fn vertices(&self) -> &[PhysicsVertex] {
        &self.vertices
    }

    pub fn normalization(&self) -> &PhysicsNormalization {
        &self.normalization
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PhysicsInput {
    #[serde(rename = "Source")]
    source: PhysicsSource,
    #[serde(rename = "Weight")]
    weight: f32,
    #[serde(rename = "Type")]
    kind: PhysicsValueKind,
    #[serde(rename = "Reflect")]
    reflect: bool,
}

impl PhysicsInput {
    pub fn source(&self) -> &PhysicsSource {
        &self.source
    }

    pub fn weight(&self) -> f32 {
        self.weight
    }

    pub fn kind(&self) -> PhysicsValueKind {
        self.kind
    }

    pub fn reflect(&self) -> bool {
        self.reflect
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PhysicsOutput {
    #[serde(rename = "Destination")]
    destination: PhysicsSource,
    #[serde(rename = "VertexIndex")]
    vertex_index: u32,
    #[serde(rename = "Scale")]
    scale: f32,
    #[serde(rename = "Weight")]
    weight: f32,
    #[serde(rename = "Type")]
    kind: PhysicsValueKind,
    #[serde(rename = "Reflect")]
    reflect: bool,
}

impl PhysicsOutput {
    pub fn destination(&self) -> &PhysicsSource {
        &self.destination
    }

    pub fn vertex_index(&self) -> u32 {
        self.vertex_index
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn weight(&self) -> f32 {
        self.weight
    }

    pub fn kind(&self) -> PhysicsValueKind {
        self.kind
    }

    pub fn reflect(&self) -> bool {
        self.reflect
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PhysicsSource {
    #[serde(rename = "Target")]
    target: String,
    #[serde(rename = "Id")]
    id: String,
}

impl PhysicsSource {
    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub enum PhysicsValueKind {
    #[serde(rename = "X")]
    X,
    #[serde(rename = "Y")]
    Y,
    #[serde(rename = "Angle")]
    Angle,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PhysicsVertex {
    #[serde(rename = "Mobility")]
    mobility: f32,
    #[serde(rename = "Delay")]
    delay: f32,
    #[serde(rename = "Acceleration")]
    acceleration: f32,
    #[serde(rename = "Radius")]
    radius: f32,
    #[serde(rename = "Position")]
    position: Vector2,
}

impl PhysicsVertex {
    pub fn mobility(&self) -> f32 {
        self.mobility
    }

    pub fn delay(&self) -> f32 {
        self.delay
    }

    pub fn acceleration(&self) -> f32 {
        self.acceleration
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn position(&self) -> &Vector2 {
        &self.position
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PhysicsNormalization {
    #[serde(rename = "Position")]
    position: PhysicsNormalizationValue,
    #[serde(rename = "Angle")]
    angle: PhysicsNormalizationValue,
}

impl PhysicsNormalization {
    pub fn position(&self) -> &PhysicsNormalizationValue {
        &self.position
    }

    pub fn angle(&self) -> &PhysicsNormalizationValue {
        &self.angle
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PhysicsNormalizationValue {
    #[serde(rename = "Minimum")]
    minimum: f32,
    #[serde(rename = "Default")]
    default: f32,
    #[serde(rename = "Maximum")]
    maximum: f32,
}

impl PhysicsNormalizationValue {
    pub fn minimum(&self) -> f32 {
        self.minimum
    }

    pub fn default(&self) -> f32 {
        self.default
    }

    pub fn maximum(&self) -> f32 {
        self.maximum
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Vector2 {
    #[serde(rename = "X")]
    x: f32,
    #[serde(rename = "Y")]
    y: f32,
}

impl Vector2 {
    pub fn x(&self) -> f32 {
        self.x
    }

    pub fn y(&self) -> f32 {
        self.y
    }
}
