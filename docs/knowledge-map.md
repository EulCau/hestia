# Knowledge Map

## 1. Must Know First

### Rust Runtime Basics

You need:

- ownership and borrowing
- error handling with `Result`
- async Rust basics, especially `tokio`
- channels and task cancellation
- serde serialization and deserialization
- structured logging with `tracing`

Reason:

The scheduler, worker lifecycle, and config validation are long-running runtime problems. Rust helps, but only if ownership and async boundaries are explicit.

### Tauri Desktop Development

You need:

- Tauri command model
- frontend to backend invocation
- system tray
- window control
- transparent and always-on-top windows
- platform-specific permissions

Reason:

Hestia is a desktop runtime, not a web service.

### Internal Protocol Design

You need:

- schema versioning
- Job state machine design
- event-driven architecture
- typed payloads
- backward compatibility

Reason:

If the internal protocol is loose, prompts and strings will leak across module boundaries and the system will become hard to debug.

## 2. AI / Model Integration Knowledge

### LLM API Integration

You need:

- OpenAI-compatible chat completion format
- streaming responses
- token usage accounting
- retry and timeout policy
- API key management

### Local Model Runtime

You need:

- llama.cpp server or process mode
- model quantization formats, especially GGUF
- context length and KV cache
- CPU/GPU layer split
- model load/unload cost

### Vision and Image Generation

You need:

- ComfyUI API workflow format
- image artifact management
- prompt builder separation
- basic OCR and screen capture constraints
- Qwen2.5-VL or similar vision model deployment

## 3. GPU / Systems Knowledge

You need:

- VRAM capacity and fragmentation
- CUDA context lifetime
- model load/unload behavior
- process isolation
- long-running application resource leaks
- Windows and Linux GPU tooling differences

The first version should use coarse exclusivity. Do not attempt fine-grained VRAM preemption early.

## 4. Product / UX Knowledge

You need:

- desktop overlay UX
- tray and settings UX
- privacy controls
- explicit user consent for screenshots
- non-intrusive proactive behavior

The companion should feel configurable and predictable. Active behavior must be explainable and controllable.

See [docs/desktop-companion.md](/home/eulcau/CXTX/hestia/docs/desktop-companion.md) for detailed design of the desktop companion rendering pipeline (Live2D, Spine, Unity+VRM).

## 5. Suggested Learning Order

1. Rust data model, serde, tracing.
2. Tauri command and window model.
3. Job queue and scheduler state machine.
4. Remote API worker.
5. Prompt assembly and persona config.
6. Local worker process management.
7. GPU resource policy.
8. Screenshot and multimodal routing.
9. Initiative scoring.
10. Plugin boundaries.

