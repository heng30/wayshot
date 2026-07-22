#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelUpdateStep {
    PreUpdateDynamicFlags,
    UpdateParameters,
    UpdateParameterBindings,
    UpdateBlendShapeParameterBindings,
    UpdateKeyformBindings,
    UpdateBlendShapeKeyformBindings,
    ClampBlendShapeWeights,
    UpdatePartsHierarchy,
    UpdatePartKeyformCaches,
    InterpolateParts,
    UpdateDeformerHierarchy,
    UpdateWarpDeformerKeyformCaches,
    UpdateRotationDeformerKeyformCaches,
    InterpolateWarpDeformers,
    InterpolateRotationDeformers,
    UpdateArtMeshHierarchy,
    UpdateArtMeshKeyformCaches,
    InterpolateArtMeshes,
    UpdateGlueKeyformCaches,
    InterpolateGlues,
    UpdateOffscreenRenderingHierarchy,
    UpdateOffscreenRenderingKeyformCaches,
    InterpolateOffscreenRendering,
    BlendParts,
    BlendWarpDeformers,
    BlendRotationDeformers,
    BlendArtMeshes,
    BlendGlues,
    BlendOffscreenRendering,
    TransformDeformers,
    DeformerTransformArtMeshes,
    TransformParts,
    PartTransformArtMeshes,
    AffectArtMeshes,
    ReverseCoordinate,
    CalculateRenderOrder,
    PostUpdateDynamicFlags,
}

const SEMANTIC_MODEL_UPDATE_ORDER: &[ModelUpdateStep] = &[
    ModelUpdateStep::PreUpdateDynamicFlags,
    ModelUpdateStep::UpdateParameters,
    ModelUpdateStep::UpdateParameterBindings,
    ModelUpdateStep::UpdateBlendShapeParameterBindings,
    ModelUpdateStep::UpdateKeyformBindings,
    ModelUpdateStep::UpdateBlendShapeKeyformBindings,
    ModelUpdateStep::ClampBlendShapeWeights,
    ModelUpdateStep::UpdatePartsHierarchy,
    ModelUpdateStep::UpdatePartKeyformCaches,
    ModelUpdateStep::InterpolateParts,
    ModelUpdateStep::UpdateDeformerHierarchy,
    ModelUpdateStep::UpdateWarpDeformerKeyformCaches,
    ModelUpdateStep::UpdateRotationDeformerKeyformCaches,
    ModelUpdateStep::InterpolateWarpDeformers,
    ModelUpdateStep::InterpolateRotationDeformers,
    ModelUpdateStep::UpdateArtMeshHierarchy,
    ModelUpdateStep::UpdateArtMeshKeyformCaches,
    ModelUpdateStep::InterpolateArtMeshes,
    ModelUpdateStep::UpdateGlueKeyformCaches,
    ModelUpdateStep::InterpolateGlues,
    ModelUpdateStep::UpdateOffscreenRenderingHierarchy,
    ModelUpdateStep::UpdateOffscreenRenderingKeyformCaches,
    ModelUpdateStep::InterpolateOffscreenRendering,
    ModelUpdateStep::BlendParts,
    ModelUpdateStep::BlendWarpDeformers,
    ModelUpdateStep::BlendRotationDeformers,
    ModelUpdateStep::BlendArtMeshes,
    ModelUpdateStep::BlendGlues,
    ModelUpdateStep::BlendOffscreenRendering,
    ModelUpdateStep::TransformDeformers,
    ModelUpdateStep::DeformerTransformArtMeshes,
    ModelUpdateStep::TransformParts,
    ModelUpdateStep::PartTransformArtMeshes,
    ModelUpdateStep::AffectArtMeshes,
    ModelUpdateStep::ReverseCoordinate,
    ModelUpdateStep::CalculateRenderOrder,
    ModelUpdateStep::PostUpdateDynamicFlags,
];

pub fn semantic_model_update_order() -> &'static [ModelUpdateStep] {
    SEMANTIC_MODEL_UPDATE_ORDER
}

pub fn should_affect_glues(glue_count: u32) -> bool {
    glue_count > 0
}

pub fn should_blend_glues(moc_version: u32) -> bool {
    moc_version > 4
}

pub fn should_run_offscreen_stage(moc_version: u32) -> bool {
    moc_version > 5
}
