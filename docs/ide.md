# JetBrains IDE Notes

## RustRover

RustRover is suitable as the main IDE for this project. JetBrains documents JavaScript and TypeScript support in RustRover, including TypeScript code analysis and TypeScript run/debug workflows.

Recommended usage:

- open the repository root
- keep Rust backend code in `src-tauri`
- keep TypeScript UI code in `frontend`
- use RustRover as the primary IDE while the Rust runtime is the main focus

## When WebStorm Still Helps

WebStorm can still be useful if the frontend becomes complex:

- advanced frontend refactoring
- design-system work
- UI testing
- framework-specific frontend workflows

For the current project stage, RustRover alone is enough.

## Project Shape

The repository uses the standard Tauri split:

```text
src-tauri/    Rust backend
frontend/     TypeScript frontend
```

This is clearer than putting everything under a single top-level `src` directory, because Tauri projects are mixed-language projects.

