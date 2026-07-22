use crate::core::Vector2;

use super::{
    Moc3ArtMeshKeyformInfo, Moc3ArtMeshKeyforms, Moc3ArtMeshes, Moc3Deformers, Moc3DrawableMesh,
    Moc3DrawableVertex, Moc3Ids, Moc3KeyformBindings, Moc3OffscreenInfo, build_moc3_drawable_mesh,
    compose::ComposedDeformers, keyform_bindings::Moc3KeyformSlot,
};

pub fn build_moc3_drawable_meshes_for_default_pose(
    art_meshes: &Moc3ArtMeshes,
    art_mesh_keyforms: &Moc3ArtMeshKeyforms,
    deformers: &Moc3Deformers,
    bindings: &Moc3KeyformBindings,
) -> Option<Vec<Moc3DrawableMesh>> {
    build_moc3_drawable_meshes_with_parameters(
        art_meshes,
        art_mesh_keyforms,
        deformers,
        bindings,
        bindings.parameter_default_values(),
    )
}

pub fn build_moc3_drawable_meshes_with_parameters(
    art_meshes: &Moc3ArtMeshes,
    art_mesh_keyforms: &Moc3ArtMeshKeyforms,
    deformers: &Moc3Deformers,
    bindings: &Moc3KeyformBindings,
    parameter_values: &[f32],
) -> Option<Vec<Moc3DrawableMesh>> {
    let composed = deformers.compose(bindings, parameter_values)?;
    let mut meshes = Vec::with_capacity(art_meshes.meshes().len());
    for art_mesh_index in 0..art_meshes.meshes().len() {
        meshes.push(build_moc3_drawable_mesh_for_pose(
            art_meshes,
            art_mesh_keyforms,
            &composed,
            bindings,
            parameter_values,
            art_mesh_index,
        )?);
    }

    Some(meshes)
}

pub fn build_moc3_drawable_meshes_for_default_pose_with_offscreen_state(
    art_meshes: &Moc3ArtMeshes,
    art_mesh_keyforms: &Moc3ArtMeshKeyforms,
    deformers: &Moc3Deformers,
    bindings: &Moc3KeyformBindings,
    ids: &Moc3Ids,
    offscreen: &Moc3OffscreenInfo,
) -> Option<Vec<Moc3DrawableMesh>> {
    build_moc3_drawable_meshes_with_parameters_and_offscreen_state(
        art_meshes,
        art_mesh_keyforms,
        deformers,
        bindings,
        ids,
        offscreen,
        bindings.parameter_default_values(),
    )
}

pub fn build_moc3_drawable_meshes_with_parameters_and_offscreen_state(
    art_meshes: &Moc3ArtMeshes,
    art_mesh_keyforms: &Moc3ArtMeshKeyforms,
    deformers: &Moc3Deformers,
    bindings: &Moc3KeyformBindings,
    ids: &Moc3Ids,
    offscreen: &Moc3OffscreenInfo,
    parameter_values: &[f32],
) -> Option<Vec<Moc3DrawableMesh>> {
    build_moc3_drawable_meshes_with_parameters_offscreen_and_part_opacities(
        art_meshes,
        art_mesh_keyforms,
        deformers,
        bindings,
        ids,
        offscreen,
        parameter_values,
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_moc3_drawable_meshes_with_parameters_offscreen_and_part_opacities(
    art_meshes: &Moc3ArtMeshes,
    art_mesh_keyforms: &Moc3ArtMeshKeyforms,
    deformers: &Moc3Deformers,
    bindings: &Moc3KeyformBindings,
    ids: &Moc3Ids,
    offscreen: &Moc3OffscreenInfo,
    parameter_values: &[f32],
    drawable_part_opacities: &[f32],
) -> Option<Vec<Moc3DrawableMesh>> {
    let mut meshes = build_moc3_drawable_meshes_with_parameters(
        art_meshes,
        art_mesh_keyforms,
        deformers,
        bindings,
        parameter_values,
    )?;

    for (drawable_index, part_opacity) in drawable_part_opacities.iter().copied().enumerate() {
        let mesh = meshes.get_mut(drawable_index)?;
        mesh.set_opacity(mesh.opacity() * part_opacity);
    }

    for drawable_index in offscreen.effect_source_drawable_indices(ids) {
        meshes.get_mut(drawable_index)?.set_opacity(0.0);
    }

    Some(meshes)
}

fn build_moc3_drawable_mesh_for_pose(
    art_meshes: &Moc3ArtMeshes,
    art_mesh_keyforms: &Moc3ArtMeshKeyforms,
    composed: &ComposedDeformers,
    bindings: &Moc3KeyformBindings,
    parameter_values: &[f32],
    art_mesh_index: usize,
) -> Option<Moc3DrawableMesh> {
    let keyform_count = art_mesh_keyforms.art_mesh_keyforms(art_mesh_index)?.len();
    let slots = bindings.keyform_slots(
        art_meshes.art_mesh_keyform_binding_band_index(art_mesh_index)?,
        keyform_count,
        parameter_values,
    )?;
    let base_local_keyform_index = slots.first()?.local_index;
    let mesh = build_moc3_drawable_mesh(
        art_meshes,
        art_mesh_keyforms,
        art_mesh_index,
        base_local_keyform_index,
    )?;
    let parent_deformer_index = art_meshes.art_mesh_parent_deformer_index(art_mesh_index)?;
    let opacity = interpolate_art_mesh_opacity(art_mesh_keyforms, art_mesh_index, &slots)?
        * composed.deformer_opacity(parent_deformer_index);
    let draw_order = interpolate_art_mesh_draw_order(art_mesh_keyforms, art_mesh_index, &slots)?;
    let multiply_color =
        interpolate_art_mesh_color(art_mesh_keyforms, art_mesh_index, &slots, |k| {
            k.multiply_color()
        })?;
    let screen_color =
        interpolate_art_mesh_color(art_mesh_keyforms, art_mesh_index, &slots, |k| {
            k.screen_color()
        })?;
    let (parent_multiply_color, parent_screen_color) =
        composed.deformer_colors(parent_deformer_index);
    let multiply_color = combine_multiply_color(multiply_color, parent_multiply_color);
    let screen_color = combine_screen_color(screen_color, parent_screen_color);
    let mut positions = interpolate_art_mesh_positions(art_mesh_keyforms, art_mesh_index, &slots)?;

    composed.transform_vertices(parent_deformer_index, &mut positions)?;

    let vertices = mesh
        .vertices()
        .iter()
        .zip(positions)
        .map(|(vertex, position)| {
            Moc3DrawableVertex::new([position.x(), -position.y()], vertex.uv())
        })
        .collect();

    let mut mesh = Moc3DrawableMesh::from_parts_with_render_order(
        mesh.texture_index(),
        mesh.drawable_flags(),
        opacity,
        draw_order,
        mesh.render_order(),
        vertices,
        mesh.indices().to_vec(),
        mesh.masks().to_vec(),
    );
    mesh.set_multiply_color(multiply_color);
    mesh.set_screen_color(screen_color);
    Some(mesh)
}

fn combine_multiply_color(local: [f32; 3], parent: [f32; 3]) -> [f32; 3] {
    [
        clamp01(local[0] * parent[0]),
        clamp01(local[1] * parent[1]),
        clamp01(local[2] * parent[2]),
    ]
}

fn combine_screen_color(local: [f32; 3], parent: [f32; 3]) -> [f32; 3] {
    [
        clamp01(local[0] + parent[0] - local[0] * parent[0]),
        clamp01(local[1] + parent[1] - local[1] * parent[1]),
        clamp01(local[2] + parent[2] - local[2] * parent[2]),
    ]
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn interpolate_art_mesh_color(
    keyforms: &Moc3ArtMeshKeyforms,
    art_mesh_index: usize,
    slots: &[Moc3KeyformSlot],
    channels: impl Fn(Moc3ArtMeshKeyformInfo) -> [f32; 3],
) -> Option<[f32; 3]> {
    let keyforms = keyforms.art_mesh_keyforms(art_mesh_index)?;
    let mut color = [0.0f32; 3];
    for slot in slots {
        let value = channels(*keyforms.get(slot.local_index)?);
        for (acc, channel) in color.iter_mut().zip(value) {
            *acc += channel * slot.weight;
        }
    }
    Some(color)
}

fn interpolate_art_mesh_positions(
    keyforms: &Moc3ArtMeshKeyforms,
    art_mesh_index: usize,
    slots: &[Moc3KeyformSlot],
) -> Option<Vec<Vector2>> {
    let first = keyforms.art_mesh_keyform_positions(art_mesh_index, slots.first()?.local_index)?;
    let mut out = vec![Vector2::default(); first.len().checked_div(2)?];

    for slot in slots {
        let positions = keyforms.art_mesh_keyform_positions(art_mesh_index, slot.local_index)?;
        if positions.len() != first.len() || positions.len() % 2 != 0 {
            return None;
        }
        for (target, position) in out.iter_mut().zip(positions.chunks_exact(2)) {
            *target = Vector2::new(
                target.x() + position[0] * slot.weight,
                target.y() + position[1] * slot.weight,
            );
        }
    }

    Some(out)
}

fn interpolate_art_mesh_opacity(
    keyforms: &Moc3ArtMeshKeyforms,
    art_mesh_index: usize,
    slots: &[Moc3KeyformSlot],
) -> Option<f32> {
    interpolate_art_mesh_scalar(keyforms, art_mesh_index, slots, |keyform| keyform.opacity())
}

fn interpolate_art_mesh_draw_order(
    keyforms: &Moc3ArtMeshKeyforms,
    art_mesh_index: usize,
    slots: &[Moc3KeyformSlot],
) -> Option<f32> {
    interpolate_art_mesh_scalar(keyforms, art_mesh_index, slots, |keyform| {
        keyform.draw_order()
    })
}

fn interpolate_art_mesh_scalar(
    keyforms: &Moc3ArtMeshKeyforms,
    art_mesh_index: usize,
    slots: &[Moc3KeyformSlot],
    value: impl Fn(Moc3ArtMeshKeyformInfo) -> f32,
) -> Option<f32> {
    let keyforms = keyforms.art_mesh_keyforms(art_mesh_index)?;
    let mut out = 0.0f32;
    for slot in slots {
        out += value(*keyforms.get(slot.local_index)?) * slot.weight;
    }
    Some(out)
}
