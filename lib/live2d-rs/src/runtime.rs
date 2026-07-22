use std::collections::HashMap;

use crate::{
    core::{clamp_parameter_value, draw_order_from_raw},
    json::{Model3, Pose3, copy_pose_link_opacities, update_pose_group_opacities},
    moc3::{
        Moc3ArtMeshKeyforms, Moc3ArtMeshes, Moc3CanvasInfo, Moc3Deformers, Moc3DrawOrderGroups,
        Moc3DrawableMesh, Moc3Glues, Moc3Ids, Moc3KeyformBindings, Moc3OffscreenInfo, Moc3Parts,
        build_moc3_drawable_meshes_with_parameters_offscreen_and_part_opacities,
    },
};

#[derive(Debug, Clone)]
struct PoseGroup {
    members: Vec<usize>,
    links: Vec<Vec<usize>>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ParameterInfo<'a> {
    id: &'a str,
    minimum: f32,
    maximum: f32,
    default: f32,
    value: f32,
}

impl<'a> ParameterInfo<'a> {
    pub fn id(&self) -> &'a str {
        self.id
    }

    pub fn minimum(&self) -> f32 {
        self.minimum
    }

    pub fn maximum(&self) -> f32 {
        self.maximum
    }

    pub fn default(&self) -> f32 {
        self.default
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn normalized_value(&self) -> f32 {
        normalized_parameter_value(self.value, self.minimum, self.maximum)
    }
}

#[derive(Debug, Clone)]
pub struct ModelRuntime {
    model: Model3,
    canvas: Moc3CanvasInfo,
    art_meshes: Moc3ArtMeshes,
    art_mesh_keyforms: Moc3ArtMeshKeyforms,
    deformers: Moc3Deformers,
    bindings: Moc3KeyformBindings,
    ids: Moc3Ids,
    offscreen: Moc3OffscreenInfo,
    glues: Moc3Glues,
    parts: Moc3Parts,
    draw_order_groups: Option<Moc3DrawOrderGroups>,
    parameter_index: HashMap<String, usize>,
    parameter_values: Vec<f32>,
    parameter_overrides: Vec<Option<f32>>,
    part_index: HashMap<String, usize>,
    part_opacity_overrides: Vec<Option<f32>>,
    part_opacities: Vec<f32>,
    pose_groups: Vec<PoseGroup>,
    pose_fade_time: f32,
    pose_opacities: Vec<f32>,
    meshes: Vec<Moc3DrawableMesh>,
}

impl ModelRuntime {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model: Model3,
        canvas: Moc3CanvasInfo,
        art_meshes: Moc3ArtMeshes,
        art_mesh_keyforms: Moc3ArtMeshKeyforms,
        deformers: Moc3Deformers,
        bindings: Moc3KeyformBindings,
        ids: Moc3Ids,
        offscreen: Moc3OffscreenInfo,
        glues: Moc3Glues,
        parts: Moc3Parts,
        draw_order_groups: Option<Moc3DrawOrderGroups>,
        pose: Option<Pose3>,
    ) -> Option<Self> {
        let parameter_values = bindings.parameter_default_values().to_vec();
        let parameter_overrides = vec![None; parameter_values.len()];
        let parameter_index = ids
            .parameters()
            .iter()
            .enumerate()
            .map(|(index, id)| (id.clone(), index))
            .collect();
        let part_index: HashMap<String, usize> = ids
            .parts()
            .iter()
            .enumerate()
            .map(|(index, id)| (id.clone(), index))
            .collect();
        let part_count = parts.part_count();

        let pose_fade_time = pose
            .as_ref()
            .map(Pose3::resolved_fade_in_time)
            .unwrap_or_default();
        let pose_groups = pose
            .as_ref()
            .map(|pose| build_pose_groups(pose, &part_index))
            .unwrap_or_default();
        let pose_opacities = initial_pose_opacities(&pose_groups, part_count);

        let mut runtime = Self {
            model,
            canvas,
            art_meshes,
            art_mesh_keyforms,
            deformers,
            bindings,
            ids,
            offscreen,
            glues,
            parts,
            draw_order_groups,
            parameter_index,
            parameter_values,
            parameter_overrides,
            part_index,
            part_opacity_overrides: vec![None; part_count],
            part_opacities: vec![1.0; part_count],
            pose_groups,
            pose_fade_time,
            pose_opacities,
            meshes: Vec::new(),
        };
        runtime.update_meshes()?;
        Some(runtime)
    }

    pub fn model(&self) -> &Model3 {
        &self.model
    }

    pub fn canvas(&self) -> Moc3CanvasInfo {
        self.canvas
    }

    pub fn parameter_ids(&self) -> &[String] {
        self.ids.parameters()
    }

    pub fn parameter_index(&self, id: &str) -> Option<usize> {
        self.parameter_index.get(id).copied()
    }

    pub fn parameter_value(&self, id: &str) -> Option<f32> {
        let index = self.parameter_index(id)?;
        self.parameter_values.get(index).copied()
    }

    pub fn parameter_value_by_index(&self, index: usize) -> Option<f32> {
        self.parameter_values.get(index).copied()
    }

    pub fn parameter_values(&self) -> &[f32] {
        &self.parameter_values
    }

    pub fn parameter_info(&self, id: &str) -> Option<ParameterInfo<'_>> {
        let index = self.parameter_index(id)?;
        self.parameter_info_by_index(index)
    }

    pub fn parameter_info_by_index(&self, index: usize) -> Option<ParameterInfo<'_>> {
        Some(ParameterInfo {
            id: self.ids.parameters().get(index)?.as_str(),
            minimum: self.parameter_minimum_by_index(index)?,
            maximum: self.parameter_maximum_by_index(index)?,
            default: self.parameter_default_by_index(index)?,
            value: self.parameter_value_by_index(index)?,
        })
    }

    pub fn parameter_infos(&self) -> impl Iterator<Item = ParameterInfo<'_>> + '_ {
        (0..self.ids.parameters().len()).filter_map(|index| self.parameter_info_by_index(index))
    }

    pub fn parameter_minimum_by_index(&self, index: usize) -> Option<f32> {
        self.bindings.parameter_min_values().get(index).copied()
    }

    pub fn parameter_maximum_by_index(&self, index: usize) -> Option<f32> {
        self.bindings.parameter_max_values().get(index).copied()
    }

    pub fn parameter_default_by_index(&self, index: usize) -> Option<f32> {
        self.bindings.parameter_default_values().get(index).copied()
    }

    pub fn parameter_normalized_value(&self, id: &str) -> Option<f32> {
        let index = self.parameter_index(id)?;
        self.parameter_normalized_value_by_index(index)
    }

    pub fn parameter_normalized_value_by_index(&self, index: usize) -> Option<f32> {
        let minimum = self.parameter_minimum_by_index(index)?;
        let maximum = self.parameter_maximum_by_index(index)?;
        let value = self.parameter_value_by_index(index)?;
        Some(normalized_parameter_value(value, minimum, maximum))
    }

    pub fn set_parameter(&mut self, id: &str, value: f32) -> bool {
        match self.parameter_index(id) {
            Some(index) => self.set_parameter_by_index(index, value),
            None => false,
        }
    }

    pub fn set_parameter_by_index(&mut self, index: usize, value: f32) -> bool {
        let Some(slot) = self.parameter_values.get_mut(index) else {
            return false;
        };
        let minimum = self
            .bindings
            .parameter_min_values()
            .get(index)
            .copied()
            .unwrap_or(f32::MIN);
        let maximum = self
            .bindings
            .parameter_max_values()
            .get(index)
            .copied()
            .unwrap_or(f32::MAX);
        *slot = clamp_parameter_value(value, minimum, maximum);
        true
    }

    pub fn set_parameter_normalized(&mut self, id: &str, value: f32) -> bool {
        match self.parameter_index(id) {
            Some(index) => self.set_parameter_normalized_by_index(index, value),
            None => false,
        }
    }

    pub fn set_parameter_normalized_by_index(&mut self, index: usize, value: f32) -> bool {
        let Some(raw) = self.raw_parameter_value_from_normalized_index(index, value) else {
            return false;
        };
        self.set_parameter_by_index(index, raw)
    }

    pub fn parameter_override_value(&self, id: &str) -> Option<f32> {
        let index = self.parameter_index(id)?;
        self.parameter_override_value_by_index(index)
    }

    pub fn parameter_override_value_by_index(&self, index: usize) -> Option<f32> {
        self.parameter_overrides.get(index).copied().flatten()
    }

    pub fn parameter_override_normalized_value(&self, id: &str) -> Option<f32> {
        let index = self.parameter_index(id)?;
        self.parameter_override_normalized_value_by_index(index)
    }

    pub fn parameter_override_normalized_value_by_index(&self, index: usize) -> Option<f32> {
        let minimum = self.parameter_minimum_by_index(index)?;
        let maximum = self.parameter_maximum_by_index(index)?;
        let value = self.parameter_override_value_by_index(index)?;
        Some(normalized_parameter_value(value, minimum, maximum))
    }

    pub fn set_parameter_override(&mut self, id: &str, value: f32) -> bool {
        match self.parameter_index(id) {
            Some(index) => self.set_parameter_override_by_index(index, value),
            None => false,
        }
    }

    pub fn set_parameter_override_by_index(&mut self, index: usize, value: f32) -> bool {
        if index >= self.parameter_overrides.len() {
            return false;
        }
        let Some(minimum) = self.parameter_minimum_by_index(index) else {
            return false;
        };
        let Some(maximum) = self.parameter_maximum_by_index(index) else {
            return false;
        };
        self.parameter_overrides[index] = Some(clamp_parameter_value(value, minimum, maximum));
        true
    }

    pub fn set_parameter_override_normalized(&mut self, id: &str, value: f32) -> bool {
        match self.parameter_index(id) {
            Some(index) => self.set_parameter_override_normalized_by_index(index, value),
            None => false,
        }
    }

    pub fn set_parameter_override_normalized_by_index(&mut self, index: usize, value: f32) -> bool {
        let Some(raw) = self.raw_parameter_value_from_normalized_index(index, value) else {
            return false;
        };
        self.set_parameter_override_by_index(index, raw)
    }

    pub fn clear_parameter_override(&mut self, id: &str) -> bool {
        match self.parameter_index(id) {
            Some(index) => self.clear_parameter_override_by_index(index),
            None => false,
        }
    }

    pub fn clear_parameter_override_by_index(&mut self, index: usize) -> bool {
        let Some(slot) = self.parameter_overrides.get_mut(index) else {
            return false;
        };
        *slot = None;
        true
    }

    pub fn clear_parameter_overrides(&mut self) {
        self.parameter_overrides.fill(None);
    }

    pub fn apply_parameter_overrides(&mut self) {
        for index in 0..self.parameter_overrides.len() {
            if let Some(value) = self.parameter_overrides[index] {
                self.set_parameter_by_index(index, value);
            }
        }
    }

    fn raw_parameter_value_from_normalized_index(&self, index: usize, value: f32) -> Option<f32> {
        let minimum = self.parameter_minimum_by_index(index)?;
        let maximum = self.parameter_maximum_by_index(index)?;
        let amount = value.clamp(0.0, 1.0);
        Some(minimum + (maximum - minimum) * amount)
    }

    pub fn reset_parameters(&mut self) {
        self.parameter_values
            .copy_from_slice(self.bindings.parameter_default_values());
    }

    pub fn part_ids(&self) -> &[String] {
        self.ids.parts()
    }

    pub fn part_index(&self, id: &str) -> Option<usize> {
        self.part_index.get(id).copied()
    }

    pub fn set_part_opacity(&mut self, id: &str, value: f32) -> bool {
        match self.part_index(id) {
            Some(index) => self.set_part_opacity_by_index(index, value),
            None => false,
        }
    }

    pub fn set_part_opacity_by_index(&mut self, index: usize, value: f32) -> bool {
        let Some(slot) = self.part_opacity_overrides.get_mut(index) else {
            return false;
        };
        *slot = Some(value.clamp(0.0, 1.0));
        true
    }

    pub fn reset_part_opacities(&mut self) {
        self.part_opacity_overrides
            .iter_mut()
            .for_each(|o| *o = None);
    }

    pub fn apply_pose(&mut self, delta_seconds: f32) {
        for group in &self.pose_groups {
            let selection: Vec<f32> = group
                .members
                .iter()
                .map(|&part| self.part_selection_opacity(part))
                .collect();
            let mut faded: Vec<f32> = group
                .members
                .iter()
                .map(|&part| self.pose_opacities[part])
                .collect();

            if update_pose_group_opacities(
                &selection,
                &mut faded,
                delta_seconds,
                self.pose_fade_time,
            )
            .is_none()
            {
                continue;
            }

            for (opacity, &part) in faded.iter().zip(&group.members) {
                self.pose_opacities[part] = *opacity;
            }
            for (member_position, &part) in group.members.iter().enumerate() {
                let _ = copy_pose_link_opacities(
                    &mut self.pose_opacities,
                    part,
                    &group.links[member_position],
                );
            }
        }
    }

    fn part_selection_opacity(&self, part_index: usize) -> f32 {
        self.part_opacity_overrides[part_index].unwrap_or_else(|| {
            self.parts
                .interpolate_opacity(part_index, &self.bindings, &self.parameter_values)
                .unwrap_or(1.0)
        })
    }

    fn update_part_opacities(&mut self) {
        for index in 0..self.part_opacities.len() {
            let base = self.part_opacity_overrides[index].unwrap_or_else(|| {
                self.parts
                    .interpolate_opacity(index, &self.bindings, &self.parameter_values)
                    .unwrap_or(1.0)
            });
            self.part_opacities[index] = base * self.pose_opacities[index];
        }

        for index in 0..self.part_opacities.len() {
            let mut opacity = self.part_opacities[index];
            let mut parent = self.parts.parent_part_index(index);
            while let Some(parent_index) = parent.and_then(|p| usize::try_from(p).ok()) {
                opacity *= self.part_opacities[parent_index];
                parent = self.parts.parent_part_index(parent_index);
            }
            self.part_opacities[index] = opacity;
        }
    }

    fn drawable_part_opacities(&self) -> Vec<f32> {
        (0..self.art_meshes.meshes().len())
            .map(|drawable_index| {
                self.offscreen
                    .drawable_parent_part_index(drawable_index)
                    .and_then(|p| usize::try_from(p).ok())
                    .and_then(|part_index| self.part_opacities.get(part_index).copied())
                    .unwrap_or(1.0)
            })
            .collect()
    }

    pub fn update_meshes(&mut self) -> Option<()> {
        self.update_part_opacities();
        let drawable_part_opacities = self.drawable_part_opacities();
        self.meshes = build_moc3_drawable_meshes_with_parameters_offscreen_and_part_opacities(
            &self.art_meshes,
            &self.art_mesh_keyforms,
            &self.deformers,
            &self.bindings,
            &self.ids,
            &self.offscreen,
            &self.parameter_values,
            &drawable_part_opacities,
        )?;
        self.glues
            .apply(&mut self.meshes, &self.bindings, &self.parameter_values)?;
        self.apply_group_render_orders();
        Some(())
    }

    fn apply_group_render_orders(&mut self) {
        let Some(groups) = self.draw_order_groups.as_ref() else {
            return;
        };
        let drawable_draw_orders: Vec<i32> = self
            .meshes
            .iter()
            .map(|mesh| draw_order_from_raw(mesh.draw_order()))
            .collect();

        let part_count = self.parts.part_count();
        let mut part_draw_orders = vec![0i32; part_count];
        let mut part_enable = vec![false; part_count];
        for index in 0..part_count {
            if let Some(raw) =
                self.parts
                    .interpolate_draw_order(index, &self.bindings, &self.parameter_values)
            {
                part_draw_orders[index] = draw_order_from_raw(raw);
                part_enable[index] = true;
            }
        }

        let Some(render_orders) = groups.render_orders(
            &drawable_draw_orders,
            &part_draw_orders,
            &part_enable,
            self.offscreen.part_offscreen_indices(),
            self.offscreen.offscreen_count(),
        ) else {
            return;
        };
        for (mesh, render_order) in self.meshes.iter_mut().zip(&render_orders) {
            mesh.set_render_order(*render_order);
        }
    }

    pub fn meshes(&self) -> &[Moc3DrawableMesh] {
        &self.meshes
    }
}

fn normalized_parameter_value(value: f32, minimum: f32, maximum: f32) -> f32 {
    if maximum <= minimum {
        0.0
    } else {
        ((value - minimum) / (maximum - minimum)).clamp(0.0, 1.0)
    }
}

fn build_pose_groups(pose: &Pose3, part_index: &HashMap<String, usize>) -> Vec<PoseGroup> {
    pose.groups()
        .iter()
        .filter_map(|group| {
            let mut members = Vec::new();
            let mut links = Vec::new();
            for part in group {
                let Some(&part_idx) = part_index.get(part.id()) else {
                    continue;
                };
                members.push(part_idx);
                links.push(
                    part.links()
                        .iter()
                        .filter_map(|link| part_index.get(link).copied())
                        .collect(),
                );
            }
            (members.len() >= 2).then_some(PoseGroup { members, links })
        })
        .collect()
}

fn initial_pose_opacities(groups: &[PoseGroup], part_count: usize) -> Vec<f32> {
    let mut opacities = vec![1.0; part_count];
    for group in groups {
        for (position, &part) in group.members.iter().enumerate() {
            let opacity = if position == 0 { 1.0 } else { 0.0 };
            opacities[part] = opacity;
            for &link in &group.links[position] {
                opacities[link] = opacity;
            }
        }
    }
    opacities
}
