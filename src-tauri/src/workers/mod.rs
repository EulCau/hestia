pub mod comfyui;
pub mod local_llm;
pub mod mock;
pub mod model_loader;
pub mod remote_api;
pub mod vision_api;

use std::collections::HashMap;
use std::sync::Arc;

use tracing::{error, info};

use crate::protocol::{Capability, Job, ResourceRequirements};

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("worker {worker_id}: inference failed: {reason}")]
    InferenceFailed { worker_id: String, reason: String },

    #[error("worker {worker_id}: timed out after {elapsed_ms}ms")]
    Timeout { worker_id: String, elapsed_ms: u64 },

    #[error("worker {worker_id}: interrupted")]
    Interrupted { worker_id: String },

    #[error("no worker available for capability {capability}")]
    NoWorker { capability: String },
}

#[async_trait::async_trait]
pub trait Worker: Send + Sync {
    fn worker_id(&self) -> &str;
    fn capabilities(&self) -> Vec<Capability>;
    fn health_check(&self) -> bool;
    fn resource_requirements(&self) -> ResourceRequirements;

    async fn infer(&self, job: &Job) -> Result<serde_json::Value, WorkerError>;

    async fn infer_stream(
        &self,
        job: &Job,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<serde_json::Value, WorkerError> {
        let result = self.infer(job).await?;
        if let Some(content) = result.get("content").and_then(serde_json::Value::as_str) {
            on_delta(content.to_string());
        }
        Ok(result)
    }
}

pub struct WorkerRegistry {
    workers: HashMap<String, Arc<dyn Worker>>,
}

impl WorkerRegistry {
    pub fn new() -> Self {
        Self {
            workers: HashMap::new(),
        }
    }

    pub fn register(&mut self, worker: Arc<dyn Worker>) {
        let id = worker.worker_id().to_string();
        info!(worker_id = %id, "registering worker");
        self.workers.insert(id, worker);
    }

    pub fn find_by_capability(&self, capability: &Capability) -> Option<Arc<dyn Worker>> {
        let mut candidates: Vec<&Arc<dyn Worker>> = self
            .workers
            .values()
            .filter(|w| w.health_check() && w.capabilities().contains(capability))
            .collect();

        if candidates.is_empty() {
            error!(
                capability = ?capability,
                "no healthy worker found for capability"
            );
            return None;
        }

        candidates.sort_by_key(|w| w.worker_id().to_string());
        Some(candidates[0].clone())
    }

    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Worker>> {
        self.workers.get(id).cloned()
    }
}

impl Default for WorkerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
