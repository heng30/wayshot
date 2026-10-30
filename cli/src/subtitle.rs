use anyhow::{Context, Result, anyhow};
use clap::Subcommand;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use video_utils::subtitle::{
    Subtitle, ms_to_srt_timestamp, parse_lrc_file, save_as_srt, split_subtitle, srt_timestamp_to_ms,
};

#[derive(Subcommand)]
pub enum Cmd {
    /// 查看字幕文件信息（条数、时长）
    Info { input: PathBuf },

    /// 转成 SRT（支持 .lrc / .srt）
    ToSrt { input: PathBuf, output: PathBuf },

    /// 用 LLM 翻译字幕（OpenAI 兼容接口）
    Translate {
        input: PathBuf,
        output: PathBuf,
        #[arg(long)]
        api_base: String,
        #[arg(long)]
        api_key: String,
        #[arg(long)]
        model: String,
        /// 翻译要求，例如「翻译成简体中文」
        #[arg(
            long,
            default_value = "Translate the subtitles into Simplified Chinese"
        )]
        prompt: String,
        /// 每批发送多少条字幕
        #[arg(long, default_value_t = 40)]
        batch_size: usize,
        #[arg(long, default_value_t = 0.3)]
        temperature: f32,
    },

    /// 把过长的字幕行按标点自动对半拆分成两条
    Split {
        input: PathBuf,
        output: PathBuf,
        /// 超过多少字符才拆分
        #[arg(long, default_value_t = 18)]
        max_chars: usize,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Info { input } => info(input),
        Cmd::ToSrt { input, output } => to_srt(input, output),
        Cmd::Split {
            input,
            output,
            max_chars,
        } => split(input, output, *max_chars),
        Cmd::Translate {
            input,
            output,
            api_base,
            api_key,
            model,
            prompt,
            batch_size,
            temperature,
        } => translate(
            input,
            output,
            &crate::llm::ApiConfig {
                base_url: api_base.clone(),
                api_key: api_key.clone(),
                model: model.clone(),
                temperature: *temperature,
            },
            prompt,
            *batch_size,
        ),
    }
}

fn translate(
    input: &Path,
    output: &Path,
    api: &crate::llm::ApiConfig,
    instruction: &str,
    batch_size: usize,
) -> Result<Value> {
    let items = load(input)?;
    let batch_size = batch_size.max(1);

    let system = format!(
        r#"You are a subtitle translation assistant. {instruction}

<Input format>
[{{"index": 0, "text": "original text"}}, ...]
</Input format>

<Output format>
[{{"index": 0, "translation": "translated text"}}, ...]
</Output format>

Rules:
- Translate each subtitle text as instructed
- Keep the translation natural and concise
- Maintain the same index for each entry
- Only output the JSON array, no additional text
"#
    );

    let mut translated = Vec::with_capacity(items.len());
    for chunk in items.chunks(batch_size) {
        let request: Vec<Value> = chunk
            .iter()
            .enumerate()
            .map(|(i, item)| json!({ "index": i, "text": item.text }))
            .collect();

        let response = crate::llm::chat(&system, &serde_json::to_string(&request)?, api)?;
        let parsed = parse_translation_response(&response)?;

        for (i, item) in chunk.iter().enumerate() {
            let text = parsed
                .get(&i)
                .cloned()
                .ok_or_else(|| anyhow!("模型返回缺少第 {i} 条的译文"))?;
            translated.push(Subtitle {
                index: 0,
                start_timestamp: item.start_timestamp,
                end_timestamp: item.end_timestamp,
                text,
            });
        }
    }

    for (i, item) in translated.iter_mut().enumerate() {
        item.index = i as u32 + 1;
    }

    save_as_srt(&translated, output).map_err(|e| anyhow!("写入 SRT 失败: {e}"))?;

    Ok(json!({
        "path": output.display().to_string(),
        "count": translated.len(),
    }))
}

fn parse_translation_response(response: &str) -> Result<std::collections::HashMap<usize, String>> {
    let trimmed = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let items: Vec<Value> = serde_json::from_str(trimmed)
        .map_err(|e| anyhow!("解析模型返回的 JSON 失败: {e}。原始返回: {trimmed}"))?;

    let mut map = std::collections::HashMap::with_capacity(items.len());
    for item in items {
        let index = item["index"]
            .as_u64()
            .ok_or_else(|| anyhow!("译文缺少 index"))? as usize;
        let translation = item["translation"]
            .as_str()
            .ok_or_else(|| anyhow!("译文缺少 translation"))?;
        map.insert(index, translation.to_string());
    }

    Ok(map)
}

fn load(path: &Path) -> Result<Vec<Subtitle>> {
    let is_lrc = path.extension().and_then(|e| e.to_str()) == Some("lrc");
    if is_lrc {
        return parse_lrc_file(path).map_err(|e| anyhow!("解析 LRC 失败: {e}"));
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("无法读取字幕文件: {}", path.display()))?;
    parse_srt(&content).with_context(|| format!("解析 SRT 失败: {}", path.display()))
}

fn parse_srt(content: &str) -> Result<Vec<Subtitle>> {
    let normalized = content.replace("\r\n", "\n");
    let mut items = Vec::new();

    for block in normalized.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        let mut lines = block.lines();
        let mut timeline = lines.next().unwrap_or("").trim();
        if timeline.parse::<u32>().is_ok() {
            timeline = lines.next().unwrap_or("").trim();
        }

        let (start, end) = timeline
            .split_once("-->")
            .ok_or_else(|| anyhow!("时间轴格式错误: {timeline}"))?;
        let start_timestamp =
            srt_timestamp_to_ms(start.trim()).map_err(|e| anyhow!("开始时间错误: {e}"))?;
        let end_timestamp =
            srt_timestamp_to_ms(end.trim()).map_err(|e| anyhow!("结束时间错误: {e}"))?;

        items.push(Subtitle {
            index: items.len() as u32 + 1,
            start_timestamp,
            end_timestamp,
            text: lines.collect::<Vec<_>>().join("\n"),
        });
    }

    if items.is_empty() {
        return Err(anyhow!("没有解析到任何字幕条目"));
    }

    Ok(items)
}

fn info(input: &Path) -> Result<Value> {
    let items = load(input)?;
    let first = items.first().map(|s| s.start_timestamp).unwrap_or(0);
    let last = items.last().map(|s| s.end_timestamp).unwrap_or(first);

    Ok(json!({
        "path": input.display().to_string(),
        "count": items.len(),
        "start": ms_to_srt_timestamp(first),
        "end": ms_to_srt_timestamp(last),
        "duration_secs": (last.saturating_sub(first)) as f64 / 1000.0,
    }))
}

fn to_srt(input: &Path, output: &Path) -> Result<Value> {
    let items = load(input)?;
    save_as_srt(&items, output).map_err(|e| anyhow!("写入 SRT 失败: {e}"))?;

    Ok(json!({
        "path": output.display().to_string(),
        "count": items.len(),
    }))
}

fn split(input: &Path, output: &Path, max_chars: usize) -> Result<Value> {
    let items = load(input)?;
    let before = items.len();
    let mut result: Vec<Subtitle> = Vec::with_capacity(before);

    for item in &items {
        let mut queue = vec![(item.start_timestamp, item.end_timestamp, item.text.clone())];

        while let Some((start, end, text)) = queue.pop() {
            let too_long = text.chars().count() > max_chars;
            // split_subtitle 对单个字符会返回「空 + 原文」，用它做进展判据避免死循环。
            match split_subtitle(start, end, &text) {
                Some((first, second))
                    if too_long
                        && first.2 != text
                        && second.2 != text
                        && !first.2.is_empty()
                        && !second.2.is_empty() =>
                {
                    queue.push(second);
                    queue.push(first);
                }
                _ => result.push(Subtitle {
                    index: 0,
                    start_timestamp: start,
                    end_timestamp: end,
                    text,
                }),
            }
        }
    }

    result.sort_by_key(|s| s.start_timestamp);
    for (i, item) in result.iter_mut().enumerate() {
        item.index = i as u32 + 1;
    }

    save_as_srt(&result, output).map_err(|e| anyhow!("写入 SRT 失败: {e}"))?;

    Ok(json!({
        "path": output.display().to_string(),
        "before": before,
        "after": result.len(),
    }))
}
