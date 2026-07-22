use serde::{Deserialize, Deserializer};

use crate::{Error, Result};

const FORMAT: &str = "exp3.json";
pub const DEFAULT_EXPRESSION_FADE_IN_TIME: f32 = 1.0;
pub const DEFAULT_EXPRESSION_FADE_OUT_TIME: f32 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Expression3 {
    kind: String,
    fade_in_time: Option<f32>,
    fade_out_time: Option<f32>,
    parameters: Vec<ExpressionParameter>,
}

impl Expression3 {
    pub fn from_json_str(source: &str) -> Result<Self> {
        let raw: RawExpression3 =
            serde_json::from_str(source).map_err(|error| Error::InvalidJson {
                format: FORMAT,
                message: error.to_string(),
            })?;

        Ok(Self {
            kind: raw.kind,
            fade_in_time: raw.fade_in_time,
            fade_out_time: raw.fade_out_time,
            parameters: raw.parameters,
        })
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn fade_in_time(&self) -> Option<f32> {
        self.fade_in_time
    }

    pub fn resolved_fade_in_time(&self) -> f32 {
        resolved_expression_fade_in_time(self.fade_in_time)
    }

    pub fn fade_out_time(&self) -> Option<f32> {
        self.fade_out_time
    }

    pub fn resolved_fade_out_time(&self) -> f32 {
        resolved_expression_fade_out_time(self.fade_out_time)
    }

    pub fn parameters(&self) -> &[ExpressionParameter] {
        &self.parameters
    }
}

pub fn resolved_expression_fade_in_time(fade_in_time: Option<f32>) -> f32 {
    fade_in_time
        .unwrap_or(DEFAULT_EXPRESSION_FADE_IN_TIME)
        .max(0.0)
}

pub fn resolved_expression_fade_out_time(fade_out_time: Option<f32>) -> f32 {
    fade_out_time
        .unwrap_or(DEFAULT_EXPRESSION_FADE_OUT_TIME)
        .max(0.0)
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct RawExpression3 {
    #[serde(rename = "Type")]
    kind: String,
    #[serde(rename = "FadeInTime", default)]
    fade_in_time: Option<f32>,
    #[serde(rename = "FadeOutTime", default)]
    fade_out_time: Option<f32>,
    #[serde(rename = "Parameters", default)]
    parameters: Vec<ExpressionParameter>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ExpressionParameter {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Value")]
    value: f32,
    #[serde(
        rename = "Blend",
        default,
        deserialize_with = "deserialize_expression_blend"
    )]
    blend: ExpressionBlend,
}

impl ExpressionParameter {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn blend(&self) -> ExpressionBlend {
        self.blend
    }
}

pub fn apply_expression_parameter(
    current: f32,
    parameter: &ExpressionParameter,
    weight: f32,
) -> f32 {
    apply_expression_blend(current, parameter.value, parameter.blend, weight)
}

pub fn apply_expression_blend(
    current: f32,
    value: f32,
    blend: ExpressionBlend,
    weight: f32,
) -> f32 {
    match blend {
        ExpressionBlend::Add => current + (value * weight),
        ExpressionBlend::Multiply => current * (1.0 + (value - 1.0) * weight),
        ExpressionBlend::Overwrite if weight == 1.0 => value,
        ExpressionBlend::Overwrite => (current * (1.0 - weight)) + (value * weight),
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum ExpressionBlend {
    #[default]
    Add,
    Multiply,
    Overwrite,
}

impl ExpressionBlend {
    fn from_raw(value: Option<&str>) -> Self {
        match value {
            Some("Multiply") => Self::Multiply,
            Some("Overwrite") => Self::Overwrite,
            _ => Self::Add,
        }
    }
}

fn deserialize_expression_blend<'de, D>(
    deserializer: D,
) -> std::result::Result<ExpressionBlend, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
        .map(|value| ExpressionBlend::from_raw(value.as_deref()))
}
