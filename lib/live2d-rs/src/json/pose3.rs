use serde::Deserialize;

use crate::{Error, Result};

const FORMAT: &str = "pose3.json";
pub const DEFAULT_POSE_FADE_IN_TIME: f32 = 0.5;

#[derive(Debug, Clone, PartialEq)]
pub struct Pose3 {
    kind: String,
    fade_in_time: Option<f32>,
    groups: Vec<Vec<PosePart>>,
}

impl Pose3 {
    pub fn from_json_str(source: &str) -> Result<Self> {
        let raw: RawPose3 = serde_json::from_str(source).map_err(|error| Error::InvalidJson {
            format: FORMAT,
            message: error.to_string(),
        })?;

        Ok(Self {
            kind: raw.kind,
            fade_in_time: raw.fade_in_time,
            groups: raw.groups,
        })
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn fade_in_time(&self) -> Option<f32> {
        self.fade_in_time
    }

    pub fn resolved_fade_in_time(&self) -> f32 {
        resolved_pose_fade_in_time(self.fade_in_time)
    }

    pub fn groups(&self) -> &[Vec<PosePart>] {
        &self.groups
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct RawPose3 {
    #[serde(rename = "Type")]
    kind: String,
    #[serde(rename = "FadeInTime", default)]
    fade_in_time: Option<f32>,
    #[serde(rename = "Groups", default)]
    groups: Vec<Vec<PosePart>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PosePart {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Link", default)]
    links: Vec<String>,
}

impl PosePart {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn links(&self) -> &[String] {
        &self.links
    }
}

pub fn resolved_pose_fade_in_time(fade_in_time: Option<f32>) -> f32 {
    match fade_in_time {
        Some(value) if value >= 0.0 => value,
        _ => DEFAULT_POSE_FADE_IN_TIME,
    }
}

pub fn update_pose_group_opacities(
    parameter_values: &[f32],
    part_opacities: &mut [f32],
    delta_time_seconds: f32,
    fade_time_seconds: f32,
) -> Option<()> {
    if parameter_values.is_empty() || parameter_values.len() != part_opacities.len() {
        return None;
    }

    let delta_time_seconds = delta_time_seconds.max(0.0);
    let mut visible_part_index = None;
    let mut new_opacity = 1.0;

    for (index, parameter_value) in parameter_values.iter().enumerate() {
        if *parameter_value <= 0.001 {
            continue;
        }

        if visible_part_index.is_some() {
            break;
        }

        visible_part_index = Some(index);
        if fade_time_seconds == 0.0 {
            new_opacity = 1.0;
            continue;
        }

        new_opacity = (part_opacities[index] + (delta_time_seconds / fade_time_seconds)).min(1.0);
    }

    let visible_part_index = visible_part_index.unwrap_or(0);

    for (index, opacity) in part_opacities.iter_mut().enumerate() {
        if index == visible_part_index {
            *opacity = new_opacity;
            continue;
        }

        let mut target_opacity = if new_opacity < 0.5 {
            new_opacity * (0.5 - 1.0) / 0.5 + 1.0
        } else {
            (1.0 - new_opacity) * 0.5 / (1.0 - 0.5)
        };

        let back_opacity = (1.0 - target_opacity) * (1.0 - new_opacity);
        if back_opacity > 0.15 {
            target_opacity = 1.0 - 0.15 / (1.0 - new_opacity);
        }

        if *opacity > target_opacity {
            *opacity = target_opacity;
        }
    }

    Some(())
}

pub fn copy_pose_link_opacities(
    part_opacities: &mut [f32],
    source_index: usize,
    link_indices: &[usize],
) -> Option<()> {
    if source_index >= part_opacities.len()
        || link_indices
            .iter()
            .any(|link_index| *link_index >= part_opacities.len())
    {
        return None;
    }

    let opacity = part_opacities[source_index];
    for link_index in link_indices {
        part_opacities[*link_index] = opacity;
    }

    Some(())
}
