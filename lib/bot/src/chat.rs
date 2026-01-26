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

        let url = format!("{}{}", self.config.api_base_url, "/chat/completions");
        let request_body = request::ChatCompletion {
            messages: self.messages,
            model: self.config.api_model,
            temperature: self.config.temperature,
            stream: true,
        };

        let mut stream = client
            .post(url)
            .headers(headers)
            .json(&request_body)
            .timeout(Duration::from_secs(15))
            .send()
            .await?
            .bytes_stream();

        loop {
            match stream.next().await {
                Some(Ok(chunk)) => {
                    let body = String::from_utf8_lossy(&chunk);

                    // log::debug!("{body:?}");

                    if let Ok(err) = serde_json::from_str::<response::Error>(&body) {
                        if let Some(estr) = err.error.get("message") {
                            let item = response::StreamTextItem {
                                etext: Some(estr.clone()),
                                ..Default::default()
                            };
                            if self.chat_tx.send(item).await.is_err() {
                                log::info!("receiver dropped");
                                break;
                            }
                            log::info!("{}", estr);
                        }
                        break;
                    }

                    if body.starts_with("data: [DONE]") {
                        break;
                    }

                    let lines: Vec<_> = body.split("\n\n").collect();

                    for line in lines.into_iter() {
                        if !line.starts_with("data:") {
                            continue;
                        }

                        match serde_json::from_str::<response::ChatCompletionChunk>(&line[5..]) {
                            Ok(chunk) => {
                                let choice = &chunk.choices[0];
                                if choice.finish_reason.is_some() {
                                    let item = response::StreamTextItem {
                                        finished: true,
                                        ..Default::default()
                                    };
                                    if self.chat_tx.send(item).await.is_err() {
                                        log::info!("receiver dropped");
                                        break;
                                    }

                                    log::info!(
                                        "finish_reason: {}",
                                        choice.finish_reason.as_ref().unwrap()
                                    );
                                    break;
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
                                    log::info!("role: {:?}", choice.delta["role"]);
                                    None
                                } else {
                                    None
                                };

                                if let Some(item) = item
                                    && self.chat_tx.send(item).await.is_err()
                                {
                                    log::info!("receiver dropped");
                                    break;
                                }
                            }
                            Err(e) => {
                                log::info!("{e:?} {}", &line);
                                break;
                            }
                        }
                    }
                }
                Some(Err(_)) => (),
                None => break,
            }
        }
        Ok(())
    }
}
