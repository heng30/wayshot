use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use super::{AudioEntry, Match, VisualItem};

#[allow(clippy::too_many_arguments)]
pub(super) fn ai_match(
    audio: &[AudioEntry],
    visuals: &[VisualItem],
    api: &crate::llm::ApiConfig,
    reuse_media: bool,
    sequential: bool,
    cover_all: bool,
) -> Result<Vec<Match>> {
    let prompt = match_prompt(reuse_media, sequential, cover_all);

    let input = json!({
        "audio_segments": audio.iter().enumerate().map(|(index, entry)| json!({
            "index": index,
            "start_ms": entry.start_ms,
            "end_ms": entry.end_ms,
            "text": entry.text,
        })).collect::<Vec<_>>(),
        "visual_segments": visuals.iter().enumerate().map(|(index, item)| json!({
            "index": index,
            "description": item.description,
            "duration_ms": item.duration_ms,
            "is_image": item.is_image,
            "original_duration_ms": item.original_duration_ms,
        })).collect::<Vec<_>>(),
    });

    let response = crate::llm::chat(&prompt, &serde_json::to_string(&input)?, api)?;
    parse_matches(&response, audio, visuals)
}
pub(super) fn match_prompt(reuse_media: bool, sequential: bool, cover_all: bool) -> String {
    let multi_visual_rule = if reuse_media {
        "5. One audio segment can match multiple visual segments."
    } else {
        "5. One audio segment can match multiple DIFFERENT visual segments, but each visual must be distinct; a single visual can cover multiple consecutive audio segments as one continuous block."
    };
    let reuse_rule = if reuse_media {
        "6. A visual segment CAN be reused by multiple audio segments at different time offsets."
    } else if cover_all {
        "6. NO REUSE: each visual_index appears ONCE; every audio segment must be covered."
    } else {
        "6. NO REUSE: each visual_index appears ONCE as a single continuous block; unmatched audio is allowed."
    };
    let passage_rule = "9. Consecutive audio segments about the same topic should use the SAME visual as one continuous block.";
    let sequential_rule = if sequential {
        "10. SEQUENTIAL MATCH: assign visuals to audio segments in order."
    } else {
        "10. Visual segments can be matched to audio segments in any order based on semantic relevance."
    };
    let cover_rule = if cover_all {
        "11. MUST COVER ALL AUDIO: every audio segment must be matched."
    } else {
        "11. Audio segments without a good visual match can be left unmatched."
    };

    format!(
        r#"You are a smart video editing assistant. Given transcribed audio segments and visual (image/video) descriptions, create the best matching plan to produce a mixed video.

Rules:
1. Match audio content with the most semantically relevant visual content.
2. You do NOT need to match every visual segment. Only use visuals that are semantically relevant to the audio.
3. Video segments have a maximum duration limited by their original_duration_ms. source_offset_ms + duration_ms must not exceed original_duration_ms.
4. Image segments can be stretched to any duration.
{multi_visual_rule}
{reuse_rule}
7. The total matched duration for each audio segment should approximately equal (end_ms - start_ms).
8. Sort results by audio_index, then by source_offset_ms.
{passage_rule}
{sequential_rule}
{cover_rule}

Only output the JSON array, no additional text.

<Input format>
{{"audio_segments": [{{"index": 0, "start_ms": 0, "end_ms": 5000, "text": "..."}}], "visual_segments": [{{"index": 0, "description": "...", "duration_ms": 10000, "is_image": true, "original_duration_ms": 10000}}]}}
</Input format>

<Output format>
[{{"audio_index": 0, "visual_index": 0, "source_offset_ms": 0, "duration_ms": 5000}}]
</Output format>
"#
    )
}
pub(super) fn parse_matches(
    response: &str,
    audio: &[AudioEntry],
    visuals: &[VisualItem],
) -> Result<Vec<Match>> {
    let trimmed = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let parsed: Value = serde_json::from_str(trimmed)
        .map_err(|e| anyhow!("解析匹配结果失败: {e}。原始返回: {trimmed}"))?;
    let items = parsed
        .as_array()
        .ok_or_else(|| anyhow!("模型返回不是数组"))?;

    let mut matches = Vec::with_capacity(items.len());
    for item in items {
        matches.push(parse_one_match(item, audio, visuals)?);
    }

    matches.sort_by_key(|m| (m.audio_index, m.source_offset_ms));
    Ok(matches)
}
pub(super) fn parse_one_match(
    item: &Value,
    audio: &[AudioEntry],
    visuals: &[VisualItem],
) -> Result<Match> {
    let audio_index = item["audio_index"]
        .as_u64()
        .ok_or_else(|| anyhow!("匹配项缺少 audio_index"))? as usize;
    let visual_index = item["visual_index"]
        .as_u64()
        .ok_or_else(|| anyhow!("匹配项缺少 visual_index"))? as usize;
    if audio_index >= audio.len() {
        return Err(anyhow!("audio_index 越界: {audio_index}"));
    }
    if visual_index >= visuals.len() {
        return Err(anyhow!("visual_index 越界: {visual_index}"));
    }

    let source_offset_ms = item["source_offset_ms"].as_u64().unwrap_or(0);
    let duration_ms = item["duration_ms"]
        .as_u64()
        .ok_or_else(|| anyhow!("匹配项缺少 duration_ms"))?;

    Ok(Match {
        audio_index,
        visual_index,
        source_offset_ms,
        duration_ms: clamp_duration(
            duration_ms,
            source_offset_ms,
            audio_index,
            visual_index,
            audio,
            visuals,
        ),
    })
}
pub(super) fn clamp_duration(
    duration_ms: u64,
    source_offset_ms: u64,
    audio_index: usize,
    visual_index: usize,
    audio: &[AudioEntry],
    visuals: &[VisualItem],
) -> u64 {
    let audio_len = audio[audio_index]
        .end_ms
        .saturating_sub(audio[audio_index].start_ms);
    let mut duration_ms = duration_ms;
    if duration_ms == 0 || duration_ms > audio_len.max(1) * 4 {
        duration_ms = audio_len;
    }

    let visual_limit = visuals[visual_index].original_duration_ms;
    if !visuals[visual_index].is_image && visual_limit > 0 {
        let max_duration = visual_limit.saturating_sub(source_offset_ms).max(1);
        duration_ms = duration_ms.min(max_duration);
    }
    duration_ms
}
