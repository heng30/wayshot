use std::{fs, path::Path};

use crate::{
    json::{
        Expression3, ExpressionBlend, ExpressionParameter, apply_expression_parameter, easing_sine,
    },
    runtime::ModelRuntime,
};

const DEFAULT_ADDITIVE_VALUE: f32 = 0.0;
const DEFAULT_MULTIPLY_VALUE: f32 = 1.0;

#[derive(Debug, Clone)]
pub struct ExpressionPlayer {
    expression: Expression3,
    time: f32,
    weight: f32,
    fade_out_started_at: Option<f32>,
    finished: bool,
}

impl ExpressionPlayer {
    pub fn new(expression: Expression3) -> Self {
        Self {
            expression,
            time: 0.0,
            weight: 1.0,
            fade_out_started_at: None,
            finished: false,
        }
    }

    pub fn expression(&self) -> &Expression3 {
        &self.expression
    }

    pub fn time(&self) -> f32 {
        self.time
    }

    pub fn weight(&self) -> f32 {
        self.weight
    }

    pub fn set_weight(&mut self, weight: f32) {
        self.weight = weight.clamp(0.0, 1.0);
    }

    pub fn fade_weight(&self) -> f32 {
        self.weight * self.fade_in_weight() * self.fade_out_weight()
    }

    pub fn is_fading_out(&self) -> bool {
        self.fade_out_started_at.is_some()
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn restart(&mut self) {
        self.time = 0.0;
        self.fade_out_started_at = None;
        self.finished = false;
    }

    /// Seek to an absolute time without triggering fade-out.
    /// For idempotent rendering: the expression is simply evaluated at the
    /// given time with full fade-in weight (no fade-out since no external
    /// trigger to stop it).
    pub fn seek_to(&mut self, time: f32) {
        self.time = time.max(0.0);
        // Do NOT start fade_out — idempotent rendering treats the expression
        // as perpetually active at the seeked time.
        self.finished = false;
    }

    pub fn start_fade_out(&mut self) {
        if self.finished || self.fade_out_started_at.is_some() {
            return;
        }

        let fade_out = self.expression.resolved_fade_out_time();
        if fade_out == 0.0 {
            self.finished = true;
        } else {
            self.fade_out_started_at = Some(self.time);
        }
    }

    pub fn tick(&mut self, delta_seconds: f32) {
        if self.finished {
            return;
        }

        self.time += delta_seconds.max(0.0);
        if let Some(fade_out_started_at) = self.fade_out_started_at {
            let fade_out = self.expression.resolved_fade_out_time();
            if self.time >= fade_out_started_at + fade_out {
                self.finished = true;
            }
        }
    }

    pub fn apply(&self, runtime: &mut ModelRuntime) {
        if self.finished {
            return;
        }

        let weight = self.fade_weight();
        for parameter in self.expression.parameters() {
            let Some(index) = runtime.parameter_index(parameter.id()) else {
                continue;
            };
            let Some(current) = runtime.parameter_value_by_index(index) else {
                continue;
            };
            let value = apply_expression_parameter(current, parameter, weight);
            runtime.set_parameter_by_index(index, value);
        }
    }

    fn fade_in_weight(&self) -> f32 {
        let fade_in = self.expression.resolved_fade_in_time();
        if fade_in == 0.0 {
            1.0
        } else {
            easing_sine(self.time / fade_in)
        }
    }

    fn fade_out_weight(&self) -> f32 {
        let Some(fade_out_started_at) = self.fade_out_started_at else {
            return 1.0;
        };
        let fade_out = self.expression.resolved_fade_out_time();
        if fade_out == 0.0 {
            0.0
        } else {
            easing_sine((fade_out_started_at + fade_out - self.time) / fade_out)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExpressionManager {
    pub(crate) players: Vec<ExpressionPlayer>,
}

impl ExpressionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn play(&mut self, expression: Expression3) {
        for player in &mut self.players {
            player.start_fade_out();
        }
        self.players.push(ExpressionPlayer::new(expression));
    }

    pub fn stop_all(&mut self) {
        for player in &mut self.players {
            player.start_fade_out();
        }
    }

    pub fn tick(&mut self, delta_seconds: f32) {
        for player in &mut self.players {
            player.tick(delta_seconds);
        }

        let latest_is_fully_faded_in = self
            .players
            .last()
            .map(|player| player.fade_weight() >= 1.0)
            .unwrap_or(false);
        if self.players.len() > 1 && latest_is_fully_faded_in {
            if let Some(latest) = self.players.pop() {
                self.players.clear();
                self.players.push(latest);
            }
        } else if self.players.len() <= 1 {
            self.players.retain(|player| !player.is_finished());
        }
    }

    pub fn apply(&self, runtime: &mut ModelRuntime) {
        let mut values = expression_parameter_values(&self.players, runtime);
        if values.is_empty() {
            return;
        }

        let mut expression_weight = 0.0_f32;
        for (expression_index, player) in self.players.iter().enumerate() {
            let fade_weight = player.fade_weight();
            calculate_expression_values(
                runtime,
                player,
                expression_index,
                fade_weight,
                &mut values,
            );
            expression_weight += player.fade_in_weight();
        }
        expression_weight = expression_weight.min(1.0);

        for value in values {
            let target = (value.overwrite + value.additive) * value.multiply;
            let Some(current) = runtime.parameter_value_by_index(value.parameter_index) else {
                continue;
            };
            runtime.set_parameter_by_index(
                value.parameter_index,
                current + (target - current) * expression_weight,
            );
        }
    }

    pub fn active_expression_count(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }
}

#[derive(Debug, Clone)]
struct ExpressionParameterValue {
    id: String,
    parameter_index: usize,
    additive: f32,
    multiply: f32,
    overwrite: f32,
}

fn expression_parameter_values(
    players: &[ExpressionPlayer],
    runtime: &ModelRuntime,
) -> Vec<ExpressionParameterValue> {
    let mut values = Vec::new();
    for parameter in players
        .iter()
        .flat_map(|player| player.expression().parameters())
    {
        if values
            .iter()
            .any(|value: &ExpressionParameterValue| value.id == parameter.id())
        {
            continue;
        }
        let Some(parameter_index) = runtime.parameter_index(parameter.id()) else {
            continue;
        };
        let Some(overwrite) = runtime.parameter_value_by_index(parameter_index) else {
            continue;
        };
        values.push(ExpressionParameterValue {
            id: parameter.id().to_owned(),
            parameter_index,
            additive: DEFAULT_ADDITIVE_VALUE,
            multiply: DEFAULT_MULTIPLY_VALUE,
            overwrite,
        });
    }
    values
}

fn calculate_expression_values(
    runtime: &ModelRuntime,
    player: &ExpressionPlayer,
    expression_index: usize,
    fade_weight: f32,
    values: &mut [ExpressionParameterValue],
) {
    for value in values {
        let current = runtime
            .parameter_value_by_index(value.parameter_index)
            .unwrap_or(value.overwrite);
        let parameter = player
            .expression()
            .parameters()
            .iter()
            .find(|parameter| parameter.id() == value.id);

        let (new_additive, new_multiply, new_overwrite) = match parameter {
            Some(parameter) => expression_blend_values(parameter, current),
            None => (DEFAULT_ADDITIVE_VALUE, DEFAULT_MULTIPLY_VALUE, current),
        };

        if expression_index == 0 {
            value.additive = new_additive;
            value.multiply = new_multiply;
            value.overwrite = new_overwrite;
        } else {
            value.additive = interpolate(value.additive, new_additive, fade_weight);
            value.multiply = interpolate(value.multiply, new_multiply, fade_weight);
            value.overwrite = interpolate(value.overwrite, new_overwrite, fade_weight);
        }
    }
}

fn expression_blend_values(parameter: &ExpressionParameter, current: f32) -> (f32, f32, f32) {
    match parameter.blend() {
        ExpressionBlend::Add => (parameter.value(), DEFAULT_MULTIPLY_VALUE, current),
        ExpressionBlend::Multiply => (DEFAULT_ADDITIVE_VALUE, parameter.value(), current),
        ExpressionBlend::Overwrite => (
            DEFAULT_ADDITIVE_VALUE,
            DEFAULT_MULTIPLY_VALUE,
            parameter.value(),
        ),
    }
}

fn interpolate(source: f32, destination: f32, weight: f32) -> f32 {
    (source * (1.0 - weight)) + (destination * weight)
}

pub fn load_expression(path: impl AsRef<Path>) -> Result<Expression3, ExpressionLoadError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| ExpressionLoadError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Expression3::from_json_str(&source).map_err(ExpressionLoadError::Parse)
}

#[derive(Debug)]
pub enum ExpressionLoadError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Parse(crate::Error),
}

impl std::fmt::Display for ExpressionLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "failed to read {path}: {source}"),
            Self::Parse(error) => write!(formatter, "failed to parse exp3: {error}"),
        }
    }
}

impl std::error::Error for ExpressionLoadError {}
