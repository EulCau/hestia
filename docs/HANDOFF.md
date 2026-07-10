# Hestia — Handoff Document

**Date:** 2026-06-15
**Current Phase:** 7 roles + memory core MVP, 8 hardened MVP + lifecycle polish
**Codebase:** Rust + TypeScript, 23 passing tests

---

## 1. What Has Been Built

### Phase 0 — Project Scaffold
- [x] Tauri 2.x project with Rust backend + Vite/TypeScript frontend
- [x] `tauri.conf.json`: 900x700 window, dev server on :1420
- [x] Icon assets (RGBA PNG), placeholder avatar
- [x] `config/default.toml` with typed schema
- [x] `config/user.toml` for per-user overrides (gitignored)
- [x] Structured logging via `tracing` + `tracing-subscriber`

### Phase 1 — Runtime Core
- [x] **Job state machine** (`protocol.rs`): Queued → WaitingResource → Running → Completed/Failed/Cancelled/Timeout
- [x] **Status transition validation**: legal transitions white-listed, illegal transitions rejected with error
- [x] **Scheduler** (`scheduler.rs`): tokio mpsc channel loop, dispatch to worker, timeout handling
- [x] **Worker trait** (`workers/mod.rs`): `worker_id()`, `capabilities()`, `health_check()`, `resource_requirements()`, `infer()`
- [x] **WorkerRegistry**: register workers, find by capability, sorted by health+priority
- [x] **MockWorker**: configurable delay, returns dummy JSON response

### Phase 2 — Remote Chat
- [x] **RemoteApiWorker** (`workers/remote_api.rs`): DeepSeek/OpenAI-compatible `/v1/chat/completions`
- [x] **PromptAssembler** (`personality/mod.rs`): loads role JSON, builds system prompt, assembles OpenAI messages format
- [x] **Role config** (`personality/default.json`): role identity, appearance, personality, language habits, scenario, and tone
- [x] **send_chat_message** Tauri command: loads active role → assembles prompt → calls DeepSeek → returns response
- [x] **API key management**: from `config/user.toml` `[remote_api] api_key` or `DEEPSEEK_API_KEY` env var
- [x] **Observability**: prompt logs, token usage logs with `tracing` targets
- [x] **Chat UI** (`frontend/src/main.ts`): message bubbles, input area, loading indicator, error display
- [x] **Theme system** (`frontend/src/style.css`): dark/light/system, CSS custom properties, sidebar toggle
- [x] **Settings panel**: modal overlay, API key/base URL/model/theme configuration
- [x] **Sidebar**: avatar placeholder, companion name, status indicators
- [x] **Avatar adapter interface**: `mount(container)`, `unmount()`, `onEvent?()` — forward-compatible with Live2D/digital human
- [x] **Session context**: `chatHistory` array maintained in frontend, passed to backend on each message

### Phase 3 — Local Personality Rewrite
- [x] **LocalLlmWorker** (`workers/local_llm.rs`): HTTP client for llama.cpp/vLLM OpenAI-compatible servers
- [x] **PersonaRewriter** (`personality/mod.rs`): builds rewrite prompt from template with `{tone}`, `{style_rules}`, `{content}`
- [x] **Rewrite pipeline in send_chat_message**: DeepSeek → (if enabled) PersonaRewriter → LocalLlmWorker → final response
- [x] **Degradation**: if rewrite fails, returns raw DeepSeek response (non-blocking)
- [x] **Backend selector**: UI dropdown for llama.cpp vs vLLM
- [x] **Role manager**: modal/form UI with load/save/generate, validates against `PersonaConfig` schema
- [x] **Qwen3 thinking mode handling**: `reasoning_content` fallback when `content` is empty
- [x] **max_tokens**: 2048 default for rewrite to accommodate thinking mode
- [x] **Rewrite status indicators**: sidebar shows `LLM: On/Off | Rewrite: On/Off`, rewritten messages have purple left border

### Phase 4 — GPU Resource Manager / Model Auto-Load
- [x] **Model discovery** (`workers/model_loader.rs`): scans configured `models_dir` for `.gguf` files
- [x] **Auto-load lifecycle** (`workers/model_loader.rs`): expands load command placeholders and spawns backend subprocesses
- [x] **Custom unload command**: optional `unload_command` runs before default process-tree termination
- [x] **ResourceManager** (`resource/mod.rs`): coarse exclusivity for local model jobs
- [x] **Scheduler resource wait** (`scheduler.rs`): local jobs wait in `WaitingResource` and time out if resource wait exceeds `job.timeout_ms`
- [x] **Persona rewrite resource gate** (`lib.rs`): concurrent chat requests share the same local model lock
- [x] **Async health check** (`LocalLlmWorker::health_check_http`): startup checks `{base_url}/health`; `ConfigSnapshot.local_llm.available` reflects the result
- [x] **Settings UI**: auto-load toggle, backend selector, models dir, model selector, load/unload command overrides

### Phase 5 — Multimodal / ComfyUI Image Generation
- [x] **ComfyUiWorker** (`workers/comfyui.rs`): submits `/prompt`, polls `/history/{prompt_id}`, downloads image outputs from `/view`
- [x] **Workflow utilities** (`multimodal/mod.rs`): loads API workflows and converts ComfyUI UI workflow JSON into API prompt JSON
- [x] **Test workflow**: `assets/workflows/sdxl.json`
- [x] **Image generation command**: `generate_test_image` creates an `ImageGeneration` Job and routes it through the Scheduler
- [x] **Screenshot metadata command**: `get_screenshot_metadata`, conservative default with no background capture
- [x] **ComfyUI settings UI**: root dir, Python executable, env type, on-demand start, workflow path, output dir, launch command
- [x] **Image Test UI**: explicit test modal for prompt submission and artifact preview
- [x] **In-chat image generation**: `\image` / `/image`, image button, and conservative remote-model image intent routing
- [x] **Kimi vision recognition**: `recognize_image` command, `VisionApiWorker`, chat upload button, screenshot-ready backend helper

### Phase 6 — Initiative System
- [x] **InitiativeRuntime** (`initiative.rs`): tracks user activity, cooldown, recent decisions
- [x] **Policy evaluation**: enabled gate, level-based idle threshold, cooldown, score threshold
- [x] **Explainable reasons**: `initiative_disabled`, `non_companion_trigger`, `user_recently_active`, `cooldown_active`, `score_below_threshold`
- [x] **Commands**: `record_user_activity`, `evaluate_initiative`, `request_initiative_message`
- [x] **Settings UI**: enable, level, cooldown
- [x] **Frontend checks**: manual sparkles button in window mode; automatic proactive speech reserved for future companion mode

### Phase 7 — Roles and Memory Core
- [x] **Role schema** (`personality/default.json`, `personality/mod.rs`): role id, name, aliases, identity, species, appearance, personality, language habits, scenario, tone, pinned
- [x] **Role manager UI** (`frontend/src/main.ts`): create, edit, generate, save, select, pin, delete with exact confirmation
- [x] **Role generation command**: uses the configured remote chat worker to complete missing role profile fields from identity/species/personality
- [x] **Active role config**: `[personality] default_profile` in `config/user.toml`
- [x] **Role-specific memory** (`memory.rs`): stores memory under `usr/memory/{role_id}/memories.json`
- [x] **Manual memory UI**: create, edit, pin, archive, delete memories for the active role
- [x] **Prompt context injection**: chat and companion initiative load active-role memories before prompt assembly
- [x] **Base prompt rules**: global formatting and action rules live in `PromptAssembler`, not in role personality files

---

## 2. Complete Tauri Command Reference

All commands are registered in `src/lib.rs` invoke_handler. The frontend calls them via `invoke()` from `@tauri-apps/api/core`.

### Query Commands

| Command | Args | Returns | Notes |
|---|---|---|---|
| `get_app_info` | none | `string` (JSON: name, version, phase) | Synchronous |
| `get_config_snapshot` | none | `string` (JSON: ConfigSnapshot) | See §3 for shape |
| `list_personas` | none | `Vec<String>` | Legacy persona profile list, reads available role/persona profile ids |
| `list_roles` | none | `string` (JSON: Vec<RoleProfile>) | Reads bundled and user role files |
| `list_memories` | `query?: string, includeArchived?: boolean` | `string` (JSON: Vec<MemoryItem>) | Reads active-role memory store |
| `list_available_models` | none | `string` (JSON: Vec<ModelInfo>) | Scans configured local models directory |
| `get_screenshot_metadata` | none | `string` (JSON) | Returns screenshot settings and capture availability |
| `read_image_artifact` | `path: string` | `string` data URL | Previews generated image artifacts under configured output dir |
| `get_persona_content` | `profile: string` | `string` (raw JSON) | Reads user role JSON if present, otherwise bundled `personality/{profile}.json` |

### Mutate Commands

| Command | Args | Returns | Notes |
|---|---|---|---|
| `update_settings` | `updates: object` | `string` "ok" | Writes `config/user.toml`. Recognized keys include theme, remote API, local LLM backend/model/auto-load/commands, and persona rewrite toggles |
| `save_persona_content` | `profile: string, content: string` | `string` "ok" | Validates JSON against PersonaConfig schema before writing `usr/roles/{profile}.json` |
| `role_storage_paths` | `profile: string` | `string` (JSON: `{role, assets, memory}`) | Displays editable role, asset, and memory paths |
| `prepare_role_avatar_content` | `profile: string, path: string, modelType: string` | `string` path | Copies image, Live2D, or future 3D avatar content into `usr/roles/{profile}/avatar/` |
| `set_active_role` | `profile: string` | `string` (JSON: `{active_role}`) | Writes `[personality] default_profile` and applies role avatar if configured |
| `delete_role` | `profile: string, confirmation: string` | `string` "ok" | Requires exact `我确认删除{profile}`; cannot delete `default` |
| `generate_role_profile` | `seed: RoleGenerationSeed` | `string` (raw JSON) | Uses configured remote chat API to generate a role JSON draft |
| `create_memory` | `kind: string, content: string, source?: string, pinned?: boolean` | `string` (JSON: MemoryItem) | Adds a manual memory for the active role |
| `update_memory` | `id: string, patch: object` | `string` (JSON: MemoryItem) | Edits, pins, archives, or restores memory |
| `delete_memory` | `id: string` | `string` "ok" | Deletes a memory |
| `send_chat_message` | `message: string, history: ChatMessage[]` | `string` (JSON: `{content, rewritten, generated_image?, images?, image_prompt?}`) | Main chat pipeline plus chat image generation. `history` is array of `{role, content}` |
| `generate_test_image` | `prompt: string, negativePrompt?: string` | `string` (JSON: `{prompt_id, images, workflow_path}`) | Explicit ComfyUI test generation |
| `recognize_image` | `path: string, prompt?: string` | `string` (JSON: `{content, model, source, image_path}`) | Kimi vision recognition for local uploads; future screenshots can reuse the internal helper |
| `record_user_activity` | none | `string` "ok" | Updates initiative runtime user activity timestamp |
| `evaluate_initiative` | `trigger?: string` | `string` (JSON: `{decision, recent_decisions}`) | Explains whether proactive speech is currently allowed |
| `request_initiative_message` | `history: ChatMessage[], trigger?: string` | `string` (JSON: `{allowed, content, decision}`) | Generates one proactive message only if policy allows |
| `set_companion_visible` | `visible: boolean` | `string` "ok" | Shows/hides the preconfigured `companion` Tauri window |
| `set_companion_dialog_visible` | `visible: boolean` | `string` "ok" | Shows/hides the preconfigured `companion_dialog` Tauri window as a frontend fallback |
| `set_companion_always_on_top` | `enabled: boolean` | `string` "ok" | Hot toggles the companion always-on-top flag |
| `show_main_window` | none | `string` "ok" | Shows, unminimizes, and focuses the main chat window |
| `open_settings_window` | none | `string` "ok" | Shows main window and emits `open-settings` |
| `restart_backend` | none | `string` "ok" | Stops Hestia-managed backend child processes |
| `submit_test_job` | none | `string` (job_id) | Debug only |

---

## 3. ConfigSnapshot Shape (Frontend)

Returned by `get_config_snapshot`. Must stay in sync with `config::config_snapshot()`.

```typescript
interface ConfigSnapshot {
  app: {
    name: string;
    environment: string;
    theme: { mode: "system" | "dark" | "light" };
    avatar: {
      enabled: boolean;
      image_path: string;
      model_type: "placeholder" | "live2d" | "digital_human";
    };
  };
  remote_api: {
    base_url: string;
    model: string;
    has_api_key: boolean;
  };
  local_llm: {
    backend: "llama_cpp" | "ollama" | "vllm";
    base_url: string;
    model: string;
    enabled: boolean;
    available: boolean;     // startup async health check result
    auto_load: boolean;
    models_dir: string;
    load_command: string;
    unload_command: string;
  };
  persona_rewrite: {
    enabled: boolean;
    temperature: number;
  };
  personality: { default_profile: string };
  runtime: { job_timeout_ms: number };
  observability: {
    job_timeline: boolean;
    prompt_logs: boolean;
    token_usage: boolean;
  };
  multimodal: {
    screenshot: {
      enabled: boolean;
      interval_ms: number;
      retention: number;
    };
    comfyui: {
      enabled: boolean;
      available: boolean;
      base_url: string;
      root_dir: string;
      python_path: string;
      env_type: "system" | "venv" | "conda";
      auto_start: boolean; // start backend service on demand for image jobs
      managed_process: boolean;
      launch_command: string;
      workflow_path: string;
      output_dir: string;
      startup_timeout_ms: number;
    };
    vision: {
      enabled: boolean;
      available: boolean;
      base_url: string;
      model: string;
      has_api_key: boolean;
      api_key_env: string;
      system_prompt: string;
      default_prompt: string;
      max_image_bytes: number;
    };
  };
}
```

---

## 4. Configuration Files

### `config/default.toml` (template, committed)
```toml
[app]                # name, environment
[app.theme]          # mode = "system"|"dark"|"light"
[app.avatar]         # enabled, image_path, model_type
[runtime]            # job_timeout_ms, concurrency limits
[remote_api]         # base_url, api_key_env, model
[local_llm]          # backend, base_url, model, enabled, auto_load, models_dir, commands
[persona_rewrite]    # enabled, temperature, max_tokens, prompt_template
[models.default_chat]
[personality]        # default_profile, active role id
[observability]      # job_timeline, prompt_logs, token_usage, vram_logs
[multimodal.screenshot]
[multimodal.comfyui]
[initiative]
```

### `config/user.toml` (per-user, gitignored)
Overrides `default.toml` via deep merge. Contains secrets (API keys). Format is the same TOML sections.

### Loading order
1. `config/default.toml` is parsed
2. If `config/user.toml` exists, its values override matching keys (deep merge)
3. The merged result is deserialized into `AppConfig`

---

## 5. Rust Module Map

| File | Lines | Responsibility |
|---|---|---|
| `lib.rs` | 300 | App composition, Tauri commands, entry point, worker initialization |
| `config.rs` | 224 | Typed config schema, TOML loading, deep merge, snapshot, update_user_config |
| `protocol.rs` | 241 | Job, JobStatus, Capability, WorkerInfo, ResourceRequirements, transition validation |
| `scheduler.rs` | 194 | JobQueue, Scheduler, dispatch loop, timeout, cancellation |
| `workers/mod.rs` | 92 | Worker trait, WorkerRegistry, WorkerError |
| `workers/mock.rs` | 86 | MockWorker for testing |
| `workers/remote_api.rs` | 208 | DeepSeek/OpenAI API worker, finish_reason logging |
| `workers/local_llm.rs` | 175 | llama.cpp/vLLM HTTP worker, reasoning_content fallback |
| `personality/mod.rs` | — | PromptAssembler, PersonaRewriter, PersonaConfig, role editor I/O |
| `observability.rs` | 27 | prompt_log, token_usage logging helpers |
| `runtime.rs` | 6 | Placeholder |

---

## 6. Frontend Structure

| File | Lines | Content |
|---|---|---|
| `src/main.ts` | — | App bootstrap, chat UI, settings panel, role manager, memory panel, companion view |
| `src/style.css` | — | Theme tokens (`:root`, `[data-theme="light"]`), layout, chat, companion styles |
| `index.html` | — | Root HTML with JS error catcher |
| `public/companion-cat-placeholder.png` | — | Anime cat companion placeholder |

### Key DOM IDs and Classes
- `#app` — root mount
- `#chat-messages` — scrollable message list
- `#chat-input`, `#send-btn` — chat input
- `#theme-select` — theme dropdown in sidebar
- `#avatar-container` — avatar mount point
- `.message.user`, `.message.assistant`, `.message.error`, `.message.loading`
- `.message.assistant[data-rewritten="true"]` — rewritten message (purple left border)
- `.settings-overlay`, `.settings-panel`

---

## 7. Tests

23 unit/integration tests in `cargo test`:

| Test | Module | What it covers |
|---|---|---|
| `test_valid_transitions` | protocol | 6 legal state transitions |
| `test_invalid_transitions` | protocol | 5 illegal transitions |
| `test_terminal_status` | protocol | 4 terminal states |
| `test_job_transition_mutation` | protocol | Full queued→running→completed chain + error on illegal transition |
| `test_deep_merge_overrides_string` | config | user.toml overrides default |
| `test_deep_merge_adds_new_table` | config | user.toml adds new section |
| `test_mock_worker_infer` | workers::mock | Mock worker returns correct JSON |
| `test_dispatch_completes` | scheduler | Job submitted → dispatched → completed |
| `test_cancel_job` | scheduler | Job submitted → cancelled |
| `test_local_resource_exclusivity` | resource | Local model jobs acquire one exclusive slot |
| `test_remote_resource_does_not_lock` | resource | Remote-style jobs do not occupy the local model slot |
| `test_history_images_extracts_outputs` | workers::comfyui | Extracts ComfyUI image outputs from history |
| `test_convert_sdxl_ui_workflow_to_api_prompt` | multimodal | Converts bundled SDXL UI workflow to API prompt |
| `test_load_config_current_files` | config | Current default/user config can be loaded |
| `test_encode_base64` | lib | Encodes image preview data URLs correctly |
| `test_explicit_image_prompt` | lib | Parses explicit `\image` and `/image` commands |
| `test_parse_image_intent_from_fenced_text` | lib | Parses strict JSON image intent from fenced model output |
| `test_image_mime_for_path` | lib | Accepts supported local image formats and rejects unsupported files |
| `test_chat_completions_url` | workers::vision_api | Builds Moonshot-compatible chat completions URL |
| `test_disabled_blocks_initiative` | initiative | Disabled initiative gate blocks proactive speech |
| `test_recent_user_activity_blocks_initiative` | initiative | Recent activity blocks proactive speech |
| `test_cooldown_blocks_initiative` | initiative | Cooldown blocks proactive speech |
| `test_non_companion_trigger_blocks_automatic_initiative` | initiative | Non-manual automatic triggers are blocked unless they come from companion mode |

Run: `cd src-tauri && cargo test`

---

## 8. How to Run

### Development
```bash
cd /home/eulcau/CXTX/hestia
./frontend/node_modules/.bin/tauri dev
```

### With local LLM for rewrite
```bash
# Terminal 1: Start llama.cpp
llama-server \
  -m ~/models/qwen3/Qwen3-8B-Q4_K_M.gguf \
  --ctx-size 4096 \
  --n-gpu-layers 999 \
  --flash-attn \
  --port 8080 \
  --reasoning off

# Terminal 2: Start Hestia
cd /home/eulcau/CXTX/hestia
./frontend/node_modules/.bin/tauri dev
```

Then in Settings UI: enable Local LLM + Enable rewrite → Save → Restart

### If Wayland/GBM errors
```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 ./frontend/node_modules/.bin/tauri dev
```

### API Key
Set via `config/user.toml`:
```toml
[remote_api]
api_key = "sk-your-deepseek-key"
```

Or via env var: `export DEEPSEEK_API_KEY=sk-...`

---

## 9. Known Issues & Gotchas

### Qwen3 Thinking Mode
- **Problem**: Qwen3-8B has thinking mode enabled by default, generating `reasoning_content` before `content`. With insufficient `max_tokens`, `content` is empty.
- **Fix applied**: `max_tokens` default raised to 2048. `LocalLlmWorker` falls back to `reasoning_content` when `content` is empty.
- **Better fix**: Start llama-server with `--reasoning off` to skip thinking entirely. This roughly doubles rewrite speed.

### Remote Role Prompting
- **Current behavior**: `PromptAssembler` injects base rules, active role fields, and relevant active-role memories into the chat prompt.
- **Design constraint**: Role JSON contains character traits only. Global rules such as punctuation policy, parenthetical action syntax, memory conflict priority, and safety/factual priority stay in `PromptAssembler`.
- **Rewrite path**: If persona rewrite is enabled, the local rewrite prompt also uses active role fields, but failure degrades to the remote response.

### Wayland GBM Buffer Errors
- **Problem**: WebKitGTK on KDE Plasma + Wayland fails to create GBM buffers.
- **Workaround**: `WEBKIT_DISABLE_COMPOSITING_MODE=1`

### Config Changes Require Restart
- **Problem**: Settings UI writes to `config/user.toml` but the running process doesn't hot-reload.
- **Workaround**: Restart the app after changing API key, base URL, model, or local LLM settings. Theme changes apply immediately without restart.

### Rewrite Pipeline Degradation
- **Problem**: If local LLM is unreachable during rewrite, `send_chat_message` returns the raw DeepSeek response.
- **Behavior**: Non-blocking. A warn-level log is emitted. The user sees the un-rewritten DeepSeek reply (no purple border).

---

## 10. Phase 8 Desktop Companion Window MVP

| Component | Status |
|---|---|
| Transparent `companion` Tauri window | Done |
| Main-window show/hide button | Done |
| Companion-only frontend view | Done |
| Placeholder anime cat avatar | Done |
| Live2D avatar adapter MVP | Done |
| Companion timer calling `request_initiative_message` with `trigger = "companion_timer"` | Done |
| Main-window automatic proactive speech disabled | Done |
| Hover toolbar: top, proactive, chat, dialogue | Done |
| Companion drag and lower-right resize handle | Done |
| Independent dialogue window with adaptive placement | Done |
| Tray icon/menu and hide-on-close behavior | Done |
| Initiative hot config reload | Done |
| Companion position/size persistence | Done |
| Companion dialogue lifecycle sync | Done |

The companion window starts hidden. The main window controls it with `set_companion_visible`. The companion restores saved bounds at startup, persists drag/resize changes to `config/user.toml`, has hover controls, and the separate `companion_dialog` window follows it using top, bottom, right, then left placement so it avoids overlapping the avatar when screen space allows. Dialogue visibility is synchronized with `companion-dialog-visible-changed`, including direct dialogue close, companion hide, and proactive message open paths. Blocked automatic initiative checks stay silent. Closing main, companion, or dialog hides the window; if all are hidden, managed backend child processes are stopped while the tray process remains alive.

## 11. Next Phases (from work-order.md)

| Phase | Description | Key New Components |
|---|---|---|
| Memory polish | Memory storage hardening | packaged user data dir, import/export, tests |
| 8 polish | Companion polish | click-through body |
| Live2D polish | Animated avatar | expression/motion tuning, visual QA, mouse tracking polish |
| 7.5 | Plugin Boundary | Plugin manifest, permission model, event contract |

Do not do yet:
- Do not implement Plugin Boundary before the companion event shape settles.
- Do not add automatic memory writes before a user confirmation workflow exists.
- Do not add VRM assets until the Live2D behavior is stable.
- Do not let the main chat window use automatic proactive triggers.
- Do not call `request_initiative_message` with `trigger = "timer"` or `window_timer` for automatic speech; backend will block it via `non_companion_trigger`.

### Quick Start for the Next Codex Window

This section is the resume point when continuing in a new conversation. Read it before
making changes, then inspect only the files relevant to the selected next task.

Read these first:
1. [AGENTS.md](/home/eulcau/CXTX/hestia/AGENTS.md)
2. [docs/HANDOFF.md](/home/eulcau/CXTX/hestia/docs/HANDOFF.md), §10 and §11
3. [docs/ui-interface-contract.md](/home/eulcau/CXTX/hestia/docs/ui-interface-contract.md), §2, §12.2, and §12.3
4. [docs/roles.md](/home/eulcau/CXTX/hestia/docs/roles.md)
5. [docs/memory.md](/home/eulcau/CXTX/hestia/docs/memory.md)
6. [docs/desktop-companion.md](/home/eulcau/CXTX/hestia/docs/desktop-companion.md), §3.3 and §4

Likely first files to inspect:
- [frontend/src/main.ts](/home/eulcau/CXTX/hestia/frontend/src/main.ts): main UI, companion view, companion dialogue view, initiative timer, bounds persistence
- [src-tauri/src/lib.rs](/home/eulcau/CXTX/hestia/src-tauri/src/lib.rs): Tauri commands, tray, close/hide lifecycle, companion visibility events
- [src-tauri/src/memory.rs](/home/eulcau/CXTX/hestia/src-tauri/src/memory.rs): manual memory storage, retrieval, prompt context formatting
- [src-tauri/src/personality/mod.rs](/home/eulcau/CXTX/hestia/src-tauri/src/personality/mod.rs): role schema, role file I/O, prompt assembly
- [src-tauri/src/config.rs](/home/eulcau/CXTX/hestia/src-tauri/src/config.rs): `ConfigSnapshot`, `update_settings`, `[companion.window]`
- [config/default.toml](/home/eulcau/CXTX/hestia/config/default.toml): default companion bounds and runtime settings
- [src-tauri/tauri.conf.json](/home/eulcau/CXTX/hestia/src-tauri/tauri.conf.json): `main`, `companion`, and `companion_dialog` window definitions

Stable companion contracts:
- Main chat does not speak proactively on a timer.
- Only the `companion` frontend calls `request_initiative_message` with `trigger = "companion_timer"`.
- `companion-visible-changed` is the source of truth for companion show/hide state.
- `companion-dialog-visible-changed` is the source of truth for Bubble button state and dialogue request cleanup.
- `companion-avatar-event` carries avatar renderer events: `expression`, `motion`, `speak_start`, `speak_stop`, `look_at`, and `idle`.
- Companion position and size are restored from `[companion.window]` and persisted through `update_settings`.
- User-managed memory is stored under `usr/memory/{role_id}/memories.json` in development and injected as bounded prompt context. Archived memories are excluded; pinned memories are preferred.
- User-created roles are stored under `usr/roles/{id}.json` in development, with copied role avatar assets under `usr/roles/{id}/avatar/`. The bundled `default` role remains in `personality/default.json`.

Recommended next task:
- Harden role and memory storage for packaged builds by moving `usr/roles` and `usr/memory` to the system user data directory, then add role/memory storage tests. Do not add Plugin Boundary work until memory state ownership is stable.

Validation commands:
```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
./frontend/node_modules/.bin/tauri build --debug --no-bundle
```

---

## 11. Key Design Decisions

1. **Models do not call models.** All work goes through the Scheduler as Jobs.
2. **Personality is application state, not model state.** Persona JSON lives on disk, assembled into prompts by the application layer.
3. **Rewrite pipeline**: DeepSeek (cognition) → Qwen3-8B (style). Two-pass architecture with non-blocking degradation.
4. **Config deep merge**: `default.toml` (committed, template) + `user.toml` (gitignored, secrets).
5. **Coarse GPU exclusivity** (implemented Phase 4): one large local model at a time. Conservative but stable for long-running desktop apps.

---

## 12. Documentation Index

| Document | Purpose |
|---|---|
| [docs/HANDOFF.md](/home/eulcau/CXTX/hestia/docs/HANDOFF.md) | This document |
| [docs/implementation-summary.md](/home/eulcau/CXTX/hestia/docs/implementation-summary.md) | Phase-by-phase implementation log |
| [docs/ui-interface-contract.md](/home/eulcau/CXTX/hestia/docs/ui-interface-contract.md) | Frontend-backend API contract |
| [docs/work-order.md](/home/eulcau/CXTX/hestia/docs/work-order.md) | Planned phases |
| [docs/technical-route.md](/home/eulcau/CXTX/hestia/docs/technical-route.md) | Architecture decisions |
| [docs/architecture.md](/home/eulcau/CXTX/hestia/docs/architecture.md) | Job state machine, worker contract |
| [docs/knowledge-map.md](/home/eulcau/CXTX/hestia/docs/knowledge-map.md) | Required knowledge areas |
| [docs/project-structure.md](/home/eulcau/CXTX/hestia/docs/project-structure.md) | Directory layout |
| [docs/ide.md](/home/eulcau/CXTX/hestia/docs/ide.md) | JetBrains IDE setup |
| [docs/desktop-companion.md](/home/eulcau/CXTX/hestia/docs/desktop-companion.md) | Desktop companion avatar rendering design (Live2D, Spine, Unity+VRM) |
| [docs/roles.md](/home/eulcau/CXTX/hestia/docs/roles.md) | Role profile schema, storage, generation, deletion, and active-role memory behavior |
| [docs/memory.md](/home/eulcau/CXTX/hestia/docs/memory.md) | Long-term memory storage, retrieval, and prompt injection |
| [AGENTS.md](/home/eulcau/CXTX/hestia/AGENTS.md) | Codex project rules |
