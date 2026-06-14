use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{info, warn};

use crate::config::VisionSection;
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
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

pub struct VisionApiWorker {
    id: String,
    caps: Vec<Capability>,
    config: VisionSection,
    client: Client,
    api_key: String,
}

impl VisionApiWorker {
    pub fn new(
        id: impl Into<String>,
        caps: Vec<Capability>,
        config: VisionSection,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let api_key = if let Some(ref key) = config.api_key {
            if key.is_empty() {
                return Err("vision api_key is empty".into());
            }
            key.clone()
        } else {
            std::env::var(&config.api_key_env).map_err(|_| {
                format!(
                    "environment variable {} not set and no vision api_key in user config",
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
}

#[async_trait]
impl Worker for VisionApiWorker {
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

    async fn infer(&self, job: &Job) -> Result<serde_json::Value, WorkerError> {
        let image_data_url = job
            .payload
            .get("image_data_url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: "vision job missing image_data_url".into(),
            })?;
        let prompt = job
            .payload
            .get("prompt")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&self.config.default_prompt);
        let source = job
            .payload
            .get("source")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("upload");

        let url = chat_completions_url(&self.config.base_url);
        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                {
                    "role": "system",
                    "content": self.config.system_prompt
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "image_url",
                            "image_url": { "url": image_data_url }
                        },
                        {
                            "type": "text",
                            "text": prompt
                        }
                    ]
                }
            ],
            "stream": false
        });

        info!(
            worker_id = %self.id,
            job_id = %job.id,
            model = %self.config.model,
            source,
            "vision API worker sending request"
        );

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
            .map(|choice| choice.message.content.clone())
            .unwrap_or_default();

        if content.is_empty() || finish_reason == "content_filter" {
            warn!(
                worker_id = %self.id,
                job_id = %job.id,
                finish_reason,
                "vision API returned empty or filtered response"
            );
        }

        let usage = parsed.usage.map(|u| {
            serde_json::json!({
                "prompt_tokens": u.prompt_tokens,
                "completion_tokens": u.completion_tokens,
                "total_tokens": u.total_tokens,
            })
        });

        Ok(serde_json::json!({
            "content": content,
            "usage": usage,
            "model": self.config.model,
            "source": source,
        }))
    }
}

fn chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    }
}

#[cfg(test)]
mod tests {
    use super::chat_completions_url;

    #[test]
    fn test_chat_completions_url() {
        assert_eq!(
            chat_completions_url("https://api.moonshot.ai"),
            "https://api.moonshot.ai/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://api.moonshot.ai/v1"),
            "https://api.moonshot.ai/v1/chat/completions"
        );
    }
}
