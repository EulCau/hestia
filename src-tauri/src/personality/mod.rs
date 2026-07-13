use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
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
    system_prompt_language: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemPromptTemplate {
    pub id: String,
    pub title: String,
    pub description: String,
    pub content: String,
    pub default_content: String,
    pub overridden: bool,
}

pub fn runtime_metadata_message() -> serde_json::Value {
    runtime_metadata_message_with_language("en")
}

pub fn runtime_metadata_message_with_language(language: &str) -> serde_json::Value {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let content = render_named_template(
        &read_system_prompt_template("runtime_metadata", language)
            .unwrap_or_else(|_| default_runtime_metadata_template(language)),
        &[("timestamp_ms", timestamp_ms.to_string())],
    );
    serde_json::json!({
        "role": "system",
        "content": content,
    })
}

impl PromptAssembler {
    pub fn load(profile_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        info!("loading persona profile {}", profile_name);
        let content = read_persona_raw(profile_name)?;
        let persona: PersonaConfig = serde_json::from_str(&content)?;
        Ok(Self {
            persona,
            system_prompt_language: "en".into(),
        })
    }

    pub fn with_system_prompt_language(mut self, language: impl Into<String>) -> Self {
        self.system_prompt_language = language.into();
        self
    }

    pub fn build_system_prompt(&self) -> String {
        let aliases = if self.persona.aliases.is_empty() {
            self.persona.name.clone()
        } else {
            format!("{}, {}", self.persona.name, self.persona.aliases.join(", "))
        };
        let language = self.system_prompt_language.as_str();
        let template = read_system_prompt_template("role_system", language)
            .unwrap_or_else(|_| default_role_system_template(language));
        let unspecified = if language == "zh-CN" {
            empty_as_unspecified_zh
        } else {
            empty_as_unspecified
        };
        render_named_template(
            &template,
            &[
                ("name", self.persona.name.clone()),
                ("aliases", aliases),
                ("identity", unspecified(&self.persona.identity).to_string()),
                ("species", unspecified(&self.persona.species).to_string()),
                (
                    "appearance",
                    unspecified(&self.persona.appearance).to_string(),
                ),
                (
                    "personality",
                    unspecified(&self.persona.personality).to_string(),
                ),
                (
                    "language_style",
                    unspecified(&self.persona.language_style).to_string(),
                ),
                ("scenario", unspecified(&self.persona.scenario).to_string()),
                ("tone", unspecified(&self.persona.tone).to_string()),
            ],
        )
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
        messages.push(runtime_metadata_message_with_language(
            &self.system_prompt_language,
        ));
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

fn empty_as_unspecified_zh(value: &str) -> &str {
    if value.trim().is_empty() {
        "未指定"
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

fn user_prompt_dir() -> PathBuf {
    user_data_root().join("prompts")
}

fn sanitize_prompt_language(language: &str) -> String {
    match language.trim() {
        "zh-CN" => "zh-CN".into(),
        _ => "en".into(),
    }
}

fn sanitize_prompt_id(id: &str) -> String {
    match id.trim() {
        "role_system" | "memory_context" | "runtime_metadata" => id.trim().into(),
        _ => "role_system".into(),
    }
}

fn prompt_template_path(id: &str, language: &str) -> PathBuf {
    user_prompt_dir()
        .join(sanitize_prompt_language(language))
        .join(format!("{}.md", sanitize_prompt_id(id)))
}

fn default_role_system_template(language: &str) -> String {
    if language == "zh-CN" {
        [
            "你正在扮演下方描述的角色. 用户提到任何列出的名字或别名时, 都是在指代你扮演的这个角色.",
            "",
            "## 基础风格规则",
            "- 使用中文回复时, 只使用半角标点: , . ; : ? !",
            "- 合适时可以用括号写简短动作, 状态, 语气或表情.",
            "- 风格不能覆盖事实, 推理, 安全要求或用户当前请求.",
            "- 如果长期记忆和用户当前消息冲突, 以用户当前消息为准.",
            "- 动态运行信息, 包括时间戳, 会放在消息列表末尾.",
            "",
            "## 角色设定",
            "- 名字和别名: {aliases}",
            "- 身份: {identity}",
            "- 物种: {species}",
            "- 外观: {appearance}",
            "- 性格: {personality}",
            "- 语言习惯: {language_style}",
            "- 场景: {scenario}",
            "- 总体语气: {tone}",
        ]
        .join("\n")
    } else {
        [
            "You are the character described below. The user's references to any listed name or alias refer to you, the character you are role-playing.",
            "",
            "## Base style rules",
            "- When replying in Chinese, use halfwidth punctuation only: , . ; : ? !",
            "- Parentheses may be used for brief actions, states, tone, or expressions when appropriate.",
            "- Do not let style override facts, reasoning, safety, or the user's current request.",
            "- If long-term memory conflicts with the current user message, prefer the current user message.",
            "- Dynamic runtime metadata, including timestamps, is supplied at the end of the message list.",
            "",
            "## Character profile",
            "- Name and aliases: {aliases}",
            "- Identity: {identity}",
            "- Species: {species}",
            "- Appearance: {appearance}",
            "- Personality: {personality}",
            "- Language habits: {language_style}",
            "- Scenario: {scenario}",
            "- Overall tone: {tone}",
        ]
        .join("\n")
    }
}

fn default_memory_context_template(language: &str) -> String {
    if language == "zh-CN" {
        [
            "相关长期记忆. 只在有助于回答当前请求时使用. 如果它和用户当前消息冲突, 优先相信用户当前消息.",
            "",
            "{memory_items}",
        ]
        .join("\n")
    } else {
        [
            "Relevant long-term memory. Use only when it helps answer the current request. If it conflicts with the current user message, prefer the current user message.",
            "",
            "{memory_items}",
        ]
        .join("\n")
    }
}

fn default_runtime_metadata_template(language: &str) -> String {
    if language == "zh-CN" {
        "当前请求时间戳 (unix_ms): {timestamp_ms}. 此动态元数据故意放在消息列表末尾, 以保留提示词前缀缓存命中率.".into()
    } else {
        "Current request timestamp (unix_ms): {timestamp_ms}. This dynamic metadata is intentionally placed last to preserve prompt-prefix cacheability.".into()
    }
}

fn default_system_prompt_template(id: &str, language: &str) -> String {
    match sanitize_prompt_id(id).as_str() {
        "memory_context" => default_memory_context_template(language),
        "runtime_metadata" => default_runtime_metadata_template(language),
        _ => default_role_system_template(language),
    }
}

fn read_system_prompt_template(
    id: &str,
    language: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let path = prompt_template_path(id, language);
    if path.exists() {
        return Ok(std::fs::read_to_string(path)?);
    }
    Ok(default_system_prompt_template(
        id,
        &sanitize_prompt_language(language),
    ))
}

pub fn list_system_prompt_templates(
    language: &str,
) -> Result<Vec<SystemPromptTemplate>, Box<dyn std::error::Error>> {
    let language = sanitize_prompt_language(language);
    let items = [
        (
            "role_system",
            if language == "zh-CN" {
                "角色基础系统提示词"
            } else {
                "Role base system prompt"
            },
            if language == "zh-CN" {
                "定义角色扮演边界, 基础风格规则, 以及角色字段如何注入主聊天提示词."
            } else {
                "Defines role-play boundaries, base style rules, and how role fields enter the main chat prompt."
            },
        ),
        (
            "memory_context",
            if language == "zh-CN" {
                "长期记忆上下文提示词"
            } else {
                "Long-term memory context prompt"
            },
            if language == "zh-CN" {
                "定义相关记忆如何作为系统上下文提供给模型. {memory_items} 会被替换为记忆条目列表."
            } else {
                "Defines how relevant memories are supplied as system context. {memory_items} is replaced with memory bullet items."
            },
        ),
        (
            "runtime_metadata",
            if language == "zh-CN" {
                "动态运行信息提示词"
            } else {
                "Dynamic runtime metadata prompt"
            },
            if language == "zh-CN" {
                "定义每次请求末尾追加的动态信息. {timestamp_ms} 会被替换为当前请求时间戳."
            } else {
                "Defines dynamic metadata appended at the end of each request. {timestamp_ms} is replaced with the current request timestamp."
            },
        ),
    ];
    items
        .into_iter()
        .map(|(id, title, description)| {
            let default_content = default_system_prompt_template(id, &language);
            let path = prompt_template_path(id, &language);
            let overridden = path.exists();
            let content = if overridden {
                std::fs::read_to_string(&path)?
            } else {
                default_content.clone()
            };
            Ok(SystemPromptTemplate {
                id: id.into(),
                title: title.into(),
                description: description.into(),
                content,
                default_content,
                overridden,
            })
        })
        .collect()
}

pub fn save_system_prompt_template(
    id: &str,
    language: &str,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = prompt_template_path(id, language);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

pub fn reset_system_prompt_template(
    id: &str,
    language: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = prompt_template_path(id, language);
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    cleanup_empty_prompt_dirs(path.parent());
    Ok(())
}

fn cleanup_empty_prompt_dirs(dir: Option<&Path>) {
    let Some(dir) = dir else {
        return;
    };
    if dir
        .read_dir()
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
    {
        let _ = std::fs::remove_dir(dir);
    }
}

pub fn render_memory_context_template(language: &str, memory_items: &str) -> String {
    let template = read_system_prompt_template("memory_context", language)
        .unwrap_or_else(|_| default_memory_context_template(language));
    render_named_template(&template, &[("memory_items", memory_items.to_string())])
}

fn render_named_template(template: &str, values: &[(&str, String)]) -> String {
    let mut rendered = template.to_string();
    for (key, value) in values {
        rendered = rendered.replace(&format!("{{{key}}}"), value);
    }
    rendered
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
