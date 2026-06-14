# Hestia

Hestia is a personal AI companion runtime platform. It is not designed as a single monolithic chat application. The core idea is to keep models as replaceable inference workers, while the application layer owns scheduling, resource control, personality, memory, multimodal routing, and observability.

## Current Technical Route

- Desktop shell: Tauri, with Rust backend and a web UI.
- Runtime core: Rust first, responsible for job scheduling, resource arbitration, configuration, state, and logs.
- Model execution: Worker adapters behind a unified capability interface.
- Local model integration: llama.cpp, ComfyUI, Qwen2.5-VL, TTS services, or later vLLM/Ollama.
- Remote model integration: API-compatible workers, starting with DeepSeek-style chat APIs.
- Architecture rule: models do not call models directly. All work becomes a Job and goes through the Scheduler.

## Repository Layout

See [docs/project-structure.md](/home/eulcau/CXTX/hestia/docs/project-structure.md).

The intended JetBrains/Tauri layout is:

```text
src-tauri/    Rust runtime and Tauri backend
frontend/     TypeScript UI
docs/         architecture and implementation notes
```

## Work Order

See [docs/work-order.md](/home/eulcau/CXTX/hestia/docs/work-order.md).

## Knowledge Map

See [docs/knowledge-map.md](/home/eulcau/CXTX/hestia/docs/knowledge-map.md).
