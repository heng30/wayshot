/// A single danmaku (弹幕) entry.
#[derive(Debug, Clone, Default)]
pub struct DanmakuElem {
    /// 弹幕 dmid
    pub id: i64,
    /// 出现位置 (毫秒)
    pub progress: i32,
    /// 弹幕类型: 1=滚动, 4=底部, 5=顶部, 6=逆向, 7=高级, 8=代码, 9=BAS
    pub mode: i32,
    /// 字号
    pub fontsize: i32,
    /// 颜色值 (十进制 RGB)
    pub color: u32,
    /// 发送者 mid hash
    pub mid_hash: String,
    /// 弹幕正文
    pub content: String,
    /// 发送时间戳
    pub ctime: i64,
    /// 动作
    pub action: String,
    /// 弹幕池
    pub pool: i32,
    /// 弹幕 dmid (字符串形式)
    pub id_str: String,
    /// 属性位
    pub attr: i32,
    /// 权重 [1,10]
    pub weight: i32,
    /// 渐变色弹幕
    pub colorful: i32,
    /// 动画弹幕 JSON
    pub animation: Option<String>,
}

/// A video page (分P) entry from pagelist API.
#[derive(Debug, Clone)]
pub struct VideoPage {
    /// 视频 cid
    pub cid: i64,
    /// 分P序号 (从1开始)
    pub page: i32,
    /// 分P标题
    pub part: String,
}
