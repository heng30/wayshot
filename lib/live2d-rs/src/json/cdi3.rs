use serde::Deserialize;

use crate::{Error, Result};

const FORMAT: &str = "cdi3.json";
const SUPPORTED_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cdi3 {
    version: u32,
    parameters: Vec<CdiEntry>,
    parameter_groups: Vec<CdiEntry>,
    parts: Vec<CdiPart>,
    combined_parameters: Vec<Vec<String>>,
}

impl Cdi3 {
    pub fn from_json_str(source: &str) -> Result<Self> {
        let raw: RawCdi3 = serde_json::from_str(source).map_err(|error| Error::InvalidJson {
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
            parameters: raw.parameters,
            parameter_groups: raw.parameter_groups,
            parts: raw.parts,
            combined_parameters: raw.combined_parameters,
        })
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn parameters(&self) -> &[CdiEntry] {
        &self.parameters
    }

    pub fn parameter_groups(&self) -> &[CdiEntry] {
        &self.parameter_groups
    }

    pub fn parts(&self) -> &[CdiPart] {
        &self.parts
    }

    pub fn combined_parameters(&self) -> &[Vec<String>] {
        &self.combined_parameters
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RawCdi3 {
    #[serde(rename = "Version")]
    version: u32,
    #[serde(rename = "Parameters", default)]
    parameters: Vec<CdiEntry>,
    #[serde(rename = "ParameterGroups", default)]
    parameter_groups: Vec<CdiEntry>,
    #[serde(rename = "Parts", default)]
    parts: Vec<CdiPart>,
    #[serde(rename = "CombinedParameters", default)]
    combined_parameters: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CdiEntry {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "GroupId")]
    group_id: String,
    #[serde(rename = "Name")]
    name: String,
}

impl CdiEntry {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn group_id(&self) -> &str {
        &self.group_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CdiPart {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
}

impl CdiPart {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
