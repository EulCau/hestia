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

## Third-Party Software

Hestia is built on Tauri, Rust, TypeScript, Vite, and their package ecosystems. Runtime integrations such as ComfyUI, llama.cpp, Ollama, vLLM, Live2D, VRM assets, Stable Diffusion models, and remote AI APIs are treated as optional external backends or user-provided assets unless explicitly bundled in a future release.

ComfyUI is not vendored into this repository. If a packaged release later ships ComfyUI or other external runtimes, their licenses and source/distribution obligations must be documented separately.

Model files, generated character assets, Live2D models, VRM models, and image-generation checkpoints may have their own licenses. Users should verify those licenses before redistribution.

## License

Hestia is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
