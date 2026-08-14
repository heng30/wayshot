//! highlight, ported from effects/effect_highlight.py.

use std::collections::HashMap;

use crate::effects::common::parse_color;
use crate::effects::common::{
    parse_character_group, parse_gradient_direction, parse_gradient_steps,
};
use crate::engine::animation::{Animation, ExistingColorHandling, VisualParams};
use crate::engine::character::CharId;
use crate::engine::ctx::{EffectHooks, EngineCtx};
use crate::engine::effect::Effect;
use crate::engine::error::EngineError;
use crate::engine::events::EffectCallback;
use crate::engine::terminal::{CharacterFilter, CharacterGroup, CharacterSort};
use crate::utils::easing::{Easing, SequenceEaser};
use crate::utils::graphics::{Color, ColorPair, Gradient, GradientDirection};

#[derive(Debug, Clone)]
pub struct HighlightConfig {
    /// Brightness of the highlight color.
    pub highlight_brightness: f64,
    /// Direction the highlight will travel.
    pub highlight_direction: CharacterGroup,
    /// Width of the highlight. n >= 1
    pub highlight_width: i64,
    /// Space separated, unquoted, list of colors for the final color gradient.
    pub final_gradient_stops: Vec<Color>,
    /// Number of gradient steps to use.
    pub final_gradient_steps: Vec<i64>,
    /// Direction of the final gradient.
    pub final_gradient_direction: GradientDirection,
}

impl Default for HighlightConfig {
    fn default() -> Self {
        Self {
            highlight_brightness: 1.75,
            highlight_direction: parse_character_group("diagonal_bottom_left_to_top_right")
                .unwrap(),
            highlight_width: 8,
            final_gradient_stops: vec![
                parse_color("8A008A").unwrap(),
                parse_color("00D1FF").unwrap(),
                parse_color("FFFFFF").unwrap(),
            ],
            final_gradient_steps: vec![parse_gradient_steps("12").unwrap()],
            final_gradient_direction: parse_gradient_direction("vertical").unwrap(),
        }
    }
}

pub struct Highlight {
    config: HighlightConfig,
    #[allow(dead_code)] // upstream stores this map; nothing reads it (faithful)
    character_final_color_map: HashMap<CharId, Option<Color>>,
    easer: Option<SequenceEaser<Vec<CharId>>>,
}

impl Highlight {
    pub fn new(config: HighlightConfig) -> Self {
        Highlight {
            config,
            character_final_color_map: HashMap::new(),
            easer: None,
        }
    }
}

impl EffectHooks for Highlight {
    fn dispatch_callback(
        &mut self,
        _ctx: &mut EngineCtx,
        _character: CharId,
        _callback: &EffectCallback,
    ) {
    }
}

impl Effect for Highlight {
    fn build(&mut self, ctx: &mut EngineCtx) -> Result<(), EngineError> {
        let groups = ctx
            .terminal
            .get_characters_grouped(CharacterFilter::default(), self.config.highlight_direction);
        self.easer = Some(SequenceEaser::new(groups, Easing::InOutCirc, 100));

        let final_gradient = Gradient::new(
            &self.config.final_gradient_stops,
            &self.config.final_gradient_steps,
            false,
            false,
        )
        .map_err(EngineError::Other)?;
        let final_gradient_mapping = final_gradient
            .build_coordinate_color_mapping(
                ctx.terminal.canvas.text_bottom,
                ctx.terminal.canvas.text_top,
                ctx.terminal.canvas.text_left,
                ctx.terminal.canvas.text_right,
                self.config.final_gradient_direction,
            )
            .map_err(EngineError::Other)?;

        let dynamic = ctx.terminal.config.existing_color_handling == ExistingColorHandling::Dynamic;
        let characters = {
            let filter = CharacterFilter::default();
            ctx.terminal
                .get_characters(&mut ctx.rng, filter, CharacterSort::TopToBottomLeftToRight)
        };
        for id in characters {
            let (input_fg, input_bg, input_coord, input_symbol, uses_pre) = {
                let ch = &ctx.terminal.arena[id.0 as usize];
                (
                    ch.animation.input_fg_color.clone(),
                    ch.animation.input_bg_color.clone(),
                    ch.input_coord,
                    ch.input_symbol.clone(),
                    ch.uses_input_preexisting_colors,
                )
            };
            let (base_color, input_bg_color) = if dynamic {
                (input_fg, input_bg)
            } else {
                (
                    Some(final_gradient_mapping.get(&input_coord).unwrap().clone()),
                    None,
                )
            };
            self.character_final_color_map
                .insert(id, base_color.clone());
            let base_colors = ColorPair::new(base_color.clone(), input_bg_color.clone());
            let highlight_gradient = match &base_color {
                Some(base) => {
                    let highlight_color =
                        Animation::adjust_color_brightness(base, self.config.highlight_brightness);
                    Some(
                        Gradient::new(
                            &[
                                base.clone(),
                                highlight_color.clone(),
                                highlight_color,
                                base.clone(),
                            ],
                            &[3, self.config.highlight_width, 3],
                            false,
                            false,
                        )
                        .map_err(EngineError::Other)?,
                    )
                }
                None => None,
            };
            {
                let ch = &mut ctx.terminal.arena[id.0 as usize];
                ch.animation.set_appearance(
                    &input_symbol,
                    uses_pre,
                    Some(&input_symbol),
                    Some(base_colors.clone()),
                );
                ch.animation
                    .new_scene(false, None, None, "highlight", uses_pre);
                let scene = ch.animation.scenes.get_mut("highlight").unwrap();
                if let Some(gradient) = &highlight_gradient {
                    for color in &gradient.spectrum {
                        scene
                            .add_frame(
                                &input_symbol,
                                2,
                                VisualParams {
                                    colors: Some(ColorPair::new(
                                        Some(color.clone()),
                                        input_bg_color.clone(),
                                    )),
                                    ..Default::default()
                                },
                            )
                            .map_err(EngineError::Other)?;
                    }
                } else {
                    scene
                        .add_frame(
                            &input_symbol,
                            2,
                            VisualParams {
                                colors: Some(base_colors.clone()),
                                ..Default::default()
                            },
                        )
                        .map_err(EngineError::Other)?;
                }
            }
            ctx.terminal.set_character_visibility(id, true);
        }
        Ok(())
    }

    fn next_frame(&mut self, ctx: &mut EngineCtx) -> Option<String> {
        let easer_complete = self.easer.as_ref().unwrap().is_complete();
        if !ctx.active_characters.is_empty() || !easer_complete {
            let mut easer = self.easer.take().unwrap();
            let step = easer.step();
            for group in step.added {
                for &id in group {
                    ctx.activate_scene(self, id, "highlight");
                    ctx.active_characters.insert(id);
                }
            }
            self.easer = Some(easer);
            ctx.update(self);
            return Some(ctx.frame());
        }
        None
    }
}
