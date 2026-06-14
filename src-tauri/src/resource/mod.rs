use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::protocol::{Job, ResourceRequirements};

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveLocalJob {
    pub job_id: String,
    pub worker_id: String,
    pub vram_mb: Option<u64>,
}

#[derive(Debug, Default)]
struct ResourceState {
    active_local_job: Option<ActiveLocalJob>,
}

#[derive(Debug)]
pub struct ResourceManager {
    state: Mutex<ResourceState>,
    vram_logs: bool,
}

impl ResourceManager {
    pub fn new(vram_logs: bool) -> Self {
        Self {
            state: Mutex::new(ResourceState::default()),
            vram_logs,
        }
    }

    pub fn requires_local_exclusivity(requirements: &ResourceRequirements) -> bool {
        requirements.gpu_required || requirements.vram_mb.is_some()
    }

    pub async fn try_acquire(
        &self,
        job: &Job,
        worker_id: &str,
        requirements: &ResourceRequirements,
    ) -> bool {
        if !Self::requires_local_exclusivity(requirements) {
            return true;
        }

        let mut state = self.state.lock().await;
        if let Some(active) = &state.active_local_job {
            if self.vram_logs {
                warn!(
                    job_id = %job.id,
                    worker_id = %worker_id,
                    active_job_id = %active.job_id,
                    active_worker_id = %active.worker_id,
                    "local model resource busy"
                );
            }
            return false;
        }

        state.active_local_job = Some(ActiveLocalJob {
            job_id: job.id.clone(),
            worker_id: worker_id.to_string(),
            vram_mb: requirements.vram_mb,
        });

        if self.vram_logs {
            info!(
                job_id = %job.id,
                worker_id = %worker_id,
                vram_mb = requirements.vram_mb,
                gpu_required = requirements.gpu_required,
                "local model resource acquired"
            );
        }

        true
    }

    pub async fn release(&self, job_id: &str) {
        let mut state = self.state.lock().await;
        let Some(active) = &state.active_local_job else {
            return;
        };

        if active.job_id != job_id {
            if self.vram_logs {
                warn!(
                    job_id = %job_id,
                    active_job_id = %active.job_id,
                    "ignoring resource release for non-active job"
                );
            }
            return;
        }

        let released = state.active_local_job.take();
        if self.vram_logs {
            if let Some(active) = released {
                info!(
                    job_id = %active.job_id,
                    worker_id = %active.worker_id,
                    vram_mb = active.vram_mb,
                    "local model resource released"
                );
            }
        }
    }

    #[cfg(test)]
    pub async fn active_local_job(&self) -> Option<ActiveLocalJob> {
        self.state.lock().await.active_local_job.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Capability, Job};

    fn local_requirements() -> ResourceRequirements {
        ResourceRequirements {
            vram_mb: Some(6000),
            gpu_required: false,
            cpu_cores: Some(4),
            memory_mb: Some(8000),
        }
    }

    #[tokio::test]
    async fn test_local_resource_exclusivity() {
        let manager = ResourceManager::new(false);
        let job_a = Job::new("persona_rewrite", Capability::Chat, serde_json::json!({}));
        let job_b = Job::new("persona_rewrite", Capability::Chat, serde_json::json!({}));
        let req = local_requirements();

        assert!(manager.try_acquire(&job_a, "local_qwen", &req).await);
        assert!(!manager.try_acquire(&job_b, "local_qwen", &req).await);

        manager.release(&job_a.id).await;
        assert!(manager.try_acquire(&job_b, "local_qwen", &req).await);
    }

    #[tokio::test]
    async fn test_remote_resource_does_not_lock() {
        let manager = ResourceManager::new(false);
        let job_a = Job::new("chat", Capability::Chat, serde_json::json!({}));
        let job_b = Job::new("chat", Capability::Chat, serde_json::json!({}));
        let req = ResourceRequirements {
            vram_mb: None,
            gpu_required: false,
            cpu_cores: None,
            memory_mb: None,
        };

        assert!(manager.try_acquire(&job_a, "remote", &req).await);
        assert!(manager.try_acquire(&job_b, "remote", &req).await);
        assert!(manager.active_local_job().await.is_none());
    }
}
