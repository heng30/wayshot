use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{Error, Result};

const FORMAT: &str = "model3.json";
const SUPPORTED_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model3 {
    version: u32,
    file_references: FileReferences,
    groups: Vec<Group>,
    hit_areas: Vec<HitArea>,
}

impl Model3 {
    pub fn from_json_str(source: &str) -> Result<Self> {
        let raw: RawModel3 = serde_json::from_str(source).map_err(|error| Error::InvalidJson {
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
            file_references: raw.file_references,
            groups: raw.groups,
            hit_areas: raw.hit_areas,
        })
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn moc(&self) -> &str {
        &self.file_references.moc
    }

    pub fn textures(&self) -> &[String] {
        &self.file_references.textures
    }

    pub fn physics(&self) -> Option<&str> {
        self.file_references.physics.as_deref()
    }

    pub fn pose(&self) -> Option<&str> {
        self.file_references.pose.as_deref()
    }

    pub fn display_info(&self) -> Option<&str> {
        self.file_references.display_info.as_deref()
    }

    pub fn motions(&self) -> &BTreeMap<String, Vec<MotionReference>> {
        &self.file_references.motions
    }

    pub fn expressions(&self) -> &[ExpressionReference] {
        &self.file_references.expressions
    }

    pub fn groups(&self) -> &[Group] {
        &self.groups
    }

    pub fn hit_areas(&self) -> &[HitArea] {
        &self.hit_areas
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RawModel3 {
    #[serde(rename = "Version")]
    version: u32,
    #[serde(rename = "FileReferences")]
    file_references: FileReferences,
    #[serde(rename = "Groups", default)]
    groups: Vec<Group>,
    #[serde(rename = "HitAreas", default)]
    hit_areas: Vec<HitArea>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct FileReferences {
    #[serde(rename = "Moc")]
    moc: String,
    #[serde(rename = "Textures")]
    textures: Vec<String>,
    #[serde(rename = "Physics", default)]
    physics: Option<String>,
    #[serde(rename = "Pose", default)]
    pose: Option<String>,
    #[serde(rename = "DisplayInfo", default)]
    display_info: Option<String>,
    #[serde(rename = "Motions", default)]
    motions: BTreeMap<String, Vec<MotionReference>>,
    #[serde(rename = "Expressions", default)]
    expressions: Vec<ExpressionReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MotionReference {
    #[serde(rename = "File")]
    file: String,
}

impl MotionReference {
    pub fn file(&self) -> &str {
        &self.file
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExpressionReference {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "File")]
    file: String,
}

impl ExpressionReference {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn file(&self) -> &str {
        &self.file
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Group {
    #[serde(rename = "Target")]
    target: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Ids")]
    ids: Vec<String>,
}

impl Group {
    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HitArea {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
}

impl HitArea {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
