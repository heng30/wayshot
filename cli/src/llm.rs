use anyhow::{Result, anyhow};
use serde_json::{Value, json};

#[derive(clap::Args)]
pub struct Args {
    /// 用户输入
    pub question: String,

    /// OpenAI 兼容接口地址，如 https://api.openai.com/v1
    #[arg(long)]
    pub api_base: String,

    #[arg(long)]
    pub api_key: String,

    #[arg(long)]
    pub model: String,

    /// 系统提示词
    #[arg(long, default_value = "You are a helpful assistant.")]
    pub system: String,

    #[arg(long, default_value_t = 0.3)]
    pub temperature: f32,
}

pub struct ApiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
}

pub fn run(args: &Args) -> Result<Value> {
    let api = ApiConfig {
        base_url: args.api_base.clone(),
        api_key: args.api_key.clone(),
        model: args.model.clone(),
        temperature: args.temperature,
    };
    let text = chat(&args.system, &args.question, &api)?;
    Ok(json!({ "text": text }))
}

pub fn chat(system: &str, question: &str, api: &ApiConfig) -> Result<String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow!("创建 tokio runtime 失败: {e}"))?;

    runtime.block_on(async {
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        let config = bot::APIConfig {
            api_base_url: api.base_url.clone(),
            api_model: api.model.clone(),
            api_key: api.api_key.clone(),
            temperature: Some(api.temperature),
        };

        let chat = bot::Chat::new(system, question, bot::ChatConfig { tx }, config, Vec::new());

        let request = tokio::spawn(async move { chat.start().await });
        let mut text = String::new();
        while let Some(item) = rx.recv().await {
            if let Some(chunk) = item.text {
                text.push_str(&chunk);
            }
            if item.finished {
                break;
            }
        }

        request
            .await
            .map_err(|e| anyhow!("请求任务异常: {e}"))?
            .map_err(|e| anyhow!("请求失败: {e}"))?;

        Ok(text)
    })
}
