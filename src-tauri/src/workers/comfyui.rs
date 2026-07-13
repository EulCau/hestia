use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tokio::time::{sleep, Duration, Instant};
use tracing::{info, warn};

use crate::multimodal::{
    load_comfyui_prompt_with_overrides, resolve_project_path, ComfyPromptOverrides,
};
use crate::protocol::{Capability, Job, ResourceRequirements};

use super::{Worker, WorkerError};

#[derive(Debug, Deserialize)]
struct PromptResponse {
    prompt_id: String,
}

#[derive(Debug, Deserialize)]
struct UploadImageResponse {
    name: String,
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

    async fn upload_input_image(&self, path: &str) -> Result<String, WorkerError> {
        let image_path = std::path::Path::new(path);
        let filename = image_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("hestia-input.png")
            .to_string();
        let mime =
            image_mime_for_path(image_path).map_err(|reason| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason,
            })?;
        let bytes = std::fs::read(image_path).map_err(|e| WorkerError::InferenceFailed {
            worker_id: self.id.clone(),
            reason: format!("failed to read input image: {}", e),
        })?;
        let boundary = format!("hestia-{}", uuid::Uuid::new_v4());
        let body = build_multipart_image_body(&boundary, &filename, mime, &bytes);
        let url = format!("{}/upload/image", self.base_url);
        let response = self
            .client
            .post(&url)
            .header(
                reqwest::header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body)
            .send()
            .await
            .map_err(|e| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("input image upload failed: {}", e),
            })?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("failed to read upload response: {}", e),
            })?;
        if !status.is_success() {
            return Err(WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("image upload status {}: {}", status, text),
            });
        }
        let parsed: UploadImageResponse =
            serde_json::from_str(&text).map_err(|e| WorkerError::InferenceFailed {
                worker_id: self.id.clone(),
                reason: format!("failed to parse upload response: {}", e),
            })?;
        Ok(parsed.name)
    }
}

fn build_multipart_image_body(boundary: &str, filename: &str, mime: &str, bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"image\"; filename=\"{}\"\r\n",
            filename.replace('"', "_")
        )
        .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {mime}\r\n\r\n").as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"overwrite\"\r\n\r\ntrue\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
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
        let mode = job
            .payload
            .get("mode")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("text_to_image");
        let input_image_path = job
            .payload
            .get("input_image_path")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let denoise = job
            .payload
            .get("denoise")
            .and_then(serde_json::Value::as_f64);

        let uploaded_input_image = if let Some(path) = input_image_path {
            Some(self.upload_input_image(path).await?)
        } else {
            None
        };

        let prompt = load_comfyui_prompt_with_overrides(
            workflow_path,
            ComfyPromptOverrides {
                prompt: prompt_text,
                negative_prompt,
                input_image: uploaded_input_image.as_deref(),
                denoise,
            },
        )
        .map_err(|e| WorkerError::InferenceFailed {
            worker_id: self.id.clone(),
            reason: e,
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
            "mode": mode,
            "input_image_path": input_image_path,
        }))
    }
}

fn image_mime_for_path(path: &std::path::Path) -> Result<&'static str, String> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Ok("image/png"),
        Some("jpg" | "jpeg") => Ok("image/jpeg"),
        Some("webp") => Ok("image/webp"),
        Some("gif") => Ok("image/gif"),
        _ => Err("unsupported input image format. Supported formats: png, jpeg, webp, gif".into()),
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

    #[test]
    fn test_build_multipart_image_body() {
        let body = build_multipart_image_body("test-boundary", "a.png", "image/png", b"abc");
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("name=\"image\"; filename=\"a.png\""));
        assert!(text.contains("Content-Type: image/png"));
        assert!(text.contains("name=\"overwrite\""));
        assert!(text.ends_with("--test-boundary--\r\n"));
    }
}
