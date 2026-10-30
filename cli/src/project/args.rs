use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};
use video_editor::export::CompressionPreset;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum FilterKind {
    Video,
    Audio,
    Subtitle,
    Image,
    Global,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// 查看工程文件信息（轨道、片段、时长）
    Info { input: PathBuf },

    /// 用媒体文件创建工程（视频/图片/音频/字幕，每个文件一条轨道）
    Create {
        #[arg(short, long)]
        output: PathBuf,
        media: Vec<PathBuf>,
        /// 全局速度倍率
        #[arg(long, default_value_t = 1.0)]
        speed: f32,
    },

    /// 列出工程里的轨道与片段（含索引，供其它编辑命令使用）
    Segments {
        project: PathBuf,
        /// 只看某条轨道
        #[arg(long)]
        track: Option<usize>,
    },

    /// 裁剪片段：改源起点与时长
    Trim {
        project: PathBuf,
        #[arg(long)]
        track: usize,
        #[arg(long)]
        segment: usize,
        /// 源文件内的起始时间（秒）
        #[arg(long, default_value_t = 0.0)]
        start: f64,
        /// 片段时长（秒）
        #[arg(long)]
        duration: f64,
    },

    /// 移动片段在时间轴上的位置
    Move {
        project: PathBuf,
        #[arg(long)]
        track: usize,
        #[arg(long)]
        segment: usize,
        /// 时间轴上的起始位置（秒）
        #[arg(long)]
        to: f64,
    },

    /// 删除片段
    Remove {
        project: PathBuf,
        #[arg(long)]
        track: usize,
        #[arg(long)]
        segment: usize,
    },

    /// 列出片段上的滤镜（含 JSON 参数）
    Filters {
        project: PathBuf,
        #[arg(long)]
        track: Option<usize>,
        #[arg(long)]
        segment: Option<usize>,
    },

    /// 给片段加一个滤镜，参数用工程文件的 JSON 格式
    FilterAdd {
        project: PathBuf,
        #[arg(long)]
        track: usize,
        #[arg(long)]
        segment: usize,
        /// 例如 {"enabled":true,"type":"flip","params":{"direction":"Horizontal"}}
        #[arg(long)]
        json: String,
        /// 滤镜类别，缺省依次尝试 video/audio/subtitle/image
        #[arg(long, value_enum)]
        kind: Option<FilterKind>,
    },

    /// 删除片段上的第 N 个滤镜
    FilterRemove {
        project: PathBuf,
        #[arg(long)]
        track: usize,
        #[arg(long)]
        segment: usize,
        #[arg(long, value_enum, default_value_t = FilterKind::Video)]
        kind: FilterKind,
        #[arg(long)]
        index: usize,
    },

    /// 列出全局滤镜
    GlobalFilters { project: PathBuf },

    /// 添加全局滤镜（progress_bar / timer / global_speed / rotation / danmaku）
    GlobalFilterAdd {
        project: PathBuf,
        /// 例如 {"enabled":true,"type":"global_speed","params":{"speed":1.5}}
        #[arg(long)]
        json: String,
    },

    /// 删除第 N 个全局滤镜
    GlobalFilterRemove {
        project: PathBuf,
        #[arg(long)]
        index: usize,
    },

    /// 列出所有可用的滤镜类型名
    FilterCatalog,

    /// 在工程字幕里全文搜索
    SubtitleSearch { project: PathBuf, keyword: String },

    /// 列出工程里的字幕条目（时间轴）
    Subtitles { project: PathBuf },

    /// 用 LLM 生成章节摘要（基于字幕时间轴）
    ChapterSummary {
        project: PathBuf,
        #[arg(long)]
        api_base: String,
        #[arg(long)]
        api_key: String,
        #[arg(long)]
        model: String,
        /// 额外要求，会拼在内置提示词后面
        #[arg(long, default_value = "")]
        prompt: String,
        #[arg(long, default_value_t = 0.3)]
        temperature: f32,
        /// 生成结果写回工程文件
        #[arg(long)]
        save: bool,
    },

    /// 语音剪辑（去口误/停顿）：VAD+ASR+LLM 判定保留段，再应用到片段
    VoiceCut {
        project: PathBuf,
        #[arg(long, default_value_t = 0)]
        track: usize,
        #[arg(long, default_value_t = 0)]
        segment: usize,
        /// 直接给定条目：[{"start_ms":0,"end_ms":1000,"keep":true}]，跳过 ASR 与 LLM
        #[arg(long)]
        entries: Option<String>,
        /// ASR 模型权重
        #[arg(long)]
        asr_weights: Option<PathBuf>,
        /// ASR tokenizer
        #[arg(long)]
        asr_tokenizer: Option<PathBuf>,
        #[arg(long)]
        api_base: Option<String>,
        #[arg(long)]
        api_key: Option<String>,
        #[arg(long)]
        model: Option<String>,
        /// 参考文稿：只保留与文稿内容匹配的片段
        #[arg(long)]
        reference: Option<String>,
        #[arg(long, default_value_t = 0.01)]
        vad_threshold: f32,
        #[arg(long, default_value_t = 256)]
        max_tokens: u32,
        /// 真正改写工程；不加只输出剪辑计划
        #[arg(long)]
        apply: bool,
    },

    /// 智能混剪：按音频内容把画面片段重排到音频时间轴上
    SmartMix {
        project: PathBuf,
        /// 音频轨道索引，缺省取第一条音频轨
        #[arg(long)]
        audio_track: Option<usize>,
        /// 参与混剪的画面轨道，逗号分隔；缺省全部视频/图片轨
        #[arg(long)]
        visual_tracks: Option<String>,
        /// 音频条目：[{"start_ms":0,"end_ms":5000,"text":"..."}]，跳过 ASR
        #[arg(long)]
        audio_entries: Option<String>,
        /// 从 SRT 读音频条目（作为文案）
        #[arg(long)]
        srt: Option<PathBuf>,
        #[arg(long)]
        asr_weights: Option<PathBuf>,
        #[arg(long)]
        asr_tokenizer: Option<PathBuf>,
        /// 画面描述：[{"track":0,"segment":0,"description":"..."}]，跳过视觉模型
        #[arg(long)]
        visuals: Option<String>,
        /// LFM-VL 模型目录，自动识别画面内容
        #[arg(long)]
        vision_model: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = crate::smart_mix::PrecisionArg::Fp16)]
        vision_precision: crate::smart_mix::PrecisionArg,
        #[arg(long)]
        api_base: Option<String>,
        #[arg(long)]
        api_key: Option<String>,
        #[arg(long)]
        model: Option<String>,
        /// 允许同一画面重复使用
        #[arg(long)]
        reuse_media: bool,
        /// 按顺序匹配
        #[arg(long)]
        sequential: bool,
        /// 必须覆盖全部音频
        #[arg(long)]
        cover_all: bool,
        /// 真正改写工程；不加只输出匹配计划
        #[arg(long)]
        apply: bool,
    },

    /// 读取或设置工程备注
    Memo {
        project: PathBuf,
        /// 不给则只读取，给了则写入
        #[arg(long)]
        set: Option<String>,
    },

    /// 列出书签（时间点标记）
    Bookmarks { project: PathBuf },

    /// 添加书签
    BookmarkAdd {
        project: PathBuf,
        /// 时间点（秒）
        #[arg(long)]
        time: f64,
    },

    /// 删除书签
    BookmarkRemove {
        project: PathBuf,
        #[arg(long)]
        index: usize,
    },

    /// 列出所有文字（标题）片段
    Texts { project: PathBuf },

    /// 添加文字（标题）片段
    TextAdd {
        project: PathBuf,
        #[arg(long)]
        text: Option<String>,
        /// 时间轴起始（秒）
        #[arg(long, default_value_t = 0.0)]
        start: f64,
        /// 持续时长（秒）
        #[arg(long, default_value_t = 3.0)]
        duration: f64,
        #[arg(long, default_value_t = 64)]
        font_size: u32,
        #[arg(long, default_value = "Sans")]
        font_family: String,
        /// 字体文件路径，缺省用系统默认
        #[arg(long)]
        font_path: Option<PathBuf>,
        /// 颜色 rrggbb / rrggbbaa
        #[arg(long, default_value = "ffffffff")]
        color: String,
        /// 归一化位置 x,y
        #[arg(long, default_value = "0.5,0.5")]
        position: String,
        /// 0=左 1=中 2=右
        #[arg(long)]
        alignment: Option<u32>,
        /// 用完整 TextElementData JSON 覆盖所有样式字段
        #[arg(long)]
        json: Option<String>,
        /// 指定已有的文字轨道索引，缺省用第一条文字轨道，没有则新建
        #[arg(long)]
        track: Option<usize>,
        /// 新建文字轨道时的名字
        #[arg(long, default_value = "T")]
        name: String,
    },

    /// 删除文字片段
    TextRemove {
        project: PathBuf,
        #[arg(long)]
        track: usize,
        #[arg(long)]
        segment: usize,
    },

    /// 向已有工程追加媒体文件
    AddMedia {
        project: PathBuf,
        media: Vec<PathBuf>,
        #[arg(long, default_value_t = 1.0)]
        speed: f32,
    },

    /// 把工程文件渲染导出为 mp4
    Export {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// 输出宽度，缺省自动
        #[arg(long)]
        width: Option<u32>,
        /// 输出高度，缺省自动
        #[arg(long)]
        height: Option<u32>,
        /// 输出帧率，缺省自动
        #[arg(long)]
        fps: Option<u32>,
        /// 恒定质量，越小越清晰（0-51）
        #[arg(long)]
        crf: Option<u8>,
        #[arg(long, value_enum)]
        preset: Option<Preset>,
        #[arg(long)]
        audio_bitrate: Option<u32>,
        /// 不烧录字幕
        #[arg(long)]
        no_subtitles: bool,
        /// 低内存模式（慢但省内存）
        #[arg(long)]
        low_memory: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Preset {
    Ultrafast,
    Superfast,
    Veryfast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    Veryslow,
}

impl Preset {
    pub(super) fn to_compression(self) -> CompressionPreset {
        match self {
            Preset::Ultrafast => CompressionPreset::Ultrafast,
            Preset::Superfast => CompressionPreset::Superfast,
            Preset::Veryfast => CompressionPreset::Veryfast,
            Preset::Faster => CompressionPreset::Faster,
            Preset::Fast => CompressionPreset::Fast,
            Preset::Medium => CompressionPreset::Medium,
            Preset::Slow => CompressionPreset::Slow,
            Preset::Slower => CompressionPreset::Slower,
            Preset::Veryslow => CompressionPreset::Veryslow,
        }
    }
}
