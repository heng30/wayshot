use crate::{
    Error,
    proto::decode_dm_seg_mobile_reply,
    types::{DanmakuElem, VideoPage},
};
use std::time::Duration;

const REFERER: &str = "https://www.bilibili.com";
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Get video page list (分P列表) by BV ID. No login required.
pub async fn get_video_pages(bvid: &str, timeout: Duration) -> Result<Vec<VideoPage>, Error> {
    let client = http_client(timeout);
    let url = format!("https://api.bilibili.com/x/player/pagelist?bvid={}", bvid);
    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Referer", REFERER)
        .send()
        .await?;

    let json: serde_json::Value = resp.json().await?;
    let code = json["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        let message = json["message"].as_str().unwrap_or("unknown").to_string();
        return Err(Error::Api { code, message });
    }

    let mut pages = Vec::new();
    if let Some(arr) = json["data"].as_array() {
        for item in arr {
            pages.push(VideoPage {
                cid: item["cid"].as_i64().unwrap_or(0),
                page: item["page"].as_i64().unwrap_or(0) as i32,
                part: item["part"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    Ok(pages)
}

/// Get a single danmaku segment. No login required.
///
/// Each segment covers 6 minutes of video. `segment_index` starts at 1.
pub async fn get_danmaku_segment(
    cid: i64,
    segment_index: i64,
    timeout: Duration,
) -> Result<Vec<DanmakuElem>, Error> {
    let client = http_client(timeout);
    let url = format!(
        "https://api.bilibili.com/x/v2/dm/web/seg.so?type=1&oid={}&segment_index={}",
        cid, segment_index
    );
    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Referer", REFERER)
        .send()
        .await?;

    let bytes = resp.bytes().await?;
    decode_dm_seg_mobile_reply(&bytes).map_err(Error::Proto)
}

/// Get all danmaku for a video (all segments of a given page).
///
/// Starts from segment 1 and iterates until an empty segment is returned.
/// No login required.
pub async fn get_all_danmaku(
    bvid: &str,
    page: Option<i32>,
    timeout: Duration,
) -> Result<Vec<DanmakuElem>, Error> {
    let pages = get_video_pages(bvid, timeout).await?;
    let target_page = page.unwrap_or(1);
    let video_page = pages
        .iter()
        .find(|p| p.page == target_page)
        .ok_or(Error::NoPage)?;

    let mut all_danmaku = Vec::new();
    let mut segment_index = 1i64;

    loop {
        let danmaku = get_danmaku_segment(video_page.cid, segment_index, timeout).await?;
        if danmaku.is_empty() {
            break;
        }
        all_danmaku.extend(danmaku);
        segment_index += 1;
    }

    // Sort by progress (appearance time)
    all_danmaku.sort_by_key(|d| d.progress);

    Ok(all_danmaku)
}

/// Get danmaku for a video with a maximum count limit.
///
/// Fetches segments until the count limit is reached or all segments are exhausted.
/// No login required.
pub async fn get_all_danmaku_with_limit(
    bvid: &str,
    page: Option<i32>,
    max_count: usize,
    timeout: Duration,
) -> Result<Vec<DanmakuElem>, Error> {
    let pages = get_video_pages(bvid, timeout).await?;
    let target_page = page.unwrap_or(1);
    let video_page = pages
        .iter()
        .find(|p| p.page == target_page)
        .ok_or(Error::NoPage)?;

    let mut all_danmaku = Vec::new();
    let mut segment_index = 1i64;

    loop {
        let danmaku = get_danmaku_segment(video_page.cid, segment_index, timeout).await?;
        if danmaku.is_empty() {
            break;
        }
        all_danmaku.extend(danmaku);
        if all_danmaku.len() >= max_count {
            break;
        }
        segment_index += 1;
    }

    // Sort by progress (appearance time)
    all_danmaku.sort_by_key(|d| d.progress);

    // Truncate to max_count
    all_danmaku.truncate(max_count);

    Ok(all_danmaku)
}

fn http_client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .expect("failed to build reqwest client")
}
