# Project Structure

This repository starts with a runtime-first structure. The exact build system can still evolve, but module boundaries should remain stable.

```text
hestia/
  README.md
  hestia.md
  .gitignore

  src-tauri/
    src/
      runtime/
      scheduler/
      resource/
      workers/
      personality/
      memory/
      multimodal/
      config/
      observability/
      protocol/
      ui/

  frontend/
    src/
    public/

  bin/
  scripts/
  docs/
  config/
  personality/
  plugins/
  assets/
  data/
  logs/
  tests/
  examples/
  models/
  third_party/
```

## Directory Responsibilities

### `src-tauri`

Rust backend and Tauri application package. This should contain `Cargo.toml`, Tauri configuration, and the runtime source tree after the project is initialized.

### `src-tauri/src/runtime`

Application composition layer. It wires scheduler, config, workers, memory, observability, and UI commands together.

### `src-tauri/src/scheduler`

Job queue, priority policy, cancellation, timeout, status transition, and dispatch.

No model-specific logic should live here.

### `src-tauri/src/resource`

GPU and process resource bookkeeping.

First version policy:

```text
large local model execution is serialized by default
```

### `src-tauri/src/workers`

Worker adapters only. A worker implements a capability contract, such as chat, vision, image generation, OCR, TTS, or memory summary.

Workers should not own global application state.

### `src-tauri/src/personality`

Persona schema, prompt assembly, style rules, few-shot examples, and optional persona rewriting.

Personality is application state, not model state.

### `src-tauri/src/memory`

Conversational context, episodic memory, semantic memory, and memory extraction.

Do not mix scratchpad reasoning with user-visible chat context.

### `src-tauri/src/multimodal`

Screenshots, OCR metadata, vision routing, image generation routing, and artifact handling.

Chat prompts and image prompts must remain separate.

### `src-tauri/src/config`

Typed configuration schema, defaults, validation, migration, and import/export.

The GUI should eventually write config through this layer instead of directly editing JSON.

### `src-tauri/src/observability`

Job timeline, prompt logs, VRAM logs, token usage logs, memory logs, and health reports.

This must exist from the first runnable version.

### `src-tauri/src/protocol`

Shared internal message definitions:

- Job
- Capability
- WorkerInfo
- RuntimeEvent
- ModelResource
- Artifact

### `src-tauri/src/ui`

UI-facing commands and view state. Keep it thin. Runtime decisions should stay in `src-tauri/src/runtime` and `src-tauri/src/scheduler`.

### `frontend`

TypeScript frontend application. This is where the chat view, settings GUI, tray-facing UI pages, and desktop companion surface should live.

Recommended first version:

```text
frontend/src/
  app/
  components/
  features/
  styles/
```

### `bin`

Small executable entry points and developer commands.

Examples:

- launch desktop app
- start local worker
- inspect config
- print worker registry

### `scripts`

Development and maintenance scripts.

Examples:

- download sample assets
- generate schema
- run local worker health checks

### `config`

Default non-secret configuration templates.

Secrets should be loaded from `.env` or OS keyring later.

### `personality`

Persona presets. These should be importable, exportable, versioned, and editable through GUI later.

### `plugins`

Reserved plugin directory. First version should define the contract but keep plugins disabled by default.

### `assets`

UI assets, icons, placeholder avatar assets, and sound assets.

### `data`

Runtime data. Most subdirectories are ignored by git.

### `logs`

Runtime logs. Ignored by git.

### `tests`

Unit tests, integration tests, and fixtures.

### `models`

Local model files. Ignored by git because model files are large.

### `third_party`

Explicit third-party code or vendored definitions, only when necessary.
