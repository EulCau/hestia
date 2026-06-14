use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("invalid status transition from {from:?} to {to:?}")]
    InvalidTransition { from: JobStatus, to: JobStatus },

    #[error("job {0} not found")]
    JobNotFound(String),

    #[error("worker not found for capability {0:?}")]
    NoWorkerForCapability(Capability),
}

pub type ProtocolResult<T> = Result<T, ProtocolError>;

impl JobStatus {
    pub fn can_transition_to(&self, next: &JobStatus) -> bool {
        matches!(
            (self, next),
            (JobStatus::Queued, JobStatus::WaitingResource)
                | (JobStatus::Queued, JobStatus::Running)
                | (JobStatus::Queued, JobStatus::Cancelled)
                | (JobStatus::WaitingResource, JobStatus::Running)
                | (JobStatus::WaitingResource, JobStatus::Cancelled)
                | (JobStatus::Running, JobStatus::Completed)
                | (JobStatus::Running, JobStatus::Failed)
                | (JobStatus::Running, JobStatus::Cancelled)
                | (JobStatus::Running, JobStatus::Timeout)
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled | JobStatus::Timeout
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceRequirements {
    pub vram_mb: Option<u64>,
    pub gpu_required: bool,
    pub cpu_cores: Option<u32>,
    pub memory_mb: Option<u64>,
}

pub fn new_job_id() -> String {
    let raw = Uuid::new_v4().to_string();
    format!("job_{}", &raw.replace('-', "")[..12])
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    WaitingResource,
    Running,
    Completed,
    Failed,
    Cancelled,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Chat,
    Vision,
    ImageGeneration,
    ImageEditing,
    Tts,
    MemorySummary,
    Ocr,
    Embedding,
    Rerank,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Job {
    pub id: String,
    pub kind: String,
    pub capability: Capability,
    pub priority: i32,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub timeout_ms: u64,
    pub cancelable: bool,
    pub payload: serde_json::Value,
}

impl Job {
    pub fn new(
        kind: impl Into<String>,
        capability: Capability,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: new_job_id(),
            kind: kind.into(),
            capability,
            priority: 0,
            status: JobStatus::Queued,
            created_at: Utc::now(),
            timeout_ms: 30000,
            cancelable: true,
            payload,
        }
    }

    pub fn transition_to(&mut self, status: JobStatus) -> ProtocolResult<()> {
        if self.status.can_transition_to(&status) {
            tracing::debug!(
                id = %self.id,
                from = ?self.status,
                to = ?status,
                "job status transition"
            );
            self.status = status;
            Ok(())
        } else {
            Err(ProtocolError::InvalidTransition {
                from: self.status.clone(),
                to: status,
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub capabilities: Vec<Capability>,
    pub execution: ExecutionKind,
    pub priority: i32,
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionKind {
    RemoteApi,
    LocalProcess,
    InProcess,
    HttpAdapter,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEventKind {
    JobCreated,
    JobStatusChanged,
    WorkerRegistered,
    WorkerHealthChanged,
    ResourceStateChanged,
    ConfigReloaded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeEvent {
    pub kind: RuntimeEventKind,
    pub timestamp: DateTime<Utc>,
    pub payload: serde_json::Value,
}

impl RuntimeEvent {
    pub fn new(kind: RuntimeEventKind, payload: serde_json::Value) -> Self {
        Self {
            kind,
            timestamp: Utc::now(),
            payload,
        }
    }
}

pub fn format_job_timeline_entry(job: &Job) -> String {
    format!(
        "job={} kind={} capability={:?} status={:?}",
        job.id, job.kind, job.capability, job.status
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert!(JobStatus::Queued.can_transition_to(&JobStatus::Running));
        assert!(JobStatus::Queued.can_transition_to(&JobStatus::Cancelled));
        assert!(JobStatus::Queued.can_transition_to(&JobStatus::WaitingResource));
        assert!(JobStatus::Running.can_transition_to(&JobStatus::Completed));
        assert!(JobStatus::Running.can_transition_to(&JobStatus::Failed));
        assert!(JobStatus::Running.can_transition_to(&JobStatus::Timeout));
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(!JobStatus::Completed.can_transition_to(&JobStatus::Running));
        assert!(!JobStatus::Failed.can_transition_to(&JobStatus::Running));
        assert!(!JobStatus::Cancelled.can_transition_to(&JobStatus::Running));
        assert!(!JobStatus::Timeout.can_transition_to(&JobStatus::Running));
        assert!(!JobStatus::Completed.can_transition_to(&JobStatus::Queued));
    }

    #[test]
    fn test_terminal_status() {
        assert!(JobStatus::Completed.is_terminal());
        assert!(JobStatus::Failed.is_terminal());
        assert!(JobStatus::Cancelled.is_terminal());
        assert!(JobStatus::Timeout.is_terminal());
        assert!(!JobStatus::Queued.is_terminal());
        assert!(!JobStatus::Running.is_terminal());
    }

    #[test]
    fn test_job_transition_mutation() {
        let mut job = Job::new(
            "chat",
            Capability::Chat,
            serde_json::json!({"message": "hello"}),
        );
        assert_eq!(job.status, JobStatus::Queued);

        job.transition_to(JobStatus::Running).unwrap();
        assert_eq!(job.status, JobStatus::Running);

        job.transition_to(JobStatus::Completed).unwrap();
        assert_eq!(job.status, JobStatus::Completed);

        let err = job.transition_to(JobStatus::Running).unwrap_err();
        assert!(matches!(err, ProtocolError::InvalidTransition { .. }));
    }
}
