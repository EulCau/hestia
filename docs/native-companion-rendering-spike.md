# Native Companion Rendering Spike

**Date:** 2026-06-16
**Status:** investigation complete, implementation not started

## Problem

The Live2D model renders correctly in the main chat window, but the transparent desktop companion window leaves motion trails on Wayland.

Observed context:
- `$XDG_SESSION_TYPE = wayland`
- Main window is non-transparent and does not show trails.
- Companion window uses Tauri/WebKitGTK with `transparent = true`.
- Pixi/WebGL already clears the color buffer and uses transparent renderer settings.

Conclusion: the remaining artifact is most likely in the Wayland compositor / WebKitGTK transparent-window composition path, not in Live2D motion playback itself.

## Checked Sidecar Options

Local runtime availability:

| Candidate | Available | Notes |
|---|---:|---|
| Chromium / Chrome | No | Would be the easiest browser sidecar to test transparent WebGL outside WebKitGTK. |
| Electron | No | Same as Chromium, but would add a large runtime dependency. |
| GTK 3 / GTK 4 | Yes | Can create native transparent windows, but does not solve Live2D `.model3.json` rendering by itself. |
| WebKitGTK | Yes | Same rendering stack as Tauri on Linux, so it is unlikely to fix this artifact. |

## Feasibility

### Native Live2D Sidecar

Not a small change.

A true native renderer needs:
1. Live2D Cubism Native SDK / Core for Linux.
2. C++ or Rust bindings around Cubism model loading, motion playback, expression blending, and physics.
3. A native transparent window/rendering stack, for example winit + OpenGL/wgpu.
4. IPC from Hestia to the sidecar for the existing avatar events:
   - `expression`
   - `motion`
   - `speak_start`
   - `speak_stop`
   - `look_at`
   - `idle`

This is the cleanest long-term solution, but it is a dedicated subproject rather than a quick patch.

### Chromium/Electron Sidecar

Medium effort.

This keeps the current TypeScript/Pixi/Live2D implementation but moves rendering out of WebKitGTK. It may avoid the Wayland transparent WebKit artifact if Chromium handles the compositor path better.

Required work:
1. Add a sidecar command configuration, for example `[companion.sidecar]`.
2. Spawn the sidecar from the Rust backend.
3. Serve or load the existing companion route `/?view=companion`.
4. Forward avatar events through local IPC or a localhost WebSocket.
5. Keep models/jobs in Hestia; the sidecar must be render-only.

Risk: if Chromium/Electron is not bundled, behavior depends on the user's installed browser.

### Unity / VRM Sidecar

High effort.

This is more suitable if the project later moves to 3D VRM. It is not the fastest way to fix the current Live2D artifact.

## Recommended Next Step

Do not continue adding WebKit transparent-window workarounds unless a specific compositor/backend target is chosen.

Recommended order:
1. Add a debug switch that makes the companion window non-transparent. This confirms the artifact boundary for users.
2. Add a render backend config enum:
   - `webview_live2d`
   - `chromium_sidecar_live2d`
   - `native_live2d` (future)
   - `unity_vrm` (future)
3. Prototype `chromium_sidecar_live2d` only after choosing whether Chromium/Electron should be bundled or user-provided.
4. Treat true native Live2D as a separate phase requiring Cubism Native SDK integration.

## Current Contract To Preserve

The renderer, whether WebView or sidecar, must consume the same avatar event contract documented in `docs/ui-interface-contract.md`.

The renderer must not call LLMs or own global application state. Model work remains:

```text
User/UI event -> Job -> Scheduler -> Worker -> result -> avatar/render event
```
