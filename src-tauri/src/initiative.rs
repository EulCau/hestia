use std::time::{Duration, Instant};

use serde::Serialize;
use tracing::info;

use crate::config::InitiativeSection;

#[derive(Debug, Clone, Serialize)]
pub struct InitiativeDecision {
    pub enabled: bool,
    pub allowed: bool,
    pub level: f64,
    pub score: f64,
    pub idle_ms: u64,
    pub min_idle_ms: u64,
    pub cooldown_remaining_ms: u64,
    pub reasons: Vec<String>,
    pub suggested_prompt: String,
}

pub struct InitiativeRuntime {
    last_user_activity: Instant,
    last_initiative_at: Option<Instant>,
    decisions: Vec<InitiativeDecision>,
}

impl InitiativeRuntime {
    pub fn new() -> Self {
        Self {
            last_user_activity: Instant::now(),
            last_initiative_at: None,
            decisions: Vec::new(),
        }
    }

    pub fn record_user_activity(&mut self) {
        self.last_user_activity = Instant::now();
    }

    pub fn mark_initiative_spoken(&mut self) {
        self.last_initiative_at = Some(Instant::now());
    }

    pub fn evaluate(&mut self, config: &InitiativeSection, trigger: &str) -> InitiativeDecision {
        let now = Instant::now();
        let level = config.level.clamp(0.0, 1.0);
        let idle_ms = duration_ms(now.duration_since(self.last_user_activity));
        let min_idle_ms = min_idle_ms(level);
        let cooldown_remaining_ms = self.cooldown_remaining_ms(config.cooldown_ms, now);
        let idle_ratio = if min_idle_ms == 0 {
            1.0
        } else {
            (idle_ms as f64 / min_idle_ms as f64).min(1.0)
        };
        let score = (idle_ratio * 0.7 + level * 0.3).min(1.0);

        let mut reasons = Vec::new();
        if !config.enabled {
            reasons.push("initiative_disabled".into());
        }
        if trigger != "manual" && !trigger.starts_with("companion") {
            reasons.push("non_companion_trigger".into());
        }
        if idle_ms < min_idle_ms {
            reasons.push("user_recently_active".into());
        }
        if cooldown_remaining_ms > 0 {
            reasons.push("cooldown_active".into());
        }
        if score < 0.75 {
            reasons.push("score_below_threshold".into());
        }

        let decision = InitiativeDecision {
            enabled: config.enabled,
            allowed: reasons.is_empty(),
            level,
            score,
            idle_ms,
            min_idle_ms,
            cooldown_remaining_ms,
            reasons,
            suggested_prompt: "基于当前对话上下文, 生成一句简短、不打扰用户的主动发言. 不要假装看到了屏幕, 不要提出需要大量操作的问题.".into(),
        };

        info!(
            trigger,
            allowed = decision.allowed,
            score = decision.score,
            idle_ms = decision.idle_ms,
            min_idle_ms = decision.min_idle_ms,
            cooldown_remaining_ms = decision.cooldown_remaining_ms,
            reasons = ?decision.reasons,
            "initiative decision"
        );

        self.decisions.push(decision.clone());
        if self.decisions.len() > 20 {
            self.decisions.remove(0);
        }
        decision
    }

    pub fn recent_decisions(&self) -> &[InitiativeDecision] {
        &self.decisions
    }

    fn cooldown_remaining_ms(&self, cooldown_ms: u64, now: Instant) -> u64 {
        let Some(last) = self.last_initiative_at else {
            return 0;
        };
        let elapsed = duration_ms(now.duration_since(last));
        cooldown_ms.saturating_sub(elapsed)
    }
}

fn min_idle_ms(level: f64) -> u64 {
    let low = 15.0 * 60.0 * 1000.0;
    let high = 2.0 * 60.0 * 1000.0;
    ((1.0 - level) * low + level * high) as u64
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_config() -> InitiativeSection {
        InitiativeSection {
            enabled: true,
            level: 1.0,
            cooldown_ms: 1000,
        }
    }

    #[test]
    fn test_disabled_blocks_initiative() {
        let mut runtime = InitiativeRuntime::new();
        let mut config = enabled_config();
        config.enabled = false;
        let decision = runtime.evaluate(&config, "test");
        assert!(!decision.allowed);
        assert!(decision.reasons.contains(&"initiative_disabled".into()));
    }

    #[test]
    fn test_recent_user_activity_blocks_initiative() {
        let mut runtime = InitiativeRuntime::new();
        let decision = runtime.evaluate(&enabled_config(), "test");
        assert!(!decision.allowed);
        assert!(decision.reasons.contains(&"user_recently_active".into()));
    }

    #[test]
    fn test_cooldown_blocks_initiative() {
        let mut runtime = InitiativeRuntime::new();
        runtime.last_user_activity = Instant::now() - Duration::from_secs(600);
        runtime.mark_initiative_spoken();
        let decision = runtime.evaluate(&enabled_config(), "test");
        assert!(!decision.allowed);
        assert!(decision.reasons.contains(&"cooldown_active".into()));
    }

    #[test]
    fn test_non_companion_trigger_blocks_automatic_initiative() {
        let mut runtime = InitiativeRuntime::new();
        runtime.last_user_activity = Instant::now() - Duration::from_secs(600);
        let decision = runtime.evaluate(&enabled_config(), "timer");
        assert!(!decision.allowed);
        assert!(decision.reasons.contains(&"non_companion_trigger".into()));
    }
}
