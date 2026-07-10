# Memory Core

**Status:** MVP implemented
**Storage:** `usr/memory/{role_id}/memories.json` in development. Packaged builds should map this to the system user data directory.

---

## 1. Goal

Memory turns Hestia from a stateless chat surface into a usable personal companion. The current implementation is automatic but inspectable:

```text
Chat / companion dialogue / companion initiative
-> automatic memory write
-> local storage
-> lightweight retrieval
-> prompt context injection
-> model response
```

Every successful `send_chat_message` call writes a compact turn memory for the active role. This covers both the normal chat window and the companion dialogue window because both use the same backend command. Successful proactive companion messages from `request_initiative_message` are also recorded as system-source memories.

The Memory panel remains user-editable. Users can still create, edit, pin, archive, delete, and compress memories.

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

This is not semantic retrieval. Embeddings or vector search should be added only after the automatic memory loop proves useful.

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
- compress memories with the configured remote chat API

Memory changes are immediately available to the next chat or companion initiative request.

Compression replaces the active role's memory file with model-generated compressed entries. The prompt asks the model to preserve main facts, preferences, projects, relationships, and ongoing context, merge duplicates, keep pinned information where possible, and discard a small amount of old minor detail when useful.

---

## 7. Later Work

Recommended next memory work:

1. Add tests for storage and retrieval ranking.
2. Move storage to Tauri system user data directory for packaged builds.
3. Add import/export.
4. Add optional user review for automatic memory writes if noise becomes a problem.
5. Add semantic retrieval only after the automatic workflow is stable.
