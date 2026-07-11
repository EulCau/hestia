use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub app: AppSection,
    #[serde(default)]
    pub companion: CompanionSection,
    pub runtime: RuntimeSection,
    #[serde(default)]
    pub remote_api: RemoteApiConfig,
    #[serde(default)]
    pub local_llm: LocalLlmConfig,
    #[serde(default)]
    pub persona_rewrite: PersonaRewriteConfig,
    pub models: ModelsSection,
    pub personality: PersonalitySection,
    pub observability: ObservabilitySection,
    pub multimodal: MultimodalSection,
    pub initiative: InitiativeSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSection {
    pub name: String,
    pub environment: String,
    #[serde(default)]
    pub theme: ThemeSection,
    #[serde(default)]
    pub language: LanguageSection,
    #[serde(default)]
    pub avatar: AvatarSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSection {
    #[serde(default = "default_theme_mode")]
    pub mode: String,
}
fn default_theme_mode() -> String {
    "system".into()
}
impl Default for ThemeSection {
    fn default() -> Self {
        Self {
            mode: default_theme_mode(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageSection {
    #[serde(default = "default_ui_language")]
    pub ui: String,
    #[serde(default = "default_prompt_language")]
    pub system_prompt: String,
    #[serde(default = "default_prompt_language")]
    pub memory: String,
}
fn default_ui_language() -> String {
    "en".into()
}
fn default_prompt_language() -> String {
    "en".into()
}
impl Default for LanguageSection {
    fn default() -> Self {
        Self {
            ui: default_ui_language(),
            system_prompt: default_prompt_language(),
            memory: default_prompt_language(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvatarSection {
    #[serde(default = "default_avatar_enabled")]
    pub enabled: bool,
    #[serde(default = "default_avatar_image_path")]
    pub image_path: String,
    #[serde(default = "default_avatar_model_type")]
    pub model_type: String,
    #[serde(default = "default_avatar_auto_select")]
    pub auto_select: bool,
    #[serde(default = "default_avatar_idle_expression")]
    pub idle_expression: String,
    #[serde(default = "default_avatar_thinking_expression")]
    pub thinking_expression: String,
    #[serde(default = "default_avatar_speaking_expression")]
    pub speaking_expression: String,
    #[serde(default = "default_avatar_error_expression")]
    pub error_expression: String,
    #[serde(default = "default_avatar_idle_motion")]
    pub idle_motion: String,
    #[serde(default = "default_avatar_thinking_motion")]
    pub thinking_motion: String,
    #[serde(default = "default_avatar_speaking_motion")]
    pub speaking_motion: String,
}
fn default_avatar_enabled() -> bool {
    true
}
fn default_avatar_image_path() -> String {
    "companion-cat-placeholder.png".into()
}
fn default_avatar_model_type() -> String {
    "placeholder".into()
}
fn default_avatar_auto_select() -> bool {
    true
}
fn default_avatar_idle_expression() -> String {
    "Normal".into()
}
fn default_avatar_thinking_expression() -> String {
    "f01".into()
}
fn default_avatar_speaking_expression() -> String {
    "Normal".into()
}
fn default_avatar_error_expression() -> String {
    "Surprised".into()
}
fn default_avatar_idle_motion() -> String {
    "Idle".into()
}
fn default_avatar_thinking_motion() -> String {
    "Flick".into()
}
fn default_avatar_speaking_motion() -> String {
    "Tap".into()
}
impl Default for AvatarSection {
    fn default() -> Self {
        Self {
            enabled: true,
            image_path: default_avatar_image_path(),
            model_type: default_avatar_model_type(),
            auto_select: default_avatar_auto_select(),
            idle_expression: default_avatar_idle_expression(),
            thinking_expression: default_avatar_thinking_expression(),
            speaking_expression: default_avatar_speaking_expression(),
            error_expression: default_avatar_error_expression(),
            idle_motion: default_avatar_idle_motion(),
            thinking_motion: default_avatar_thinking_motion(),
            speaking_motion: default_avatar_speaking_motion(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanionSection {
    #[serde(default)]
    pub window: CompanionWindowSection,
}

impl Default for CompanionSection {
    fn default() -> Self {
        Self {
            window: CompanionWindowSection::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanionWindowSection {
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default = "default_companion_window_width")]
    pub width: u32,
    #[serde(default = "default_companion_window_height")]
    pub height: u32,
}

fn default_companion_window_width() -> u32 {
    260
}

fn default_companion_window_height() -> u32 {
    380
}

impl Default for CompanionWindowSection {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            width: default_companion_window_width(),
            height: default_companion_window_height(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSection {
    pub job_timeout_ms: u64,
    pub max_concurrent_remote_jobs: u32,
    pub max_concurrent_local_gpu_jobs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteApiConfig {
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_api_key_env")]
    pub api_key_env: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
}
fn default_base_url() -> String {
    "https://api.deepseek.com".into()
}
fn default_api_key_env() -> String {
    "DEEPSEEK_API_KEY".into()
}
fn default_model() -> String {
    "deepseek-chat".into()
}
impl Default for RemoteApiConfig {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
            api_key_env: default_api_key_env(),
            model: default_model(),
            api_key: None,
        }
    }
}

/// Supported local inference backends.
#[allow(dead_code)]
pub const LOCAL_LLM_BACKENDS: &[&str] = &["llama_cpp", "ollama", "vllm"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalLlmConfig {
    #[serde(default = "default_local_llm_backend")]
    pub backend: String,
    #[serde(default = "default_local_llm_url")]
    pub base_url: String,
    #[serde(default = "default_local_llm_model")]
    pub model: String,
    #[serde(default)]
    pub enabled: bool,
    /// Auto-start the inference server before use.
    #[serde(default = "default_auto_load")]
    pub auto_load: bool,
    /// Directory to scan for .gguf model files. Default: ~/models (Linux/macOS) or %%USERPROFILE%%\models (Windows).
    #[serde(default = "default_models_dir")]
    pub models_dir: String,
    /// Override the auto-generated load command. Leave empty for default.
    /// Placeholders: {model_path}, {port}, {host}
    #[serde(default)]
    pub load_command: String,
    /// Override the auto-generated unload command. Leave empty for default.
    #[serde(default)]
    pub unload_command: String,
}
fn default_local_llm_backend() -> String {
    "llama_cpp".into()
}
fn default_local_llm_url() -> String {
    "http://127.0.0.1:8080".into()
}
fn default_local_llm_model() -> String {
    "qwen3-8b".into()
}
fn default_auto_load() -> bool {
    false
}
fn default_models_dir() -> String {
    if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into()) + r"\models"
    } else {
        std::env::var("HOME").unwrap_or_else(|_| ".".into()) + "/models"
    }
}
impl Default for LocalLlmConfig {
    fn default() -> Self {
        Self {
            backend: default_local_llm_backend(),
            base_url: default_local_llm_url(),
            model: default_local_llm_model(),
            enabled: false,
            auto_load: default_auto_load(),
            models_dir: default_models_dir(),
            load_command: String::new(),
            unload_command: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaRewriteConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_rewrite_temp")]
    pub temperature: f64,
    #[serde(default = "default_rewrite_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_rewrite_template")]
    pub prompt_template: String,
}
fn default_rewrite_temp() -> f64 {
    0.7
}
fn default_rewrite_max_tokens() -> u32 {
    2048
}
fn default_rewrite_template() -> String {
    "Polish this message to fit the communication style below. Preserve the original voice, energy, sounds, and actions in parentheses. Only enforce the hard rules listed.\n\nTone: {tone}\nStyle rules:\n- {style_rules}\n\nOriginal: {content}\n\nPolished:".into()
}
impl Default for PersonaRewriteConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            temperature: default_rewrite_temp(),
            max_tokens: default_rewrite_max_tokens(),
            prompt_template: default_rewrite_template(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsSection {
    pub default_chat: ModelEntry,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub worker_id: String,
    pub capability: String,
    pub kind: String,
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalitySection {
    pub default_profile: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilitySection {
    pub job_timeline: bool,
    pub prompt_logs: bool,
    pub token_usage: bool,
    pub vram_logs: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultimodalSection {
    pub screenshot: ScreenshotSection,
    #[serde(default)]
    pub comfyui: ComfyUiSection,
    #[serde(default)]
    pub vision: VisionSection,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotSection {
    pub enabled: bool,
    pub interval_ms: u64,
    pub retention: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfyUiSection {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_comfyui_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub root_dir: String,
    #[serde(default)]
    pub python_path: String,
    #[serde(default = "default_comfyui_env_type")]
    pub env_type: String,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub launch_command: String,
    #[serde(default = "default_comfyui_workflow_path")]
    pub workflow_path: String,
    #[serde(default = "default_comfyui_output_dir")]
    pub output_dir: String,
    #[serde(default = "default_comfyui_startup_timeout_ms")]
    pub startup_timeout_ms: u64,
}
fn default_comfyui_base_url() -> String {
    "http://127.0.0.1:8188".into()
}
fn default_comfyui_env_type() -> String {
    "venv".into()
}
fn default_comfyui_workflow_path() -> String {
    "assets/workflows/sdxl.json".into()
}
fn default_comfyui_output_dir() -> String {
    "data/artifacts/images".into()
}
fn default_comfyui_startup_timeout_ms() -> u64 {
    20000
}
impl Default for ComfyUiSection {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: default_comfyui_base_url(),
            root_dir: String::new(),
            python_path: String::new(),
            env_type: default_comfyui_env_type(),
            auto_start: false,
            launch_command: String::new(),
            workflow_path: default_comfyui_workflow_path(),
            output_dir: default_comfyui_output_dir(),
            startup_timeout_ms: default_comfyui_startup_timeout_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionSection {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_vision_base_url")]
    pub base_url: String,
    #[serde(default = "default_vision_api_key_env")]
    pub api_key_env: String,
    #[serde(default = "default_vision_model")]
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_vision_system_prompt")]
    pub system_prompt: String,
    #[serde(default = "default_vision_default_prompt")]
    pub default_prompt: String,
    #[serde(default = "default_vision_max_image_bytes")]
    pub max_image_bytes: u64,
}
fn default_vision_base_url() -> String {
    "https://api.moonshot.ai".into()
}
fn default_vision_api_key_env() -> String {
    "MOONSHOT_API_KEY".into()
}
fn default_vision_model() -> String {
    "kimi-k2.6".into()
}
fn default_vision_system_prompt() -> String {
    "You are Kimi, a concise visual understanding assistant for a desktop companion app. Describe what matters for the user's current context. If the image contains text, transcribe the important text.".into()
}
fn default_vision_default_prompt() -> String {
    "请简要描述这张图片, 重点说明对桌宠对话有用的内容. 如果图中有文字, 摘录关键文字.".into()
}
fn default_vision_max_image_bytes() -> u64 {
    20 * 1024 * 1024
}
impl Default for VisionSection {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: default_vision_base_url(),
            api_key_env: default_vision_api_key_env(),
            model: default_vision_model(),
            api_key: None,
            system_prompt: default_vision_system_prompt(),
            default_prompt: default_vision_default_prompt(),
            max_image_bytes: default_vision_max_image_bytes(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiativeSection {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_initiative_level")]
    pub level: f64,
    #[serde(default = "default_initiative_cooldown_ms")]
    pub cooldown_ms: u64,
}
fn default_initiative_level() -> f64 {
    0.3
}
fn default_initiative_cooldown_ms() -> u64 {
    600000
}

fn config_path() -> PathBuf {
    let m = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let v: Vec<PathBuf> = vec![
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("config/default.toml"))),
        Some(m.join("..").join("config/default.toml")),
        Some(PathBuf::from("config/default.toml")),
    ]
    .into_iter()
    .flatten()
    .collect();
    for c in &v {
        if c.exists() {
            return c.clone();
        }
    }
    PathBuf::from("config/default.toml")
}
fn user_config_path() -> PathBuf {
    if let Ok(dir) = std::env::var("HESTIA_USER_DIR") {
        return PathBuf::from(dir).join("config").join("user.toml");
    }
    if !cfg!(debug_assertions) {
        return platform_user_data_dir().join("config").join("user.toml");
    }
    let m = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let v: Vec<PathBuf> = vec![
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("config/user.toml"))),
        Some(m.join("..").join("config/user.toml")),
        Some(PathBuf::from("config/user.toml")),
    ]
    .into_iter()
    .flatten()
    .collect();
    for c in &v {
        if c.exists() {
            return c.clone();
        }
    }
    PathBuf::from("config/user.toml")
}

fn platform_user_data_dir() -> PathBuf {
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

fn deep_merge(base: &mut toml::Value, ov: &toml::Value) {
    match (base, ov) {
        (toml::Value::Table(bt), toml::Value::Table(ot)) => {
            for (k, v) in ot {
                if bt.contains_key(k) && v.is_table() {
                    if let Some(bv) = bt.get_mut(k) {
                        deep_merge(bv, v);
                    }
                } else {
                    bt.insert(k.clone(), v.clone());
                }
            }
        }
        (b, o) => *b = o.clone(),
    }
}

pub fn load_config() -> Result<AppConfig, Box<dyn std::error::Error>> {
    let p = config_path();
    info!("loading config from {}", p.display());
    let base_content = std::fs::read_to_string(&p).unwrap_or_else(|_| {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../config/default.toml"
        ))
        .to_string()
    });
    let mut base: toml::Value = toml::from_str(&base_content)?;
    let up = user_config_path();
    if up.exists() {
        info!("merging user config from {}", up.display());
        if let Ok(uv) = toml::from_str::<toml::Value>(&std::fs::read_to_string(&up)?) {
            deep_merge(&mut base, &uv);
        }
    }
    Ok(base.try_into()?)
}

pub fn config_snapshot(c: &AppConfig) -> serde_json::Value {
    serde_json::json!({
        "app": { "name": c.app.name, "environment": c.app.environment, "theme": { "mode": c.app.theme.mode }, "language": {
            "ui": c.app.language.ui,
            "system_prompt": c.app.language.system_prompt,
            "memory": c.app.language.memory,
        }, "avatar": {
            "enabled": c.app.avatar.enabled,
            "image_path": c.app.avatar.image_path,
            "model_type": c.app.avatar.model_type,
            "auto_select": c.app.avatar.auto_select,
            "idle_expression": c.app.avatar.idle_expression,
            "thinking_expression": c.app.avatar.thinking_expression,
            "speaking_expression": c.app.avatar.speaking_expression,
            "error_expression": c.app.avatar.error_expression,
            "idle_motion": c.app.avatar.idle_motion,
            "thinking_motion": c.app.avatar.thinking_motion,
            "speaking_motion": c.app.avatar.speaking_motion,
        } },
        "companion": { "window": { "x": c.companion.window.x, "y": c.companion.window.y, "width": c.companion.window.width, "height": c.companion.window.height } },
        "remote_api": { "base_url": c.remote_api.base_url, "model": c.remote_api.model, "has_api_key": c.remote_api.api_key.is_some() || std::env::var(&c.remote_api.api_key_env).is_ok() },
        "local_llm": { "backend": c.local_llm.backend, "base_url": c.local_llm.base_url, "model": c.local_llm.model, "enabled": c.local_llm.enabled, "available": c.local_llm.enabled, "auto_load": c.local_llm.auto_load, "models_dir": c.local_llm.models_dir, "load_command": c.local_llm.load_command, "unload_command": c.local_llm.unload_command },
        "persona_rewrite": { "enabled": c.persona_rewrite.enabled, "temperature": c.persona_rewrite.temperature },
        "personality": { "default_profile": c.personality.default_profile },
        "runtime": { "job_timeout_ms": c.runtime.job_timeout_ms },
        "observability": { "job_timeline": c.observability.job_timeline, "prompt_logs": c.observability.prompt_logs, "token_usage": c.observability.token_usage },
        "initiative": { "enabled": c.initiative.enabled, "level": c.initiative.level, "cooldown_ms": c.initiative.cooldown_ms },
        "multimodal": {
            "screenshot": { "enabled": c.multimodal.screenshot.enabled, "interval_ms": c.multimodal.screenshot.interval_ms, "retention": c.multimodal.screenshot.retention },
            "comfyui": {
                "enabled": c.multimodal.comfyui.enabled,
                "available": c.multimodal.comfyui.enabled,
                "base_url": c.multimodal.comfyui.base_url,
                "root_dir": c.multimodal.comfyui.root_dir,
                "python_path": c.multimodal.comfyui.python_path,
                "env_type": c.multimodal.comfyui.env_type,
                "auto_start": c.multimodal.comfyui.auto_start,
                "launch_command": c.multimodal.comfyui.launch_command,
                "workflow_path": c.multimodal.comfyui.workflow_path,
                "output_dir": c.multimodal.comfyui.output_dir,
                "startup_timeout_ms": c.multimodal.comfyui.startup_timeout_ms,
            },
            "vision": {
                "enabled": c.multimodal.vision.enabled,
                "available": c.multimodal.vision.enabled && (c.multimodal.vision.api_key.is_some() || std::env::var(&c.multimodal.vision.api_key_env).is_ok()),
                "base_url": c.multimodal.vision.base_url,
                "model": c.multimodal.vision.model,
                "has_api_key": c.multimodal.vision.api_key.is_some() || std::env::var(&c.multimodal.vision.api_key_env).is_ok(),
                "api_key_env": c.multimodal.vision.api_key_env,
                "system_prompt": c.multimodal.vision.system_prompt,
                "default_prompt": c.multimodal.vision.default_prompt,
                "max_image_bytes": c.multimodal.vision.max_image_bytes,
            }
        },
    })
}

pub fn update_user_config(updates: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    let up = user_config_path();
    let mut cur: toml::Value = if up.exists() {
        toml::from_str(&std::fs::read_to_string(&up)?)
            .unwrap_or(toml::Value::Table(toml::value::Table::new()))
    } else {
        if let Some(parent) = up.parent() {
            std::fs::create_dir_all(parent)?;
        }
        toml::Value::Table(toml::value::Table::new())
    };
    if let toml::Value::Table(ref mut t) = cur {
        let mut upsert = |section: &str, key: &str, val: &serde_json::Value| {
            let mut table = &mut *t;
            for part in section.split('.') {
                let value = table
                    .entry(part.to_string())
                    .or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
                if !value.is_table() {
                    *value = toml::Value::Table(toml::value::Table::new());
                }
                table = value.as_table_mut().expect("section value was just set");
            }
            let tv = match val {
                serde_json::Value::String(x) => toml::Value::String(x.clone()),
                serde_json::Value::Bool(x) => toml::Value::Boolean(*x),
                serde_json::Value::Number(x) => {
                    if let Some(i) = x.as_i64() {
                        toml::Value::Integer(i)
                    } else {
                        toml::Value::Float(x.as_f64().unwrap_or(0.0))
                    }
                }
                _ => return,
            };
            table.insert(key.to_string(), tv);
        };
        if let Some(v) = updates.get("theme_mode") {
            upsert("app.theme", "mode", v);
        }
        if let Some(v) = updates.get("ui_language") {
            upsert("app.language", "ui", v);
        }
        if let Some(v) = updates.get("system_prompt_language") {
            upsert("app.language", "system_prompt", v);
        }
        if let Some(v) = updates.get("memory_language") {
            upsert("app.language", "memory", v);
        }
        if let Some(v) = updates.get("avatar_enabled") {
            upsert("app.avatar", "enabled", v);
        }
        if let Some(v) = updates.get("avatar_image_path") {
            upsert("app.avatar", "image_path", v);
        }
        if let Some(v) = updates.get("avatar_model_type") {
            upsert("app.avatar", "model_type", v);
        }
        if let Some(v) = updates.get("avatar_auto_select") {
            upsert("app.avatar", "auto_select", v);
        }
        if let Some(v) = updates.get("avatar_idle_expression") {
            upsert("app.avatar", "idle_expression", v);
        }
        if let Some(v) = updates.get("avatar_thinking_expression") {
            upsert("app.avatar", "thinking_expression", v);
        }
        if let Some(v) = updates.get("avatar_speaking_expression") {
            upsert("app.avatar", "speaking_expression", v);
        }
        if let Some(v) = updates.get("avatar_error_expression") {
            upsert("app.avatar", "error_expression", v);
        }
        if let Some(v) = updates.get("avatar_idle_motion") {
            upsert("app.avatar", "idle_motion", v);
        }
        if let Some(v) = updates.get("avatar_thinking_motion") {
            upsert("app.avatar", "thinking_motion", v);
        }
        if let Some(v) = updates.get("avatar_speaking_motion") {
            upsert("app.avatar", "speaking_motion", v);
        }
        if let Some(v) = updates.get("api_key") {
            upsert("remote_api", "api_key", v);
        }
        if let Some(v) = updates.get("base_url") {
            upsert("remote_api", "base_url", v);
        }
        if let Some(v) = updates.get("model") {
            upsert("remote_api", "model", v);
        }
        if let Some(v) = updates.get("local_llm_backend") {
            upsert("local_llm", "backend", v);
        }
        if let Some(v) = updates.get("local_llm_base_url") {
            upsert("local_llm", "base_url", v);
        }
        if let Some(v) = updates.get("local_llm_model") {
            upsert("local_llm", "model", v);
        }
        if let Some(v) = updates.get("local_llm_enabled") {
            upsert("local_llm", "enabled", v);
        }
        if let Some(v) = updates.get("persona_rewrite_enabled") {
            upsert("persona_rewrite", "enabled", v);
        }
        if let Some(v) = updates.get("personality_default_profile") {
            upsert("personality", "default_profile", v);
        }
        if let Some(v) = updates.get("local_llm_auto_load") {
            upsert("local_llm", "auto_load", v);
        }
        if let Some(v) = updates.get("local_llm_models_dir") {
            upsert("local_llm", "models_dir", v);
        }
        if let Some(v) = updates.get("local_llm_load_command") {
            upsert("local_llm", "load_command", v);
        }
        if let Some(v) = updates.get("local_llm_unload_command") {
            upsert("local_llm", "unload_command", v);
        }
        if let Some(v) = updates.get("comfyui_enabled") {
            upsert("multimodal.comfyui", "enabled", v);
        }
        if let Some(v) = updates.get("comfyui_base_url") {
            upsert("multimodal.comfyui", "base_url", v);
        }
        if let Some(v) = updates.get("comfyui_root_dir") {
            upsert("multimodal.comfyui", "root_dir", v);
        }
        if let Some(v) = updates.get("comfyui_python_path") {
            upsert("multimodal.comfyui", "python_path", v);
        }
        if let Some(v) = updates.get("comfyui_env_type") {
            upsert("multimodal.comfyui", "env_type", v);
        }
        if let Some(v) = updates.get("comfyui_auto_start") {
            upsert("multimodal.comfyui", "auto_start", v);
        }
        if let Some(v) = updates.get("comfyui_launch_command") {
            upsert("multimodal.comfyui", "launch_command", v);
        }
        if let Some(v) = updates.get("comfyui_workflow_path") {
            upsert("multimodal.comfyui", "workflow_path", v);
        }
        if let Some(v) = updates.get("comfyui_output_dir") {
            upsert("multimodal.comfyui", "output_dir", v);
        }
        if let Some(v) = updates.get("vision_enabled") {
            upsert("multimodal.vision", "enabled", v);
        }
        if let Some(v) = updates.get("vision_base_url") {
            upsert("multimodal.vision", "base_url", v);
        }
        if let Some(v) = updates.get("vision_model") {
            upsert("multimodal.vision", "model", v);
        }
        if let Some(v) = updates.get("vision_api_key") {
            upsert("multimodal.vision", "api_key", v);
        }
        if let Some(v) = updates.get("vision_system_prompt") {
            upsert("multimodal.vision", "system_prompt", v);
        }
        if let Some(v) = updates.get("vision_default_prompt") {
            upsert("multimodal.vision", "default_prompt", v);
        }
        if let Some(v) = updates.get("vision_max_image_bytes") {
            upsert("multimodal.vision", "max_image_bytes", v);
        }
        if let Some(v) = updates.get("initiative_enabled") {
            upsert("initiative", "enabled", v);
        }
        if let Some(v) = updates.get("initiative_level") {
            upsert("initiative", "level", v);
        }
        if let Some(v) = updates.get("initiative_cooldown_ms") {
            upsert("initiative", "cooldown_ms", v);
        }
        if let Some(v) = updates.get("companion_window_x") {
            upsert("companion.window", "x", v);
        }
        if let Some(v) = updates.get("companion_window_y") {
            upsert("companion.window", "y", v);
        }
        if let Some(v) = updates.get("companion_window_width") {
            upsert("companion.window", "width", v);
        }
        if let Some(v) = updates.get("companion_window_height") {
            upsert("companion.window", "height", v);
        }
    }
    std::fs::write(&up, toml::to_string_pretty(&cur)?)?;
    info!("user config updated");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_deep_merge_overrides_string() {
        let toml_str = "[app]\nname = \"Hestia\"\n[app.theme]\nmode = \"system\"\n";
        let mut base = toml::from_str::<toml::Value>(toml_str).unwrap();
        let overrides = toml::from_str::<toml::Value>("[app.theme]\nmode = \"dark\"\n").unwrap();
        deep_merge(&mut base, &overrides);
        assert_eq!(
            base.get("app")
                .and_then(|a| a.get("theme"))
                .and_then(|t| t.get("mode"))
                .and_then(|m| m.as_str()),
            Some("dark")
        );
    }
    #[test]
    fn test_deep_merge_adds_new_table() {
        let mut base = toml::from_str::<toml::Value>("[app]\nname = \"Hestia\"\n").unwrap();
        let overrides =
            toml::from_str::<toml::Value>("[remote_api]\napi_key = \"sk-test\"\n").unwrap();
        deep_merge(&mut base, &overrides);
        assert_eq!(
            base.get("remote_api")
                .and_then(|r| r.get("api_key"))
                .and_then(|k| k.as_str()),
            Some("sk-test")
        );
    }

    #[test]
    fn test_load_config_current_files() {
        let cfg = load_config().unwrap();
        assert!(!cfg.app.name.is_empty());
        assert!(!cfg.multimodal.comfyui.workflow_path.is_empty());
    }
}
