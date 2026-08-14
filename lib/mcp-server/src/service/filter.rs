use crate::{
    error::McpError,
    state::{self, UiAction},
    types::FilterInfo,
};
use video_editor::commands::filter::AddFilterCommand;

/// Parse filter type string
pub fn parse_filter_type(filter_type: &str) -> Result<String, McpError> {
    match filter_type {
        "video" | "audio" | "subtitle" | "image" => Ok(filter_type.to_string()),
        _ => Err(McpError::InvalidParameter(format!(
            "Invalid filter type: {filter_type}"
        ))),
    }
}

/// List filters on a segment
pub fn list_segment_filters(
    track_index: usize,
    segment_index: usize,
) -> Result<Vec<FilterInfo>, McpError> {
    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    let segment = manager
        .get(track_index)
        .and_then(|t| t.segments().get(segment_index))
        .ok_or(McpError::InvalidSegmentIndex {
            track: track_index,
            segment: segment_index,
        })?;

    let mut filters = Vec::new();
    let mut idx = 0;

    for f in &segment.video_filters {
        filters.push(FilterInfo {
            index: idx,
            filter_type: "video".to_string(),
            name: f.inner.name().to_string(),
            enabled: f.enabled(),
            detail: f.inner.name().to_string(),
        });
        idx += 1;
    }
    for f in &segment.audio_filters {
        filters.push(FilterInfo {
            index: idx,
            filter_type: "audio".to_string(),
            name: f.inner.name().to_string(),
            enabled: f.enabled(),
            detail: f.inner.name().to_string(),
        });
        idx += 1;
    }
    for f in &segment.subtitle_filters {
        filters.push(FilterInfo {
            index: idx,
            filter_type: "subtitle".to_string(),
            name: f.inner.name().to_string(),
            enabled: f.enabled(),
            detail: f.inner.name().to_string(),
        });
        idx += 1;
    }
    for f in &segment.image_filters {
        filters.push(FilterInfo {
            index: idx,
            filter_type: "image".to_string(),
            name: f.inner.name().to_string(),
            enabled: f.enabled(),
            detail: f.inner.name().to_string(),
        });
        idx += 1;
    }

    Ok(filters)
}

/// Remove all filters from a segment — dispatches to UI Logic callback
pub fn remove_filter(
    track_index: usize,
    segment_index: usize,
    _filter_type: &str,
    _filter_index: usize,
) -> Result<(), McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    // Remove all filters from the specified segment
    state::dispatch_action(UiAction::RemoveAllFiltersFromSegment {
        track_index,
        segment_index,
    });
    Ok(())
}

/// Toggle a filter enabled/disabled — reads current state, toggles, dispatches
/// Note: Individual filter toggle via MCP is not fully supported because the UI
/// callback `segment-remove-all-filters` removes ALL filters. For individual toggle,
/// we would need a per-filter callback. For now, this is a placeholder that returns
/// the current state.
pub fn toggle_filter(
    track_index: usize,
    segment_index: usize,
    filter_type: &str,
    _filter_index: usize,
) -> Result<bool, McpError> {
    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    let segment = manager
        .get(track_index)
        .and_then(|t| t.segments().get(segment_index))
        .ok_or(McpError::InvalidSegmentIndex {
            track: track_index,
            segment: segment_index,
        })?;

    // Find the filter and return its current enabled state
    let is_enabled = match filter_type {
        "video" => segment
            .video_filters
            .first()
            .map(|f| f.enabled())
            .unwrap_or(false),
        "audio" => segment
            .audio_filters
            .first()
            .map(|f| f.enabled())
            .unwrap_or(false),
        "subtitle" => segment
            .subtitle_filters
            .first()
            .map(|f| f.enabled())
            .unwrap_or(false),
        "image" => segment
            .image_filters
            .first()
            .map(|f| f.enabled())
            .unwrap_or(false),
        _ => false,
    };
    // Individual filter toggle is not yet supported through Logic callbacks
    log::info!(
        "MCP: toggle_filter({track_index}, {segment_index}, {filter_type}) — individual toggle not yet supported, current state: {is_enabled}"
    );
    Ok(is_enabled)
}

/// Clear all filters of a given type from a segment — dispatches to UI Logic callback
pub fn clear_filters(
    track_index: usize,
    _segment_index: usize,
    _filter_type: &str,
) -> Result<(), McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    // Remove all filters from the segment (per-type removal not available in Logic callbacks)
    state::dispatch_action(UiAction::RemoveAllFiltersFromSegment {
        track_index,
        segment_index: _segment_index,
    });
    log::info!("MCP: clear_filters({track_index}) — dispatched RemoveAllFiltersFromSegment");
    Ok(())
}

/// Add a filter to a segment by name.
/// Creates a default instance of the filter and adds it using AddFilterCommand.
pub fn add_filter(
    track_index: usize,
    segment_index: usize,
    filter_type: &str,
    filter_name: &str,
) -> Result<serde_json::Value, McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;

    let cmd = match filter_type {
        "video" => {
            let filter = create_video_filter_by_name(filter_name)
                .ok_or_else(|| McpError::InvalidParameter(
                    format!("Unknown video filter: '{}'. Available: grayscale, opacity, vignette, sharpen, blur, flip, crop, border, etc.", filter_name)
                ))?;
            AddFilterCommand::new_video(track_index, segment_index, filter)
        }
        "audio" => {
            let filter = create_audio_filter_by_name(filter_name)
                .ok_or_else(|| McpError::InvalidParameter(
                    format!("Unknown audio filter: '{}'. Available: gain, normalize, limiter, noise_gate, compressor, denoise, mute", filter_name)
                ))?;
            AddFilterCommand::new_audio(track_index, segment_index, filter)
        }
        _ => {
            return Err(McpError::InvalidParameter(format!(
                "Invalid filter_type: '{}'. Must be 'video' or 'audio'.",
                filter_type
            )));
        }
    };

    let affected = state::execute_command(Box::new(cmd))
        .map_err(|e| McpError::Internal(format!("AddFilterCommand failed: {}", e)))?;

    state::sync_ui();

    Ok(serde_json::json!({
        "success": true,
        "track_index": track_index,
        "segment_index": segment_index,
        "filter_type": filter_type,
        "filter_name": filter_name,
        "affected_segments": affected.segments.len(),
    }))
}

/// Create a default video filter by name
fn create_video_filter_by_name(
    name: &str,
) -> Option<Box<dyn video_editor::filters::traits::VideoFilter>> {
    use video_editor::filters::video::*;
    match name {
        "grayscale" => Some(Box::new(GrayscaleFilter::default())),
        "opacity" => Some(Box::new(OpacityFilter::default())),
        "vignette" => Some(Box::new(VignetteFilter::default())),
        "sharpen" => Some(Box::new(SharpenFilter::default())),
        "blur" | "gaussian_blur" => Some(Box::new(GaussianBlurFilter::default())),
        "flip" => Some(Box::new(FlipFilter::default())),
        "crop" => Some(Box::new(CropFilter::default())),
        "border" => Some(Box::new(BorderFilter::default())),
        "zoom" => Some(Box::new(ZoomFilter::default())),
        "sketch" => Some(Box::new(SketchFilter::default())),
        "grain" => Some(Box::new(GrainFilter::default())),
        "focus" => Some(Box::new(FocusFilter::default())),
        "edge_detect" => Some(Box::new(EdgeDetectFilter::default())),
        "split" => Some(Box::new(SplitFilter::default())),
        "wave" => Some(Box::new(WaveFilter::default())),
        "old_film" => Some(Box::new(OldFilmFilter::default())),
        "fisheye" => Some(Box::new(FisheyeFilter::default())),
        "hsl" | "hsl_adjust" => Some(Box::new(HSLAdjustFilter::default())),
        "shadow" => Some(Box::new(ShadowFilter::default())),
        "mosaic" => Some(Box::new(MosaicFilter::default())),
        "liquid_glass" => Some(Box::new(LiquidGlassFilter::default())),
        "text_highlight" => Some(Box::new(TextHighlightFilter::default())),
        "background" => Some(Box::new(BackgroundFilter::default())),
        "fade_in" | "image_fade_in" => Some(Box::new(FadeInFilter::default())),
        "fade_out" | "image_fade_out" => Some(Box::new(FadeOutFilter::default())),
        "linear_mask" => Some(Box::new(LinearMaskFilter::default())),
        "circle_mask" => Some(Box::new(CircleMaskFilter::default())),
        "mirror_mask" => Some(Box::new(MirrorMaskFilter::default())),
        "directional_blur" => Some(Box::new(DirectionalBlurFilter::default())),
        "draw_rectangle" => Some(Box::new(DrawRectangleFilter::default())),
        "draw_circle" => Some(Box::new(DrawCircleFilter::default())),
        "frame_extract" => Some(Box::new(FrameExtractFilter::default())),
        "wind_scatter" => Some(Box::new(WindScatterFilter::default())),
        _ => None,
    }
}

/// Create a default audio filter by name
fn create_audio_filter_by_name(
    name: &str,
) -> Option<Box<dyn video_editor::filters::traits::AudioFilter>> {
    use video_editor::filters::audio::*;
    match name {
        "gain" => Some(Box::new(GainFilter::default())),
        "normalize" => Some(Box::new(NormalizeFilter::default())),
        "limiter" => Some(Box::new(LimiterFilter::default())),
        "noise_gate" => Some(Box::new(NoiseGateFilter::default())),
        "compressor" => Some(Box::new(CompressorFilter::default())),
        "denoise" => Some(Box::new(DenoiseFilter::default())),
        "mute" => Some(Box::new(MuteFilter::default())),
        _ => None,
    }
}
