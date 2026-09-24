# NodKray V1 — Fases 3–5 (rama `feat/phases-3-4-5`)

## Owner

Agente en worktree `/home/juan-manuel-quiazua/Documentos/NodKray-phases-345`.

No edita el working tree de `main` (`/home/juan-manuel-quiazua/Documentos/NodKray/nodkray`).

## Snapshot de interfaces (no reescribir)

Tomado del scaffold de fases 0–2 el 2026-09-24:

- `AgentAdapter`: `name`, `executable`, `detect() -> Option<PathBuf>`, `capabilities` (extendido de forma aditiva)
- `ExecutionBackend`: `name`, `available` + métodos de workspace/worker con default
- Config schema §62, error model §113/§125, CLI clap global flags
- Paths: `~/.config/nodkray/config.yaml`, `~/.nodkray/`, `.nodkray/config.yaml`

## Hecho

- Fase 3: ODD `task.md`, SpecKitAdapter, constitución detect/reuse, review depth
- Fase 4: adapters Cursor/OpenCode/Claude/Codex/Pi/Generic, registry, HerdrBackend + fallback console, DAG, roles, worktrees
- Fase 5: RDD (git/tests/lint/rules/sentrux/reviewer), verdict+policy, MCP discovery, skills, AGENTS.md idempotente
- CLI: `agent`, `task run --workflow`, `review`, `worker`, `mcp`, `skills`
- Tests: `cargo test` verde (66 unit + 12 CLI)

## Merge notes para fases 0–2

Conflictos esperados al integrar:

- `src/agents/traits.rs` y adapters (ellos: Generic+Claude ST; nosotros: matriz §8 + comandos)
- `src/execution/console.rs` y `traits.rs` (ellos: spawn real; nosotros: trait extendido + fallback)
- `src/cli/task.rs`, `agent.rs`, `review.rs`, `cli/mod.rs`
- `src/config/schema.rs` (campos aditivos: `review.thresholds`, `policy.lint_failure`, `agent.mcp`)
- `src/review/*` (ellos: FAST; nosotros: engine completo)

No tocar su SQLite, init, doctor ni ST runner. Unir traits de forma aditiva.
