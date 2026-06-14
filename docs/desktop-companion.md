# Desktop Companion — Technical Design

**Date:** 2026-06-14
**Status:** Phase 8 companion MVP is implemented; next step is the Live2D expression event skeleton

---

## 1. Goal

A desktop-resident companion character that:

- Persists on the desktop (always-on-top, click-through body, tray control)
- Renders a 2D or 3D animated character
- Responds to backend events (expression changes, idle animations, speech)
- Supports user interaction (click to speak, drag to reposition)
- Does not interfere with normal desktop use

Current product boundary:

```text
Main chat/settings window:
  - never speaks proactively on a timer
  - user opens topics manually, including the sparkles button

Desktop companion window:
  - may speak proactively
  - must call `request_initiative_message` with a trigger beginning with `companion`
  - all proactive speech is still gated by InitiativeRuntime
```

The companion window lifecycle has shipped with the existing placeholder avatar. Live2D and VRM remain rendering upgrades. The next implementation should define and wire the expression/motion event skeleton before adding model assets or external runtimes.

---

## 2. Window Requirements (Tauri)

### 2.1 Window Properties

```json
{
  "transparent": true,
  "decorations": false,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "resizable": true,
  "focus": true
}
```

| Property | Value | Reason |
|---|---|---|
| `transparent` | `true` | No background — only the character is visible |
| `decorations` | `false` | No title bar, no window border |
| `alwaysOnTop` | `true` | Character stays above all other windows |
| `skipTaskbar` | `true` | Not a regular app window |
| `resizable` | `true` | MVP exposes a small custom resize handle for the companion surface |
| `focus` | `true` | Needed for reliable controls, drag-region handling, and toolbar interaction |

Linux topmost behavior:
- Tauri `alwaysOnTop` maps to window-manager hints and is not absolute on every Wayland compositor.
- If a compositor ignores those hints, strict topmost behavior requires compositor-specific support such as a layer-shell based implementation or a dedicated desktop shell extension.

### 2.2 Click-Through Strategy

Two regions:
- **Body (click-through)**: mouse events pass through to underlying windows
- **Interactive zones** (head, hands, buttons): capture clicks for user interaction

Implementation: set the window to `WS_EX_TRANSPARENT` (Windows) / `set_ignore_mouse_events(true)` (Linux/macOS) for the body canvas, with a small interactive overlay for clickable areas.

### 2.3 System Tray

- Tray icon always visible
- Right-click menu: Show/Hide companion, Settings, Quit
- Left-click: toggle companion visibility

### 2.4 Window Count

The desktop companion runs as a **separate Tauri window** from the main chat/settings window. This avoids coupling the companion rendering lifecycle with the chat UI lifecycle.

```

### 2.5 MVP Window Contract

For the first implementation:
- Label: `companion`
- Route or page: a minimal companion-only view, not the full chat UI
- Initial content: existing placeholder avatar
- Size: fixed small overlay, e.g. 220x300
- Main window remains the settings/chat control surface
- Companion window owns automatic initiative checks

The companion window may call:

```typescript
invoke("request_initiative_message", {
  history: [],
  trigger: "companion_timer",
})
```

The backend blocks automatic triggers that do not start with `companion`, so do not use `timer` or `window_timer`.
Tauri App
  Window 1: Chat + Settings (standard window, 900x700)
  Window 2: Desktop Companion (transparent overlay, variable size)
```

---

## 3. Avatar Rendering Tiers

### 3.1 Tier A: 3D Digital Human (Unity + VRM)

**Stack:**
- Unity 2022 LTS+ built as a native plugin or sidecar process
- VRM 1.0 model format (widespread in VTuber/digital human ecosystem)
- Communication between Unity and Tauri via IPC (named pipe, WebSocket, or shared memory)

**Architecture:**
```
Tauri (Rust)                         Unity (C#)
  ..............    IPC (pipe/WS)     ....................
  . Backend    . <.................> . VRM Loader       .
  . Event Bus  .                     . Animation Ctrl   .
  ..............                     . Expression Blend .
                                     . Camera/Render    .
                                     ....................
                                              |
                                     ..........v..........
                                     . Desktop Window    .
                                     . (transparent,      .
                                     .  always-on-top)    .
                                     ......................
```

**VRM Advantages:**
- Standardized format: bone hierarchy, expressions (BlendShape), physics (spring bones), look-at targets
- Expression presets: `joy`, `angry`, `sorrow`, `fun`, `surprised` — mappable to backend persona states
- Rich ecosystem: VRoid Studio for character creation, UniVRM for runtime loading
- Spring bone physics for hair/cloth out of the box

**Key Integration Points:**
1. Unity exports a render texture or native window handle that Tauri embeds or overlays
2. IPC protocol for expression commands: `{ "expression": "joy", "weight": 0.8, "duration_ms": 2000 }`
3. Idle animation loop in Unity, triggered/overridden by backend events
4. Lip-sync if TTS is added later (blend shape `a`, `i`, `u`, `e`, `o`)

**Challenges:**
- Unity runtime overhead (~100-200 MB RAM baseline)
- IPC latency for real-time expression sync
- Platform-specific window embedding (HWND on Windows, X11/Wayland on Linux)
- Build pipeline complexity (Unity build -> embed in Tauri bundle)

### 3.2 Tier B: 2D Skeletal Animation (Live2D / Spine)

**Stack:**
- Live2D Cubism SDK for Web (JavaScript/TypeScript) or Spine Runtime
- Rendered in a `<canvas>` element within the Tauri webview
- Model files: `.model3.json` (Live2D) or `.json`/`.skel` (Spine)

**Architecture:**
```
Tauri WebView
  ............................................
  .  <canvas>                              .
  .    Live2D / Spine Runtime              .
  .    - Model loading                      .
  .    - Motion playback                    .
  .    - Expression blending                .
  .    - Physics (Live2D)                   .
  .    - Eye tracking (mouse follow)        .
  ............................................
           | invoke()
  ........v..........
  . Tauri Backend    .
  . Event -> Expression .
  ....................
```

**Live2D Advantages:**
- Pure webview rendering — no external process
- Cubism SDK for Web is mature, used by VTubers and gacha games
- Model file size small (~2-5 MB)
- Built-in physics (hair, breast, clothing sway)
- Eye tracking: character follows mouse cursor
- Expression blending: smooth transitions between expressions

**Spine Advantages:**
- More flexible animation system (mesh deformation, IK)
- Better for custom animation styles
- Widely used in indie games
- Smaller runtime footprint than Live2D

**Key Integration Points:**
1. `AvatarAdapter` interface in `frontend/src/main.ts` already defines `mount(container)` / `unmount()` / `onEvent()`
2. Extend `onEvent` to accept expression/emotion commands from backend
3. Motion mapping: backend events -> Live2D motion groups (e.g., `idle`, `happy`, `thinking`, `speaking`)
4. Mouse tracking: character eyes/head follow cursor

### 3.3 Tier Comparison

| Aspect | Unity + VRM (3D) | Live2D (2D) | Spine (2D) |
|---|---|---|---|
| Visual quality | High (3D, lighting, shadows) | High (anime-style, polished) | Medium-High |
| Resource usage | High (GPU + RAM) | Low (webview canvas) | Low |
| Implementation complexity | High (Unity IPC, window embedding) | Low (JS SDK in webview) | Low-Medium |
| Model availability | VRoid Studio, Booth, VRM markets | Live2D market, custom commissions | Spine market, custom |
| Expression support | BlendShapes (standard set) | Parameter-based blending | Mesh deformation + IK |
| Physics | Spring bones in VRM spec | Built-in Cubism physics | Manual setup |
| Lip-sync readiness | Yes (blend shapes) | Yes (mouth parameters) | Manual |
| Platform risk | Unity build pipeline + embedding | Web standard, no native deps | Web standard, no native deps |

### 3.4 Recommended Approach

**Phase 1: Live2D (lowest risk, fastest to ship)**

Implement the `AvatarAdapter` with Live2D Cubism SDK for Web. This gives:
- Desktop companion in < 1 week of frontend work
- Expression mapping from backend persona states
- Mouse tracking for eye contact
- No external process, no IPC complexity

**Phase 2: Unity + VRM (when 3D is needed)**

Add Unity sidecar only when:
- 3D assets are ready
- Desktop window embedding is stable on target platforms
- Performance budget allows it

### 3.5 Replacing the Placeholder Avatar

The current MVP companion view is intentionally a small adapter surface. To replace the static cat placeholder with another visual form, keep the Tauri window contract stable and change only the frontend avatar adapter/rendering layer.

For a 2D Live2D or Spine model:
1. Add the model runtime dependency in `frontend/package.json`.
2. Put model assets under `frontend/public/` or a future configured asset directory. Live2D normally needs `.model3.json`, textures, motions, expressions, and physics files. Spine normally needs `.json` or `.skel`, atlas, and textures.
3. Extend `createAvatarAdapter()` in `frontend/src/main.ts` to return a canvas-based adapter when `ConfigSnapshot.app.avatar.model_type` is `live2d` or `spine`.
4. Mount the canvas inside the existing `.companion-avatar` element. Preserve `data-tauri-drag-region` on a non-interactive wrapper, and keep toolbar buttons, resize handle, and dialogue bubble outside the renderer.
5. Keep renderer lifecycle explicit: `mount(container)` creates the runtime/canvas, `unmount()` removes listeners and releases WebGL resources, and `onEvent(...)` maps backend/frontend events to motions or expressions.
6. Update `ConfigSnapshot.app.avatar.model_type`, `config/default.toml`, and `docs/ui-interface-contract.md` if new config keys or enum values are exposed to the UI.

For a 3D VRM or other WebGL model inside the Tauri webview:
1. Prefer Three.js or a VRM web runtime inside the same companion window before introducing an external renderer.
2. Use the same adapter boundary as Live2D: canvas mount, explicit disposal, event-to-expression mapping, and no direct model calls from backend workers.
3. Treat model pose, expression, lip-sync, and idle animation as application state/events. The model renderer should consume events; it should not own scheduler or worker state.
4. Keep the transparent window small and test GPU usage. If WebGL transparency or performance is not stable, move to the sidecar approach below.

For a Unity/VRM sidecar:
1. Keep Tauri as the controller and tray/window lifecycle owner.
2. Run Unity as a managed sidecar process only after the window lifecycle is stable.
3. Communicate through IPC such as WebSocket, named pipe, or stdin/stdout JSON messages.
4. Define a narrow event contract: `expression`, `motion`, `speak_start`, `speak_stop`, `look_at`, and `idle`.
5. Do not let the Unity process call models or own global application state. Model work still goes through Jobs, Scheduler, and Workers.

---

## 4. Backend <-> Companion Communication

### 4.1 Event Types (Backend -> Companion)

| Event | Companion Response |
|---|---|
| `chat.started` | "thinking" expression, typing animation |
| `chat.responding` | "speaking" motion, lip movement |
| `chat.idle` | Idle animation loop |
| `persona.happy` | Joy expression, bounce |
| `persona.sad` | Sorrow expression, droop |
| `persona.excited` | Surprise expression, sparkle |
| `user.idle` | Sleep/drift animation |
| `user.active` | Wake up, attention animation |
| `error.occurred` | Confused/tilt-head expression |
| `initiative.trigger` | Proactive greeting motion |
| `system.startup` | Wake-up animation |
| `system.shutdown` | Sleep/fade-out animation |

### 4.2 User Interaction (Companion -> Backend)

| Interaction | Event |
|---|---|
| Click head | `companion.interact` -> trigger chat focus |
| Click body | Pass through (no event) |
| Drag character | Reposition window |
| Right-click | Context menu (hide, settings) |

### 4.3 IPC Mechanism

For Phase 1 (Live2D in webview): standard `invoke()` / Tauri events. Both windows share the same Rust backend, so the backend can emit events to any window.

For Phase 2 (Unity sidecar): named pipe or local WebSocket. The Rust backend spawns Unity as a child process and communicates via a simple JSON protocol:

```json
{
  "type": "expression",
  "name": "joy",
  "weight": 0.8,
  "duration_ms": 2000,
  "transition_ms": 300
}
```

---

## 5. Expression / Emotion Mapping

### 5.1 Persona Config Extension

Extend `PersonaConfig` and `personality/default.json` to include expression mappings:

```json
{
  "expressions": {
    "default": "neutral",
    "thinking": "thinking",
    "speaking": "talking",
    "happy": "joy",
    "sad": "sorrow",
    "excited": "surprise",
    "confused": "confused",
    "idle": "idle",
    "sleeping": "sleep"
  },
  "expression_blend_ms": 300,
  "idle_motion_interval_s": 15,
  "eye_tracking": true,
  "mouse_follow_strength": 0.5
}
```

### 5.2 Expression Pipeline

```
Backend Persona State
  -> Expression Mapping (from persona config)
  -> Companion Window (Tauri event)
  -> AvatarAdapter.onEvent("expression", { name, weight, duration })
  -> Live2D Cubism / Unity BlendShape
```

---

## 6. Implementation Plan

### 6.1 MVP: Placeholder Companion Window (completed)

Implemented:
1. A transparent Tauri window labeled `companion`.
2. A separate transparent dialogue window labeled `companion_dialog`.
3. A companion-only frontend view using the placeholder avatar.
4. Show/hide controls from the main window and tray.
5. Companion-owned initiative timer with `trigger = "companion_timer"`.
6. Hover toolbar for always-on-top, proactive speech, chat, dialogue, and close.
7. Drag, lower-right resize, persisted position/size, and adaptive dialogue placement.
8. Dialogue lifecycle synchronization through `companion-dialog-visible-changed`.
9. Main-window automatic proactive speech disabled.

Validation:
- Main window never emits automatic proactive messages.
- Manual sparkles button still works as user-initiated topic opening.
- Companion timer calls are blocked when `initiative.enabled = false`.
- Companion timer calls are blocked during cooldown or recent user activity.
- Dialogue direct close, companion hide, and proactive-message show paths keep Bubble state synchronized.
- `cargo fmt --manifest-path src-tauri/Cargo.toml`, `cargo test --manifest-path src-tauri/Cargo.toml`, `npm run build`, and `./frontend/node_modules/.bin/tauri build --debug --no-bundle` pass.

### 6.2 Phase 1: Live2D Companion (next)

First sub-step: add the event skeleton without adding Live2D assets yet.

1. Define a narrow companion avatar event contract:
   - `expression`
   - `motion`
   - `speak_start`
   - `speak_stop`
   - `look_at`
   - `idle`
2. Add a placeholder adapter `onEvent(...)` implementation that accepts these events as no-ops or simple CSS state changes.
3. Emit local frontend events from existing lifecycle points, for example dialogue message arrival -> `speak_start` / `speak_stop`, blocked idle timer -> `idle`.
4. Document the event payloads in `docs/ui-interface-contract.md`.
5. Only after this event contract is stable, integrate Live2D Cubism SDK for Web.

Runtime integration after the skeleton:

1. Integrate Live2D Cubism SDK for Web.
2. Implement `AvatarAdapter` with Live2D:
   - `mount(container)`: initialize Cubism framework, load model
   - `unmount()`: dispose framework
   - `onEvent(type, data)`: trigger motions/expressions
3. Add mouse tracking (character eyes follow cursor)
4. Map backend/persona/UI events to expressions and motions.

### 6.3 Phase 2: Unity + VRM (estimated 1-2 weeks, optional)

1. Unity project with VRM loader, animation controller, expression blend system
2. IPC protocol between Tauri backend and Unity process
3. Native window embedding (HWND/X11 handle sharing)
4. Expression -> BlendShape mapping
5. Camera and lighting setup for desktop overlay look
6. Build pipeline integration (Unity build -> Tauri bundle asset)

### 6.4 Prerequisites for Live2D Phase

- Live2D Cubism SDK for Web (free license for small projects)
- A Live2D model file (`.model3.json` + textures + motion files)
- Tauri window config for transparent overlay

### 6.5 Asset Acquisition

| Tier | Source | Format |
|---|---|---|
| Live2D | Live2D Marketplace, Booth.pm, commission | `.model3.json` |
| VRM | VRoid Studio (free character creator), Booth.pm | `.vrm` |
| Spine | Spine Marketplace, commission | `.json` / `.skel` |

---

## 7. References

- [Live2D Cubism SDK for Web](https://www.live2d.com/en/download/cubism-sdk/download-web/)
- [VRM Specification](https://vrm.dev/en/)
- [UniVRM (Unity VRM Loader)](https://github.com/vrm-c/UniVRM)
- [VRoid Studio](https://vroid.com/en/studio/)
- [Spine Runtime](https://esotericsoftware.com/spine-runtimes)
- [Tauri Multi-Window](https://v2.tauri.app/develop/windows/multi-window/)
- [Tauri Window Config](https://v2.tauri.app/reference/config/#windowconfig)
