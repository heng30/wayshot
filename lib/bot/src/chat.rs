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

        let mut buffer: Vec<u8> = Vec::new();
        let mut stream = response.bytes_stream();

        loop {
            match stream.next().await {
                Some(Ok(chunk)) => {
                    buffer.extend_from_slice(&chunk);

                    while let Some((sep, sep_len)) = find_event_separator(&buffer) {
                        // A UTF-8 char split across network chunks can only be
                        // incomplete at the buffer tail (after the last event
                        // separator), so a complete event always decodes intact.
                        let event = match std::str::from_utf8(&buffer[..sep]) {
                            Ok(e) => e.to_string(),
                            Err(_) => {
                                log::error!("SSE event is not valid UTF-8, skipping");
                                buffer.drain(..sep + sep_len);
                                continue;
                            }
                        };
                        buffer.drain(..sep + sep_len);

                        if handle_event(&self.chat_tx, &event).await? {
                            return Ok(());
                        }
                    }
                }
                Some(Err(e)) => log::error!("Stream error: {:?}", e),
                None => {
                    // Provider closed the stream without a [DONE] marker:
                    // flush a trailing event that had no blank-line terminator.
                    if !buffer.is_empty()
                        && let Ok(event) = std::str::from_utf8(&buffer)
                        && handle_event(&self.chat_tx, event).await?
                    {
                        return Ok(());
                    }
                    break;
                }
            }
        }
        Ok(())
    }
}

/// Parse and forward a single SSE event. Returns `Ok(true)` when the
/// stream should stop after this event.
async fn handle_event(tx: &mpsc::Sender<response::StreamTextItem>, event: &str) -> Result<bool> {
    let event = event.trim_end_matches('\r');

    if event.is_empty() {
        return Ok(false);
    }

    if event == "data: [DONE]" {
        return Ok(true);
    }

    if !event.starts_with("data:") {
        return Ok(false);
    }

    let json_str = &event[5..];

    if let Ok(err) = serde_json::from_str::<response::Error>(json_str) {
        if let Some(estr) = err.error.get("message") {
            let item = response::StreamTextItem {
                etext: Some(estr.clone()),
                ..Default::default()
            };
            if tx.send(item).await.is_err() {
                log::info!("receiver dropped");
                return Ok(true);
            }
            log::error!("API error: {}", estr);
        }
        return Ok(true);
    }

    match serde_json::from_str::<response::ChatCompletionChunk>(json_str) {
        Ok(chunk) => {
            if chunk.choices.is_empty() {
                return Ok(false);
            }
            let choice = &chunk.choices[0];
            if choice.finish_reason.is_some() {
                let item = response::StreamTextItem {
                    finished: true,
                    ..Default::default()
                };
                if tx.send(item).await.is_err() {
                    log::info!("receiver dropped");
                    return Ok(true);
                }
                return Ok(true);
            }

            let item = if choice.delta.contains_key("content") && choice.delta["content"].is_some()
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
            } else {
                None
            };

            if let Some(item) = item
                && tx.send(item).await.is_err()
            {
                log::info!("receiver dropped");
                return Ok(true);
            }
        }
        // Continue processing other events instead of breaking
        Err(e) => log::error!("Parse error: {:?} event={}", e, &event),
    }
    Ok(false)
}

/// Locate the next SSE event separator ("\n\n" or "\r\n\r\n") and return
/// (end of the event content, length of the separator). `\n` never occurs
/// inside a multi-byte UTF-8 sequence, so searching raw bytes is safe.
fn find_event_separator(buf: &[u8]) -> Option<(usize, usize)> {
    if let Some(pos) = buf.windows(2).position(|w| w == b"\n\n") {
        Some((pos, 2))
    } else if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
        Some((pos, 4))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_separator_lf_and_crlf() {
        assert_eq!(find_event_separator(b"data: x\n\nmore"), Some((7, 2)));
        assert_eq!(find_event_separator(b"data: x\r\n\r\nmore"), Some((7, 4)));
        assert_eq!(find_event_separator(b"data: x"), None);
    }

    #[test]
    fn split_utf8_char_across_chunks_reassembles() {
        // "你好" is 6 bytes; split in the middle of the second char (3 bytes
        // per char) to simulate a TCP chunk boundary inside a UTF-8 sequence.
        let text = "你好世界";
        let bytes = text.as_bytes();
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&bytes[..5]);
        buffer.extend_from_slice(&bytes[5..]);

        assert_eq!(std::str::from_utf8(&buffer).unwrap(), text);
    }
}
