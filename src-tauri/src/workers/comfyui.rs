use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tokio::time::{sleep, Duration, Instant};
use tracing::{info, warn};

use crate::multimodal::{load_comfyui_prompt, resolve_project_path};
use crate::protocol::{Capability, Job, ResourceRequirements};

use super::{Worker, WorkerError};

#[derive(Debug, Deserialize)]
struct PromptResponse {
    prompt_id: String,
}

pub struct ComfyUiWorker {
    id: String,
    caps: Vec<Capability>,
    base_url: String,
    workflow_path: String,
    output_dir: String,
    client: Client,
    healthy: AtomicBool,
}

impl ComfyUiWorker {
    pub fn new(
        id: impl Into<String>,
        caps: Vec<Capability>,
        base_url: String,
        workflow_path: String,
        output_dir: String,
    ) -> Self {
        Self {
            id: id.into(),
            caps,
            base_url: base_url.trim_end_matches('/').to_string(),
            workflow_path,
            output_dir,
            client: Client::new(),
            healthy: AtomicBool::new(false),
        }
    }

    pub async fn health_check_http(&self) -> bool {
        let url = format!("{}/system_stats", self.base_url);
        let healthy = match self.client.get(&url).send().await {
            Ok(r) => r.status().is_success(),
            Err(_) => false,
        };
        self.healthy.store(healthy, Ordering::Relaxed);
        healthy
    }

    pub fn mark_unhealthy(&self) {
        self.healthy.store(false, Ordering::Relaxed);
    }

    async fn wait_for_history(
        &self,
        prompt_id: &str,
        timeout_ms: u64,
    ) -> Result<serde_json::Value, WorkerError> {
        let started = Instant::now();
        let url = format!("{}/history/{}", self.base_url, prompt_id);
        loop {
            let response =
                self.client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| WorkerError::InferenceFailed {
                        worker_id: self.id.clone(),
                        reason: format!("history request failed: {}", e),
                    })?;
            let status = response.status();
            let text = response
                .text()
                .await
                .map_err(|e| WorkerError::InferenceFailed {
                    worker_id: self.id.clone(),
                    reason: format!("failed to read history response: {}", e),
                })?;
            if !status.is_success() {
                return Err(WorkerError::InferenceFailed {
                    worker_id: self.id.clone(),
                    reason: format!("history status {}: {}", status, text),
                });
            }

            let history: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| WorkerError::InferenceFailed {
                    worker_id: self.id.clone(),
                    reason: format!("failed to parse history response: {}", e),
                })?;
            if let Some(entry) = history.get(prompt_id) {
                return Ok(entry.clone());
            }

            if started.elapsed() >= Duration::from_millis(timeout_ms) {
                return Err(WorkerError::Timeout {
                    worker_id: self.id.clone(),
                    elapsed_ms: timeout_ms,
                });
            }
            sleep(Duration::from_millis(750)).await;
        }
    }

    async fn download_images(
        &self,
        prompt_id: &str,
        history_entry: &serde_json::Value,
    ) -> Result<Vec<String>, WorkerError> {
        let output_dir = resolve_project_path(&self.output_dir);
        std::fs::create_dir_all(&output_dir).map_err(|e| WorkerError::InferenceFailed {
            worker_id: self.id.clone(),
            reason: format!("failed to create output directory: {}", e),
        })?;

        let mut saved = Vec::new();
        for image in history_images(history_entry) {
            let filename = image
                .get("filename")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("image.png");
            let subfolder = image
                .get("subfolder")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let kind = image
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("output");

            let mut url = reqwest::Url::parse(&format!("{}/view", self.base_url)).map_err(|e| {
                WorkerError::InferenceFailed {
                    worker_id: self.id.clone(),
                    reason: format!("invalid ComfyUI view URL: {}", e),
                }
            })?;
            url.query_pairs_mut()
                .append_pair("filename", filename)
                .append_pair("subfolder", subfolder)
                .append_pair("type", kind);

            let bytes = self
                .client
                .get(url)
                .send()
                .await
                .map_err(|e| WorkerError::InferenceFailed {
                    worker_id: self.id.clone(),
                    reason: format!("image download failed: {}", e),
                })?
                .bytes()
                .await
                .map_err(|e| WorkerError::InferenceFailed {
                    worker_id: self.id.clone(),
                    reason: format!("failed to read image bytes: {}", e),
                })?;

            let safe_name = filename.replace(['/', '\\'], "_");
            let local_path = output_dir.join(format!("{}_{}", prompt_id, safe_name));
            std::fs::write(&local_path, bytes).map_err(|e| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("failed to write image: {}", e),
            })?;
            saved.push(local_path.to_string_lossy().to_string());
        }

        Ok(saved)
    }
}

#[async_trait]
impl Worker for ComfyUiWorker {
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
            vram_mb: Some(8000),
            gpu_required: true,
            cpu_cores: Some(4),
            memory_mb: Some(8000),
        }
    }

    async fn infer(&self, job: &Job) -> Result<serde_json::Value, WorkerError> {
        let workflow_path = job
            .payload
            .get("workflow_path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&self.workflow_path);
        let output_dir = job
            .payload
            .get("output_dir")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&self.output_dir);
        let prompt_text = job
            .payload
            .get("prompt")
            .and_then(serde_json::Value::as_str);
        let negative_prompt = job
            .payload
            .get("negative_prompt")
            .and_then(serde_json::Value::as_str);

        let prompt =
            load_comfyui_prompt(workflow_path, prompt_text, negative_prompt).map_err(|e| {
                WorkerError::InferenceFailed {
                    worker_id: self.id.clone(),
                    reason: e,
                }
            })?;

        let url = format!("{}/prompt", self.base_url);
        let body = serde_json::json!({
            "prompt": prompt,
            "client_id": uuid::Uuid::new_v4().to_string(),
        });
        info!(
            worker_id = %self.id,
            job_id = %job.id,
            workflow_path,
            "submitting ComfyUI prompt"
        );

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("prompt request failed: {}", e),
            })?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("failed to read prompt response: {}", e),
            })?;
        if !status.is_success() {
            return Err(WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("prompt status {}: {}", status, text),
            });
        }

        let parsed: PromptResponse =
            serde_json::from_str(&text).map_err(|e| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("failed to parse prompt response: {}", e),
            })?;

        let history = self
            .wait_for_history(&parsed.prompt_id, job.timeout_ms)
            .await?;
        let worker_with_output = ComfyUiWorker {
            id: self.id.clone(),
            caps: self.caps.clone(),
            base_url: self.base_url.clone(),
            workflow_path: self.workflow_path.clone(),
            output_dir: output_dir.to_string(),
            client: self.client.clone(),
            healthy: AtomicBool::new(self.health_check()),
        };
        let images = worker_with_output
            .download_images(&parsed.prompt_id, &history)
            .await?;

        if images.is_empty() {
            warn!(
                worker_id = %self.id,
                job_id = %job.id,
                prompt_id = %parsed.prompt_id,
                "ComfyUI prompt completed without image outputs"
            );
        }

        Ok(serde_json::json!({
            "prompt_id": parsed.prompt_id,
            "images": images,
            "workflow_path": workflow_path,
        }))
    }
}

fn history_images(history_entry: &serde_json::Value) -> Vec<&serde_json::Value> {
    history_entry
        .get("outputs")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flat_map(|outputs| outputs.values())
        .filter_map(|output| output.get("images").and_then(serde_json::Value::as_array))
        .flat_map(|images| images.iter())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_images_extracts_outputs() {
        let history = serde_json::json!({
            "outputs": {
                "19": {
                    "images": [
                        { "filename": "a.png", "subfolder": "", "type": "output" }
                    ]
                }
            }
        });
        assert_eq!(history_images(&history).len(), 1);
    }
}
