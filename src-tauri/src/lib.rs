mod config;
mod initiative;
mod memory;
mod multimodal;
mod observability;
mod personality;
mod protocol;
mod resource;
mod runtime;
mod scheduler;
mod workers;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration, Instant};
use tracing::{error, info, warn};

use crate::config::AppConfig;
use crate::initiative::InitiativeRuntime;
use crate::memory::MemoryPatch;
use crate::multimodal::{
    build_comfyui_launch_command, comfyui_endpoint, resolve_project_path, screenshot_metadata,
};
use crate::personality::{PersonaRewriter, PromptAssembler};
use crate::resource::ResourceManager;
use crate::scheduler::{Scheduler, SchedulerCommand};
use crate::workers::comfyui::ComfyUiWorker;
use crate::workers::local_llm::LocalLlmWorker;
use crate::workers::mock::MockWorker;
use crate::workers::model_loader::{self, BackendProcess};
use crate::workers::remote_api::RemoteApiWorker;
use crate::workers::vision_api::VisionApiWorker;
use crate::workers::{Worker, WorkerRegistry};

pub struct AppState {
    pub job_tx: mpsc::Sender<SchedulerCommand>,
    pub remote_worker: Arc<dyn Worker>,
    pub local_worker: Option<Arc<dyn Worker>>,
    pub comfyui_worker: Option<Arc<ComfyUiWorker>>,
    pub local_llm_available: bool,
    pub comfyui_available: bool,
    pub local_backend_process: Arc<Mutex<BackendProcess>>,
    pub comfyui_backend_process: Arc<Mutex<BackendProcess>>,
    pub resources: Arc<ResourceManager>,
    pub initiative: Arc<Mutex<InitiativeRuntime>>,
    pub config: AppConfig,
}

#[derive(Debug, Default, Deserialize)]
struct ImageIntentDecision {
    should_generate: bool,
    image_prompt: Option<String>,
    negative_prompt: Option<String>,
    response_text: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct RoleGenerationSeed {
    name: Option<String>,
    aliases: Option<Vec<String>>,
    identity: Option<String>,
    species: Option<String>,
    appearance: Option<String>,
    personality: Option<String>,
    language_style: Option<String>,
    scenario: Option<String>,
}

fn current_config(state: &AppState) -> AppConfig {
    config::load_config().unwrap_or_else(|_| state.config.clone())
}

fn current_role_id(state: &AppState) -> String {
    current_config(state).personality.default_profile
}

fn extract_json_object_text(content: &str) -> &str {
    let trimmed = content.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return trimmed;
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            return &trimmed[start..=end];
        }
    }
    trimmed
}

// ── Query commands ──

#[tauri::command]
fn get_app_info() -> String {
    serde_json::json!({
        "name": "Hestia",
        "version": env!("CARGO_PKG_VERSION"),
        "phase": "8"
    })
    .to_string()
}

fn show_window(app: &tauri::AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("{label} window is not available"))?;
    window
        .show()
        .map_err(|e| format!("failed to show {label} window: {}", e))?;
    window
        .unminimize()
        .map_err(|e| format!("failed to unminimize {label} window: {}", e))?;
    window
        .set_focus()
        .map_err(|e| format!("failed to focus {label} window: {}", e))?;
    Ok(())
}

fn stop_managed_backends(state: &AppState) {
    if let Ok(mut process) = state.local_backend_process.lock() {
        process.kill("local_llm");
    }
    if let Ok(mut process) = state.comfyui_backend_process.lock() {
        process.kill("comfyui");
    }
}

fn stop_backend_processes(
    local_backend_process: &Arc<Mutex<BackendProcess>>,
    comfyui_backend_process: &Arc<Mutex<BackendProcess>>,
) {
    if let Ok(mut process) = local_backend_process.lock() {
        process.kill("local_llm");
    }
    if let Ok(mut process) = comfyui_backend_process.lock() {
        process.kill("comfyui");
    }
}

#[cfg(unix)]
fn install_signal_cleanup(
    local_backend_process: Arc<Mutex<BackendProcess>>,
    comfyui_backend_process: Arc<Mutex<BackendProcess>>,
) {
    use signal_hook::consts::signal::{SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;

    match Signals::new([SIGINT, SIGTERM]) {
        Ok(mut signals) => {
            std::thread::spawn(move || {
                if let Some(signal) = signals.forever().next() {
                    warn!(
                        signal,
                        "received shutdown signal, stopping managed backends"
                    );
                    stop_backend_processes(&local_backend_process, &comfyui_backend_process);
                    std::process::exit(128 + signal);
                }
            });
        }
        Err(error) => {
            warn!(error = %error, "failed to install shutdown signal handler");
        }
    }
}

#[cfg(not(unix))]
fn install_signal_cleanup(
    _local_backend_process: Arc<Mutex<BackendProcess>>,
    _comfyui_backend_process: Arc<Mutex<BackendProcess>>,
) {
}

fn hide_window_and_maybe_idle_backend(
    app: &tauri::AppHandle,
    state: &AppState,
    label: &str,
) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("{label} window is not available"))?;
    window
        .hide()
        .map_err(|e| format!("failed to hide {label} window: {}", e))?;
    if label == "companion" {
        let _ = app.emit_to("main", "companion-visible-changed", false);
        let _ = app.emit_to("companion", "companion-visible-changed", false);
        let _ = app.emit_to("companion_dialog", "companion-visible-changed", false);
    } else if label == "companion_dialog" {
        let _ = app.emit_to("companion", "companion-dialog-visible-changed", false);
        let _ = app.emit_to(
            "companion_dialog",
            "companion-dialog-visible-changed",
            false,
        );
    }
    let main_visible = app
        .get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    let companion_visible = app
        .get_webview_window("companion")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    let dialog_visible = app
        .get_webview_window("companion_dialog")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    if !main_visible && !companion_visible && !dialog_visible {
        info!("all frontend windows hidden, stopping managed backends");
        stop_managed_backends(state);
    }
    Ok(())
}

fn local_llm_endpoint(base_url: &str) -> (String, u16) {
    reqwest::Url::parse(base_url)
        .ok()
        .map(|url| {
            let host = url.host_str().unwrap_or("127.0.0.1").to_string();
            let port = url.port_or_known_default().unwrap_or(8080);
            (host, port)
        })
        .unwrap_or_else(|| ("127.0.0.1".into(), 8080))
}

#[tauri::command]
fn get_config_snapshot(state: tauri::State<'_, AppState>) -> String {
    let cfg = config::load_config().unwrap_or_else(|e| {
        warn!(error = %e, "failed to reload config snapshot, using startup config");
        state.config.clone()
    });
    let mut snapshot = config::config_snapshot(&cfg);
    if let Some(local_llm) = snapshot.get_mut("local_llm") {
        local_llm["available"] = serde_json::Value::Bool(state.local_llm_available);
        local_llm["managed_process"] = serde_json::Value::Bool(
            state
                .local_backend_process
                .lock()
                .map(|mut process| process.is_running())
                .unwrap_or(false),
        );
    }
    if let Some(comfyui) = snapshot
        .get_mut("multimodal")
        .and_then(|multimodal| multimodal.get_mut("comfyui"))
    {
        comfyui["available"] = serde_json::Value::Bool(state.comfyui_available);
        comfyui["managed_process"] = serde_json::Value::Bool(
            state
                .comfyui_backend_process
                .lock()
                .map(|mut process| process.is_running())
                .unwrap_or(false),
        );
    }
    snapshot.to_string()
}

#[tauri::command]
fn show_main_window(app: tauri::AppHandle) -> Result<String, String> {
    show_window(&app, "main")?;
    Ok("ok".into())
}

#[tauri::command]
fn open_settings_window(app: tauri::AppHandle) -> Result<String, String> {
    show_window(&app, "main")?;
    app.emit_to("main", "open-settings", ())
        .map_err(|e| format!("failed to emit settings event: {}", e))?;
    Ok("ok".into())
}

#[tauri::command]
fn list_personas() -> Vec<String> {
    personality::list_profiles()
}

#[tauri::command]
fn list_roles() -> Result<String, String> {
    let roles =
        personality::list_role_configs().map_err(|e| format!("failed to list roles: {}", e))?;
    serde_json::to_string(&roles).map_err(|e| format!("failed to serialize roles: {}", e))
}

#[tauri::command]
fn get_persona_content(profile: String) -> Result<String, String> {
    personality::read_persona_raw(&profile).map_err(|e| format!("failed to read persona: {}", e))
}

#[tauri::command]
fn save_persona_content(profile: String, content: String) -> Result<String, String> {
    personality::save_persona_raw(&profile, &content)
        .map_err(|e| format!("failed to save persona: {}", e))?;
    Ok("ok".into())
}

#[tauri::command]
fn role_storage_paths(profile: String) -> String {
    serde_json::json!({
        "role": personality::role_storage_path(&profile),
        "memory": memory::memory_storage_path(&profile),
    })
    .to_string()
}

#[tauri::command]
fn set_active_role(state: tauri::State<'_, AppState>, profile: String) -> Result<String, String> {
    if !personality::list_profiles().contains(&profile) {
        return Err(format!("role not found: {profile}"));
    }
    config::update_user_config(serde_json::json!({ "personality_default_profile": profile }))
        .map_err(|e| format!("failed to update active role: {}", e))?;
    let cfg = current_config(&state);
    Ok(serde_json::json!({ "active_role": cfg.personality.default_profile }).to_string())
}

#[tauri::command]
fn delete_role(
    state: tauri::State<'_, AppState>,
    profile: String,
    confirmation: String,
) -> Result<String, String> {
    let expected = format!("我确认删除{profile}");
    if confirmation != expected {
        return Err(format!("confirmation must exactly be: {expected}"));
    }
    personality::delete_persona(&profile).map_err(|e| format!("failed to delete role: {}", e))?;
    if current_role_id(&state) == profile {
        config::update_user_config(serde_json::json!({ "personality_default_profile": "default" }))
            .map_err(|e| format!("failed to reset active role: {}", e))?;
    }
    Ok("ok".into())
}

#[tauri::command]
async fn generate_role_profile(
    state: tauri::State<'_, AppState>,
    seed: RoleGenerationSeed,
) -> Result<String, String> {
    let prompt = [
        "Generate a complete character profile for a desktop AI companion.",
        "Return strict JSON only. Do not wrap it in markdown.",
        "Schema:",
        r#"{"schema_version":2,"id":"lowercase_ascii_id","name":"称呼","aliases":["别称"],"identity":"身份","species":"物种","appearance":"形象","personality":"性格","language_style":"语言习惯","scenario":"使用场景","tone":"总体语气","initiative":0.3,"humor":0.2,"verbosity":"medium","pinned":false}"#,
        "Rules:",
        "- The name and aliases must make it clear that later user references to these words mean the character being role-played.",
        "- Fill missing fields from the provided identity, species, and personality.",
        "- Keep the profile usable across ordinary conversations.",
        "- Do not include base prompt rules such as punctuation policy or parenthetical action syntax.",
        "- id must be lowercase ASCII letters, digits, '_' or '-'.",
        "",
        "User-provided partial profile:",
        &serde_json::to_string_pretty(&seed).map_err(|e| format!("failed to serialize seed: {}", e))?,
    ]
    .join("\n");
    let assembler = PromptAssembler::load(&current_role_id(&state))
        .map_err(|e| format!("failed to load active role: {}", e))?;
    let messages = assembler.assemble_messages_with_context(&prompt, &[], None);
    let job = crate::protocol::Job::new(
        "role_profile_generation",
        crate::protocol::Capability::Chat,
        serde_json::json!({ "messages": messages }),
    );
    let result = state.remote_worker.infer(&job).await.map_err(|e| {
        error!(error = %e, "role profile generation failed");
        format!("role generation failed: {}", e)
    })?;
    let content = extract_json_object_text(result["content"].as_str().unwrap_or(""));
    let parsed: personality::PersonaConfig = serde_json::from_str(content)
        .map_err(|e| format!("role generator returned invalid JSON: {}", e))?;
    serde_json::to_string_pretty(&parsed)
        .map_err(|e| format!("failed to serialize generated role: {}", e))
}

#[tauri::command]
fn list_memories(
    state: tauri::State<'_, AppState>,
    query: Option<String>,
    include_archived: Option<bool>,
) -> Result<String, String> {
    let role_id = current_role_id(&state);
    let memories = memory::list_memories(
        &role_id,
        query.as_deref(),
        include_archived.unwrap_or(false),
    )
    .map_err(|e| format!("failed to list memories: {}", e))?;
    serde_json::to_string(&memories).map_err(|e| format!("failed to serialize memories: {}", e))
}

#[tauri::command]
fn create_memory(
    state: tauri::State<'_, AppState>,
    kind: String,
    content: String,
    source: Option<String>,
    pinned: Option<bool>,
) -> Result<String, String> {
    let role_id = current_role_id(&state);
    let memory = memory::create_memory(&role_id, kind, content, source, pinned)
        .map_err(|e| format!("failed to create memory: {}", e))?;
    serde_json::to_string(&memory).map_err(|e| format!("failed to serialize memory: {}", e))
}

#[tauri::command]
fn update_memory(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: MemoryPatch,
) -> Result<String, String> {
    let role_id = current_role_id(&state);
    let memory = memory::update_memory(&role_id, id, patch)
        .map_err(|e| format!("failed to update memory: {}", e))?;
    serde_json::to_string(&memory).map_err(|e| format!("failed to serialize memory: {}", e))
}

#[tauri::command]
fn delete_memory(state: tauri::State<'_, AppState>, id: String) -> Result<String, String> {
    let role_id = current_role_id(&state);
    memory::delete_memory(&role_id, id).map_err(|e| format!("failed to delete memory: {}", e))?;
    Ok("ok".into())
}

// ── Mutate commands ──

#[tauri::command]
async fn update_settings(
    app: tauri::AppHandle,
    updates: serde_json::Value,
) -> Result<String, String> {
    info!(?updates, "updating user config");
    let avatar_changed = updates.get("avatar_enabled").is_some()
        || updates.get("avatar_image_path").is_some()
        || updates.get("avatar_model_type").is_some()
        || updates.get("avatar_auto_select").is_some()
        || updates.get("avatar_idle_expression").is_some()
        || updates.get("avatar_thinking_expression").is_some()
        || updates.get("avatar_speaking_expression").is_some()
        || updates.get("avatar_error_expression").is_some()
        || updates.get("avatar_idle_motion").is_some()
        || updates.get("avatar_thinking_motion").is_some()
        || updates.get("avatar_speaking_motion").is_some();
    config::update_user_config(updates).map_err(|e| format!("failed to update config: {}", e))?;
    if avatar_changed {
        let cfg = config::load_config().map_err(|e| format!("failed to reload config: {}", e))?;
        let payload = serde_json::json!({
            "enabled": cfg.app.avatar.enabled,
            "image_path": cfg.app.avatar.image_path,
            "model_type": cfg.app.avatar.model_type,
            "auto_select": cfg.app.avatar.auto_select,
            "idle_expression": cfg.app.avatar.idle_expression,
            "thinking_expression": cfg.app.avatar.thinking_expression,
            "speaking_expression": cfg.app.avatar.speaking_expression,
            "error_expression": cfg.app.avatar.error_expression,
            "idle_motion": cfg.app.avatar.idle_motion,
            "thinking_motion": cfg.app.avatar.thinking_motion,
            "speaking_motion": cfg.app.avatar.speaking_motion,
        });
        let _ = app.emit_to("main", "avatar-config-changed", payload.clone());
        let _ = app.emit_to("companion", "avatar-config-changed", payload.clone());
        let _ = app.emit_to("companion_dialog", "avatar-config-changed", payload);
    }
    Ok("ok".into())
}

#[tauri::command]
fn record_user_activity(state: tauri::State<'_, AppState>) -> Result<String, String> {
    state
        .initiative
        .lock()
        .map_err(|e| format!("failed to lock initiative runtime: {}", e))?
        .record_user_activity();
    Ok("ok".into())
}

#[tauri::command]
fn evaluate_initiative(
    state: tauri::State<'_, AppState>,
    trigger: Option<String>,
) -> Result<String, String> {
    let trigger = trigger.unwrap_or_else(|| "manual".into());
    let mut runtime = state
        .initiative
        .lock()
        .map_err(|e| format!("failed to lock initiative runtime: {}", e))?;
    let config = config::load_config().unwrap_or_else(|_| state.config.clone());
    let decision = runtime.evaluate(&config.initiative, &trigger);
    Ok(serde_json::json!({
        "decision": decision,
        "recent_decisions": runtime.recent_decisions(),
    })
    .to_string())
}

#[tauri::command]
async fn request_initiative_message(
    state: tauri::State<'_, AppState>,
    history: Vec<personality::ChatMessage>,
    trigger: Option<String>,
) -> Result<String, String> {
    let trigger = trigger.unwrap_or_else(|| "manual".into());
    let decision = {
        let mut runtime = state
            .initiative
            .lock()
            .map_err(|e| format!("failed to lock initiative runtime: {}", e))?;
        let config = current_config(&state);
        runtime.evaluate(&config.initiative, &trigger)
    };
    if !decision.allowed {
        return Ok(serde_json::json!({
            "allowed": false,
            "content": null,
            "decision": decision,
        })
        .to_string());
    }

    let cfg = current_config(&state);
    let role_id = cfg.personality.default_profile.clone();
    let assembler =
        PromptAssembler::load(&role_id).map_err(|e| format!("failed to load persona: {}", e))?;
    let memories = memory::relevant_memories(&role_id, &decision.suggested_prompt, 6)
        .unwrap_or_else(|e| {
            warn!(error = %e, "failed to load initiative memory context");
            Vec::new()
        });
    let memory_context = memory::format_memory_context(&memories);
    let messages = assembler.assemble_messages_with_context(
        &decision.suggested_prompt,
        &history,
        memory_context.as_deref(),
    );
    let mut job = crate::protocol::Job::new(
        "initiative_message",
        crate::protocol::Capability::Chat,
        serde_json::json!({ "messages": serde_json::Value::Array(messages) }),
    );
    job.timeout_ms = 30000;

    info!(
        job_id = %job.id,
        trigger,
        "requesting initiative message"
    );
    let result = state.remote_worker.infer(&job).await.map_err(|e| {
        error!(error = %e, "initiative message inference failed");
        format!("initiative inference failed: {}", e)
    })?;
    let content = result["content"].as_str().unwrap_or("").to_string();
    if content.trim().is_empty() {
        return Err("initiative model returned empty content".into());
    }
    state
        .initiative
        .lock()
        .map_err(|e| format!("failed to lock initiative runtime: {}", e))?
        .mark_initiative_spoken();

    Ok(serde_json::json!({
        "allowed": true,
        "content": content,
        "decision": decision,
    })
    .to_string())
}

#[tauri::command]
fn list_available_models(state: tauri::State<'_, AppState>) -> String {
    let models = model_loader::list_available_models(&state.config.local_llm.models_dir);
    serde_json::to_string(&models).unwrap_or_else(|_| "[]".into())
}

#[tauri::command]
fn get_screenshot_metadata(state: tauri::State<'_, AppState>) -> String {
    screenshot_metadata(
        state.config.multimodal.screenshot.enabled,
        state.config.multimodal.screenshot.retention,
    )
    .to_string()
}

fn apply_topmost(window: &tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    window
        .set_always_on_top(enabled)
        .map_err(|e| format!("failed to update always-on-top: {}", e))?;
    #[cfg(not(target_os = "linux"))]
    window
        .set_visible_on_all_workspaces(enabled)
        .map_err(|e| format!("failed to update workspace visibility: {}", e))?;
    Ok(())
}

fn apply_window_topmost(window: &tauri::Window, enabled: bool) -> Result<(), String> {
    window
        .set_always_on_top(enabled)
        .map_err(|e| format!("failed to update always-on-top: {}", e))?;
    #[cfg(not(target_os = "linux"))]
    window
        .set_visible_on_all_workspaces(enabled)
        .map_err(|e| format!("failed to update workspace visibility: {}", e))?;
    Ok(())
}

fn ensure_companion_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window("companion") {
        return Ok(window);
    }
    tauri::WebviewWindowBuilder::new(
        app,
        "companion",
        tauri::WebviewUrl::App("/?view=companion".into()),
    )
    .title("Hestia Companion")
    .inner_size(240.0, 340.0)
    .resizable(true)
    .decorations(false)
    .transparent(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(true)
    .build()
    .map_err(|e| format!("failed to create companion window: {}", e))
}

fn ensure_companion_dialog_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window("companion_dialog") {
        return Ok(window);
    }
    tauri::WebviewWindowBuilder::new(
        app,
        "companion_dialog",
        tauri::WebviewUrl::App("/?view=companion_dialog".into()),
    )
    .title("Hestia Companion Dialogue")
    .inner_size(320.0, 220.0)
    .resizable(false)
    .decorations(false)
    .transparent(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(true)
    .build()
    .map_err(|e| format!("failed to create companion dialog window: {}", e))
}

#[tauri::command]
fn set_companion_visible(app: tauri::AppHandle, visible: bool) -> Result<String, String> {
    if visible {
        let window = ensure_companion_window(&app)?;
        window
            .show()
            .map_err(|e| format!("failed to show companion: {}", e))?;
        apply_topmost(&window, true)?;
        let _ = window.set_focus();
    } else {
        if let Some(window) = app.get_webview_window("companion") {
            window
                .hide()
                .map_err(|e| format!("failed to hide companion: {}", e))?;
        }
        if let Some(dialog) = app.get_webview_window("companion_dialog") {
            dialog
                .hide()
                .map_err(|e| format!("failed to hide companion dialog: {}", e))?;
            let _ = app.emit_to(
                "companion_dialog",
                "companion-dialog-visible-changed",
                false,
            );
        }
    }
    let _ = app.emit_to("main", "companion-visible-changed", visible);
    let _ = app.emit_to("companion", "companion-visible-changed", visible);
    let _ = app.emit_to("companion_dialog", "companion-visible-changed", visible);
    if !visible {
        let _ = app.emit_to("companion", "companion-dialog-visible-changed", false);
    }
    Ok("ok".into())
}

#[tauri::command]
fn set_companion_dialog_visible(app: tauri::AppHandle, visible: bool) -> Result<String, String> {
    if visible {
        let window = ensure_companion_dialog_window(&app)?;
        window
            .show()
            .map_err(|e| format!("failed to show companion dialog: {}", e))?;
        apply_topmost(&window, true)?;
    } else if let Some(window) = app.get_webview_window("companion_dialog") {
        window
            .hide()
            .map_err(|e| format!("failed to hide companion dialog: {}", e))?;
    }
    let _ = app.emit_to("companion", "companion-dialog-visible-changed", visible);
    let _ = app.emit_to(
        "companion_dialog",
        "companion-dialog-visible-changed",
        visible,
    );
    Ok("ok".into())
}

#[tauri::command]
fn set_companion_always_on_top(app: tauri::AppHandle, enabled: bool) -> Result<String, String> {
    if let Some(window) = app.get_webview_window("companion") {
        apply_topmost(&window, enabled)?;
    }
    if let Some(dialog) = app.get_webview_window("companion_dialog") {
        apply_topmost(&dialog, enabled)?;
    }
    Ok("ok".into())
}

#[tauri::command]
fn restart_backend(state: tauri::State<'_, AppState>) -> Result<String, String> {
    info!("restarting managed backend processes");
    stop_managed_backends(&state);
    Ok("ok".into())
}

#[tauri::command]
fn read_image_artifact(state: tauri::State<'_, AppState>, path: String) -> Result<String, String> {
    let artifact_path = std::path::PathBuf::from(&path);
    let output_dir = resolve_project_path(&state.config.multimodal.comfyui.output_dir);
    if !artifact_path.starts_with(&output_dir) {
        return Err("image path is outside the configured artifact directory".into());
    }
    let bytes = std::fs::read(&artifact_path)
        .map_err(|e| format!("failed to read image artifact: {}", e))?;
    let mime = match artifact_path.extension().and_then(|ext| ext.to_str()) {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/png",
    };
    Ok(format!("data:{};base64,{}", mime, encode_base64(&bytes)))
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target)
        .map_err(|e| format!("failed to create {}: {}", target.display(), e))?;
    for entry in std::fs::read_dir(source)
        .map_err(|e| format!("failed to read {}: {}", source.display(), e))?
    {
        let entry = entry.map_err(|e| format!("failed to read directory entry: {}", e))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else if source_path.is_file() {
            std::fs::copy(&source_path, &target_path).map_err(|e| {
                format!(
                    "failed to copy {} to {}: {}",
                    source_path.display(),
                    target_path.display(),
                    e
                )
            })?;
        }
    }
    Ok(())
}

fn find_model3_json(path: &Path) -> Result<PathBuf, String> {
    if path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".model3.json"))
    {
        return Ok(path.to_path_buf());
    }
    if !path.is_dir() {
        return Err("Live2D path must be a .model3.json file or a directory".into());
    }
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)
            .map_err(|e| format!("failed to read {}: {}", dir.display(), e))?
        {
            let entry = entry.map_err(|e| format!("failed to read directory entry: {}", e))?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
                continue;
            }
            if entry_path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".model3.json"))
            {
                return Ok(entry_path);
            }
        }
    }
    Err("no .model3.json file found in selected directory".into())
}

fn timestamp_millis() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|e| format!("system clock is before UNIX_EPOCH: {}", e))
}

#[tauri::command]
fn prepare_avatar_content(path: String, model_type: String) -> Result<String, String> {
    let source = resolve_project_path(path.trim());
    if model_type == "live2d" {
        let model_path = find_model3_json(&source)?;
        let root = if source.is_dir() {
            source
        } else {
            model_path
                .parent()
                .ok_or_else(|| "Live2D model has no parent directory".to_string())?
                .to_path_buf()
        };
        let public_live2d = resolve_project_path("frontend/public/live2d");
        let target_name = format!("prepared-{}", timestamp_millis()?);
        let target = public_live2d.join(&target_name);
        copy_dir_recursive(&root, &target)?;
        let model_relative_to_root = model_path
            .strip_prefix(&root)
            .map_err(|_| "failed to resolve Live2D model path".to_string())?;
        return Ok(format!(
            "live2d/{}/{}",
            target_name,
            model_relative_to_root.to_string_lossy().replace('\\', "/")
        ));
    }

    if model_type == "placeholder" {
        if !source.is_file() {
            return Err("image avatar path must be a file".into());
        }
        let extension = source
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("png")
            .to_ascii_lowercase();
        let allowed = ["png", "jpg", "jpeg", "webp", "gif"];
        if !allowed.contains(&extension.as_str()) {
            return Err("image avatar must be png, jpg, jpeg, webp, or gif".into());
        }
        let public_avatar = resolve_project_path("frontend/public/avatar");
        std::fs::create_dir_all(&public_avatar).map_err(|e| {
            format!(
                "failed to create avatar directory {}: {}",
                public_avatar.display(),
                e
            )
        })?;
        let target = public_avatar.join(format!("current.{extension}"));
        std::fs::copy(&source, &target).map_err(|e| {
            format!(
                "failed to copy {} to {}: {}",
                source.display(),
                target.display(),
                e
            )
        })?;
        return Ok(format!("avatar/current.{extension}"));
    }

    Ok(path)
}

#[tauri::command]
async fn recognize_image(
    state: tauri::State<'_, AppState>,
    path: String,
    prompt: Option<String>,
) -> Result<String, String> {
    if let Ok(mut initiative) = state.initiative.lock() {
        initiative.record_user_activity();
    }
    run_vision_recognition(&state, path, prompt, "upload")
        .await
        .map(|value| value.to_string())
}

pub(crate) async fn run_vision_recognition(
    state: &AppState,
    path: String,
    prompt: Option<String>,
    source: &str,
) -> Result<serde_json::Value, String> {
    if !state.config.multimodal.vision.enabled {
        return Err(
            "Vision recognition is disabled. Enable Kimi Vision in Settings, then restart Hestia."
                .into(),
        );
    }
    let image_data_url =
        image_data_url_from_path(&path, state.config.multimodal.vision.max_image_bytes)?;
    let prompt = prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&state.config.multimodal.vision.default_prompt)
        .to_string();

    let mut job = crate::protocol::Job::new(
        "vision_recognition",
        crate::protocol::Capability::Vision,
        serde_json::json!({
            "image_data_url": image_data_url,
            "prompt": prompt,
            "source": source,
        }),
    );
    job.timeout_ms = 60000;

    let (result_tx, result_rx) = oneshot::channel();
    state
        .job_tx
        .send(SchedulerCommand::Run { job, result_tx })
        .await
        .map_err(|e| format!("failed to submit vision job: {}", e))?;
    let mut result = match result_rx.await {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => return Err(error),
        Err(error) => return Err(format!("failed to receive vision job result: {}", error)),
    };
    if let Some(object) = result.as_object_mut() {
        object.insert("image_path".into(), serde_json::Value::String(path));
    }
    Ok(result)
}

fn image_data_url_from_path(path: &str, max_bytes: u64) -> Result<String, String> {
    let image_path = Path::new(path);
    let mime = image_mime_for_path(image_path)?;
    let metadata = std::fs::metadata(image_path)
        .map_err(|e| format!("failed to read image metadata: {}", e))?;
    if metadata.len() > max_bytes {
        return Err(format!(
            "image is too large: {} bytes exceeds configured limit {} bytes",
            metadata.len(),
            max_bytes
        ));
    }
    let bytes = std::fs::read(image_path).map_err(|e| format!("failed to read image: {}", e))?;
    Ok(format!("data:{};base64,{}", mime, encode_base64(&bytes)))
}

fn image_mime_for_path(path: &Path) -> Result<&'static str, String> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Ok("image/png"),
        Some("jpg" | "jpeg") => Ok("image/jpeg"),
        Some("webp") => Ok("image/webp"),
        Some("gif") => Ok("image/gif"),
        _ => Err("unsupported image format. Supported formats: png, jpeg, webp, gif".into()),
    }
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[tauri::command]
async fn generate_test_image(
    state: tauri::State<'_, AppState>,
    prompt: String,
    negative_prompt: Option<String>,
) -> Result<String, String> {
    run_image_generation(&state, prompt, negative_prompt)
        .await
        .map(|value| value.to_string())
}

async fn run_image_generation(
    state: &AppState,
    prompt: String,
    negative_prompt: Option<String>,
) -> Result<serde_json::Value, String> {
    let Some(worker) = &state.comfyui_worker else {
        return Err(
            "ComfyUI worker is disabled. Enable it in Settings, then restart Hestia.".into(),
        );
    };
    let mut started_for_job = false;
    if !worker.health_check_http().await {
        if !state.config.multimodal.comfyui.auto_start {
            return Err("ComfyUI is not available. Enable on-demand start in Settings or start ComfyUI manually.".into());
        }
        let Some((command, working_dir)) =
            build_comfyui_launch_command(&state.config.multimodal.comfyui)
        else {
            return Err("ComfyUI on-demand start requires a root directory.".into());
        };
        let (_, port) = comfyui_endpoint(&state.config.multimodal.comfyui.base_url);
        model_loader::kill_port(port);
        info!(
            command = %command,
            working_dir = %working_dir.display(),
            "launching ComfyUI backend"
        );
        let started = state
            .comfyui_backend_process
            .lock()
            .map_err(|e| format!("failed to lock ComfyUI process: {}", e))?
            .spawn_in_dir(&command, &working_dir, "comfyui", "comfyui");
        if !started {
            return Err("failed to start ComfyUI with the configured command".into());
        }
        started_for_job = true;
        if !wait_for_comfyui_health(worker, state.config.multimodal.comfyui.startup_timeout_ms)
            .await
        {
            if let Ok(mut process) = state.comfyui_backend_process.lock() {
                process.kill("comfyui");
            }
            worker.mark_unhealthy();
            return Err("ComfyUI did not become healthy before startup timeout".into());
        }
    }

    let mut job = crate::protocol::Job::new(
        "image_generation",
        crate::protocol::Capability::ImageGeneration,
        serde_json::json!({
            "prompt": prompt,
            "negative_prompt": negative_prompt.unwrap_or_else(|| "text, watermark".into()),
            "workflow_path": state.config.multimodal.comfyui.workflow_path.clone(),
            "output_dir": state.config.multimodal.comfyui.output_dir.clone(),
        }),
    );
    job.timeout_ms = 180000;
    let (result_tx, result_rx) = oneshot::channel();
    let result = async {
        state
            .job_tx
            .send(SchedulerCommand::Run { job, result_tx })
            .await
            .map_err(|e| format!("failed to submit image job: {}", e))?;
        match result_rx.await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(error),
            Err(error) => Err(format!("failed to receive image job result: {}", error)),
        }
    }
    .await;

    if started_for_job {
        if let Ok(mut process) = state.comfyui_backend_process.lock() {
            process.kill("comfyui");
        }
        worker.mark_unhealthy();
    }

    result
}

#[tauri::command]
async fn submit_test_job(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let job = crate::protocol::Job::new(
        "chat",
        crate::protocol::Capability::Chat,
        serde_json::json!({"message": "hello from Phase 1"}),
    );
    let job_id = job.id.clone();
    info!(job_id = %job_id, "submitting test job");
    state
        .job_tx
        .send(SchedulerCommand::Submit(job))
        .await
        .map_err(|e| format!("failed to submit job: {}", e))?;
    Ok(job_id)
}

fn explicit_image_prompt(message: &str) -> Option<String> {
    let trimmed = message.trim();
    for prefix in ["\\image", "/image"] {
        if trimmed == prefix {
            return Some(String::new());
        }
        if let Some(rest) = trimmed.strip_prefix(&format!("{prefix} ")) {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if start <= end {
        Some(&text[start..=end])
    } else {
        None
    }
}

fn parse_image_intent(text: &str) -> ImageIntentDecision {
    extract_json_object(text)
        .and_then(|json| serde_json::from_str::<ImageIntentDecision>(json).ok())
        .unwrap_or_default()
}

async fn classify_image_intent(
    state: &AppState,
    message: &str,
    history: &[personality::ChatMessage],
) -> ImageIntentDecision {
    let recent_history = history
        .iter()
        .rev()
        .take(6)
        .rev()
        .map(|entry| format!("{}: {}", entry.role, entry.content))
        .collect::<Vec<_>>()
        .join("\n");
    let messages = serde_json::json!([
        {
            "role": "system",
            "content": "You are an intent router for an assistant with a ComfyUI image generator. Return only one JSON object with this exact shape: {\"should_generate\": boolean, \"image_prompt\": string|null, \"negative_prompt\": string|null, \"response_text\": string|null}. Set should_generate=true only when the user clearly wants an image to be generated, drawn, rendered, illustrated, designed, or made as a picture. If the user asks for analysis, coding, math, planning, or talks about image generation in the abstract, return false. If true, write image_prompt in concise English suitable for SDXL/ComfyUI. Use negative_prompt only for obvious exclusions. If uncertain, return false."
        },
        {
            "role": "user",
            "content": format!("Recent history:\n{}\n\nCurrent user message:\n{}", recent_history, message)
        }
    ]);
    let mut job = crate::protocol::Job::new(
        "image_intent",
        crate::protocol::Capability::Chat,
        serde_json::json!({ "messages": messages }),
    );
    job.timeout_ms = 20000;

    match state.remote_worker.infer(&job).await {
        Ok(value) => {
            let content = value["content"].as_str().unwrap_or_default();
            parse_image_intent(content)
        }
        Err(error) => {
            warn!(error = %error, "image intent classification failed");
            ImageIntentDecision::default()
        }
    }
}

#[tauri::command]
async fn send_chat_message(
    state: tauri::State<'_, AppState>,
    message: String,
    history: Vec<personality::ChatMessage>,
) -> Result<String, String> {
    if let Ok(mut initiative) = state.initiative.lock() {
        initiative.record_user_activity();
    }
    if let Some(prompt) = explicit_image_prompt(&message) {
        if prompt.is_empty() {
            return Err("image prompt is empty".into());
        }
        let result = run_image_generation(&state, prompt.clone(), None).await?;
        return Ok(serde_json::json!({
            "content": "已完成生图.",
            "rewritten": false,
            "generated_image": true,
            "image_prompt": prompt,
            "images": result.get("images").cloned().unwrap_or_else(|| serde_json::json!([])),
            "prompt_id": result.get("prompt_id").cloned(),
            "workflow_path": result.get("workflow_path").cloned(),
        })
        .to_string());
    }

    let image_intent = classify_image_intent(&state, &message, &history).await;
    if image_intent.should_generate {
        let prompt = image_intent
            .image_prompt
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(message.trim())
            .to_string();
        let result =
            run_image_generation(&state, prompt.clone(), image_intent.negative_prompt.clone())
                .await?;
        let content = image_intent
            .response_text
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("我判断这条消息是在请求生图, 已把提示词交给 ComfyUI.");
        return Ok(serde_json::json!({
            "content": content,
            "rewritten": false,
            "generated_image": true,
            "image_prompt": prompt,
            "negative_prompt": image_intent.negative_prompt,
            "images": result.get("images").cloned().unwrap_or_else(|| serde_json::json!([])),
            "prompt_id": result.get("prompt_id").cloned(),
            "workflow_path": result.get("workflow_path").cloned(),
        })
        .to_string());
    }

    let cfg = current_config(&state);
    let role_id = cfg.personality.default_profile.clone();
    let assembler =
        PromptAssembler::load(&role_id).map_err(|e| format!("failed to load persona: {}", e))?;

    let persona_name = assembler.persona_name().to_string();

    let memories = memory::relevant_memories(&role_id, &message, 8).unwrap_or_else(|e| {
        warn!(error = %e, "failed to load memory context");
        Vec::new()
    });
    let memory_context = memory::format_memory_context(&memories);
    let messages =
        assembler.assemble_messages_with_context(&message, &history, memory_context.as_deref());
    let messages_json = serde_json::Value::Array(messages);

    if cfg.observability.prompt_logs {
        observability::log_prompt(&messages_json);
    }

    let job = crate::protocol::Job::new(
        "chat",
        crate::protocol::Capability::Chat,
        serde_json::json!({ "messages": messages_json }),
    );

    info!(
        job_id = %job.id,
        persona = %persona_name,
        "sending chat message to remote API"
    );

    // Step 1: Get raw response from remote (DeepSeek)
    let result = state.remote_worker.infer(&job).await.map_err(|e| {
        error!(error = %e, "remote inference failed");
        format!("inference failed: {}", e)
    })?;

    let raw_content = result["content"].as_str().unwrap_or("").to_string();

    if cfg.observability.token_usage {
        if let Some(usage) = result.get("usage") {
            let model = result["model"].as_str().unwrap_or("unknown");
            observability::log_token_usage(model, usage);
        }
    }

    // Step 2: Optionally rewrite through local personality layer
    if cfg.persona_rewrite.enabled {
        if !state.local_llm_available {
            warn!("persona rewrite enabled but local LLM is not available");
            return Ok(serde_json::json!({
                "content": raw_content,
                "rewritten": false
            })
            .to_string());
        }

        if let Some(ref local_worker) = state.local_worker {
            info!("persona rewrite enabled, routing through local LLM");

            let persona = assembler.persona_config();
            let rewriter = PersonaRewriter::new(
                persona,
                cfg.persona_rewrite.prompt_template.clone(),
                cfg.persona_rewrite.temperature,
                cfg.persona_rewrite.max_tokens,
            );

            let rewrite_payload = rewriter.build_rewrite_job_payload(&raw_content);
            let rewrite_job = crate::protocol::Job::new(
                "persona_rewrite",
                crate::protocol::Capability::Chat,
                rewrite_payload,
            );

            let requirements = local_worker.resource_requirements();
            let local_exclusive = ResourceManager::requires_local_exclusivity(&requirements);
            let wait_started = Instant::now();
            while !state
                .resources
                .try_acquire(&rewrite_job, local_worker.worker_id(), &requirements)
                .await
            {
                if wait_started.elapsed() >= Duration::from_millis(rewrite_job.timeout_ms) {
                    warn!(
                        job_id = %rewrite_job.id,
                        worker_id = %local_worker.worker_id(),
                        timeout_ms = rewrite_job.timeout_ms,
                        "persona rewrite timed out while waiting for local model resource"
                    );
                    return Ok(serde_json::json!({
                        "content": raw_content,
                        "rewritten": false
                    })
                    .to_string());
                }
                sleep(Duration::from_millis(50)).await;
            }

            let rewrite_result = local_worker.infer(&rewrite_job).await;
            if local_exclusive {
                state.resources.release(&rewrite_job.id).await;
            }

            match rewrite_result {
                Ok(rewritten) => {
                    let final_content = rewritten["content"]
                        .as_str()
                        .unwrap_or(&raw_content)
                        .to_string();
                    info!(
                        rewrite_len = final_content.len(),
                        original_len = raw_content.len(),
                        "persona rewrite complete"
                    );
                    return Ok(serde_json::json!({
                        "content": final_content,
                        "rewritten": true
                    })
                    .to_string());
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        "persona rewrite failed, falling back to raw response"
                    );
                    // Fall through to return raw_content
                }
            }
        } else {
            warn!("persona rewrite enabled but no local worker available");
        }
    }

    Ok(serde_json::json!({
        "content": raw_content,
        "rewritten": false
    })
    .to_string())
}

async fn wait_for_local_worker_health(worker: &LocalLlmWorker, timeout_ms: u64) -> bool {
    let started = Instant::now();
    loop {
        if worker.health_check_http().await {
            info!(
                worker_id = %worker.worker_id(),
                elapsed_ms = started.elapsed().as_millis(),
                "local LLM health check passed"
            );
            return true;
        }

        if started.elapsed() >= Duration::from_millis(timeout_ms) {
            warn!(
                worker_id = %worker.worker_id(),
                timeout_ms,
                "local LLM health check timed out"
            );
            return false;
        }

        sleep(Duration::from_millis(500)).await;
    }
}

async fn wait_for_comfyui_health(worker: &ComfyUiWorker, timeout_ms: u64) -> bool {
    let started = Instant::now();
    loop {
        if worker.health_check_http().await {
            info!(
                worker_id = %worker.worker_id(),
                elapsed_ms = started.elapsed().as_millis(),
                "ComfyUI health check passed"
            );
            return true;
        }

        if started.elapsed() >= Duration::from_millis(timeout_ms) {
            warn!(
                worker_id = %worker.worker_id(),
                timeout_ms,
                "ComfyUI health check timed out"
            );
            return false;
        }

        sleep(Duration::from_millis(500)).await;
    }
}

// ── Entrypoint ──

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Hestia runtime starting (Phase 5)");

    let app_config = config::load_config().expect("failed to load configuration");
    info!(
        "loaded config: app.name={}, theme={}, persona_rewrite={}",
        app_config.app.name, app_config.app.theme.mode, app_config.persona_rewrite.enabled,
    );

    observability::init(&app_config);

    // Remote API worker (DeepSeek)
    let remote_worker: Arc<dyn Worker> = match RemoteApiWorker::new(
        "deepseek_chat",
        vec![protocol::Capability::Chat],
        app_config.remote_api.clone(),
    ) {
        Ok(w) => {
            info!("remote API worker initialized");
            Arc::new(w)
        }
        Err(e) => {
            warn!("remote API worker unavailable: {}, falling back to mock", e);
            Arc::new(MockWorker::new(
                "mock_chat",
                vec![protocol::Capability::Chat],
                100,
            ))
        }
    };

    let backend_process = Arc::new(Mutex::new(BackendProcess::new()));
    // Local LLM worker (llama.cpp) for persona rewriting
    let local_worker_concrete: Option<Arc<LocalLlmWorker>> = if app_config.local_llm.enabled {
        let w = Arc::new(LocalLlmWorker::new(
            "local_qwen",
            vec![protocol::Capability::Chat],
            app_config.local_llm.base_url.clone(),
            app_config.local_llm.model.clone(),
        ));
        info!(
            base_url = %app_config.local_llm.base_url,
            "local LLM worker created for persona rewriting"
        );
        Some(w)
    } else {
        info!("local LLM worker disabled in config");
        None
    };

    // Auto-load local model if configured
    if app_config.local_llm.auto_load && app_config.local_llm.enabled {
        let (host, port) = local_llm_endpoint(&app_config.local_llm.base_url);
        if app_config.local_llm.backend == "llama_cpp" {
            model_loader::kill_port(port);
        }
        let model_path = model_loader::find_model_path(
            &app_config.local_llm.models_dir,
            &app_config.local_llm.model,
        );
        if let Some(model_path) = model_path {
            let model_path_str = model_path.to_string_lossy().to_string();
            let load_cmd = if app_config.local_llm.load_command.is_empty() {
                model_loader::build_default_load_command(
                    &app_config.local_llm.backend,
                    &model_path_str,
                    port,
                    &host,
                )
            } else {
                model_loader::expand_command_placeholders(
                    &app_config.local_llm.load_command,
                    &model_path_str,
                    port,
                    &host,
                )
            };
            let unload_cmd = model_loader::expand_command_placeholders(
                &app_config.local_llm.unload_command,
                &model_path_str,
                port,
                &host,
            );
            if let Ok(mut process) = backend_process.lock() {
                process.set_unload_command(unload_cmd);
            }
            if !load_cmd.is_empty() {
                info!(command = %load_cmd, "auto-loading local model");
                if let Ok(mut process) = backend_process.lock() {
                    process.spawn(
                        &load_cmd,
                        &app_config.local_llm.model,
                        &app_config.local_llm.backend,
                    );
                }
            }
        } else {
            warn!(
                model = %app_config.local_llm.model,
                models_dir = %app_config.local_llm.models_dir,
                "model not found, auto-load skipped"
            );
        }
    }

    let local_llm_available = if let Some(worker) = &local_worker_concrete {
        let timeout_ms = if app_config.local_llm.auto_load {
            20000
        } else {
            2000
        };
        tauri::async_runtime::block_on(wait_for_local_worker_health(worker, timeout_ms))
    } else {
        false
    };
    let local_worker: Option<Arc<dyn Worker>> =
        local_worker_concrete.map(|worker| worker as Arc<dyn Worker>);
    let resources = Arc::new(ResourceManager::new(app_config.observability.vram_logs));

    let comfyui_process = Arc::new(Mutex::new(BackendProcess::new()));
    let comfyui_worker_concrete: Option<Arc<ComfyUiWorker>> =
        if app_config.multimodal.comfyui.enabled {
            let worker = Arc::new(ComfyUiWorker::new(
                "comfyui_image",
                vec![protocol::Capability::ImageGeneration],
                app_config.multimodal.comfyui.base_url.clone(),
                app_config.multimodal.comfyui.workflow_path.clone(),
                app_config.multimodal.comfyui.output_dir.clone(),
            ));
            if app_config.multimodal.comfyui.auto_start {
                if tauri::async_runtime::block_on(wait_for_comfyui_health(&worker, 1000)) {
                    info!("using externally available ComfyUI backend");
                } else {
                    info!("ComfyUI will be started on demand for image generation");
                }
            }
            Some(worker)
        } else {
            info!("ComfyUI worker disabled in config");
            None
        };

    let comfyui_available = if let Some(worker) = &comfyui_worker_concrete {
        tauri::async_runtime::block_on(wait_for_comfyui_health(worker, 1000))
    } else {
        false
    };

    let mut registry = WorkerRegistry::new();
    registry.register(Arc::new(MockWorker::new(
        "mock_chat",
        vec![protocol::Capability::Chat],
        100,
    )));
    let comfyui_worker_for_state = comfyui_worker_concrete.clone();
    if let Some(worker) = comfyui_worker_concrete {
        registry.register(worker as Arc<dyn Worker>);
    }
    if app_config.multimodal.vision.enabled {
        match VisionApiWorker::new(
            "kimi_vision",
            vec![protocol::Capability::Vision],
            app_config.multimodal.vision.clone(),
        ) {
            Ok(worker) => registry.register(Arc::new(worker) as Arc<dyn Worker>),
            Err(error) => warn!("vision API worker unavailable: {}", error),
        }
    } else {
        info!("vision worker disabled in config");
    }
    let registry = Arc::new(registry);

    let (job_tx, job_rx) = mpsc::channel::<SchedulerCommand>(64);
    let scheduler_resources = resources.clone();
    let initiative_runtime = Arc::new(Mutex::new(InitiativeRuntime::new()));
    install_signal_cleanup(backend_process.clone(), comfyui_process.clone());
    info!("config loaded and observability initialized");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            job_tx,
            remote_worker,
            local_worker,
            comfyui_worker: comfyui_worker_for_state,
            local_llm_available,
            comfyui_available,
            local_backend_process: backend_process,
            comfyui_backend_process: comfyui_process,
            resources,
            initiative: initiative_runtime,
            config: app_config,
        })
        .on_window_event(move |window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if matches!(window.label(), "main" | "companion" | "companion_dialog") {
                    api.prevent_close();
                    let app = window.app_handle();
                    let state = app.state::<AppState>();
                    if window.label() == "companion" {
                        if let Some(dialog) = app.get_webview_window("companion_dialog") {
                            let _ = dialog.hide();
                        }
                        let _ = app.emit_to("companion", "companion-dialog-visible-changed", false);
                        let _ = app.emit_to(
                            "companion_dialog",
                            "companion-dialog-visible-changed",
                            false,
                        );
                    } else if window.label() == "companion_dialog" {
                        let _ = app.emit_to("companion", "companion-dialog-visible-changed", false);
                        let _ = app.emit_to(
                            "companion_dialog",
                            "companion-dialog-visible-changed",
                            false,
                        );
                    }
                    if let Err(error) =
                        hide_window_and_maybe_idle_backend(app, &state, window.label())
                    {
                        warn!(
                            window = window.label(),
                            error, "failed to hide window on close"
                        );
                    }
                }
            } else if let tauri::WindowEvent::Focused(false) = event {
                if matches!(window.label(), "companion" | "companion_dialog") {
                    let _ = apply_window_topmost(window, true);
                }
            }
        })
        .setup(move |app| {
            let menu = MenuBuilder::new(app)
                .text("open_main", "Open Chat")
                .text("open_settings", "Open Settings")
                .text("open_companion", "Open Companion")
                .separator()
                .text("restart_backend", "Restart Backend")
                .separator()
                .text("quit", "Quit")
                .build()?;
            let _tray = TrayIconBuilder::with_id("hestia")
                .icon(app.default_window_icon().cloned().unwrap())
                .tooltip("Hestia")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Err(error) = show_window(&tray.app_handle(), "main") {
                            warn!(error, "failed to open main window from tray");
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open_main" => {
                        if let Err(error) = show_window(app, "main") {
                            warn!(error, "failed to open main window from tray menu");
                        }
                    }
                    "open_settings" => {
                        if let Err(error) = show_window(app, "main").and_then(|_| {
                            app.emit_to("main", "open-settings", ())
                                .map_err(|e| format!("failed to emit settings event: {}", e))
                        }) {
                            warn!(error, "failed to open settings from tray menu");
                        }
                    }
                    "open_companion" => {
                        if let Err(error) = set_companion_visible(app.clone(), true) {
                            warn!(error, "failed to open companion from tray menu");
                        }
                    }
                    "restart_backend" => {
                        let state = app.state::<AppState>();
                        stop_managed_backends(&state);
                    }
                    "quit" => {
                        let state = app.state::<AppState>();
                        stop_managed_backends(&state);
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;
            let scheduler = Scheduler::new(job_rx, registry, scheduler_resources);
            tauri::async_runtime::spawn(scheduler.run());
            info!("scheduler spawned");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            get_config_snapshot,
            show_main_window,
            open_settings_window,
            list_personas,
            list_roles,
            list_available_models,
            get_screenshot_metadata,
            set_companion_visible,
            set_companion_dialog_visible,
            set_companion_always_on_top,
            restart_backend,
            read_image_artifact,
            prepare_avatar_content,
            recognize_image,
            record_user_activity,
            evaluate_initiative,
            request_initiative_message,
            update_settings,
            submit_test_job,
            get_persona_content,
            save_persona_content,
            role_storage_paths,
            set_active_role,
            delete_role,
            generate_role_profile,
            list_memories,
            create_memory,
            update_memory,
            delete_memory,
            send_chat_message,
            generate_test_image,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Hestia");
    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            let state = app_handle.state::<AppState>();
            stop_managed_backends(&state);
        }
    });
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{encode_base64, explicit_image_prompt, image_mime_for_path, parse_image_intent};

    #[test]
    fn test_encode_base64() {
        assert_eq!(encode_base64(b""), "");
        assert_eq!(encode_base64(b"f"), "Zg==");
        assert_eq!(encode_base64(b"fo"), "Zm8=");
        assert_eq!(encode_base64(b"foo"), "Zm9v");
    }

    #[test]
    fn test_explicit_image_prompt() {
        assert_eq!(
            explicit_image_prompt("\\image a red moon").as_deref(),
            Some("a red moon")
        );
        assert_eq!(
            explicit_image_prompt("/image a blue lake").as_deref(),
            Some("a blue lake")
        );
        assert!(explicit_image_prompt("please make an image").is_none());
    }

    #[test]
    fn test_parse_image_intent_from_fenced_text() {
        let parsed = parse_image_intent(
            "```json\n{\"should_generate\":true,\"image_prompt\":\"cinematic lake\",\"negative_prompt\":null,\"response_text\":\"ok\"}\n```",
        );
        assert!(parsed.should_generate);
        assert_eq!(parsed.image_prompt.as_deref(), Some("cinematic lake"));
    }

    #[test]
    fn test_image_mime_for_path() {
        assert_eq!(
            image_mime_for_path(Path::new("screen.PNG")).unwrap(),
            "image/png"
        );
        assert_eq!(
            image_mime_for_path(Path::new("photo.jpeg")).unwrap(),
            "image/jpeg"
        );
        assert!(image_mime_for_path(Path::new("archive.zip")).is_err());
    }
}
