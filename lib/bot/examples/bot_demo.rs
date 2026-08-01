use bot::{APIConfig, Chat, ChatConfig, HistoryChat, StreamTextItem};

#[tokio::main]
async fn main() {
    env_logger::init();

    let api_key =
        std::env::var("BOT_DEMO_API_KEY").expect("Missing BOT_DEMO_API_KEY in environment");

    let prompt = "Your are a chat bot.";
    let question = "给我一个Rust程序。要求中文输出。";

    let request_config = APIConfig {
        // api_base_url: "https://api.deepseek.com/v1".to_string(),
        api_base_url: "https://opencode.ai/zen/go/v1".to_string(),
        api_model: "deepseek-v4-flash".to_string(),
        api_key,
        temperature: None,
    };

    let histories = vec![HistoryChat {
        utext: "hi".to_string(),
        btext: "Hello! 👋 How can I assist you today? 😊".to_string(),
    }];

    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamTextItem>(100);

    let chat_config = ChatConfig { tx };
    let chat = Chat::new(prompt, question, chat_config, request_config, histories);
    let mut content = String::new();

    let handle = tokio::spawn(async move {
        while let Some(item) = rx.recv().await {
            if let Some(ref text) = item.reasoning_text {
                content.push_str(&text);
            } else if let Some(ref text) = item.text {
                content.push_str(&text);
            }

            log::debug!("{item:?}");
        }

        log::debug!("{content}");
    });

    if let Err(e) = chat.start().await {
        log::warn!("Chat error: {e:?}");
    }

    _ = handle.await;
}
