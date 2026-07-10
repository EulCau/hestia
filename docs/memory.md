# Memory Core

**Status:** MVP implemented
**Storage:** `usr/memory/{role_id}/memories.json` in development. Packaged builds should map this to the system user data directory.

---

## 1. Goal

Memory turns Hestia from a stateless chat surface into a usable personal companion. The first implementation is intentionally manual and inspectable:

```text
User-managed memory
-> local storage
-> lightweight retrieval
-> prompt context injection
-> model response
```

The model does not write memories automatically in the MVP. This avoids long-term state pollution from incorrect or overconfident extraction. Automatic memory should be added later as a suggestion workflow where the user confirms what is saved.

---

## 2. Data Model

```typescript
interface MemoryItem {
  id: string;
  kind: "fact" | "preference" | "project" | "relationship" | "note" | string;
  content: string;
  source: "chat" | "user" | "system" | string;
  confidence: number;      // clamped to [0, 1]
  created_at: number;      // unix milliseconds
  updated_at: number;      // unix milliseconds
  last_used_at?: number | null;
  pinned: boolean;
  archived: boolean;
}
```

Kinds are normalized to `fact`, `preference`, `project`, `relationship`, or `note`. Unknown kinds fall back to `note`. Sources are normalized to `chat`, `user`, or `system`; unknown sources fall back to `user`.

---

## 3. Storage

Development storage path:

```text
usr/memory/{role_id}/memories.json
```

The `usr/` directory is gitignored. This keeps user memory out of version control and matches the existing local override pattern for persona files.

Each role has its own memory file. The legacy development path `usr/memory/memories.json` is still read as a fallback for the `default` role, but new writes use the role-specific directory.

Packaging requirement:

```text
usr/memory/ -> system user data directory
```

The storage module is centralized in `src-tauri/src/memory.rs`, so the packaged path migration should be implemented there.

---

## 4. Retrieval

Chat and companion initiative requests retrieve a small memory set for the active role before prompt assembly:

- normal chat: up to 8 memories
- companion initiative: up to 6 memories

Selection is deliberately simple:

1. Exclude archived memories.
2. Include pinned memories.
3. Score content and kind by exact query substring and keyword overlap.
4. Sort by score, pinned status, and recent update time.
5. Mark selected memories with `last_used_at`.

This is not semantic retrieval. Embeddings or vector search should be added only after the manual memory loop proves useful.

---

## 5. Prompt Injection

Relevant memories are injected as an additional system message after the base assistant system prompt:

```text
Relevant long-term memory. Use only when it helps answer the current request.
If it conflicts with the current user message, prefer the current user message.
- [preference, pinned] ...
- [project] ...
```

This keeps memory separate from chat history and makes the conflict rule explicit.

---

## 6. UI

The main sidebar includes a Memory button. The panel supports:

- search
- include archived toggle
- create memory
- edit memory
- pin/unpin
- archive/restore
- delete

Memory changes are immediately available to the next chat or companion initiative request.

---

## 7. Later Work

Recommended next memory work:

1. Add tests for storage and retrieval ranking.
2. Move storage to Tauri system user data directory for packaged builds.
3. Add import/export.
4. Add model-suggested memories with explicit user confirmation.
5. Add semantic retrieval only after the manual workflow is stable.
