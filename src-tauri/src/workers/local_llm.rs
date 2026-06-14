use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, warn};

use crate::protocol::{Capability, Job, ResourceRequirements};

use super::{Worker, WorkerError};

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    content: String,
    #[serde(default)]
    reasoning_content: Option<String>,
}

pub struct LocalLlmWorker {
    id: String,
    caps: Vec<Capability>,
    base_url: String,
    model: String,
    client: Client,
    healthy: AtomicBool,
}

impl LocalLlmWorker {
    pub fn new(
        id: impl Into<String>,
        caps: Vec<Capability>,
        base_url: String,
        model: String,
    ) -> Self {
        Self {
            id: id.into(),
            caps,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            client: Client::new(),
            healthy: AtomicBool::new(false),
        }
    }

    pub async fn health_check_http(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        let healthy = match self.client.get(&url).send().await {
            Ok(r) => r.status().is_success(),
            Err(_) => false,
        };
        self.healthy.store(healthy, Ordering::Relaxed);
        healthy
    }
}

#[async_trait]
impl Worker for LocalLlmWorker {
    fn worker_id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> Vec<Capability> {
        self.caps.clone()
    }

    fn health_check(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }

    fn resource_requirements(&self) -> ResourceRequirements {
        ResourceRequirements {
            vram_mb: Some(6000),
            gpu_required: false,
            cpu_cores: Some(4),
            memory_mb: Some(8000),
        }
    }

    async fn infer(&self, job: &Job) -> Result<serde_json::Value, WorkerError> {
        let messages = job.payload["messages"].clone();
        let temperature = job.payload["temperature"].as_f64().unwrap_or(0.7);
        let max_tokens = job.payload["max_tokens"].as_u64().unwrap_or(512);

        let url = format!("{}/v1/chat/completions", self.base_url);
        info!(
            worker_id = %self.id,
            job_id = %job.id,
            url = %url,
            model = %self.model,
            "local LLM worker sending request"
        );

        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
            "stream": false,
        });

        let response = self
            .client
            .post(&url)
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
                reason: format!("failed to read response: {}", e),
            })?;

        if !status.is_success() {
            return Err(WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("API status {}: {}", status, response_text),
            });
        }

        let parsed: ChatCompletionResponse =
            serde_json::from_str(&response_text).map_err(|e| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("parse error: {}", e),
            })?;

        let mut content = parsed
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        // Qwen3 thinking mode: if content is empty, use reasoning_content
        if content.is_empty() {
            if let Some(rc) = parsed
                .choices
                .first()
                .and_then(|c| c.message.reasoning_content.clone())
            {
                warn!(
                    worker_id = %self.id,
                    "Qwen3 thinking mode: content empty, falling back to reasoning_content"
                );
                content = rc;
            }
        }

        info!(
            worker_id = %self.id,
            job_id = %job.id,
            response_len = content.len(),
            "local LLM worker response received"
        );

        Ok(serde_json::json!({ "content": content }))
    }
}
