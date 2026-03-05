#![forbid(unsafe_code)]

use futures::stream::{self, BoxStream};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub type Result<T> = std::result::Result<T, ProviderError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCard {
    pub id: String,
    pub provider: String,
    pub context_window: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<CompletionMessage>,
    pub tools: Vec<String>,
    pub stream: bool,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
}

impl CompletionRequest {
    pub fn user_prompt(&self) -> Option<&str> {
        self.messages
            .iter()
            .rev()
            .find(|message| message.role == "user")
            .map(|message| message.content.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderEvent {
    TextDelta {
        request_id: Option<String>,
        text: String,
    },
    ThinkingDelta {
        request_id: Option<String>,
        text: String,
    },
    ToolCall {
        request_id: Option<String>,
        tool_name: String,
        args: Value,
    },
    Done {
        request_id: Option<String>,
        stop_reason: String,
    },
    Usage {
        request_id: Option<String>,
        input_tokens: u32,
        output_tokens: u32,
        cached_tokens: u32,
        cost_usd: f64,
    },
    Error {
        request_id: Option<String>,
        message: String,
    },
}

#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn list_models(&self) -> Result<Vec<ModelCard>>;
    fn stream(
        &self,
        request: CompletionRequest,
        request_id: Option<String>,
    ) -> BoxStream<'static, ProviderEvent>;
}

#[derive(Debug, Clone)]
pub struct MockProvider;

#[async_trait::async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    async fn list_models(&self) -> Result<Vec<ModelCard>> {
        Ok(vec![ModelCard {
            id: "mock-tool-call".to_string(),
            provider: self.name().to_string(),
            context_window: Some(16_384),
        }])
    }

    fn stream(
        &self,
        request: CompletionRequest,
        request_id: Option<String>,
    ) -> BoxStream<'static, ProviderEvent> {
        let prompt = request.user_prompt().unwrap_or_default().to_string();
        let prompt_lc = prompt.to_lowercase();
        let model = request.model.clone();

        let events =
            if prompt_lc.contains("ls") || prompt_lc.contains("find") || prompt_lc.contains("grep")
            {
                vec![
                    ProviderEvent::TextDelta {
                        request_id: request_id.clone(),
                        text: "Scanning workspace via tools…\n".to_string(),
                    },
                    ProviderEvent::ToolCall {
                        request_id: request_id.clone(),
                        tool_name: "find".to_string(),
                        args: serde_json::json!({ "query": "src", "limit": 20 }),
                    },
                    ProviderEvent::Done {
                        request_id,
                        stop_reason: "complete".to_string(),
                    },
                ]
            } else if model == "silent" {
                vec![ProviderEvent::Done {
                    request_id,
                    stop_reason: "complete".to_string(),
                }]
            } else {
                vec![
                    ProviderEvent::TextDelta {
                        request_id: request_id.clone(),
                        text: format!("Mock response: {prompt}"),
                    },
                    ProviderEvent::Done {
                        request_id,
                        stop_reason: "complete".to_string(),
                    },
                ]
            };

        Box::pin(stream::iter(events))
    }
}

#[cfg(feature = "openai")]
pub mod openai {
    use super::*;
    use futures::StreamExt;
    use reqwest::Client;
    use serde_json::{json, Value};

    #[derive(Debug, Clone)]
    pub struct OpenAIProvider {
        pub base_url: String,
        pub api_key: Option<String>,
    }

    impl OpenAIProvider {
        pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
            Self {
                base_url: base_url.into(),
                api_key,
            }
        }
    }

    #[async_trait::async_trait]
    impl Provider for OpenAIProvider {
        fn name(&self) -> &'static str {
            "openai-compatible"
        }

        async fn list_models(&self) -> Result<Vec<ModelCard>> {
            Ok(vec![ModelCard {
                id: "gpt-mock".to_string(),
                provider: self.name().to_string(),
                context_window: Some(128_000),
            }])
        }

        fn stream(
            &self,
            request: CompletionRequest,
            request_id: Option<String>,
        ) -> BoxStream<'static, ProviderEvent> {
            let base_url = self.base_url.clone();
            let api_key = self.api_key.clone();
            let request_id_for_events = request_id.clone();

            let payload = serde_json::json!({
                "model": request.model,
                "messages": request
                    .messages
                    .into_iter()
                    .map(|message| json!({"role": message.role, "content": message.content}))
                    .collect::<Vec<Value>>(),
                "stream": true,
            });

            let stream = async move {
                let client = Client::new();
                let mut request = client
                    .post(format!("{}/v1/chat/completions", base_url))
                    .json(&payload);
                if let Some(api_key) = api_key {
                    request = request.bearer_auth(api_key);
                }

                let mut out = Vec::new();
                match request.send().await {
                    Ok(response) => {
                        if !response.status().is_success() {
                            out.push(ProviderEvent::Error {
                                request_id: request_id_for_events.clone(),
                                message: format!("openai request failed: {}", response.status()),
                            });
                            return out;
                        }

                        let raw = response.text().await.unwrap_or_default();
                        if raw.starts_with("{") && raw.ends_with('}') {
                            // Non-stream payload fallback.
                            match serde_json::from_str::<Value>(&raw) {
                                Ok(value) => {
                                    if let Some(content) = value
                                        .get("choices")
                                        .and_then(Value::as_array)
                                        .and_then(|items| items.first())
                                        .and_then(|choice| choice.get("message"))
                                        .and_then(|message| message.get("content"))
                                        .and_then(Value::as_str)
                                    {
                                        out.push(ProviderEvent::TextDelta {
                                            request_id: request_id_for_events.clone(),
                                            text: content.to_string(),
                                        });
                                    }
                                }
                                Err(err) => {
                                    out.push(ProviderEvent::Error {
                                        request_id: request_id_for_events.clone(),
                                        message: format!("openai parse error: {err}"),
                                    });
                                }
                            }
                        } else {
                            // SSE style: one JSON object per line with "data:" prefix.
                            for line in raw.lines() {
                                let payload = line.trim();
                                if !payload.starts_with("data:") {
                                    continue;
                                }
                                let payload = payload.trim_start_matches("data:").trim();
                                if payload == "[DONE]" {
                                    out.push(ProviderEvent::Done {
                                        request_id: request_id_for_events.clone(),
                                        stop_reason: "done".to_string(),
                                    });
                                    continue;
                                }
                                if payload.is_empty() {
                                    continue;
                                }

                                let parsed = match serde_json::from_str::<Value>(payload) {
                                    Ok(payload) => payload,
                                    Err(err) => {
                                        out.push(ProviderEvent::Error {
                                            request_id: request_id_for_events.clone(),
                                            message: format!("openai stream parse error: {err}"),
                                        });
                                        continue;
                                    }
                                };

                                if let Some(choice) = parsed
                                    .get("choices")
                                    .and_then(Value::as_array)
                                    .and_then(|items| items.first())
                                    .and_then(|item| item.get("delta"))
                                {
                                    if let Some(text) =
                                        choice.get("content").and_then(Value::as_str)
                                    {
                                        if !text.is_empty() {
                                            out.push(ProviderEvent::TextDelta {
                                                request_id: request_id_for_events.clone(),
                                                text: text.to_string(),
                                            });
                                        }
                                    }
                                    if let Some(tool_calls) =
                                        choice.get("tool_calls").and_then(Value::as_array)
                                    {
                                        for tool_call in tool_calls {
                                            let fn_name = tool_call
                                                .get("function")
                                                .and_then(|value| value.get("name"))
                                                .and_then(Value::as_str)
                                                .unwrap_or("unknown");
                                            let args = tool_call
                                                .get("function")
                                                .and_then(|value| value.get("arguments"))
                                                .cloned()
                                                .unwrap_or_else(|| {
                                                    Value::Object(serde_json::Map::new())
                                                });
                                            out.push(ProviderEvent::ToolCall {
                                                request_id: request_id_for_events.clone(),
                                                tool_name: fn_name.to_string(),
                                                args,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => {
                        out.push(ProviderEvent::Error {
                            request_id: request_id_for_events.clone(),
                            message: format!("openai request failed: {err}"),
                        });
                    }
                }

                if !out
                    .iter()
                    .any(|event| matches!(event, ProviderEvent::Done { .. }))
                {
                    out.push(ProviderEvent::Done {
                        request_id: request_id_for_events,
                        stop_reason: "complete".to_string(),
                    });
                }
                out
            };

            Box::pin(
                stream::once(async move { stream.await }).flat_map(|events| stream::iter(events)),
            )
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("provider error: {0}")]
    Provider(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
}
