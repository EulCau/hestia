# Technical Route

## 1. Positioning

Hestia should be treated as an application layer runtime orchestration platform for a personal AI companion. The project should not start from a chat page. It should start from the runtime contract:

```text
User / UI / System Event
-> Job
-> Scheduler
-> Worker
-> Result
-> Runtime State / UI / Memory
```

The most important rule is:

```text
Model output may propose actions, but the runtime decides whether and how to execute them.
```

## 2. Recommended Stack

### Desktop Application

- Tauri 2.x
- Rust backend
- TypeScript frontend
- Vite for frontend development

Reason:

- lower long-running memory pressure than Electron
- better fit for tray, transparent windows, and desktop overlay
- Rust is suitable for scheduler, process control, and resource bookkeeping
- frontend can remain fast to iterate

### Runtime Core

Rust should own:

- Job model
- Capability registry
- Scheduler
- Worker lifecycle
- configuration loading and validation
- event bus
- observability logs
- platform services, such as tray, screenshot, and window control

### Worker Layer

Workers should be replaceable adapters. Use the simplest stable boundary first:

- HTTP adapter for ComfyUI and remote APIs
- process adapter for llama.cpp or local command workers
- future gRPC or IPC adapter only if HTTP/process becomes insufficient

Do not embed every model runtime directly into the main desktop process. Long-running model processes are easier to restart, monitor, and isolate when they are sidecars.

## 3. First Version Architecture

```text
src-tauri/
  src/
    protocol/        internal message, job, capability schemas
    scheduler/       queue, priority, cancellation, timeout
    resource/        GPU/VRAM state and model exclusivity policy
    workers/         local and remote worker adapters
    personality/     persona config, prompt assembly, rewriting
    memory/          conversational, episodic, semantic memory
    multimodal/      screenshots, OCR, vision, image generation routing
    config/          config schema and validation
    observability/   job timeline, prompt logs, token logs, VRAM logs
    runtime/         application service composition
    ui/              UI-facing commands exposed to the frontend

frontend/
  src/               TypeScript UI
  public/            static frontend assets
```

The first implementation should prioritize contracts and logs before model variety.

## 4. Key Technical Decisions

### 4.1 Job First

Every inference or tool operation is represented as a Job. This avoids hidden model-to-model calls.

Minimum fields:

```json
{
  "id": "job_...",
  "kind": "chat",
  "capability": "chat",
  "priority": 0,
  "status": "queued",
  "created_at": "2026-05-28T00:00:00Z",
  "timeout_ms": 30000,
  "cancelable": true,
  "payload": {}
}
```

### 4.2 Capability Routing

Workers declare capabilities:

```json
{
  "worker_id": "deepseek_chat",
  "capabilities": ["chat"],
  "execution": "remote_api",
  "priority": 10
}
```

The scheduler selects a worker using:

- requested capability
- worker health
- current resource state
- user configuration
- job priority

### 4.3 Coarse GPU Exclusivity

For local large models, use coarse-grained exclusivity in the first version:

```text
one large local model active on one GPU at a time
```

This is conservative but appropriate for long-running desktop software. It reduces CUDA fragmentation and unpredictable crashes.

### 4.4 Personality Is Application State

Personality must not be stored inside a model adapter. It belongs to:

```text
personality config -> prompt assembler -> optional persona rewriter
```

This keeps persona portable across DeepSeek, Qwen, local llama.cpp, or future models.

## 5. Recommended MVP

The MVP should be:

```text
Tauri shell
-> config loading
-> Job queue
-> DeepSeek-compatible chat worker
-> prompt assembler
-> local persona config
-> job timeline logs
-> simple chat UI
```

After this is stable, add:

1. local persona rewriter
2. screenshot capture and lightweight OCR metadata
3. ComfyUI image worker
4. initiative scoring
5. local vision worker
