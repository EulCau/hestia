# Hestia Implementation Summary

**Date:** 2026-06-14
**Current Phase:** 8 hardened MVP + lifecycle polish (Desktop Companion Window)

---

## 1. Architecture Overview

```
┌─────────────────────────────────────────────────┐
│                    Tauri 2.x                      │
│  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  Frontend     │  │  Rust Backend             │  │
│  │  TypeScript   │  │  ┌────────────────────┐  │  │
│  │  Vite         │◄─┤  │  Scheduler          │  │  │
│  │               │  │  │  Job Queue           │  │  │
│  │  Chat UI      │  │  │  Worker Registry     │  │  │
│  │  Settings UI  │  │  │  Prompt Assembler    │  │  │
│  │  Theme System │  │  └────────────────────┘  │  │
│  │  Avatar Slot  │  │  ┌────────────────────┐  │  │
│  └──────────────┘  │  │  Workers             │  │  │
│                     │  │  - RemoteApiWorker   │  │  │
│                     │  │  - MockWorker        │  │  │
│                     │  └────────────────────┘  │  │
│                     └──────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

---

## 2. Implemented Features

### 2.1 Runtime Core (Phase 1)

| Component | File | Status |
|---|---|---|
| Job state machine | `src/protocol.rs` | Done |
| Status transition validation | `src/protocol.rs` | Done |
| Scheduler (tokio event loop) | `src/scheduler.rs` | Done |
| Worker trait + registry | `src/workers/mod.rs` | Done |
| Mock worker | `src/workers/mock.rs` | Done |
| Resource requirements struct | `src/protocol.rs` | Done |

**Job states:** Queued → WaitingResource → Running → Completed / Failed / Cancelled / Timeout

**Tests:** 7 unit/integration tests covering state transitions, dispatch, cancellation.

### 2.2 Remote Chat (Phase 2)

| Component | File | Status |
|---|---|---|
| Remote API worker (DeepSeek/OpenAI) | `src/workers/remote_api.rs` | Done |
| Prompt assembler (system prompt + persona) | `src/personality/mod.rs` | Done |
| Persona config loader | `src/personality/mod.rs` | Done |
| `send_chat_message` Tauri command | `src/lib.rs` | Done |
| Fallback to mock worker when no API key | `src/lib.rs` | Done |

### 2.3 Configuration System

| Component | File | Status |
|---|---|---|
| Typed config schema (serde + toml) | `src/config.rs` | Done |
| `config/default.toml` (template) | `config/default.toml` | Done |
| `config/user.toml` (user overrides, gitignored) | `config/user.toml` | Done |
| Deep merge (user overrides default) | `src/config.rs` | Done |
| `get_config_snapshot` Tauri command | `src/lib.rs` | Done |
| `update_settings` Tauri command | `src/lib.rs` | Done |

**API key storage:** Set in `config/user.toml` under `[remote_api] api_key = "sk-..."` or via `DEEPSEEK_API_KEY` env var. The user config takes priority.

### 2.4 UI

| Feature | Status |
|---|---|
| Chat interface (message bubbles, input, loading) | Done |
| Dark / Light / System theme | Done |
| Theme toggle in sidebar | Done |
| Theme auto-follows system preference | Done |
| Settings panel (API key, base URL, model, theme) | Done |
| Left sidebar with companion avatar | Done |
| Avatar placeholder image (generated) | Done |
| Clear chat button | Done |
| Error display on API failure | Done |

### 2.5 Observability

| Metric | Status |
|---|---|
| Job timeline logs | Done |
| Prompt logs (assembled prompts) | Done |
| Token usage logs | Done |
| Structured tracing output | Done |

---

## 3. Interfaces for Future Use

### 3.1 Avatar Adapter Interface

```typescript
// frontend/src/main.ts
interface AvatarAdapter {
  type: "placeholder" | "live2d" | "digital_human";
  mount(container: HTMLElement): void;
  unmount(): void;
  onEvent?(event: string, data: unknown): void;
}

function createAvatarAdapter(modelType: string, imagePath: string): AvatarAdapter
```

**How to use later:** When integrating Live2D or a digital human model, add a new case in `createAvatarAdapter()` that instantiates the appropriate renderer. The `mount` function receives the sidebar `<div class="avatar-container">`.

### 3.2 Worker Trait

```rust
// src/workers/mod.rs
#[async_trait]
pub trait Worker: Send + Sync {
    fn worker_id(&self) -> &str;
    fn capabilities(&self) -> Vec<Capability>;
    fn health_check(&self) -> bool;
    fn resource_requirements(&self) -> ResourceRequirements;
    async fn infer(&self, job: &Job) -> Result<serde_json::Value, WorkerError>;
}
```

**How to add a new worker:** Implement this trait and register via `WorkerRegistry::register()`. Examples for future: `LlamaCppWorker`, `ComfyUIWorker`, `VisionWorker`, `TtsWorker`.

### 3.3 Tauri Command Interface

| Command | Type | Purpose |
|---|---|---|
| `get_app_info` | Query | App name, version, phase |
| `get_config_snapshot` | Query | Full config for settings UI |
| `list_personas` | Query | Available persona profiles |
| `update_settings` | Mutate | Write settings to user.toml |
| `send_chat_message` | Mutate | Send message, get AI reply |
| `submit_test_job` | Mutate | Submit job to scheduler (debug) |

### 3.4 Config Schema

```toml
[app.theme]
mode = "system"       # "system" | "dark" | "light"

[app.avatar]
enabled = true
image_path = "companion-cat-placeholder.png"
model_type = "placeholder"  # "placeholder" | "live2d" | "digital_human"

[remote_api]
base_url = "https://api.deepseek.com"
api_key_env = "DEEPSEEK_API_KEY"
model = "deepseek-chat"
api_key = "sk-..."    # Only in user.toml (gitignored)

[personality]
default_profile = "default"
```

---

## 4. File Inventory

```
src-tauri/
  Cargo.toml              Tauri 2.x + reqwest + tokio + tracing
  tauri.conf.json          Window config, dev URL
  src/
    main.rs                Binary entry
    lib.rs                 App composition, Tauri commands
    config.rs              Config loading, merge, update
    protocol.rs            Job, Capability, WorkerInfo, state machine
    scheduler.rs           Job queue dispatch loop
    workers/
      mod.rs               Worker trait, registry
      mock.rs              Mock worker for testing
      remote_api.rs        DeepSeek/OpenAI API worker
    personality/
      mod.rs               Prompt assembler, persona config
    observability.rs       Prompt/token logging
    runtime.rs             Placeholder

frontend/
  package.json             Vite + TypeScript + @tauri-apps/api
  vite.config.ts
  tsconfig.json
  index.html               Error catcher + module entry
  public/
    companion-cat-placeholder.png  Anime cat companion placeholder
  src/
    main.ts                Chat UI + settings + sidebar
    style.css              Theme system + layout

config/
  default.toml             Template config
  user.toml                User overrides (gitignored)

personality/
  default.json             Default persona

assets/
  companion-cat-placeholder.png  Original generated cat placeholder

docs/
  implementation-summary.md  This document
```

---

## 5. Running the App

```bash
# From project root:
./frontend/node_modules/.bin/tauri dev

# With API key (choose one):
# Option A: Set in config/user.toml
echo '[remote_api]' > config/user.toml
echo 'api_key = "sk-your-key"' >> config/user.toml

# Option B: Set env var
export DEEPSEEK_API_KEY=sk-your-key
./frontend/node_modules/.bin/tauri dev

# With software rendering if GBM buffer errors:
WEBKIT_DISABLE_COMPOSITING_MODE=1 ./frontend/node_modules/.bin/tauri dev
```

---

## 6. Next Phases (from work-order.md)

| Phase | Description | Status |
|---|---|---|
| 3 | Local personality rewriter (llama.cpp) | Done |
| 4 | GPU Resource Manager / Model Auto-Load | Done |
| 5 | Multimodal (screenshots, ComfyUI, vision) | Done (MVP) |
| 6 | Initiative system | Done (MVP) |
| 8 | Desktop companion window | Done (hardened MVP + lifecycle polish) |
| 7 | Plugin boundary + packaging | Pending |

Recommended next step: add the Live2D expression event skeleton before Plugin Boundary. Phase 8 now has the second-window lifecycle, placeholder avatar, show/hide control, tray controls, companion-owned initiative timer, hover toolbar, local dialogue bubble, persisted companion position/size, and synchronized dialogue visibility lifecycle.

Live2D/VRM remain rendering upgrades after the companion window contract is stable.

Resume note for a new conversation:
- Start from [docs/HANDOFF.md](/home/eulcau/CXTX/hestia/docs/HANDOFF.md), §10 and §11.
- Treat [docs/ui-interface-contract.md](/home/eulcau/CXTX/hestia/docs/ui-interface-contract.md), §3.4, as the authoritative companion window/dialogue state contract.
- The next implementation should add only the Live2D expression event skeleton. Do not add Live2D assets or Plugin Boundary work until the event names and placeholder adapter behavior are stable.
- Validate with `cargo fmt --manifest-path src-tauri/Cargo.toml`, `cargo test --manifest-path src-tauri/Cargo.toml`, `npm run build`, and `./frontend/node_modules/.bin/tauri build --debug --no-bundle`.

## 6.1 Phase 6: Initiative System (2026-06-13)

| Component | File | Status |
|---|---|---|
| Initiative policy runtime | `src/initiative.rs` | Done |
| `record_user_activity` command | `src/lib.rs` | Done |
| `evaluate_initiative` command | `src/lib.rs` | Done |
| `request_initiative_message` command | `src/lib.rs` | Done |
| Initiative settings UI | `frontend/src/main.ts` | Done |
| Manual main-window check | `frontend/src/main.ts` | Done |

Policy inputs:
- `initiative.enabled`
- `initiative.level`
- `initiative.cooldown_ms`
- runtime idle time since latest user activity

Decision reasons:
- `initiative_disabled`
- `non_companion_trigger`
- `user_recently_active`
- `cooldown_active`
- `score_below_threshold`

Frontend behavior:
- Manual sparkles button asks for a guarded proactive message.
- Main window mode does not speak proactively on a timer. The user must click the sparkles button to open a proactive topic.
- Automatic proactive speech is owned by the desktop companion window and must use a trigger beginning with `companion`.

## 6.2 Phase 8: Desktop Companion Window MVP (2026-06-13)

| Component | File | Status |
|---|---|---|
| Companion Tauri window (`companion`) | `src-tauri/tauri.conf.json` | Done |
| Companion dialogue window (`companion_dialog`) | `src-tauri/tauri.conf.json` | Done |
| Companion visibility command | `src-tauri/src/lib.rs` | Done |
| Companion capability window access | `src-tauri/capabilities/default.json` | Done |
| Main-window show/hide control | `frontend/src/main.ts` | Done |
| Companion-only frontend view | `frontend/src/main.ts` | Done |
| Companion timer using `companion_timer` | `frontend/src/main.ts` | Done |
| Companion styles | `frontend/src/style.css` | Done |
| Anime cat placeholder asset | `frontend/public/companion-cat-placeholder.png` | Done |
| Companion hover toolbar | `frontend/src/main.ts` | Done |
| Companion drag and resize | `frontend/src/main.ts` | Done |
| Companion dialogue bubble window | `frontend/src/main.ts` | Done |
| Tray icon and menu | `src-tauri/src/lib.rs` | Done |
| Main-window backend restart control | `frontend/src/main.ts` | Done |
| Initiative hot config reload | `src-tauri/src/lib.rs` | Done |
| Companion position/size persistence | `config/default.toml`, `src-tauri/src/config.rs`, `frontend/src/main.ts` | Done |
| Companion dialogue lifecycle sync | `src-tauri/src/lib.rs`, `frontend/src/main.ts` | Done |

Behavior:
- The main window remains the chat/settings control surface.
- The companion window starts hidden and is shown/hidden through `set_companion_visible`.
- The companion view renders only the placeholder avatar and hover controls.
- The companion dialogue uses a separate transparent window that follows the companion without overlapping the avatar when screen space allows, with `set_companion_dialog_visible` as a backend show/hide fallback.
- Timer-triggered proactive speech is requested only from the companion view with `trigger = "companion_timer"`.
- Blocked timer decisions and model errors stay silent in the companion surface.
- Hover controls expose always-on-top, proactive speech, open chat, and dialogue toggles.
- Closing main/companion/dialog hides the window. Closing companion also hides the dialog. If all are hidden, managed backend processes are stopped while the tray process remains alive.
- Always-on-top is set through Tauri and reapplied on focus loss, but true global stacking can still be limited by Wayland compositors.
- The tray left-click opens chat. The tray menu opens chat/settings/companion, restarts managed backend processes, or quits.
- Initiative settings are hot-read before evaluation, so proactive speech enable/disable applies immediately.
- The companion window restores configured bounds on startup and persists drag/resize changes to `config/user.toml`.
- Dialogue visibility is synchronized through `companion-dialog-visible-changed`; hiding the companion or dialogue invalidates in-flight local dialogue requests.

---

## 7. Phase 3: Local Personality Layer (2026-06-06)

### 7.1 Architecture

```
User message
  → PromptAssembler (system prompt + persona)
  → RemoteApiWorker (DeepSeek) → raw_response
  → [if persona_rewrite enabled]
      → PersonaRewriter builds rewrite prompt ({tone}, {style_rules}, {content})
      → LocalLlmWorker (llama.cpp via HTTP) → rewritten final_response
  → [if disabled or unavailable]
      → raw_response as final_response
```

### 7.2 New Components

| Component | File | Status |
|---|---|---|
| LocalLlmWorker | `src/workers/local_llm.rs` | Done |
| PersonaRewriter | `src/personality/mod.rs` | Done |
| Persona rewrite config | `config/default.toml` [persona_rewrite] | Done |
| Local LLM config | `config/default.toml` [local_llm] | Done |
| Rewrite toggle in settings UI | `frontend/src/main.ts` | Done |
| Bypass fallback on rewrite failure | `src/lib.rs` | Done |

### 7.3 How to Enable

1. Start a llama.cpp server:
   ```bash
   llama-server -m qwen2.5-7b-instruct-q4_k_m.gguf --port 8080
   ```

2. Enable in settings UI or edit `config/user.toml`:
   ```toml
   [local_llm]
   enabled = true

   [persona_rewrite]
   enabled = true
   ```

3. Restart Hestia. DeepSeek responses will be rewritten through the local model.

### 7.4 Degradation Behavior

- If `persona_rewrite.enabled = true` but no local LLM is configured → warn log, return raw DeepSeek response
- If rewrite succeeds but local LLM errors → warn log, return raw DeepSeek response
- Both paths are non-blocking; chat always returns a response

### 7.5 Tests

23 unit/integration tests all passing:
- 4 protocol state machine tests
- 2 config merge tests
- 1 mock worker test
- 2 scheduler integration tests
- 2 resource manager exclusivity tests
- 1 ComfyUI history output extraction test
- 1 SDXL UI workflow -> API prompt conversion test
- 1 current config load test
- 1 image preview base64 encoding test
- 1 explicit image command parsing test
- 1 image intent JSON parsing test
- 1 local image MIME validation test
- 1 Kimi/Moonshot chat completions URL test
- 3 initiative policy tests for disabled, recent activity, and cooldown blocking
- 1 initiative trigger source test reserving automatic speech for companion mode

### 7.6 Config Reference

```toml
[local_llm]
base_url = "http://127.0.0.1:8080"
model = "qwen2.5-7b"
enabled = false

[persona_rewrite]
enabled = false
temperature = 0.7
max_tokens = 512
prompt_template = "Rewrite the following message to match this tone: {tone}. Style rules: {style_rules}. Original message: {content}\n\nRewritten:"
```

The `prompt_template` supports three placeholders: `{tone}`, `{style_rules}`, `{content}`.

---

## 8. Phase 4: Model Auto-Load (2026-06-07)

### 8.1 New Components

| Component | File | Status |
|---|---|---|
| Model discovery (scan models_dir) | `src/workers/model_loader.rs` | Done |
| Auto-load backend process lifecycle | `src/workers/model_loader.rs` | Done |
| `list_available_models` Tauri command | `src/lib.rs` | Done |
| Backend expansion (llama_cpp, ollama, vllm) | `src/config.rs` | Done |
| Auto-load toggle in Settings UI | `frontend/src/main.ts` | Done |
| Custom load/unload command inputs | `frontend/src/main.ts` | Done |
| ResourceManager coarse local exclusivity | `src/resource/mod.rs` | Done |
| Scheduler resource wait/release flow | `src/scheduler.rs` | Done |
| Persona rewrite resource gate | `src/lib.rs` | Done |
| Async local worker health check | `src/workers/local_llm.rs`, `src/lib.rs` | Done |

### 8.2 Architecture

```
App Start
  -> if [local_llm] auto_load = true && enabled = true
      -> find_model_path(models_dir, model)
      -> build_default_load_command(backend, model_path, port, host)
        or use user-provided load_command with {model_path}, {port}, {host} expansion
      -> spawn subprocess (BackendProcess)
      -> wait for LocalLlmWorker /health until timeout
  -> LocalLlmWorker connects to base_url as before

App Exit
  -> if unload_command is configured, run it first
  -> BackendProcess::drop() kills the spawned process tree as fallback
```

### 8.3 Config Reference

```toml
[local_llm]
backend = "llama_cpp"     # "llama_cpp" | "ollama" | "vllm"
base_url = "http://127.0.0.1:8080"
model = "qwen3-8b"        # or "qwen/Qwen3-8B-Q4_K_M" for manufacturer/model_name
enabled = false
auto_load = false
models_dir = ""            # default: ~/models (Linux) or %USERPROFILE%\models (Windows)
load_command = ""          # overrides auto-generated command
unload_command = ""        # overrides auto-generated unload command
```

### 8.4 Default Load Commands

| Backend | Default Command |
|---|---|
| `llama_cpp` | `llama-server -m {model_path} --port {port} --host {host} --ctx-size 4096 --n-gpu-layers 999 --flash-attn --reasoning off` |
| `ollama` | `ollama pull {model_name}` |
| `vllm` | Not supported (manual start required) |

### 8.5 Model Path Resolution

- `model = "qwen/Qwen3-8B-Q4_K_M"` -> `models_dir/qwen/Qwen3-8B-Q4_K_M.gguf`
- `model = "qwen3-8b"` -> searched through models_dir subdirectories for matching .gguf

### 8.6 Arch Linux Package Dependency (Future)

When packaging Hestia as an Arch Linux package, `llama.cpp` should be listed as an optional dependency (provides `llama-server`). `ollama` is an alternative. This is deferred to Phase 7 or a dedicated packaging sub-phase (see [docs/work-order.md](/home/eulcau/CXTX/hestia/docs/work-order.md)).

### 8.7 Resource Policy

Phase 4 uses coarse local model exclusivity:

- Any worker with `ResourceRequirements.gpu_required = true` or `vram_mb = Some(...)` requires the local model resource.
- The scheduler moves a queued job to `WaitingResource` while the local resource is occupied.
- If the wait exceeds `job.timeout_ms`, the job transitions to `Timeout`.
- Resource acquire, busy, and release events are logged when `[observability] vram_logs = true`.
- The persona rewrite path uses the same `ResourceManager`, so concurrent chat requests cannot send multiple rewrite jobs to the local LLM at the same time.

This is intentionally conservative. It does not estimate free VRAM or attempt fine-grained GPU preemption.

### 8.8 Health Check Behavior

`LocalLlmWorker` performs an async HTTP health check against `{base_url}/health` during startup:

- If `auto_load = true`, Hestia waits up to 20 seconds after spawning the backend.
- If `auto_load = false` but local LLM is enabled, Hestia checks for up to 2 seconds.
- `ConfigSnapshot.local_llm.available` reflects the startup health check result.
- If persona rewrite is enabled but the local worker is unavailable, chat falls back to the raw remote response.

---

## 9. Phase 5: Multimodal / ComfyUI Image Generation (2026-06-11)

### 9.1 New Components

| Component | File | Status |
|---|---|---|
| ComfyUI worker | `src/workers/comfyui.rs` | Done |
| Multimodal workflow utilities | `src/multimodal/mod.rs` | Done |
| SDXL test workflow | `assets/workflows/sdxl.json` | Done |
| `generate_test_image` Tauri command | `src/lib.rs` | Done |
| `get_screenshot_metadata` Tauri command | `src/lib.rs` | Done |
| `read_image_artifact` preview command | `src/lib.rs` | Done |
| ComfyUI settings UI | `frontend/src/main.ts` | Done |
| Image test modal | `frontend/src/main.ts` | Done |
| In-chat image generation | `src/lib.rs`, `frontend/src/main.ts` | Done |
| Kimi vision recognition | `src/workers/vision_api.rs`, `src/lib.rs`, `frontend/src/main.ts` | Done |

### 9.2 ComfyUI Configuration

```toml
[multimodal.comfyui]
enabled = true
base_url = "http://127.0.0.1:8188"
root_dir = "/home/eulcau/models/ComfyUI"
python_path = "/home/eulcau/miniconda3/envs/comfyui/bin/python"
env_type = "conda"
auto_start = false
launch_command = ""
workflow_path = "assets/workflows/sdxl.json"
output_dir = "data/artifacts/images"
startup_timeout_ms = 20000
```

`python_path` points directly to the Python executable inside conda or venv. This avoids shell activation and is portable across conda, venv, and system Python.

`auto_start` now means on-demand start for image jobs. Hestia does not launch ComfyUI at app startup. When image generation is requested and no external ComfyUI server is healthy, Hestia launches the configured process, runs the job, then stops that managed process. If an external ComfyUI server is already available, Hestia uses it but does not terminate it.

For llama.cpp, starting the server with a model path loads the model and may occupy VRAM immediately. Therefore local LLM auto-load is useful for low-latency rewrite, but it cannot satisfy “backend started, model not resident in VRAM” with the current llama.cpp process mode.

### 9.3 Image Generation Flow

```
Image Test UI or chat image request
  -> invoke("generate_test_image", { prompt, negativePrompt })
  -> or invoke("send_chat_message", { message: "\\image ...", history })
  -> Job(capability = ImageGeneration)
  -> Scheduler
  -> ResourceManager local GPU slot
  -> ComfyUiWorker
  -> POST /prompt
  -> poll /history/{prompt_id}
  -> download /view outputs
  -> save images under data/artifacts/images
  -> stop managed ComfyUI process if Hestia started it for this job
```

Chat image generation supports explicit `\image prompt` and `/image prompt`, an input image button, and conservative model-routed intent detection. The router uses the configured remote chat worker and must return strict JSON before Hestia runs ComfyUI.

### 9.4 Kimi Vision Recognition

```toml
[multimodal.vision]
enabled = false
base_url = "https://api.moonshot.ai"
api_key_env = "MOONSHOT_API_KEY"
model = "kimi-k2.6"
max_image_bytes = 20971520
```

The `recognize_image` Tauri command accepts a local image path plus an optional prompt. It converts supported local images (`png`, `jpeg`, `webp`, `gif`) to a base64 data URL and submits a `Vision` Job through the Scheduler to `VisionApiWorker`.

The current UI exposes this through the eye icon in the chat input area. The textarea content is used as the question about the selected image; empty input uses `multimodal.vision.default_prompt`.

The backend helper also accepts a `source` label. Future screenshot capture should call the same helper with `source = "screenshot"` and the screenshot file path.

### 9.5 Workflow Support

The ComfyUI worker supports:

- API workflow JSON with `class_type`
- UI workflow JSON exported from the ComfyUI canvas, converted into API prompt format
- Prompt override for PrimitiveNode titles containing `Positive` / `Negative`

### 9.6 Test Workflow Requirements

The bundled `assets/workflows/sdxl.json` references:

- `sd_xl_base_1.0.safetensors`
- `sd_xl_refiner_1.0.safetensors`

These files must exist in the configured ComfyUI `models/checkpoints` directory for real generation to succeed.

### 9.7 Screenshot / Vision Boundary

Phase 5 keeps screenshots conservative:

- `get_screenshot_metadata` exposes current screenshot settings and reports `capture_available = false`.
- No automatic screenshot capture runs in the background.
- Vision jobs are implemented for local image paths. Screenshot capture remains disabled, but the backend vision helper is ready to accept future screenshot file paths.
