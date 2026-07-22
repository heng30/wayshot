use super::math::Vector2;

pub fn affect_art_mesh_pair(
    a: Vector2,
    b: Vector2,
    weight_a: f32,
    weight_b: f32,
    glue_opacity: f32,
) -> (Vector2, Vector2) {
    (
        a.lerp(b, weight_a * glue_opacity),
        b.lerp(a, weight_b * glue_opacity),
    )
}

pub fn apply_art_mesh_blend_shape_delta(
    positions: &mut [f32],
    deltas: &[f32],
    weight: f32,
) -> Option<()> {
    if positions.len() != deltas.len() {
        return None;
    }

    if weight == 0.0 {
        return Some(());
    }

    for (position, delta) in positions.iter_mut().zip(deltas) {
        *position += delta * weight;
    }

    Some(())
}

pub fn apply_parent_part_opacity(opacity: f32, parent_opacity: f32) -> f32 {
    opacity * parent_opacity
}

pub fn reverse_coordinate_y(vertices: &mut [Vector2]) {
    for vertex in vertices {
        *vertex = Vector2::new(vertex.x(), -vertex.y());
    }
}

pub fn draw_order_from_raw(raw: f32) -> i32 {
    ((raw + 0.001).trunc() as i32).clamp(0, 1000)
}
