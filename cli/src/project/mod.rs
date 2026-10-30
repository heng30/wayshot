use anyhow::Result;
use serde_json::Value;

mod args;
mod common;
mod export;
mod filters;
mod media;
mod meta;
mod subtitles;
mod text;

pub use args::Cmd;
use args::Preset;
pub(crate) use common::{refresh_track_duration, track_segments_mut};

fn run_voice_cut(cmd: &Cmd) -> Result<Value> {
    let Cmd::VoiceCut {
        project,
        track,
        segment,
        entries,
        asr_weights,
        asr_tokenizer,
        api_base,
        api_key,
        model,
        reference,
        vad_threshold,
        max_tokens,
        apply,
    } = cmd
    else {
        unreachable!()
    };

    let asr = match (asr_weights, asr_tokenizer) {
        (Some(weights), Some(tokenizer)) => Some((weights.as_path(), tokenizer.as_path())),
        (None, None) => None,
        _ => {
            return Err(anyhow::anyhow!(
                "--asr-weights 与 --asr-tokenizer 必须同时给出"
            ));
        }
    };

    let api = crate::llm::ApiConfig {
        base_url: api_base.clone().unwrap_or_default(),
        api_key: api_key.clone().unwrap_or_default(),
        model: model.clone().unwrap_or_default(),
        temperature: 0.0,
    };

    crate::voice_cut::run(
        project,
        *track,
        *segment,
        entries.as_deref(),
        asr,
        &api,
        reference.as_deref(),
        *vad_threshold,
        *max_tokens,
        *apply,
    )
}

fn run_smart_mix(cmd: &Cmd) -> Result<Value> {
    let Cmd::SmartMix {
        project,
        audio_track,
        visual_tracks,
        audio_entries,
        srt,
        asr_weights,
        asr_tokenizer,
        visuals,
        vision_model,
        vision_precision,
        api_base,
        api_key,
        model,
        reuse_media,
        sequential,
        cover_all,
        apply,
    } = cmd
    else {
        unreachable!()
    };

    let asr = match (asr_weights, asr_tokenizer) {
        (Some(weights), Some(tokenizer)) => Some((weights.as_path(), tokenizer.as_path())),
        (None, None) => None,
        _ => {
            return Err(anyhow::anyhow!(
                "--asr-weights 与 --asr-tokenizer 必须同时给出"
            ));
        }
    };

    let api = crate::llm::ApiConfig {
        base_url: api_base.clone().unwrap_or_default(),
        api_key: api_key.clone().unwrap_or_default(),
        model: model.clone().unwrap_or_default(),
        temperature: 0.0,
    };

    crate::smart_mix::run(
        project,
        *audio_track,
        visual_tracks.as_deref(),
        audio_entries.as_deref(),
        srt.as_deref(),
        asr,
        visuals.as_deref(),
        vision_model
            .as_deref()
            .map(|dir| (dir, vision_precision.to_precision())),
        &api,
        *reuse_media,
        *sequential,
        *cover_all,
        *apply,
    )
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Info { .. }
        | Cmd::Create { .. }
        | Cmd::AddMedia { .. }
        | Cmd::Segments { .. }
        | Cmd::Trim { .. }
        | Cmd::Move { .. }
        | Cmd::Remove { .. } => media::run(cmd),
        Cmd::Filters { .. }
        | Cmd::FilterAdd { .. }
        | Cmd::FilterRemove { .. }
        | Cmd::FilterCatalog
        | Cmd::GlobalFilters { .. }
        | Cmd::GlobalFilterAdd { .. }
        | Cmd::GlobalFilterRemove { .. } => filters::run(cmd),
        Cmd::Memo { .. }
        | Cmd::Bookmarks { .. }
        | Cmd::BookmarkAdd { .. }
        | Cmd::BookmarkRemove { .. } => meta::run(cmd),
        Cmd::Texts { .. } | Cmd::TextAdd { .. } | Cmd::TextRemove { .. } => text::run(cmd),
        Cmd::Subtitles { .. } | Cmd::SubtitleSearch { .. } | Cmd::ChapterSummary { .. } => {
            subtitles::run(cmd)
        }
        Cmd::VoiceCut { .. } => run_voice_cut(cmd),
        Cmd::SmartMix { .. } => run_smart_mix(cmd),
        Cmd::Export { .. } => export::run(cmd),
    }
}
