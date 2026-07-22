use crate::{Result, request, response};
use reqwest::header::{ACCEPT, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, HeaderMap};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

#[derive(Debug)]
pub struct ChatConfig {
    pub tx: mpsc::Sender<response::StreamTextItem>,
}

#[derive(Debug)]
pub struct Chat {
    pub config: request::APIConfig,
    messages: Vec<request::Message>,
    chat_tx: mpsc::Sender<response::StreamTextItem>,
}

impl Chat {
    pub fn new(
        prompt: impl ToString,
        question: impl ToString,
        config: ChatConfig,
        request_config: request::APIConfig,
        chats: Vec<request::HistoryChat>,
    ) -> Chat {
        let mut messages = vec![];
        messages.push(request::Message {
            role: "system".to_string(),
            content: prompt.to_string(),
        });

        for item in chats.into_iter() {
            messages.push(request::Message {
                role: "user".to_string(),
                content: item.utext,
            });

            messages.push(request::Message {
                role: "assistant".to_string(),
                content: item.btext,
            })
        }

        messages.push(request::Message {
            role: "user".to_string(),
            content: question.to_string(),
        });

        Chat {
            messages,
            config: request_config,
            chat_tx: config.tx,
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
        headers.insert(
            AUTHORIZATION,
            format!("Bearer {}", self.config.api_key).parse().unwrap(),
        );
        headers.insert(ACCEPT, "text/event-stream".parse().unwrap());
        headers.insert(CACHE_CONTROL, "no-cache".parse().unwrap());

        headers
    }

    pub async fn start(self) -> Result<()> {
        let headers = self.headers();
        let client = reqwest::Client::new();

        // Handle base_url that may or may not already include /chat/completions
        let url = if self.config.api_base_url.ends_with("/chat/completions") {
            self.config.api_base_url.clone()
        } else {
            // Remove trailing slash if present, then append endpoint
            let base = self.config.api_base_url.trim_end_matches('/');
            format!("{}{}", base, "/chat/completions")
        };

        let request_body = request::ChatCompletion {
            messages: self.messages,
            model: self.config.api_model,
            temperature: self.config.temperature,
            stream: true,
        };

        let response = client
            .post(&url)
            .headers(headers)
            .json(&request_body)
            .timeout(Duration::from_secs(15))
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_body = response.text().await?;
            log::error!("API error: status={}, body={}", status, error_body);
            let item = response::StreamTextItem {
                etext: Some(format!("API error: {}", error_body)),
                ..Default::default()
            };
            if self.chat_tx.send(item).await.is_err() {
                log::info!("receiver dropped");
            }
            return Ok(());
        }

        let mut buffer = String::new();
        let mut stream = response.bytes_stream();

        loop {
            match stream.next().await {
                Some(Ok(chunk)) => {
                    buffer.push_str(&String::from_utf8_lossy(&chunk));

                    while let Some(event_end) = buffer.find("\n\n") {
                        let event = buffer[..event_end].to_string();
                        buffer = buffer[event_end + 2..].to_string();

                        if event.is_empty() {
                            continue;
                        }

                        if event == "data: [DONE]" {
                            break;
                        }

                        if !event.starts_with("data:") {
                            continue;
                        }

                        let json_str = &event[5..];

                        if let Ok(err) = serde_json::from_str::<response::Error>(json_str) {
                            if let Some(estr) = err.error.get("message") {
                                let item = response::StreamTextItem {
                                    etext: Some(estr.clone()),
                                    ..Default::default()
                                };
                                if self.chat_tx.send(item).await.is_err() {
                                    log::info!("receiver dropped");
                                    return Ok(());
                                }
                                log::error!("API error: {}", estr);
                            }
                            return Ok(());
                        }

                        match serde_json::from_str::<response::ChatCompletionChunk>(json_str) {
                            Ok(chunk) => {
                                if chunk.choices.is_empty() {
                                    continue;
                                }
                                let choice = &chunk.choices[0];
                                if choice.finish_reason.is_some() {
                                    let item = response::StreamTextItem {
                                        finished: true,
                                        ..Default::default()
                                    };
                                    if self.chat_tx.send(item).await.is_err() {
                                        log::info!("receiver dropped");
                                        return Ok(());
                                    }
                                    return Ok(());
                                }

                                let item = if choice.delta.contains_key("content")
                                    && choice.delta["content"].is_some()
                                {
                                    Some(response::StreamTextItem {
                                        text: choice.delta["content"].clone(),
                                        ..Default::default()
                                    })
                                } else if choice.delta.contains_key("reasoning_content")
                                    && choice.delta["reasoning_content"].is_some()
                                {
                                    Some(response::StreamTextItem {
                                        reasoning_text: choice.delta["reasoning_content"].clone(),
                                        ..Default::default()
                                    })
                                } else if choice.delta.contains_key("role") {
                                    None
                                } else {
                                    None
                                };

                                if let Some(item) = item
                                    && self.chat_tx.send(item).await.is_err()
                                {
                                    log::info!("receiver dropped");
                                    return Ok(());
                                }
                            }
                            // Continue processing other events instead of breaking
                            Err(e) => log::error!("Parse error: {:?} event={}", e, &event),
                        }
                    }
                }
                Some(Err(e)) => log::error!("Stream error: {:?}", e),
                None => break,
            }
        }
        Ok(())
    }
}
