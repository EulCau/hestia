# AGENTS.md

## User Profile

The user is a computational mathematics student focusing on:

- High-dimensional PDEs
- BSDE-based numerical methods
- Random Feature Method (RFM)
- Scientific computing and numerical analysis
- Deep learning methods for PDEs

---

## General Principles

### 1. Mathematical Rigor
- Always prioritize correctness over intuition
- Explicitly state assumptions when needed

### 2. Clarity Over Cleverness
- Prefer simple and explicit formulations
- Avoid unnecessary abstraction

### 3. No Silent Assumptions
- Do NOT assume missing definitions
- If information is missing, ask or state uncertainty

---

## Coding Guidelines

### 1. Minimal Change Principle
- Make the smallest change necessary
- Do NOT refactor unrelated code
- Do NOT introduce new abstractions unless required

### 2. Tensor / Numerical Awareness
- Always be explicit about: tensor shape, dtype, device (CPU / CUDA)
- Avoid implicit broadcasting unless verified

### 3. Numerical Stability
- Avoid explicit matrix inversion
- Prefer stable methods (solve, lstsq)

### 4. Performance Awareness
- Avoid unnecessary CPU-GPU synchronization
- Do NOT optimize prematurely

---

## Hestia Project Rules

### Architecture Rule
- Models do not call models directly. All work becomes a Job and goes through the Scheduler.
- Workers implement a capability contract and must not own global application state.
- Personality is application state, not model state.

### Rust Conventions
- Use `tracing` crate for all logging (info!, warn!, error!)
- Config loading goes through `config::load_config()` which deep-merges user.toml over default.toml
- New Tauri commands are registered in `src/lib.rs` invoke_handler
- Worker implementations go in `src/workers/`

### Frontend Conventions
- All communication with backend uses `invoke()` from `@tauri-apps/api/core`
- Theme colors are CSS custom properties on `:root` and `[data-theme="light"]`
- Settings are persisted via `invoke("update_settings", {updates})` which writes to user.toml

---

## Maintenance Rules

### UI Interface Contract
When any of the following changes occur in the Rust backend, **you must update** [docs/ui-interface-contract.md](/home/eulcau/CXTX/hestia/docs/ui-interface-contract.md):

1. A new `#[tauri::command]` is added, removed, or its signature changes
2. `config_snapshot()` in `src/config.rs` changes its output shape (ConfigSnapshot)
3. A new config section is added to `config/default.toml`
4. `AppState` struct changes (new fields that the frontend may reference)
5. Worker capabilities or types change

The update should:
1. Add/update the command in the "Tauri Commands" section
2. If ConfigSnapshot shape changed, update the TypeScript interface definition
3. If new UI state transitions were introduced, update the "UI State Machine" section
4. Bump the "Last updated" date at the top of the document

### Documentation
- [docs/implementation-summary.md](/home/eulcau/CXTX/hestia/docs/implementation-summary.md) records what has been built per phase
- [docs/ui-interface-contract.md](/home/eulcau/CXTX/hestia/docs/ui-interface-contract.md) is the frontend-backend API contract
- [docs/work-order.md](/home/eulcau/CXTX/hestia/docs/work-order.md) defines the planned phases

---

## Communication Style
- Be precise and direct
- Avoid unnecessary verbosity
- Provide structured explanations when needed

---

## What NOT to Do
- Do NOT invent mathematical definitions
- Do NOT silently modify equations
- Do NOT change problem formulation without notice
- Do NOT introduce unrelated dependencies
- Do NOT over-refactor code
