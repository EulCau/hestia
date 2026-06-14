# Architecture Notes

## 1. Core Boundary

The core architecture is:

```text
UI / Event Source
-> Runtime Command
-> Job
-> Scheduler
-> Worker
-> Result
-> Event Log / State Update
```

The runtime owns state. Workers own only their local execution lifecycle.

## 2. Job State Machine

Allowed states:

```text
queued
waiting_resource
running
completed
failed
cancelled
timeout
```

Suggested transitions:

```text
queued -> waiting_resource
queued -> running
waiting_resource -> running
running -> completed
running -> failed
running -> cancelled
running -> timeout
waiting_resource -> cancelled
queued -> cancelled
```

Invalid transitions should be rejected and logged.

## 3. Worker Contract

Each worker should expose:

```text
load
unload
infer
interrupt
health_check
capabilities
resource_requirements
```

The concrete implementation may be HTTP, local process, or in-process library, but the scheduler should not care.

## 4. Observability Contract

Every Job should produce a timeline:

```text
created
queued
resource_wait_started
started
worker_selected
completed_or_failed
```

Prompt logs should record:

- raw user request
- selected persona
- assembled prompt
- final worker request
- redacted secrets

VRAM logs should record:

- active local model
- load start and end
- unload start and end
- failure reason
- approximate VRAM before and after, if available

## 5. Configuration Principle

Configuration must be typed and validated.

Avoid:

```text
read arbitrary JSON everywhere
```

Prefer:

```text
load config -> validate -> runtime config object -> read-only access by modules
```

## 6. Security and Privacy Principle

Screenshots and prompt logs are sensitive. The default should be conservative:

- screenshot capture off by default
- prompt logs can be disabled
- logs should redact API keys
- user should know when screen analysis is active

