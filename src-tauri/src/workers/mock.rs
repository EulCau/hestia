use std::time::Duration;

use async_trait::async_trait;
use tracing::info;

use crate::protocol::{Capability, Job, ResourceRequirements};

use super::{Worker, WorkerError};

pub struct MockWorker {
    id: String,
    caps: Vec<Capability>,
    delay_ms: u64,
}

impl MockWorker {
    pub fn new(id: impl Into<String>, caps: Vec<Capability>, delay_ms: u64) -> Self {
        Self {
            id: id.into(),
            caps,
            delay_ms,
        }
    }
}

#[async_trait]
impl Worker for MockWorker {
    fn worker_id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> Vec<Capability> {
        self.caps.clone()
    }

    fn health_check(&self) -> bool {
        true
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
        info!(
            worker_id = %self.id,
            job_id = %job.id,
            delay_ms = self.delay_ms,
            "mock worker infer start"
        );
        tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        info!(
            worker_id = %self.id,
            job_id = %job.id,
            "mock worker infer done"
        );
        Ok(serde_json::json!({
            "worker_id": self.id,
            "response": format!("mock response for job {}", job.id),
            "processed_at": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_worker_infer() {
        let worker = MockWorker::new("mock_chat", vec![Capability::Chat], 10);
        let job = Job::new("chat", Capability::Chat, serde_json::json!({"m": "hi"}));
        let result = worker.infer(&job).await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["worker_id"], "mock_chat");
    }
}
