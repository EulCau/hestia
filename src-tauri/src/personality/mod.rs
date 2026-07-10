use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaConfig {
    pub schema_version: u32,
    pub name: String,
    pub tone: String,
    pub initiative: f64,
    pub humor: f64,
    pub verbosity: String,
    pub style_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub struct PromptAssembler {
    persona: PersonaConfig,
}

impl PromptAssembler {
    pub fn load(profile_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let path = persona_read_path(profile_name);
        info!("loading persona from {}", path.display());
        let content = std::fs::read_to_string(&path)?;
        let persona: PersonaConfig = serde_json::from_str(&content)?;
        Ok(Self { persona })
    }

    pub fn build_system_prompt(&self) -> String {
        // Keep system prompt minimal and functional to avoid triggering
        // content safety filters. Style rules are applied via the persona
        // rewrite layer, not via system prompt instructions.
        "You are a helpful assistant. Respond concisely and precisely.".into()
    }

    pub fn assemble_messages_with_context(
        &self,
        user_message: &str,
        history: &[ChatMessage],
        context: Option<&str>,
    ) -> Vec<serde_json::Value> {
        let mut messages: Vec<serde_json::Value> = Vec::new();
        messages.push(serde_json::json!({
            "role": "system",
            "content": self.build_system_prompt(),
        }));
        if let Some(context) = context.map(str::trim).filter(|value| !value.is_empty()) {
            messages.push(serde_json::json!({
                "role": "system",
                "content": context,
            }));
        }
        for msg in history {
            messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            }));
        }
        messages.push(serde_json::json!({
            "role": "user",
            "content": user_message,
        }));
        messages
    }

    pub fn persona_name(&self) -> &str {
        &self.persona.name
    }

    pub fn persona_config(&self) -> PersonaConfig {
        self.persona.clone()
    }
}

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn user_persona_dir() -> PathBuf {
    project_root().join("usr").join("personality")
}

fn bundled_persona_dir_candidates() -> Vec<PathBuf> {
    vec![
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("personality"))),
        Some(project_root().join("personality")),
        Some(PathBuf::from("personality")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn persona_read_path(profile_name: &str) -> PathBuf {
    let filename = format!("{}.json", profile_name);
    let mut candidates = vec![user_persona_dir().join(&filename)];
    candidates.extend(
        bundled_persona_dir_candidates()
            .into_iter()
            .map(|dir| dir.join(&filename)),
    );
    for candidate in &candidates {
        if candidate.exists() {
            return candidate.clone();
        }
    }
    user_persona_dir().join(&filename)
}

fn persona_write_path(profile_name: &str) -> PathBuf {
    user_persona_dir().join(format!("{}.json", profile_name))
}

pub fn list_profiles() -> Vec<String> {
    let mut dirs = vec![user_persona_dir()];
    dirs.extend(bundled_persona_dir_candidates());
    let mut profiles = Vec::new();
    for dir in &dirs {
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for profile in entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
                    .filter_map(|e| {
                        e.path()
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                    })
                {
                    if !profiles.contains(&profile) {
                        profiles.push(profile);
                    }
                }
            }
        }
    }
    profiles
}

/// Builds a rewrite prompt for the local personality layer.
pub struct PersonaRewriter {
    persona: PersonaConfig,
    template: String,
    temperature: f64,
    max_tokens: u32,
}

impl PersonaRewriter {
    pub fn new(
        persona: PersonaConfig,
        template: String,
        temperature: f64,
        max_tokens: u32,
    ) -> Self {
        Self {
            persona,
            template,
            temperature,
            max_tokens,
        }
    }

    pub fn build_rewrite_job_payload(&self, raw_content: &str) -> serde_json::Value {
        let style_rules = self.persona.style_rules.join(
            "
- ",
        );
        let rewrite_prompt = self
            .template
            .replace("{tone}", &self.persona.tone)
            .replace("{style_rules}", &style_rules)
            .replace("{content}", raw_content);
        let messages = vec![
            serde_json::json!({
                "role": "system",
                "content": "You polish text to match a desired communication style. Given a message and style rules, enhance it to fit the rules while preserving the original voice and energy. If the original has personality, keep it. If it uses brief parenthetical states, actions, or tone markers, preserve them only when they remain generic and restrained. Only enforce hard constraints explicitly listed in the rules, such as punctuation format or no exclamation marks. Do not add fixed species, identity, age, gender, job, catchphrase, or body-feature cues. Do not make the text colder or more robotic. Output only the result."
            }),
            serde_json::json!({
                "role": "user",
                "content": rewrite_prompt,
            }),
        ];
        serde_json::json!({
            "messages": messages,
            "temperature": self.temperature,
            "max_tokens": self.max_tokens,
        })
    }
}

/// Read the raw JSON content of a persona profile.
pub fn read_persona_raw(profile_name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let path = persona_read_path(profile_name);
    Ok(std::fs::read_to_string(&path)?)
}

/// Save raw JSON content to a persona profile.
pub fn save_persona_raw(
    profile_name: &str,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate it's valid JSON and matches PersonaConfig schema
    let _parsed: PersonaConfig =
        serde_json::from_str(content).map_err(|e| format!("invalid persona JSON: {}", e))?;
    let path = persona_write_path(profile_name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    info!("persona saved to {}", path.display());
    Ok(())
}
