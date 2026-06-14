use anyhow::Result;
use futures::{Stream, StreamExt};
use serde_json::Value;
use std::pin::Pin;
use crate::context_store::ChatMessage;

pub async fn stream_chat_completion(
    base_url: &str,
    model: &str,
    api_key: &str,
    temperature: f64,
    history: &[ChatMessage],
    system_prompt: &str,
) -> Result<Pin<Box<dyn Stream<Item = Result<String, anyhow::Error>> + Send>>> {
    let client = crate::get_http_client();
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let mut messages = vec![
        serde_json::json!({
            "role": "system",
            "content": system_prompt
        })
    ];
    for msg in history {
        messages.push(serde_json::json!({
            "role": msg.role,
            "content": msg.content
        }));
    }

    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "stream": true
    });

    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    if !res.status().is_success() {
        let err_text = res.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("LLM API Error ({}): {}", url, err_text));
    }

    let byte_stream = res.bytes_stream();

    struct StreamState<S> {
        stream: S,
        buffer: Vec<u8>,
        done: bool,
    }

    let state = StreamState {
        stream: byte_stream,
        buffer: Vec::new(),
        done: false,
    };

    let text_stream = futures::stream::unfold(state, |mut state| async move {
        if state.done {
            return None;
        }

        loop {
            // Ищем перевод строки
            if let Some(pos) = state.buffer.iter().position(|&b| b == b'\n') {
                let line_bytes = state.buffer.drain(..=pos).collect::<Vec<u8>>();
                let line = String::from_utf8_lossy(&line_bytes);
                let line = line.trim();

                if line.is_empty() {
                    continue;
                }

                if line.starts_with("data: ") {
                    let data = &line["data: ".len()..];
                    if data == "[DONE]" {
                        state.done = true;
                        return Some((Ok("".to_string()), state));
                    }
                    if let Ok(json) = serde_json::from_str::<Value>(data) {
                        if let Some(choices) = json["choices"].as_array() {
                            if !choices.is_empty() {
                                if let Some(content) = choices[0]["delta"]["content"].as_str() {
                                    if !content.is_empty() {
                                        return Some((Ok(content.to_string()), state));
                                    }
                                }
                            }
                        }
                    }
                }
                continue;
            }

            // Читаем больше байт
            match state.stream.next().await {
                Some(Ok(bytes)) => {
                    state.buffer.extend_from_slice(&bytes);
                }
                Some(Err(e)) => {
                    state.done = true;
                    return Some((Err(anyhow::anyhow!("Ошибка чтения потока: {:?}", e)), state));
                }
                None => {
                    if state.buffer.is_empty() {
                        return None;
                    } else {
                        state.buffer.push(b'\n');
                    }
                }
            }
        }
    });

    let filtered_stream = text_stream.filter(|res| {
        let is_empty = match res {
            Ok(s) => s.is_empty(),
            Err(_) => false,
        };
        futures::future::ready(!is_empty)
    });

    Ok(Box::pin(filtered_stream))
}
