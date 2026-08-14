//! Effect trait (base_effect.py equivalents).

use crate::engine::ctx::{EffectHooks, EngineCtx};
use crate::engine::error::EngineError;

/// One effect: build() once (upstream iterator __init__/build), then
/// next_frame() until None (upstream __next__/StopIteration). Every effect
/// also implements EffectHooks for its registered callbacks.
pub trait Effect: EffectHooks {
    fn build(&mut self, ctx: &mut EngineCtx) -> Result<(), EngineError>;
    fn next_frame(&mut self, ctx: &mut EngineCtx) -> Option<String>;
}
