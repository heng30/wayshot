mod cdi3;
mod expression3;
mod model3;
mod motion3;
mod physics3;
mod pose3;

pub use cdi3::{Cdi3, CdiEntry, CdiPart};
pub use expression3::{
    DEFAULT_EXPRESSION_FADE_IN_TIME, DEFAULT_EXPRESSION_FADE_OUT_TIME, Expression3,
    ExpressionBlend, ExpressionParameter, apply_expression_blend, apply_expression_parameter,
    resolved_expression_fade_in_time, resolved_expression_fade_out_time,
};
pub use model3::{ExpressionReference, Group, HitArea, Model3, MotionReference};
pub use motion3::{
    Motion3, MotionCurve, MotionMeta, MotionPoint, MotionSegment, apply_motion_fade, easing_sine,
    motion_fade_in_weight, motion_fade_out_weight, parameter_curve_fade_weight,
};
pub use physics3::{
    EffectiveForces, Physics3, PhysicsDictionaryEntry, PhysicsInput, PhysicsMeta,
    PhysicsNormalization, PhysicsNormalizationValue, PhysicsOutput, PhysicsSetting, PhysicsSource,
    PhysicsValueKind, PhysicsVertex, Vector2,
};
pub use pose3::{
    DEFAULT_POSE_FADE_IN_TIME, Pose3, PosePart, copy_pose_link_opacities,
    resolved_pose_fade_in_time, update_pose_group_opacities,
};
