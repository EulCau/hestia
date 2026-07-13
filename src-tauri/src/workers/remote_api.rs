use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{info, warn};

use crate::config::RemoteApiConfig;
use crate::protocol::{Capability, Job, ResourceRequirements};

use super::{Worker, WorkerError};

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChoiceMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionStreamResponse {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

fn process_stream_line(
    line: &str,
    content: &mut String,
    usage: &mut Option<Usage>,
    finish_reason: &mut String,
    on_delta: &mut (dyn FnMut(String) + Send),
) -> Result<bool, serde_json::Error> {
    let Some(data) = line.trim().strip_prefix("data:") else {
        return Ok(false);
    };
    let data = data.trim();
    if data == "[DONE]" {
        return Ok(true);
    }
    if data.is_empty() {
        return Ok(false);
    }

    let parsed: ChatCompletionStreamResponse = serde_json::from_str(data)?;
    if let Some(chunk_usage) = parsed.usage {
        *usage = Some(chunk_usage);
    }
    for choice in parsed.choices {
        if let Some(reason) = choice.finish_reason {
            *finish_reason = reason;
        }
        if let Some(delta) = choice.delta.content {
            if !delta.is_empty() {
                content.push_str(&delta);
                on_delta(delta);
            }
        }
    }
    Ok(false)
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

pub struct RemoteApiWorker {
    id: String,
    caps: Vec<Capability>,
    config: RemoteApiConfig,
    client: Client,
    api_key: String,
}

impl RemoteApiWorker {
    pub fn new(
        id: impl Into<String>,
        caps: Vec<Capability>,
        config: RemoteApiConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Prefer direct api_key from user config, then fall back to env var
        let api_key = if let Some(ref key) = config.api_key {
            if key.is_empty() {
                return Err("api_key is empty".into());
            }
            key.clone()
        } else {
            std::env::var(&config.api_key_env).map_err(|_| {
                format!(
                    "environment variable {} not set and no api_key in user config",
                    config.api_key_env
                )
            })?
        };

        Ok(Self {
            id: id.into(),
            caps,
            config,
            client: Client::new(),
            api_key,
        })
    }

    pub fn usage_from_response(body: &str) -> Option<Usage> {
        serde_json::from_str::<ChatCompletionResponse>(body)
            .ok()
            .and_then(|r| r.usage)
    }

    async fn infer_stream_inner(
        &self,
        job: &Job,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<serde_json::Value, WorkerError> {
        let messages = job.payload["messages"].clone();
        let model = self.config.model.clone();
        let url = format!("{}/v1/chat/completions", self.config.base_url);
        info!(
            worker_id = %self.id,
            job_id = %job.id,
            model = %model,
            "remote API worker sending streaming request"
        );

        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true,
            "stream_options": { "include_usage": true },
        });

        let mut response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("HTTP streaming request failed: {}", e),
            })?;

        let status = response.status();
        if !status.is_success() {
            let response_text = response.text().await.unwrap_or_default();
            return Err(WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("API returned status {}: {}", status, response_text),
            });
        }

        let mut buffer = String::new();
        let mut content = String::new();
        let mut usage: Option<Usage> = None;
        let mut finish_reason = "unknown".to_string();

        'stream: while let Some(chunk) =
            response
                .chunk()
                .await
                .map_err(|e| WorkerError::InferenceFailed {
                    worker_id: self.id.clone(),
                    reason: format!("failed to read streaming response: {}", e),
                })?
        {
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(line_end) = buffer.find('\n') {
                let line: String = buffer.drain(..=line_end).collect();
                let done = process_stream_line(
                    &line,
                    &mut content,
                    &mut usage,
                    &mut finish_reason,
                    on_delta,
                )
                .map_err(|e| WorkerError::InferenceFailed {
                    worker_id: self.id.clone(),
                    reason: format!("failed to parse streaming chunk: {}", e),
                })?;
                if done {
                    break 'stream;
                }
            }
        }

        if content.is_empty() || finish_reason == "content_filter" {
            warn!(
                worker_id = %self.id,
                job_id = %job.id,
                finish_reason,
                "streaming API returned empty or filtered response"
            );
        }

        let usage = usage.map(|u| {
            serde_json::json!({
                "prompt_tokens": u.prompt_tokens,
                "completion_tokens": u.completion_tokens,
                "total_tokens": u.total_tokens,
            })
        });

        info!(
            worker_id = %self.id,
            job_id = %job.id,
            response_len = content.len(),
            "remote API worker streaming response received"
        );

        Ok(serde_json::json!({
            "content": content,
            "usage": usage,
            "model": model,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::process_stream_line;

    #[test]
    fn process_stream_line_emits_delta() {
        let mut content = String::new();
        let mut usage = None;
        let mut finish_reason = "unknown".to_string();
        let mut emitted = Vec::new();
        let mut on_delta = |delta| emitted.push(delta);

        let done = process_stream_line(
            r#"data: {"choices":[{"delta":{"content":"hello"},"finish_reason":null}]}"#,
            &mut content,
            &mut usage,
            &mut finish_reason,
            &mut on_delta,
        )
        .unwrap();

        assert!(!done);
        assert_eq!(content, "hello");
        assert_eq!(emitted, vec!["hello"]);
    }

    #[test]
    fn process_stream_line_stops_on_done_event() {
        let mut content = String::new();
        let mut usage = None;
        let mut finish_reason = "unknown".to_string();
        let mut on_delta = |_| {};

        let done = process_stream_line(
            "data: [DONE]",
            &mut content,
            &mut usage,
            &mut finish_reason,
            &mut on_delta,
        )
        .unwrap();

        assert!(done);
    }
}

#[async_trait]
impl Worker for RemoteApiWorker {
    fn worker_id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> Vec<Capability> {
        self.caps.clone()
    }

    fn health_check(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn resource_requirements(&self) -> ResourceRequirements {
        ResourceRequirements {
            vram_mb: None,
            gpu_required: false,
            cpu_cores: None,
            memory_mb: None,
        }
    }

    async fn infer_stream(
        &self,
        job: &Job,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<serde_json::Value, WorkerError> {
        self.infer_stream_inner(job, on_delta).await
    }

    async fn infer(&self, job: &Job) -> Result<serde_json::Value, WorkerError> {
        let messages = job.payload["messages"].clone();
        let model = self.config.model.clone();

        let url = format!("{}/v1/chat/completions", self.config.base_url);
        info!(
            worker_id = %self.id,
            job_id = %job.id,
            model = %model,
            "remote API worker sending request"
        );

        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": false,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("HTTP request failed: {}", e),
            })?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("failed to read response body: {}", e),
            })?;

        if !status.is_success() {
            return Err(WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("API returned status {}: {}", status, response_text),
            });
        }

        let parsed: ChatCompletionResponse =
            serde_json::from_str(&response_text).map_err(|e| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("failed to parse response: {}", e),
            })?;

        let first_choice = parsed.choices.first();
        let finish_reason = first_choice
            .and_then(|c| c.finish_reason.as_deref())
            .unwrap_or("unknown");

        let content = first_choice
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        if content.is_empty() || finish_reason == "content_filter" {
            warn!(
                worker_id = %self.id,
                job_id = %job.id,
                finish_reason,
                response_len = response_text.len(),
                "API returned empty or filtered response"
            );
            // Log a truncated version of the response for debugging
            let preview: String = response_text.chars().take(500).collect();
            warn!(raw_preview = %preview, "raw API response (truncated)");
        }

        let usage = parsed.usage.map(|u| {
            serde_json::json!({
                "prompt_tokens": u.prompt_tokens,
                "completion_tokens": u.completion_tokens,
                "total_tokens": u.total_tokens,
            })
        });

        info!(
            worker_id = %self.id,
            job_id = %job.id,
            response_len = content.len(),
            "remote API worker response received"
        );

        Ok(serde_json::json!({
            "content": content,
            "usage": usage,
            "model": model,
        }))
    }
}
