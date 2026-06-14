use crate::config::AppConfig;
use tracing::info;

pub fn init(config: &AppConfig) {
    let obs = &config.observability;
    info!(
        "observability: job_timeline={} prompt_logs={} token_usage={} vram_logs={}",
        obs.job_timeline, obs.prompt_logs, obs.token_usage, obs.vram_logs
    );
}

pub fn log_prompt(assembled_messages: &serde_json::Value) {
    info!(
        target: "prompt_log",
        messages = %assembled_messages,
        "assembled prompt"
    );
}

pub fn log_token_usage(model: &str, usage: &serde_json::Value) {
    info!(
        target: "token_usage",
        model = %model,
        usage = %usage,
        "token usage"
    );
}
