# Work Order

## Phase 0. Project Grounding

Goal: make the project buildable and observable before adding complex AI behavior.

Tasks:

1. Initialize package/build system.
2. Add Tauri desktop application skeleton.
3. Define config schema.
4. Define Job, Capability, Worker, and Event schemas.
5. Add structured logs from the first runnable version.

Deliverable:

```text
An empty desktop app that can load config and write runtime logs.
```

## Phase 1. Runtime Core

Goal: implement the smallest complete scheduling loop.

Tasks:

1. Implement JobQueue.
2. Implement Scheduler.
3. Implement WorkerRegistry.
4. Implement timeout, cancellation, and status transition.
5. Write unit tests for Job lifecycle.

Do not integrate many models in this phase. A mock worker is enough.

Deliverable:

```text
Submit Job -> Scheduler -> Mock Worker -> Result -> Timeline log
```

## Phase 2. Remote Chat

Goal: finish the first useful conversation loop.

Tasks:

1. Implement DeepSeek/OpenAI-compatible RemoteApiWorker.
2. Implement PromptAssembler.
3. Add default personality JSON.
4. Add chat UI.
5. Record prompt logs and token usage.

Deliverable:

```text
User message -> prompt assembly -> remote chat worker -> UI response
```

## Phase 3. Local Personality Layer

Goal: separate cognition from persona style.

Tasks:

1. Add llama.cpp process worker or HTTP worker.
2. Add PersonaRewriteJob.
3. Route DeepSeek output through persona rewriting when enabled.
4. Add switch to bypass rewriting for debugging.

Deliverable:

```text
Remote answer -> local persona rewrite -> final answer
```

## Phase 4. Resource Manager (Completed)

Goal: avoid unstable local model switching.

Completed:
- [x] Model auto-discovery from models_dir
- [x] Auto-load backend process (llama.cpp, ollama, vllm)
- [x] Custom load/unload command override
- [x] Settings UI for auto-load, backend selection, command overrides
- [x] Coarse model exclusivity for local model jobs
- [x] VRAM/resource acquire, busy, release event logs
- [x] Async local worker health check integration

Completed tasks:

1. Track configured local models and their approximate VRAM requirements.
2. Implement coarse model exclusivity.
3. Implement unload/load lifecycle.
4. Record VRAM events.
5. Add worker health checks (async health check integration).

Note for packaging (Phase 7 or dedicated sub-phase):
- Arch Linux: `llama.cpp` (provides `llama-server`) as optional dependency
- Alternative: `ollama` as optional dependency

Deliverable:

```text
Local worker jobs are serialized according to GPU policy.
```

## Phase 5. Multimodal (Completed MVP)

Goal: add screen and image workflows without breaking the runtime.

Tasks:

1. Add screenshot service.
2. Add lightweight screen metadata path.
3. Add VisionJob only when explicitly triggered or threshold conditions pass.
4. Add ComfyUI ImageGenerationWorker.
5. Keep chat prompt and image prompt builders separate.

Completed:
- [x] Screenshot metadata command with conservative default (`capture_available = false`)
- [x] Explicit image generation command routed as Job -> Scheduler -> ComfyUI worker
- [x] In-chat image generation via `\image`, `/image`, input button, and conservative remote-model intent routing
- [x] Kimi vision recognition via local image upload -> Vision Job -> Scheduler -> VisionApiWorker
- [x] Screenshot-ready vision backend helper for future screenshot image paths
- [x] ComfyUI worker for `/prompt`, `/history`, and `/view`
- [x] ComfyUI UI/API workflow loader, including conversion from UI workflow JSON
- [x] Separate multimodal prompt/workflow builder from chat prompt assembly
- [x] Settings UI for ComfyUI root directory, Python executable, env type, on-demand start, workflow, output directory
- [x] Start ComfyUI only when image generation needs it, then stop Hestia-managed process after completion
- [x] Test workflow stored at `assets/workflows/sdxl.json`

Deliverable:

```text
Screenshot or image request -> Job -> selected worker -> result artifact
```

## Phase 6. Initiative System (Completed MVP)

Goal: controlled proactive behavior.

Tasks:

1. Define initiative score inputs.
2. Add cooldown and user activity constraints.
3. Add explainable decision logs.
4. Add GUI controls for initiative level.

Deliverable:

```text
The AI can speak proactively only when runtime policy permits it.
```

Completed:
- [x] Initiative score inputs: enabled, level, idle time, cooldown
- [x] Runtime user activity tracking
- [x] Cooldown enforcement after proactive messages
- [x] Explainable decision logs with reasons
- [x] Tauri commands for activity recording, evaluation, and guarded proactive message generation
- [x] Settings UI controls for enable, level, and cooldown
- [x] Header action for user-initiated topic opening in window mode
- [x] Backend guard so automatic proactive speech is reserved for future desktop companion triggers

## Phase 7. Roles and Memory Core (Completed MVP)

Goal: provide explicit, user-controlled character roles and long-term memory before expanding the plugin surface.

Completed:
- [x] Role schema and role management UI
- [x] Role generation through the configured chat API
- [x] Active role selection through config
- [x] Local memory schema and storage under ignored `usr/memory/`
- [x] Per-role memory isolation
- [x] Manual memory CRUD commands
- [x] Automatic memory writes from chat, companion dialogue, and companion initiative
- [x] API-backed memory compression command and UI button
- [x] Memory management UI
- [x] Lightweight pinned/recent/keyword retrieval
- [x] Prompt context injection for chat and companion initiative

Deferred:
- [ ] User confirmation workflow for model-suggested memories
- [ ] Semantic/vector retrieval
- [ ] Packaged-build migration for roles and memory to system user data directory

Deliverable:

```text
Active role + role-managed memories -> retrieved context -> chat/companion responses
```

## Phase 7.5. Plugin Boundary (defer until companion events and memory state stabilize)

Goal: prepare future expansion without implementing all plugins.

Tasks:

1. Define plugin manifest.
2. Define permission model.
3. Define plugin events.
4. Keep plugins disabled by default.

Deliverable:

```text
A documented plugin contract, not a large plugin ecosystem.
```


---

## Phase 8. Desktop Companion (parallel — can start at any point)

Goal: a desktop-resident animated companion character.

See [docs/desktop-companion.md](/home/eulcau/CXTX/hestia/docs/desktop-companion.md) for full technical design.

Recommended next step after Phase 6. The application rule is:

```text
Main chat window: user must initiate topics.
Desktop companion window: may speak proactively only through initiative policy.
```

MVP tasks:

1. Create transparent always-on-top Tauri companion window.
2. Use existing placeholder avatar as the first companion surface.
3. Keep the companion view separate from the main chat/settings UI.
4. Add show/hide control from the main window.
5. Let only the companion window call `request_initiative_message` with `trigger = "companion_timer"`.
6. Keep main-window automatic proactive speech disabled.
7. Document the companion trigger contract in `docs/ui-interface-contract.md`.

Later tasks:

1. Integrate Live2D Cubism SDK for Web (2D).
2. Implement AvatarAdapter with Live2D (mount, unmount, expression events).
3. Map backend persona states to character expressions and motions.
4. Add mouse tracking (eye follow), idle animations, system tray integration.
5. (Optional) Unity + VRM 3D sidecar with IPC.

Deliverable:

```text
Animated desktop character that responds to backend persona events
with expressions, motions, and user interaction.
```
