use super::math::{Vector2, degrees_to_radian, direction_to_radian, radian_to_direction};

const MAXIMUM_WEIGHT: f32 = 100.0;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PhysicsRange {
    minimum: f32,
    maximum: f32,
    default: f32,
}

impl PhysicsRange {
    pub fn new(minimum: f32, maximum: f32, default: f32) -> Self {
        Self {
            minimum,
            maximum,
            default,
        }
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
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct PhysicsInputAccumulator {
    translation_x: f32,
    translation_y: f32,
    angle: f32,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PhysicsParticle {
    position: Vector2,
    last_position: Vector2,
    velocity: Vector2,
    force: Vector2,
    last_gravity: Vector2,
    mobility: f32,
    delay: f32,
    acceleration: f32,
    radius: f32,
}

impl PhysicsParticle {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        position: Vector2,
        last_position: Vector2,
        velocity: Vector2,
        force: Vector2,
        last_gravity: Vector2,
        mobility: f32,
        delay: f32,
        acceleration: f32,
        radius: f32,
    ) -> Self {
        Self {
            position,
            last_position,
            velocity,
            force,
            last_gravity,
            mobility,
            delay,
            acceleration,
            radius,
        }
    }

    pub fn position(&self) -> Vector2 {
        self.position
    }

    pub fn last_position(&self) -> Vector2 {
        self.last_position
    }

    pub fn velocity(&self) -> Vector2 {
        self.velocity
    }

    pub fn force(&self) -> Vector2 {
        self.force
    }

    pub fn last_gravity(&self) -> Vector2 {
        self.last_gravity
    }
}

impl PhysicsInputAccumulator {
    pub fn add_translation_x(
        &mut self,
        value: f32,
        parameter: PhysicsRange,
        normalization: PhysicsRange,
        reflect: bool,
        weight_percent: f32,
    ) {
        self.translation_x +=
            weighted_normalized_value(value, parameter, normalization, reflect, weight_percent);
    }

    pub fn add_translation_y(
        &mut self,
        value: f32,
        parameter: PhysicsRange,
        normalization: PhysicsRange,
        reflect: bool,
        weight_percent: f32,
    ) {
        self.translation_y +=
            weighted_normalized_value(value, parameter, normalization, reflect, weight_percent);
    }

    pub fn add_angle(
        &mut self,
        value: f32,
        parameter: PhysicsRange,
        normalization: PhysicsRange,
        reflect: bool,
        weight_percent: f32,
    ) {
        self.angle +=
            weighted_normalized_value(value, parameter, normalization, reflect, weight_percent);
    }

    pub fn translation_x(&self) -> f32 {
        self.translation_x
    }

    pub fn translation_y(&self) -> f32 {
        self.translation_y
    }

    pub fn angle(&self) -> f32 {
        self.angle
    }
}

pub fn normalize_physics_parameter(
    value: f32,
    parameter: PhysicsRange,
    normalized: PhysicsRange,
    reflect: bool,
) -> f32 {
    let maximum = parameter.maximum.max(parameter.minimum);
    let minimum = parameter.maximum.min(parameter.minimum);
    let value = value.clamp(minimum, maximum);
    let normalized_minimum = normalized.minimum.min(normalized.maximum);
    let normalized_maximum = normalized.minimum.max(normalized.maximum);
    let normalized_middle = normalized.default;
    let middle = minimum + ((maximum - minimum).abs() / 2.0);
    let parameter_value = value - middle;

    let result = match parameter_value.total_cmp(&0.0) {
        std::cmp::Ordering::Greater => {
            let normalized_length = normalized_maximum - normalized_middle;
            let parameter_length = maximum - middle;
            if parameter_length == 0.0 {
                0.0
            } else {
                parameter_value * (normalized_length / parameter_length) + normalized_middle
            }
        }
        std::cmp::Ordering::Less => {
            let normalized_length = normalized_minimum - normalized_middle;
            let parameter_length = minimum - middle;
            if parameter_length == 0.0 {
                0.0
            } else {
                parameter_value * (normalized_length / parameter_length) + normalized_middle
            }
        }
        std::cmp::Ordering::Equal => normalized_middle,
    };

    if reflect { result } else { -result }
}

fn weighted_normalized_value(
    value: f32,
    parameter: PhysicsRange,
    normalization: PhysicsRange,
    reflect: bool,
    weight_percent: f32,
) -> f32 {
    normalize_physics_parameter(value, parameter, normalization, reflect)
        * (weight_percent / MAXIMUM_WEIGHT)
}

pub fn physics_output_translation_x(translation: Vector2, reflect: bool) -> f32 {
    let value = translation.x();
    if reflect { -value } else { value }
}

pub fn physics_output_translation_y(translation: Vector2, reflect: bool) -> f32 {
    let value = translation.y();
    if reflect { -value } else { value }
}

pub fn parent_gravity_for_physics_output(
    particles: &[Vector2],
    particle_index: usize,
    parent_gravity: Vector2,
) -> Option<Vector2> {
    if particle_index >= 2 {
        let current = particles.get(particle_index - 1)?;
        let previous = particles.get(particle_index - 2)?;
        return Some(Vector2::new(
            current.x() - previous.x(),
            current.y() - previous.y(),
        ));
    }

    Some(Vector2::new(-parent_gravity.x(), -parent_gravity.y()))
}

pub fn physics_output_angle_with_parent_gravity(
    translation: Vector2,
    parent_gravity: Vector2,
    reflect: bool,
) -> f32 {
    let value = direction_to_radian(parent_gravity, translation);
    if reflect { -value } else { value }
}

pub fn update_physics_particles(
    strand: &mut [PhysicsParticle],
    total_translation: Vector2,
    total_angle: f32,
    wind_direction: Vector2,
    threshold_value: f32,
    delta_time_seconds: f32,
    air_resistance: f32,
) {
    let Some((first, rest)) = strand.split_first_mut() else {
        return;
    };

    first.position = total_translation;
    let current_gravity = normalize(radian_to_direction(degrees_to_radian(total_angle)));
    let mut previous_position = first.position;

    for particle in rest {
        particle.force = add(mul(current_gravity, particle.acceleration), wind_direction);
        particle.last_position = particle.position;

        let delay = particle.delay * delta_time_seconds * 30.0;
        let mut direction = sub(particle.position, previous_position);
        let radian = direction_to_radian(particle.last_gravity, current_gravity) / air_resistance;

        let direction_x = radian.cos() * direction.x() - direction.y() * radian.sin();
        let direction_y = radian.sin() * direction_x + direction.y() * radian.cos();
        direction = Vector2::new(direction_x, direction_y);

        particle.position = add(previous_position, direction);
        let velocity = mul(particle.velocity, delay);
        let force = mul(particle.force, delay * delay);
        particle.position = add(add(particle.position, velocity), force);

        let new_direction = normalize(sub(particle.position, previous_position));
        particle.position = add(previous_position, mul(new_direction, particle.radius));

        if particle.position.x().abs() < threshold_value {
            particle.position = Vector2::new(0.0, particle.position.y());
        }

        if delay != 0.0 {
            particle.velocity = mul(
                div(sub(particle.position, particle.last_position), delay),
                particle.mobility,
            );
        }

        particle.force = Vector2::new(0.0, 0.0);
        particle.last_gravity = current_gravity;
        previous_position = particle.position;
    }
}

pub fn stabilize_physics_particles(
    strand: &mut [PhysicsParticle],
    total_translation: Vector2,
    total_angle: f32,
    wind_direction: Vector2,
    threshold_value: f32,
) {
    let Some((first, rest)) = strand.split_first_mut() else {
        return;
    };

    first.position = total_translation;
    let current_gravity = normalize(radian_to_direction(degrees_to_radian(total_angle)));
    let mut previous_position = first.position;

    for particle in rest {
        particle.force = add(mul(current_gravity, particle.acceleration), wind_direction);
        particle.last_position = particle.position;
        particle.velocity = Vector2::new(0.0, 0.0);

        let force = mul(normalize(particle.force), particle.radius);
        particle.position = add(previous_position, force);

        if particle.position.x().abs() < threshold_value {
            particle.position = Vector2::new(0.0, particle.position.y());
        }

        particle.force = Vector2::new(0.0, 0.0);
        particle.last_gravity = current_gravity;
        previous_position = particle.position;
    }
}

fn add(a: Vector2, b: Vector2) -> Vector2 {
    Vector2::new(a.x() + b.x(), a.y() + b.y())
}

fn sub(a: Vector2, b: Vector2) -> Vector2 {
    Vector2::new(a.x() - b.x(), a.y() - b.y())
}

fn mul(value: Vector2, factor: f32) -> Vector2 {
    Vector2::new(value.x() * factor, value.y() * factor)
}

fn div(value: Vector2, factor: f32) -> Vector2 {
    Vector2::new(value.x() / factor, value.y() / factor)
}

fn normalize(value: Vector2) -> Vector2 {
    let length = (value.x() * value.x() + value.y() * value.y()).sqrt();
    if length == 0.0 {
        value
    } else {
        div(value, length)
    }
}
