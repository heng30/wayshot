use bili_danmaku::{get_all_danmaku, get_video_pages, DEFAULT_TIMEOUT};

fn format_color(color: u32) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (color >> 16) & 0xFF,
        (color >> 8) & 0xFF,
        color & 0xFF
    )
}

fn mode_name(mode: i32) -> &'static str {
    match mode {
        1 => "滚动",
        4 => "底部",
        5 => "顶部",
        6 => "逆向",
        7 => "高级",
        8 => "代码",
        9 => "BAS",
        _ => "未知",
    }
}

#[tokio::main]
async fn main() {
    let bvid = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "BV1GJ411x7h7".to_string());

    println!("BV号: {}", bvid);

    // 获取分P列表
    let pages = get_video_pages(&bvid, DEFAULT_TIMEOUT).await.unwrap();
    println!("分P列表:");
    for p in &pages {
        println!("  P{}: {} (cid={})", p.page, p.part, p.cid);
    }

    // 获取第1P的全部弹幕
    let danmaku = get_all_danmaku(&bvid, Some(1), DEFAULT_TIMEOUT).await.unwrap();
    println!("\n弹幕总数: {}", danmaku.len());

    let counts = 1000;

    // 打印前20条弹幕
    for (i, d) in danmaku.iter().take(counts).enumerate() {
        let time = format!(
            "{:02}:{:02}",
            d.progress / 60000,
            (d.progress % 60000) / 1000
        );
        println!(
            "#{:>3} [{}] {} {} | {}",
            i + 1,
            time,
            mode_name(d.mode),
            format_color(d.color),
            d.content
        );
    }

    if danmaku.len() > 20 {
        println!("... (还有 {} 条弹幕)", danmaku.len().saturating_sub(counts));
    }
}

