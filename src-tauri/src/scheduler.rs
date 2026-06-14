use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, timeout, Duration, Instant};
use tracing::{error, info, warn};

use crate::protocol::{Job, JobStatus};
use crate::resource::ResourceManager;
use crate::workers::{WorkerError, WorkerRegistry};

#[derive(Debug)]
pub enum SchedulerCommand {
    Submit(Job),
    Run {
        job: Job,
        result_tx: oneshot::Sender<Result<serde_json::Value, String>>,
    },
    Cancel {
        job_id: String,
    },
    Shutdown,
}

pub struct Scheduler {
    cmd_rx: mpsc::Receiver<SchedulerCommand>,
    registry: Arc<WorkerRegistry>,
    resources: Arc<ResourceManager>,
    active_jobs: HashMap<String, Job>,
}

impl Scheduler {
    pub fn new(
        cmd_rx: mpsc::Receiver<SchedulerCommand>,
        registry: Arc<WorkerRegistry>,
        resources: Arc<ResourceManager>,
    ) -> Self {
        Self {
            cmd_rx,
            registry,
            resources,
            active_jobs: HashMap::new(),
        }
    }

    pub async fn run(mut self) {
        info!("scheduler loop started");

        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                SchedulerCommand::Submit(mut job) => {
                    job.transition_to(JobStatus::Queued).unwrap_or_else(|e| {
                        error!(job_id = %job.id, error = %e, "failed to queue job");
                    });
                    if job.status == JobStatus::Queued {
                        info!(
                            job_id = %job.id,
                            kind = %job.kind,
                            capability = ?job.capability,
                            "job queued"
                        );
                        let _ = self.dispatch_job(job).await;
                    }
                }
                SchedulerCommand::Run { mut job, result_tx } => {
                    job.transition_to(JobStatus::Queued).unwrap_or_else(|e| {
                        error!(job_id = %job.id, error = %e, "failed to queue job");
                    });
                    let result = self.dispatch_job(job).await.map_err(|e| e.to_string());
                    let _ = result_tx.send(result);
                }
                SchedulerCommand::Cancel { job_id } => {
                    if let Some(job) = self.active_jobs.get_mut(&job_id) {
                        if job.cancelable && !job.status.is_terminal() {
                            let _ = job.transition_to(JobStatus::Cancelled);
                            info!(job_id = %job_id, "job cancelled");
                        }
                    }
                }
                SchedulerCommand::Shutdown => {
                    info!("scheduler shutting down");
                    break;
                }
            }
        }

        info!("scheduler loop ended");
    }

    async fn dispatch_job(&mut self, mut job: Job) -> Result<serde_json::Value, WorkerError> {
        let worker = match self.registry.find_by_capability(&job.capability) {
            Some(w) => w,
            None => {
                error!(
                    job_id = %job.id,
                    capability = ?job.capability,
                    "no worker available, failing job"
                );
                let _ = job.transition_to(JobStatus::Failed);
                return Err(WorkerError::NoWorker {
                    capability: format!("{:?}", job.capability),
                });
            }
        };

        let requirements = worker.resource_requirements();
        let local_exclusive = ResourceManager::requires_local_exclusivity(&requirements);
        let wait_started = Instant::now();
        while !self
            .resources
            .try_acquire(&job, worker.worker_id(), &requirements)
            .await
        {
            if job.status == JobStatus::Queued {
                let _ = job.transition_to(JobStatus::WaitingResource);
                info!(
                    job_id = %job.id,
                    worker_id = %worker.worker_id(),
                    "job waiting for local model resource"
                );
            }

            if wait_started.elapsed() >= Duration::from_millis(job.timeout_ms) {
                let _ = job.transition_to(JobStatus::Timeout);
                warn!(
                    job_id = %job.id,
                    worker_id = %worker.worker_id(),
                    timeout_ms = job.timeout_ms,
                    "job timed out while waiting for local model resource"
                );
                return Err(WorkerError::Timeout {
                    worker_id: worker.worker_id().to_string(),
                    elapsed_ms: job.timeout_ms,
                });
            }

            sleep(Duration::from_millis(50)).await;
        }

        let _ = job.transition_to(JobStatus::Running);
        info!(
            job_id = %job.id,
            worker_id = %worker.worker_id(),
            "job dispatched"
        );

        let job_id = job.id.clone();
        self.active_jobs.insert(job_id.clone(), job.clone());

        let timeout_ms = job.timeout_ms;
        let infer_fut = worker.infer(&job);

        let result = match timeout(Duration::from_millis(timeout_ms), infer_fut).await {
            Ok(Ok(val)) => {
                if let Some(j) = self.active_jobs.get_mut(&job_id) {
                    let _ = j.transition_to(JobStatus::Completed);
                }
                info!(
                    job_id = %job_id,
                    "job completed successfully"
                );
                Ok(val)
            }
            Ok(Err(e)) => {
                if let Some(j) = self.active_jobs.get_mut(&job_id) {
                    let _ = j.transition_to(JobStatus::Failed);
                }
                error!(job_id = %job_id, error = %e, "job failed");
                Err(e)
            }
            Err(_elapsed) => {
                if let Some(j) = self.active_jobs.get_mut(&job_id) {
                    let _ = j.transition_to(JobStatus::Timeout);
                }
                warn!(job_id = %job_id, timeout_ms, "job timed out");
                Err(WorkerError::Timeout {
                    worker_id: worker.worker_id().to_string(),
                    elapsed_ms: timeout_ms,
                })
            }
        };

        if local_exclusive {
            self.resources.release(&job_id).await;
        }
        self.active_jobs.remove(&job_id);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Capability;
    use crate::resource::ResourceManager;
    use crate::workers::mock::MockWorker;

    fn make_registry() -> WorkerRegistry {
        let mut reg = WorkerRegistry::new();
        let mock = Arc::new(MockWorker::new("mock_chat", vec![Capability::Chat], 10));
        reg.register(mock);
        reg
    }

    fn make_chat_job() -> Job {
        Job::new(
            "chat",
            Capability::Chat,
            serde_json::json!({"message": "hello"}),
        )
    }

    #[tokio::test]
    async fn test_dispatch_completes() {
        let reg = Arc::new(make_registry());
        let (tx, rx) = mpsc::channel(32);
        let resources = Arc::new(ResourceManager::new(false));
        let scheduler = Scheduler::new(rx, reg.clone(), resources);

        let handle = tokio::spawn(scheduler.run());

        let job = make_chat_job();
        tx.send(SchedulerCommand::Submit(job)).await.unwrap();
        tx.send(SchedulerCommand::Shutdown).await.unwrap();

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_cancel_job() {
        let reg = Arc::new(make_registry());
        let (tx, rx) = mpsc::channel(32);
        let resources = Arc::new(ResourceManager::new(false));
        let scheduler = Scheduler::new(rx, reg.clone(), resources);

        let handle = tokio::spawn(scheduler.run());

        let job = make_chat_job();
        let job_id = job.id.clone();
        tx.send(SchedulerCommand::Submit(job)).await.unwrap();
        // Give it a moment to start running
        tokio::time::sleep(Duration::from_millis(5)).await;
        tx.send(SchedulerCommand::Cancel { job_id }).await.unwrap();
        tx.send(SchedulerCommand::Shutdown).await.unwrap();

        handle.await.unwrap();
    }
}
