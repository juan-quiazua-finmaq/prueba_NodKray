# NodKray Constitution

This constitution governs **the NodKray repository itself**.

It is not a constitution for target repositories that NodKray orchestrates.
Those live in each target's `.specify/memory/constitution.md` and must never
be mixed with these rules.

## I. Architecture

- NodKray is a local-first Rust CLI orchestrator, not an agent and not a
  replacement for Cursor, Claude, Codex, OpenCode, Pi or Spec-Kit.
- Keep a single crate at the repository root with domain modules. Extract
  crates only when a module is independently reusable.
- The CLI parses arguments and renders output. It does not speak SQL, Git or
  agent protocols. Workflows go through `ExecutionBackend` and
  `SpecKitAdapter`; they never call Herdr or `specify` directly.
- Decision, workflow, memory, execution and review stay separate engines
  behind traits and registries.

## II. Testability

- Domain logic lives in the library target so tests can exercise it without
  spawning the binary.
- Every new behaviour needs a unit or CLI test that would fail if the
  behaviour were removed.
- Integration tests run in sandboxed `NODKRAY_CONFIG_HOME` /
  `NODKRAY_DATA_HOME` directories and must not touch the user's real config.
- Required checks that are absent fail the review. They never pass silently.

## III. Deterministic Core

- Classification, thresholds, review policy and merge decisions are
  deterministic given the same inputs.
- JEV and other remote providers are optional. Their output is accepted only
  after Decision Schema validation. On failure, fall back to local reasoning.
- Session start stores a logical config snapshot plus tool versions so a run
  can be reconstructed later.

## IV. Agent Agnosticism

- Agents are adapters. Adding Gemini, Copilot or a custom worker must not
  require edits to Decision, Workflow or Review engines.
- Capabilities are declared, not assumed. A missing headless binary falls
  back to the generic adapter instead of inventing behaviour.
- `--yolo` only relaxes worker permissions. It never changes workflow,
  review or merge.

## V. Local-First Persistence

- SQLite + FTS5 is the source of truth. There is no required cloud database
  in V1.
- Memory search is project-scoped unless `--global` is explicit.
- Credentials never go into SQLite. Tokens and API keys stay in environment
  variables.

## VI. Security

- The control API is optional, disabled by default, authenticated, and bound
  to loopback (`127.0.0.1:8787`).
- The remote client contains no business logic. It only calls the Control API.
- Merge conflicts are reported, never auto-resolved.
- NodKray does not store secrets in the repository or in memory rows.

## VII. Observability

- Structured logs go to `~/.nodkray/logs/`. `--json` keeps stdout machine
  readable; logs stay on stderr.
- Every task state change is persisted with a `task_events` row in the same
  transaction.
- `nodkray status` must remain truthful after a crash.

## VIII. Compatibility

- `nodkray init` is idempotent. It may append a delimited NodKray block to
  `AGENTS.md` and `.gitignore` but never deletes existing user text.
- `nodkray uninstall` removes only NodKray-owned files. Pre-existing MCP
  markers, Spec-Kit, Herdr and other tool configs are never deleted.
- Config is additive (`version: 1`) and unknown keys are ignored.
- Optional tools (Herdr, Spec-Kit, Sentrux, JEV, remote control) never fail
  the core path when they are absent.
- Target constitutions and this constitution remain distinct files with
  distinct purposes.
