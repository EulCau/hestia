# Roles

**Status:** MVP implemented
**Storage:** bundled default role in `personality/default.json`; user-created roles in `usr/roles/{id}.json`.

---

## 1. Goal

Roles define the character Hestia should play. They are separate from base runtime style rules.

```text
Role profile
-> PromptAssembler base rules + role profile
-> Chat / initiative response
-> Role-specific memory retrieval
```

The role profile describes the character. It should not contain global prompt rules such as punctuation policy or parenthetical action syntax.

---

## 2. Role Schema

```typescript
interface RoleProfile {
  schema_version: 2;
  id: string;
  name: string;
  aliases: string[];
  identity: string;
  species: string;
  appearance: string;
  avatar: {
    enabled: boolean;
    model_type: "placeholder" | "live2d" | "digital_human" | string;
    image_path: string;
  };
  personality: string;
  language_style: string;
  scenario: string;
  tone: string;
  initiative: number;
  humor: number;
  verbosity: "short" | "medium" | "long" | string;
  pinned: boolean;
}
```

`name` and `aliases` are explicitly injected into the system prompt as references to the role itself. If the user later says one of those names, the model should understand it means the character being role-played.

Role ids are restricted to ASCII letters, digits, `_`, and `-` because they are used as config file names.

`appearance` is the textual visual description used in prompts. `avatar` is the renderer-facing visual configuration. If `avatar.image_path` is present, activating the role applies that image, Live2D model, or future 3D model to the current avatar config.

---

## 3. Base Prompt Rules

Base behavior is implemented in `PromptAssembler::build_system_prompt()` rather than stored in each role:

- Chinese replies use halfwidth punctuation only: `, . ; : ? !`
- Parentheses may contain brief actions, states, tone, or expressions when appropriate.
- Current user messages override conflicting memory.
- Style must not override facts, reasoning, safety, or the current request.

The previous default role constraints such as "do not use exclamation marks" were removed from the role file.

---

## 4. Role Management

The Roles panel supports:

- select an existing role
- create a new role
- edit name, aliases, identity, species, appearance, personality, language habits, scenario, and tone
- select role-specific avatar content: image, Live2D runtime directory, or future 3D model file
- generate missing fields from the configured chat API
- save and activate
- pin
- delete with exact confirmation text

Deletion requires:

```text
我确认删除{id}
```

The bundled `default` role cannot be deleted.

Selected avatar files are copied under:

```text
usr/roles/{role_id}/avatar/
```

This keeps each role self-contained. The role JSON stores the copied path, not the original source path. Deleting a user-created role also removes its copied avatar directory.

---

## 5. Role-Specific Memory

Each role owns a separate memory file:

```text
usr/memory/{role_id}/memories.json
```

The old development path `usr/memory/memories.json` is read as a legacy fallback for the `default` role, but new writes go to the role-specific directory.

---

## 6. Automatic Role Generation

`generate_role_profile` uses the configured remote chat worker and returns a complete role JSON draft. The prompt asks the model to:

- return strict JSON
- keep global style rules out of the role profile
- fill missing fields based on identity, species, and personality
- make `name` and `aliases` clear references to the role itself

The generated profile is validated against `RoleProfile` before being returned to the frontend.

---

## 7. Later Work

Recommended next work:

1. Move `usr/roles` and `usr/memory` to the system user data directory for packaged builds.
2. Add import/export for roles and memories.
3. Add tests for role id validation, role deletion, and role-specific memory isolation.
4. Add optional model-suggested memory writes with user confirmation.
