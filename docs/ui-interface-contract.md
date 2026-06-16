# UI Interface Contract

**Last updated:** 2026-06-16 (Phase 8 Live2D avatar adapter MVP)
**Purpose:** Defines every backend command, every config key, and every async contract that the frontend depends on. When backend changes are made, this document must be updated.

---

## 1. Architecture Boundary

```
┌──────────────────────────────────────────────────────┐
│  Frontend (TypeScript / Vite / Tauri API)            │
│                                                       │
│  invoke("command", {args})  ──────────────────────►  │
│  ◄──────────────────────────  Promise<Result>        │
│                                                       │
├──────────────────────────────────────────────────────┤
│  Rust Backend (Tauri commands / AppState)             │
│  commands: lib.rs #[tauri::command]                  │
│  state: AppState { config, workers, scheduler_tx,     │
│                    resources, initiative runtime,     │
│                    local/comfy health }               │
└──────────────────────────────────────────────────────┘
```

The frontend communicates with the backend exclusively through `invoke()` calls. There is no direct filesystem access, no direct HTTP, no WebSocket. Every interaction is a Tauri IPC command.

---

## 2. Tauri Commands (API Surface)

### 2.1 Query Commands (non-mutating reads)

#### `get_app_info`
```
Arguments: none
Returns:  string (JSON)
  {
    "name": "Hestia",
    "version": "0.1.0",
    "phase": "8"
  }
Errors:   never (synchronous, no I/O)
```

#### `get_config_snapshot`
```
Arguments: none
Returns:  string (JSON) — ConfigSnapshot (see §4)
Errors:   never
Usage:    Called on app start to populate UI state.
          Called again after closing Settings to refresh.
```

#### `list_personas`
```
Arguments: none
Returns:  Vec<String> — e.g. ["default"]
Errors:   never
Usage:    Used by persona-related UI flows and reserved for future selector expansion.
```

#### `list_available_models`
```

#### `get_screenshot_metadata`
```

#### `read_image_artifact`
```
Arguments: { path: string }
Returns:  string — data URL for frontend preview
Errors:   string if the path is outside configured output_dir or unreadable
Usage:    Used by the Image Test UI to preview generated artifacts without exposing arbitrary filesystem reads.
```
Arguments: none
Returns:  string (JSON)
  {
    "enabled": boolean,
    "retention": number,
    "capture_available": false,
    "reason": string
  }
Errors:   never
Usage:    Exposes the conservative screenshot state. No automatic capture is active.
```
Arguments: none
Returns:  string (JSON) — Vec<ModelInfo>
  ModelInfo: {
    manufacturer: string,   // subdirectory name, e.g. "qwen"
    model_name: string,     // file name without .gguf, e.g. "Qwen3-8B-Q4_K_M"
    file_path: string,      // full absolute path
    size_bytes: number|null // file size in bytes
  }
Errors:   never
Usage:    Scans the configured models_dir (default ~/models) for .gguf files.
          Directory structure: $models_dir/$manufacturer/$modelName.gguf
```

### 2.2 Mutate Commands (writes / side effects)

#### `update_settings`
```
Arguments: { updates: Record<string, string|boolean> }
  Recognized keys:
    "theme_mode"              → writes [app.theme] mode = value
    "api_key"                 → writes [remote_api] api_key = value
    "base_url"                → writes [remote_api] base_url = value
    "model"                   → writes [remote_api] model = value
    "local_llm_enabled"       → writes [local_llm] enabled = value (bool)
    "local_llm_base_url"      → writes [local_llm] base_url = value (string)
    "local_llm_model"         → writes [local_llm] model = value (string)
    "local_llm_auto_load"     → writes [local_llm] auto_load = value (bool)
    "local_llm_models_dir"    → writes [local_llm] models_dir = value (string)
    "local_llm_load_command"  → writes [local_llm] load_command = value (string)
    "local_llm_unload_command"→ writes [local_llm] unload_command = value (string)
    "persona_rewrite_enabled" → writes [persona_rewrite] enabled = value (bool)
    "comfyui_enabled"         → writes [multimodal.comfyui] enabled = value (bool)
    "comfyui_base_url"        → writes [multimodal.comfyui] base_url = value (string)
    "comfyui_root_dir"        → writes [multimodal.comfyui] root_dir = value (string)
    "comfyui_python_path"     → writes [multimodal.comfyui] python_path = value (string)
    "comfyui_env_type"        → writes [multimodal.comfyui] env_type = value (string)
    "comfyui_auto_start"      → writes [multimodal.comfyui] auto_start = value (bool, on-demand start)
    "comfyui_launch_command"  → writes [multimodal.comfyui] launch_command = value (string)
    "comfyui_workflow_path"   → writes [multimodal.comfyui] workflow_path = value (string)
    "comfyui_output_dir"      → writes [multimodal.comfyui] output_dir = value (string)
    "vision_enabled"          → writes [multimodal.vision] enabled = value (bool)
    "vision_base_url"         → writes [multimodal.vision] base_url = value (string)
    "vision_model"            → writes [multimodal.vision] model = value (string)
    "vision_api_key"          → writes [multimodal.vision] api_key = value (string)
    "vision_system_prompt"    → writes [multimodal.vision] system_prompt = value (string)
    "vision_default_prompt"   → writes [multimodal.vision] default_prompt = value (string)
    "vision_max_image_bytes"  → writes [multimodal.vision] max_image_bytes = value (number)
    "initiative_enabled"      → writes [initiative] enabled = value (bool)
    "initiative_level"        → writes [initiative] level = value (number)
    "initiative_cooldown_ms"  → writes [initiative] cooldown_ms = value (number)
    "companion_window_x"      → writes [companion.window] x = value (number, physical px)
    "companion_window_y"      → writes [companion.window] y = value (number, physical px)
    "companion_window_width"  → writes [companion.window] width = value (number, logical px)
    "companion_window_height" → writes [companion.window] height = value (number, logical px)
Returns:  string "ok"
Errors:   string with error message
Side effect: Writes to config/user.toml. Does NOT hot-reload the backend.
             API key / base URL / model / local LLM changes require app restart.
             Theme mode changes take effect immediately (applied in frontend).
```

#### `send_chat_message`
```
Arguments: { message: string, history: { role: string, content: string }[] }
Returns:  string (JSON)
  {
    "content": string,
    "rewritten": boolean,
    "generated_image"?: boolean,
    "image_prompt"?: string,
    "negative_prompt"?: string|null,
    "images"?: string[],
    "prompt_id"?: string|null,
    "workflow_path"?: string|null
  }
Errors:   string — e.g. "inference failed: ..." or "failed to load persona: ..."
Async:    YES. This is the primary long-running operation.
          Timeline:
            1. If message starts with "\image " or "/image ", run image generation directly
            2. Otherwise ask the remote chat worker for strict JSON image-intent routing
            3. If should_generate=true, run ImageGeneration through Scheduler
            4. If should_generate=false or routing fails, PromptAssembler loads persona JSON
            5. RemoteApiWorker.infer() → HTTP POST to DeepSeek
            6. [if rewrite enabled] checks local_llm_available
            7. [if rewrite enabled] PersonaRewriter builds prompt
            8. [if rewrite enabled] acquires ResourceManager local model slot
            9. [if rewrite enabled] LocalLlmWorker.infer() → HTTP POST to llama.cpp
            10. [if rewrite enabled] releases ResourceManager local model slot
            11. Returns final JSON string
UI State:
  BEFORE:  disable textarea and send/image buttons, show "Thinking..." or "Generating image..."
  AFTER:   remove placeholder, render response and generated image previews, re-enable input
  ON ERR:  remove placeholder, render error message, re-enable input
```

#### `submit_test_job`
```

#### `generate_test_image`
```
Arguments: { prompt: string, negativePrompt?: string }
Returns:  string (JSON)
  {
    "prompt_id": string,
    "images": string[],       // absolute local artifact paths
    "workflow_path": string
  }
Errors:   string — e.g. ComfyUI unavailable, workflow parse error, prompt failure
Async:    YES. Explicit image generation test path.
          Timeline:
            1. Re-check ComfyUI /system_stats
            2. If unavailable and auto_start=true, launch configured ComfyUI process on demand
            3. Create Job(capability = ImageGeneration)
            4. Scheduler acquires local GPU resource slot
            5. ComfyUiWorker loads workflow and builds API prompt
            6. POST /prompt
            7. Poll /history/{prompt_id}
            8. Download /view image outputs
            9. Save images under configured output_dir
            10. If Hestia started ComfyUI for this job, stop that managed process
UI State:
  BEFORE:  disable Generate button, show submitted status
  AFTER:   show prompt_id and generated images
  ON ERR:  show error status, re-enable Generate button
```
Arguments: none
Returns:  string — job_id
Errors:   string
Purpose:  Debug only. Submits a dummy chat job to the scheduler.
```

#### `recognize_image`
```
Arguments: { path: string, prompt?: string|null }
Returns:  string (JSON)
  {
    "content": string,
    "usage"?: object|null,
    "model": string,
    "source": "upload" | "screenshot" | string,
    "image_path": string
  }
Errors:   string — e.g. vision disabled, unsupported image type, image too large, no Vision worker
Async:    YES. Image recognition through Kimi / Moonshot-compatible API.
          Timeline:
            1. Validate local image extension and configured max_image_bytes
            2. Read the local image and build a base64 data URL
            3. Create Job(capability = Vision)
            4. Scheduler routes to VisionApiWorker
            5. POST OpenAI-compatible /v1/chat/completions with message.content as an array
            6. Return text content and metadata
Future screenshot use: screenshot capture should call the same internal backend helper with source = "screenshot" and a screenshot image path.
UI State:
  BEFORE:  disable textarea and send/image/vision buttons, show "Recognizing image..."
  AFTER:   remove placeholder, render response, re-enable input
  ON ERR:  remove placeholder, render error message, re-enable input
```

#### `record_user_activity`
```
Arguments: none
Returns:  string "ok"
Errors:   string if initiative runtime lock fails
Usage:    Frontend calls this on throttled user activity events. `send_chat_message` and `recognize_image` also record activity internally.
```

#### `evaluate_initiative`
```
Arguments: { trigger?: string }
Returns:  string (JSON)
  {
    "decision": {
      "enabled": boolean,
      "allowed": boolean,
      "level": number,
      "score": number,
      "idle_ms": number,
      "min_idle_ms": number,
      "cooldown_remaining_ms": number,
      "reasons": string[],
      "suggested_prompt": string
    },
    "recent_decisions": Decision[]
  }
Errors:   string if initiative runtime lock fails
Usage:    Debug/explainability path. Does not generate text.
```

#### `request_initiative_message`
```
Arguments: { history: { role: string, content: string }[], trigger?: string }
Returns:  string (JSON)
  {
    "allowed": boolean,
    "content": string|null,
    "decision": Decision
  }
Errors:   string if persona load or model inference fails
Async:    YES. Evaluates policy first. If disallowed, no model call is made.
          If allowed, it asks the configured remote chat worker for one short proactive message,
          then records `last_initiative_at` for cooldown enforcement.
UI State:
  Manual: show "Checking initiative..." and render either the proactive message or the reasons it was blocked.
  Timer:  silent when blocked; render a message only when policy allows.
```

#### `set_companion_visible`
```
Arguments: { visible: boolean }
Returns:  string "ok"
Errors:   string if the companion window is unavailable or cannot be shown/hidden
Side effect:
  - visible = true: shows the Tauri window labeled `companion` and pins it always-on-top
  - visible = false: hides the Tauri window labeled `companion` and its dialogue window
Usage:
  Called from the main window sidebar show/hide control.
  Does not create or destroy the companion window; it controls visibility for the preconfigured second window.
```

#### `set_companion_dialog_visible`
```
Arguments: { visible: boolean }
Returns:  string "ok"
Errors:   string if the companion dialogue window is unavailable or cannot be shown/hidden
Side effect:
  - visible = true: shows the Tauri window labeled `companion_dialog` and pins it always-on-top
  - visible = false: hides the Tauri window labeled `companion_dialog`
  Emits `companion-dialog-visible-changed` to `companion` and `companion_dialog`.
Usage:
  Companion dialogue bubble show/hide fallback when frontend window APIs are unavailable or denied.
```

#### `show_main_window`
```
Arguments: none
Returns:  string "ok"
Errors:   string if the main window is unavailable or cannot be shown
Side effect: Shows, unminimizes, and focuses the Tauri window labeled `main`.
Usage:    Called by the companion "Chat" control and tray left-click/menu actions.
```

#### `open_settings_window`
```
Arguments: none
Returns:  string "ok"
Errors:   string if the main window cannot be shown or the settings event cannot be emitted
Side effect: Shows the main window and emits `open-settings` to the frontend.
Usage:    Tray menu action for opening Settings directly.
```

#### `set_companion_always_on_top`
```
Arguments: { enabled: boolean }
Returns:  string "ok"
Errors:   string if the companion window is unavailable or the window flag cannot be updated
Side effect: Updates the always-on-top state of the `companion` window immediately.
Usage:    Companion hover toolbar pin toggle.
```

#### `restart_backend`
```
Arguments: none
Returns:  string "ok"
Errors:   currently none
Side effect: Stops Hestia-managed local backend processes (`local_llm`, `comfyui`).
Usage:    Main chat header restart button and tray menu. This does not restart the Tauri process itself.
```

---

## 3. UI State Machine

### 3.1 Chat Input State

| State | input[disabled] | sendBtn[disabled] | loading indicator |
|---|---|---|---|
| Idle | false | false | hidden |
| Sending | true | true | element with class "loading" appended to messages |
| Done (success) | false | false | removed; new "assistant" message appended |
| Done (error) | false | false | removed; "error" message appended |

### 3.2 Settings Panel State

Settings is a modal overlay (`<div class="settings-overlay">`). It is created and destroyed on each open/close.

| State | Behavior |
|---|---|
| Open | `buildSettingsPanel(cfg, onClose)` → `document.body.append(panel)` |
| Save | `invoke("update_settings", {updates})` → status bar OK/Error |
| Close | `overlay.remove()` → `onClose()` refreshes cfg via `get_config_snapshot` |

### 3.3 Theme State

| Internal value | CSS attribute |
|---|---|
| `"dark"` | `[data-theme="dark"]` |
| `"light"` | `[data-theme="light"]` |
| `"system"` | follows `prefers-color-scheme` media query |

The `applyTheme(mode)` function is called:
- On app startup
- When sidebar `<select>` changes
- When system theme changes (if mode is "system")

### 3.4 Companion Window State

The app has three Tauri windows:

| Label | Initial visibility | Purpose |
|---|---|---|
| `main` | visible | Chat, settings, manual controls |
| `companion` | hidden | Transparent always-on-top desktop companion surface |
| `companion_dialog` | hidden | Independent transparent dialogue bubble following the companion |

The main window controls companion visibility through:

```typescript
invoke("set_companion_visible", { visible: boolean })
```

`set_companion_visible` emits `companion-visible-changed` to `main`, `companion`, and `companion_dialog`.
The main window must update its local show/hide button state from this event, because
the companion window and tray menu may also change companion visibility. The companion
window must stop proactive checks and local dialogue-visible state while the event
payload is `false`. The dialogue window uses the same event to reset any in-flight local
conversation request state when the companion is hidden.

`set_companion_dialog_visible` and close/hide paths emit `companion-dialog-visible-changed`
to `companion` and `companion_dialog`. The companion window must treat this event as the
source of truth for the Bubble button active state, because the dialogue window can be
hidden by its own close event as well as by companion controls.

The companion dialogue can also be shown/hidden from the backend when frontend window API calls fail:

```typescript
invoke("set_companion_dialog_visible", { visible: boolean })
```

The companion window is loaded with `/?view=companion`. Its frontend path renders only:
- the configured avatar through `AvatarAdapter.mount(...)`
- a hover-only toolbar with always-on-top, proactive speech, open chat, dialogue, and close controls
- a custom lower-right resize handle

The dialogue bubble is loaded in a separate window with `/?view=companion_dialog`. It owns its local message history and calls `send_chat_message` for typed companion dialogue.
If the bubble or companion is hidden while a typed dialogue request is in flight, the frontend
invalidates that request generation and ignores its eventual result.

Only the companion window owns automatic initiative checks:

```typescript
invoke("request_initiative_message", {
  history: [],
  trigger: "companion_timer",
})
```

Blocked timer decisions remain silent in the companion UI. The main window must not call `request_initiative_message` with timer-like automatic triggers.

Companion avatar events:

The companion window owns the active avatar adapter and listens for local frontend events named `companion-avatar-event`. The event payload is:

```typescript
type CompanionAvatarEventType =
  | "expression"
  | "motion"
  | "speak_start"
  | "speak_stop"
  | "look_at"
  | "idle";

interface CompanionAvatarEventPayload {
  name?: string;
  weight?: number;
  duration_ms?: number;
  transition_ms?: number;
  x?: number; // normalized look-at coordinate in [-1, 1]
  y?: number; // normalized look-at coordinate in [-1, 1]
}

interface CompanionAvatarEvent {
  type: CompanionAvatarEventType;
  data?: CompanionAvatarEventPayload;
}
```

Current event sources:
- Companion proactive timer request start emits `expression` with `name = "thinking"`.
- Proactive or dialogue assistant text emits `speak_start` with an approximate `duration_ms`, followed by `speak_stop` from the companion window timer.
- Companion pointer movement emits `look_at` with normalized coordinates.
- Companion hide, dialogue hide, blocked initiative, or model error emits `idle` or an error expression.

The placeholder adapter maps these events to CSS state changes. The Live2D adapter consumes the same contract through Cubism expressions, motion groups, and focus coordinates. Avatar adapters must not call backend models directly.

Live2D control interface:

The public control surface is the frontend event `companion-avatar-event`. Any frontend window may emit this event to the `companion` window:

```typescript
import { getCurrentWindow } from "@tauri-apps/api/window";

getCurrentWindow().emitTo("companion", "companion-avatar-event", {
  type: "motion",
  data: { name: "Tap" },
});
```

Supported event forms:

```typescript
// Set expression. The adapter accepts mapped names and raw Cubism expression names.
{ type: "expression", data: { name: "thinking" } }
{ type: "expression", data: { name: "Surprised" } }

// Play a motion group. The adapter accepts mapped names and raw Cubism motion groups.
{ type: "motion", data: { name: "tap" } }
{ type: "motion", data: { name: "FlickDown" } }

// Start/stop speaking animation.
{ type: "speak_start", data: { duration_ms: 3000 } }
{ type: "speak_stop" }

// Move gaze/head focus. Coordinates are normalized to [-1, 1].
{ type: "look_at", data: { x: 0.4, y: -0.2 } }

// Return to idle.
{ type: "idle" }
```

Current Live2D mappings in `frontend/src/main.ts`:

| Semantic name | Live2D expression |
|---|---|
| `thinking` | `f01` |
| `normal`, `idle`, `speaking` | `Normal` |
| `happy`, `blushing` | `Blushing` |
| `sad` | `Sad` |
| `angry` | `Angry` |
| `surprised`, `confused`, `error`, `excited` | `Surprised` |

| Semantic name | Live2D motion group |
|---|---|
| `idle` | `Idle` |
| `speaking`, `tap` | `Tap` |
| `thinking` | `Flick` |

Raw model motion group names may also be used directly. Available group names depend on the user-provided `.model3.json`.

Companion dialogue bubble placement:
- Default placement is above the avatar.
- If there is not enough room above, placement switches below.
- If vertical placement would hit screen bounds, placement switches to the right or left.
- The dialog window is positioned outside the companion window bounds whenever screen space allows, so it does not overlap the avatar image.
- The companion window restores `[companion.window]` bounds on startup.
- Drag/resize changes are debounced and persisted through `update_settings`.
- `x` and `y` are saved in physical pixels from Tauri window position APIs; `width` and `height` are saved in logical pixels and clamped to frontend min/max companion size.

Window close behavior:
- Closing `main` or `companion` hides that window instead of destroying it.
- Closing `companion` also hides `companion_dialog`.
- If all frontend windows are hidden, Hestia stops currently managed backend processes but keeps the tray/Tauri process alive.
- Tray left-click opens `main`.
- Tray right-click menu exposes Open Chat, Open Settings, Open Companion, Restart Backend, and Quit.

Initiative hot update:
- `evaluate_initiative` and `request_initiative_message` reload current config before policy evaluation.
- Changes to `initiative_enabled`, including the companion toolbar Talk toggle, take effect immediately.

Wayland note:
- The companion uses Tauri `alwaysOnTop` and `visibleOnAllWorkspaces`, reapplies them on focus loss, and the frontend periodically reasserts the flag while the companion Top toggle is enabled.
- Some Wayland compositors may still restrict true global always-on-top behavior; compositor-specific approaches should be considered separately.

---

## 4. ConfigSnapshot Type (Frontend)

This is the JSON shape returned by `get_config_snapshot`. It must stay in sync with `config::config_snapshot()` in `src/config.rs`.

```typescript
interface ConfigSnapshot {
  app: {
    name: string;
    environment: string;
    theme: { mode: "system" | "dark" | "light" };
    avatar: {
      enabled: boolean;
      image_path: string;    // relative to frontend public/, e.g. a placeholder image or user-provided .model3.json
      model_type: "placeholder" | "live2d" | "digital_human";
    };
  };
  companion: {
    window: {
      x: number | null;
      y: number | null;
      width: number;
      height: number;
    };
  };
  remote_api: {
    base_url: string;
    model: string;
    has_api_key: boolean;    // derived: true if api_key set in user.toml OR env var
  };
  local_llm: {
    backend: string;
    base_url: string;
    model: string;
    enabled: boolean;
    available: boolean;    // startup async health check result from {base_url}/health
    auto_load: boolean;
    models_dir: string;
    load_command: string;
    unload_command: string;
  };
  persona_rewrite: {
    enabled: boolean;
    temperature: number;
  };
  personality: {
    default_profile: string;
  };
  initiative: {
    enabled: boolean;
    level: number;
    cooldown_ms: number;
  };
  runtime: {
    job_timeout_ms: number;
  };
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
      available: boolean;       // startup health, command can re-check before generation
      managed_process: boolean; // true if Hestia started the backend process
      base_url: string;         // default http://127.0.0.1:8188
      root_dir: string;         // Linux placeholder ~/models/ComfyUI
      python_path: string;      // conda/venv Python executable path
      env_type: "system" | "venv" | "conda";
      auto_start: boolean;      // start backend service on demand for image jobs
      launch_command: string;   // placeholders: {python}, {root_dir}, {host}, {port}
      workflow_path: string;    // default assets/workflows/sdxl.json
      output_dir: string;       // default data/artifacts/images
      startup_timeout_ms: number;
    };
    vision: {
      enabled: boolean;
      available: boolean;
      base_url: string;         // default https://api.moonshot.ai
      model: string;            // default kimi-k2.6
      has_api_key: boolean;
      api_key_env: string;      // default MOONSHOT_API_KEY
      system_prompt: string;
      default_prompt: string;
      max_image_bytes: number;
    };
  };
}
```

---

## 5. DOM Element IDs and Classes

These are the stable selectors that the frontend creates. CSS, event handlers, and future modifications should reference these.

### 5.1 Element IDs

| ID | Element | Purpose |
|---|---|---|
| `app` | `<div>` | Root mount point (in index.html) |
| `chat-messages` | `<div>` | Scrollable message container |
| `chat-input` | `<textarea>` | Message text input |
| `send-btn` | `<button>` | Send button |
| `theme-select` | `<select>` | Theme dropdown in sidebar |
| `avatar-container` | `<div>` | Avatar mount point (future adapters mount here) |

### 5.2 CSS Classes (semantic)

| Class | Where | Meaning |
|---|---|---|
| `app-layout` | Root flex container | sidebar + main |
| `sidebar` | Left column | 220px fixed width |
| `main-content` | Right column | flex-1, chat area |
| `chat-container` | Flex column | header + messages + input |
| `chat-header` | Top bar | Title + clear button |
| `chat-messages` | Scrollable area | Message list |
| `chat-input-area` | Bottom bar | Input + send button |
| `message` | Each message bubble | Base class |
| `message.user` | User message | Right-aligned, accent bg |
| `message.assistant` | AI response | Left-aligned, muted bg |
| `message.error` | Error message | Red bg |
| `message.loading` | "Thinking..." | Italic, muted |
| `settings-overlay` | Modal backdrop | Fixed fullscreen, dimmer |
| `settings-panel` | Modal content | 480px card |
| `settings-section` | Settings group | Bordered section |
| `settings-row` | Label + input pair | Flex row |
| `sidebar-btn` | Sidebar action button | Icon + text |
| `btn-primary` | Save button | Accent color |

### 5.3 CSS Variables (Theme Tokens)

All colors are controlled by CSS custom properties on `:root` and `[data-theme="light"]`. See [frontend/src/style.css](/home/eulcau/CXTX/hestia/frontend/src/style.css) for the full list.

Key tokens:

| Variable | Dark default | Light default | Usage |
|---|---|---|---|
| `--bg-primary` | `#1a1a2e` | `#f5f3ff` | Page background |
| `--bg-sidebar` | `#151530` | `#ede9fe` | Sidebar background |
| `--bg-message-user` | `#3b3b6a` | `#ddd6fe` | User bubble |
| `--bg-message-assistant` | `#252547` | `#f0edff` | Assistant bubble |
| `--accent` | `#7c6ff7` | `#7c6ff7` | Buttons, focus rings |
| `--text-primary` | `#e0e0e0` | `#1e1b4b` | Body text |
| `--border` | `#2a2a4a` | `#d4c8f0` | Separators |

---

## 6. Avatar Adapter Interface

The avatar slot supports both the placeholder `<img>` adapter and the Live2D canvas adapter. The interface is:

```typescript
interface AvatarAdapter {
  type: "placeholder" | "live2d" | "digital_human";
  mount(container: HTMLElement): void;
  unmount(): void;
  onEvent?(event: string, data: unknown): void;
}
```

- `mount(container)` — receives the avatar mount element. The adapter is responsible for creating and appending its own DOM subtree (canvas, video, img, etc.).
- `unmount()` — cleans up event listeners, animation loops, WebGL contexts.
- `onEvent` — maps companion avatar events to renderer behavior. The Live2D adapter maps expression, motion, speaking, idle, and look-at events to Cubism expressions, motion groups, and focus coordinates.

When adding a new model type:
1. Add a case in `createAvatarAdapter()`
2. Update `ConfigSnapshot.app.avatar.model_type` union type
3. Ensure the adapter's lifecycle (mount/unmount) is clean on theme changes or settings reloads

---

## 7. Rules for Modifying This Document

When any of the following changes occur in the Rust backend, this document must be updated:

- A new `#[tauri::command]` is added, removed, or its signature changes
- `config_snapshot()` in `src/config.rs` changes its output shape
- A new config section is added to `default.toml`
- `AppState` struct changes (new fields that the frontend may reference)
- Worker capabilities or types change

The update should:
1. Add/update the command in §2
2. If the ConfigSnapshot shape changed, update §4
3. If new UI state transitions were introduced, update §3
4. Bump the "Last updated" date at the top of this document.

This rule is enforced by the project's AGENTS.md (§Maintenance Rules).

---

## 8. Phase 3 Extension: vLLM Backend (2026-06-07)

### 8.1 Configuration

```toml
[local_llm]
backend = "llama_cpp"    # "llama_cpp" | "vllm"
base_url = "http://127.0.0.1:8080"
model = "qwen2.5-7b"
enabled = false
```

| Backend | Default Port | Notes |
|---|---|---|
| `llama_cpp` | 8080 | llama.cpp HTTP server |
| `vllm` | 8000 | vLLM OpenAI-compatible server |

### 8.2 Worker Behavior

Both backends use OpenAI-compatible `/v1/chat/completions`. The `LocalLlmWorker` implementation is identical regardless of backend. Switching backends only changes the `base_url` and `model` parameters sent to the server.

### 8.3 UI Control

The Settings panel "Persona Rewrite" section now includes a **Backend** dropdown with options:
- `llama.cpp (localhost:8080)`
- `vLLM (localhost:8000)`

Selecting a backend and saving writes `local_llm.backend` to `config/user.toml`. The `base_url` is not automatically changed — the user configures both independently.

### 8.4 update_settings Key

| Key | Type | Config Path |
|---|---|---|
| `local_llm_backend` | `string` | `[local_llm] backend` |
| `local_llm_base_url` | `string` | `[local_llm] base_url` |
| `local_llm_enabled` | `boolean` | `[local_llm] enabled` |
| `persona_rewrite_enabled` | `boolean` | `[persona_rewrite] enabled` |

---

## 9. Per-Session Context (2026-06-07)

### 9.1 `send_chat_message` now accepts history

```
Arguments: { message: string, history: { role: string, content: string }[] }
```

The frontend maintains a `chatHistory` array. Each successful exchange appends `{role:"user", content}` and `{role:"assistant", content}`. The clear button empties both the DOM and the array.

### 9.2 New Commands: Persona Editor

#### `get_persona_content`
```
Arguments: { profile: string }  — e.g. "default"
Returns:  string — raw JSON content of personality/{profile}.json
Errors:   string if file not found or unreadable
```

#### `save_persona_content`
```
Arguments: { profile: string, content: string }
Returns:  string "ok"
Errors:   string if JSON invalid or write fails
Side effect: Overwrites personality/{profile}.json. Requires restart or page reload to apply to active prompt assembler.
```

### 9.3 Persona Editor UI

Sidebar button "Persona" (pencil icon) opens a modal with:
- Textarea pre-loaded with current persona JSON
- "Load" button — re-reads from disk
- "Save" button — validates JSON schema, writes to disk
- Changes take effect on next `send_chat_message` call (PromptAssembler loads fresh on each message)

### 9.4 Updated ConfigSnapshot

```
local_llm.model default changed to "qwen3-8b"
```


---

## 10. Phase 4: Model Auto-Load (2026-06-07)

### 10.1 Configuration

```toml
[local_llm]
backend = "llama_cpp"     # "llama_cpp" | "ollama" | "vllm"
auto_load = false          # Auto-start the inference server on app launch
models_dir = ""            # Directory to scan for .gguf files. Default: ~/models (Linux/macOS) or %%USERPROFILE%%\models (Windows)
load_command = ""          # Override auto-generated load command. Placeholders: {model_path}, {port}, {host}
unload_command = ""        # Override auto-generated unload command. Empty = SIGTERM
```

### 10.2 Default Load Commands

| Backend | Default Command |
|---|---|
| `llama_cpp` | `llama-server -m {model_path} --port {port} --host {host} --ctx-size 4096 --n-gpu-layers 999 --flash-attn --reasoning off` |
| `ollama` | `ollama pull {model_name}` (pre-pull only; ollama serve must already be running) |
| `vllm` | Not supported; manual start required |

### 10.3 Model Path Resolution

The `model` config field supports two formats:
- `manufacturer/model_name` — resolves to `models_dir/manufacturer/model_name.gguf`
- `model_name` (no slash) — searched through all .gguf files in `models_dir`

### 10.4 Auto-Load Lifecycle

1. On app start, if `auto_load = true` and `enabled = true`, the backend process is spawned
2. If `unload_command` is configured, it is expanded and stored for shutdown
3. The local worker waits for `{base_url}/health` after startup
4. The process is killed on app exit (via Drop) after optional `unload_command`
5. If the model file is not found, auto-load is skipped with a warning

### 10.5 Resource Exclusivity

Phase 4 uses a coarse local model resource policy:

| Condition | Behavior |
|---|---|
| `ResourceRequirements.gpu_required = true` | Job requires the local model slot |
| `ResourceRequirements.vram_mb = Some(...)` | Job requires the local model slot |
| Slot free | Job acquires the slot and runs |
| Slot occupied | Scheduler transitions job to `WaitingResource` |
| Wait exceeds `job.timeout_ms` | Job transitions to `Timeout` |

The persona rewrite path also uses the same resource slot before calling `LocalLlmWorker.infer()`. This prevents concurrent chat requests from sending overlapping rewrite jobs to the local LLM.

When `[observability] vram_logs = true`, acquire, busy, and release events are emitted through `tracing`.

### 10.6 Local Health State

`LocalLlmWorker::health_check_http()` checks `{base_url}/health` asynchronously and caches the result for synchronous `Worker::health_check()`.

Startup behavior:
- With `auto_load = true`, Hestia waits up to 20 seconds for health.
- With `auto_load = false` and `enabled = true`, Hestia checks for up to 2 seconds.
- `ConfigSnapshot.local_llm.available` is set from this startup health result.
- If persona rewrite is enabled while local LLM is unavailable, `send_chat_message` returns the raw remote response with `"rewritten": false`.

### 10.7 Arch Linux Package Dependency (Future)

When packaging Hestia as an Arch Linux package, at least one backend should be listed as an optional dependency:
- `llama.cpp` (provides `llama-server`) — primary recommendation
- `ollama` (provides `ollama`) — alternative

This should be addressed in Phase 7 (Plugin Boundary) or a dedicated packaging sub-phase.

---

## 11. Phase 5: ComfyUI Image Generation (2026-06-11)

### 11.1 Configuration

```toml
[multimodal.comfyui]
enabled = false
base_url = "http://127.0.0.1:8188"
root_dir = ""          # Linux placeholder: ~/models/ComfyUI
python_path = ""       # e.g. ~/miniconda3/envs/comfyui/bin/python
env_type = "venv"      # "system" | "venv" | "conda"
auto_start = false       # start backend service on demand for image jobs
launch_command = ""    # placeholders: {python}, {root_dir}, {host}, {port}
workflow_path = "assets/workflows/sdxl.json"
output_dir = "data/artifacts/images"
startup_timeout_ms = 20000
```

Windows placeholders shown in the Settings UI:
- ComfyUI root: `%USERPROFILE%\models\ComfyUI`
- Conda Python: `%USERPROFILE%\miniconda3\envs\comfyui\python.exe`

`python_path` should point directly to the Python executable inside the selected conda or venv environment. The app does not activate shell environments.

`auto_start` starts the ComfyUI backend service only when an image job needs it. If Hestia launches ComfyUI for a job, it stops that managed process after the job completes. If the user started ComfyUI externally, Hestia uses it but does not terminate it.

ComfyUI normally loads diffusion checkpoints when a workflow runs, but Hestia keeps the backend process closed until image generation is requested to avoid idle port and process usage. llama.cpp is different: starting `llama-server -m ...` loads the model and may occupy VRAM immediately.

### 11.2 Workflow Contract

`workflow_path` may point to either:
- ComfyUI API workflow JSON (`class_type` nodes)
- ComfyUI UI workflow JSON (`nodes` and `links`)

UI workflows are converted to API prompt JSON before POSTing to `/prompt`. The bundled test workflow is:

```text
assets/workflows/sdxl.json
```

It references these checkpoint filenames:
- `sd_xl_base_1.0.safetensors`
- `sd_xl_refiner_1.0.safetensors`

They must be available under the configured ComfyUI installation's `models/checkpoints`.

### 11.3 Runtime Contract

Image generation follows the project architecture rule:

```text
Frontend Image Test or chat image request
-> generate_test_image command or send_chat_message image branch
-> ImageGeneration Job
-> Scheduler
-> ResourceManager
-> ComfyUiWorker
-> ComfyUI HTTP API
-> artifact paths
```

The worker uses:
- `GET /system_stats` for health
- `POST /prompt` for submission
- `GET /history/{prompt_id}` for completion
- `GET /view?...` for image download

Image artifacts are saved under `ConfigSnapshot.multimodal.comfyui.output_dir`.

### 11.4 Chat Image Generation

Chat supports three image entry points:
- Explicit command: `\image prompt` or `/image prompt`
- Input image button: wraps the current textarea value as `\image prompt`
- Model-routed intent: `send_chat_message` asks the remote chat worker for strict JSON and only runs ComfyUI when `should_generate=true`

Router failure falls back to normal chat. The router currently uses the configured remote chat worker. A future local router can reuse the same strict JSON contract through a lightweight local model.

### 11.5 Kimi Vision Recognition

Configuration:

```toml
[multimodal.vision]
enabled = false
base_url = "https://api.moonshot.ai"
api_key_env = "MOONSHOT_API_KEY"
model = "kimi-k2.6"
system_prompt = "..."
default_prompt = "..."
max_image_bytes = 20971520
```

The Vision worker uses Kimi's OpenAI-compatible chat completions endpoint. Image input is sent as base64 `image_url` inside `message.content` array, followed by a text prompt. This matches Kimi's documented vision request format and avoids exposing API keys in the frontend.

Supported local image formats:
- png
- jpeg / jpg
- webp
- gif

Runtime flow:

```text
Frontend image upload button
-> recognize_image command
-> local image path validation and base64 data URL creation
-> Vision Job
-> Scheduler
-> VisionApiWorker
-> Kimi / Moonshot HTTP API
-> text description returned to chat
```

Screenshot compatibility: the backend helper accepts a `source` string, so a future screenshot capture module can pass `source = "screenshot"` and reuse the same job path without a new worker.

### 11.6 Screenshot Boundary

`get_screenshot_metadata` is intentionally conservative. It exposes screenshot settings and reports `capture_available = false` until a platform-specific capture backend is added. There is no background screenshot capture in Phase 5 MVP.

## 12. Phase 6: Initiative System (2026-06-13)

Configuration:

```toml
[initiative]
enabled = false
level = 0.3
cooldown_ms = 600000
```

Runtime state lives in `AppState.initiative` and is not persisted:
- `last_user_activity`
- `last_initiative_at`
- recent explainable decisions, capped at 20

Policy inputs:
- `enabled`: hard gate. If false, proactive messages are blocked.
- `level`: 0.0 to 1.0. Higher values reduce required idle time.
- `cooldown_ms`: minimum time between proactive messages.
- `idle_ms`: time since latest user activity reported by chat, vision upload, or throttled frontend events.

Decision outputs include `allowed`, `score`, `idle_ms`, `min_idle_ms`, `cooldown_remaining_ms`, and `reasons`.

Reasons currently include:
- `initiative_disabled`
- `non_companion_trigger`
- `user_recently_active`
- `cooldown_active`
- `score_below_threshold`

Frontend behavior:
- Settings exposes Enable, Level, and Cooldown.
- Header sparkles button manually calls `request_initiative_message`.
- Main window mode does not run automatic proactive checks. The user must click the sparkles button to open a proactive topic.
- Automatic proactive speech is owned by the desktop companion window, which uses a trigger beginning with `companion`, currently `companion_timer`.
- Backend policy blocks non-manual automatic triggers that do not start with `companion`.

### 12.1 Companion Trigger Contract

The desktop companion UI calls:

```ts
invoke("request_initiative_message", {
  history: [],
  trigger: "companion_timer",
})
```

Rules:
- Main window must not run automatic proactive timers.
- Main window sparkles button uses `trigger = "manual"` and is considered user-initiated.
- Companion automatic triggers must start with `companion`.
- Backend returns `allowed = false` with reason `non_companion_trigger` for automatic non-companion callers.
- Companion UI may render the returned proactive text locally or forward it into the main chat, but it must not bypass `request_initiative_message`.
