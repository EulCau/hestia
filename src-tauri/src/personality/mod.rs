use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

fn default_schema_version() -> u32 {
    2
}

fn default_role_id() -> String {
    "default".into()
}

fn default_role_name() -> String {
    "Hestia".into()
}

fn default_verbosity() -> String {
    "medium".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_role_id")]
    pub id: String,
    #[serde(default = "default_role_name")]
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub identity: String,
    #[serde(default)]
    pub species: String,
    #[serde(default)]
    pub appearance: String,
    #[serde(default)]
    pub avatar: RoleAvatarConfig,
    #[serde(default)]
    pub personality: String,
    #[serde(default)]
    pub language_style: String,
    #[serde(default)]
    pub scenario: String,
    #[serde(default)]
    pub tone: String,
    #[serde(default)]
    pub initiative: f64,
    #[serde(default)]
    pub humor: f64,
    #[serde(default = "default_verbosity")]
    pub verbosity: String,
    #[serde(default)]
    pub style_rules: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub created_at: Option<u64>,
    #[serde(default)]
    pub updated_at: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoleAvatarConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub model_type: String,
    #[serde(default)]
    pub image_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub struct PromptAssembler {
    persona: PersonaConfig,
}

pub fn runtime_metadata_message() -> serde_json::Value {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    serde_json::json!({
        "role": "system",
        "content": format!("Current request timestamp (unix_ms): {timestamp_ms}. This dynamic metadata is intentionally placed last to preserve prompt-prefix cacheability."),
    })
}

impl PromptAssembler {
    pub fn load(profile_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        info!("loading persona profile {}", profile_name);
        let content = read_persona_raw(profile_name)?;
        let persona: PersonaConfig = serde_json::from_str(&content)?;
        Ok(Self { persona })
    }

    pub fn build_system_prompt(&self) -> String {
        let aliases = if self.persona.aliases.is_empty() {
            self.persona.name.clone()
        } else {
            format!("{}, {}", self.persona.name, self.persona.aliases.join(", "))
        };
        [
            "You are the character described below. The user's references to any listed name or alias refer to you, the character you are role-playing.",
            "Base style rules:",
            "- When replying in Chinese, use halfwidth punctuation only: , . ; : ? !",
            "- Parentheses may be used for brief actions, states, tone, or expressions when appropriate.",
            "- Do not let style override facts, reasoning, safety, or the user's current request.",
            "- If long-term memory conflicts with the current user message, prefer the current user message.",
            "- Dynamic runtime metadata, including timestamps, is supplied at the end of the message list.",
            "",
            "Character profile:",
            &format!("- Name and aliases: {aliases}"),
            &format!("- Identity: {}", empty_as_unspecified(&self.persona.identity)),
            &format!("- Species: {}", empty_as_unspecified(&self.persona.species)),
            &format!("- Appearance: {}", empty_as_unspecified(&self.persona.appearance)),
            &format!("- Personality: {}", empty_as_unspecified(&self.persona.personality)),
            &format!("- Language habits: {}", empty_as_unspecified(&self.persona.language_style)),
            &format!("- Scenario: {}", empty_as_unspecified(&self.persona.scenario)),
        ]
        .join("\n")
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
        messages.push(runtime_metadata_message());
        messages
    }

    pub fn persona_name(&self) -> &str {
        &self.persona.name
    }

    pub fn persona_config(&self) -> PersonaConfig {
        self.persona.clone()
    }
}

fn empty_as_unspecified(value: &str) -> &str {
    if value.trim().is_empty() {
        "unspecified"
    } else {
        value
    }
}

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn user_data_root() -> PathBuf {
    if let Ok(dir) = std::env::var("HESTIA_USER_DIR") {
        return PathBuf::from(dir);
    }
    if cfg!(debug_assertions) {
        return project_root().join("usr");
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("hestia");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("hestia");
        }
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("hestia");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("hestia");
    }
    PathBuf::from(".").join("hestia-user-data")
}

fn user_persona_dir() -> PathBuf {
    user_data_root().join("roles")
}

fn bundled_persona_dir_candidates() -> Vec<PathBuf> {
    vec![
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("roles"))),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("personality"))),
        Some(project_root().join("roles")),
        Some(project_root().join("personality")),
        Some(PathBuf::from("personality")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn persona_read_path(profile_name: &str) -> PathBuf {
    let profile_name = sanitize_profile_id(profile_name);
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
    user_persona_dir().join(format!("{}.json", sanitize_profile_id(profile_name)))
}

fn sanitize_profile_id(profile_name: &str) -> String {
    let value: String = profile_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .collect();
    if value.trim().is_empty() {
        "default".into()
    } else {
        value
    }
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
    if !profiles.contains(&"default".to_string()) {
        profiles.push("default".into());
    }
    profiles
}

pub fn list_role_configs() -> Result<Vec<PersonaConfig>, Box<dyn std::error::Error>> {
    let mut roles = Vec::new();
    for profile in list_profiles() {
        if let Ok(content) = read_persona_raw(&profile) {
            if let Ok(mut role) = serde_json::from_str::<PersonaConfig>(&content) {
                if role.id.trim().is_empty() {
                    role.id = profile;
                }
                roles.push(role);
            }
        }
    }
    roles.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(roles)
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
        let role_rules = [
            format!("Character name: {}", self.persona.name),
            format!("Aliases: {}", self.persona.aliases.join(", ")),
            format!("Identity: {}", self.persona.identity),
            format!("Species: {}", self.persona.species),
            format!("Appearance: {}", self.persona.appearance),
            format!("Personality: {}", self.persona.personality),
            format!("Language habits: {}", self.persona.language_style),
            format!("Tone: {}", self.persona.tone),
        ]
        .join(
            "
- ",
        );
        let rewrite_prompt = self
            .template
            .replace("{tone}", &self.persona.tone)
            .replace("{style_rules}", &role_rules)
            .replace("{content}", raw_content);
        let messages = vec![
            serde_json::json!({
                "role": "system",
                "content": "You polish text to match the character profile and base style. Preserve meaning. Chinese replies must use halfwidth punctuation only. Parentheses may contain brief actions, states, tone, or expressions when appropriate. Do not add constraints that are not in the character profile. Output only the result."
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
    match std::fs::read_to_string(&path) {
        Ok(content) => Ok(content),
        Err(_) if sanitize_profile_id(profile_name) == "default" => Ok(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../personality/default.json"
        ))
        .to_string()),
        Err(error) => Err(error.into()),
    }
}

/// Save raw JSON content to a persona profile.
pub fn save_persona_raw(
    profile_name: &str,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate it's valid JSON and matches PersonaConfig schema
    let parsed: PersonaConfig =
        serde_json::from_str(content).map_err(|e| format!("invalid persona JSON: {}", e))?;
    let sanitized_profile = sanitize_profile_id(profile_name);
    if parsed.id != sanitized_profile {
        return Err(format!("role JSON id must equal file id: {sanitized_profile}").into());
    }
    let path = persona_write_path(profile_name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    info!("persona saved to {}", path.display());
    Ok(())
}

pub fn delete_persona(profile_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if profile_name == "default" {
        return Err("default role cannot be deleted".into());
    }
    let path = persona_write_path(profile_name);
    if !path.exists() {
        return Err(format!("role override not found: {profile_name}").into());
    }
    std::fs::remove_file(&path)?;
    let assets = role_asset_dir(profile_name);
    if assets.exists() {
        std::fs::remove_dir_all(assets)?;
    }
    Ok(())
}

pub fn role_storage_path(profile_name: &str) -> String {
    persona_write_path(profile_name).display().to_string()
}

pub fn role_asset_dir(profile_name: &str) -> PathBuf {
    user_persona_dir()
        .join(sanitize_profile_id(profile_name))
        .join("avatar")
}
