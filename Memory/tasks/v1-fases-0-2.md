# NodKray V1 — Fases 0–2 (ST slice)

## Objective

Repo git (rama `main`) con un crate binario `nodkray/` que implementa las fases 0,
1 y 2 de `Docs/plan.md` según el diseño de `Docs/NodKrayDocs.md`:
scaffold (CLI/config/errors/logging/doctor), persistencia SQLite+FTS5 con
`memory` CLI, y la rebanada ST end-to-end (decisión local → ConsoleBackend →
worker headless → FAST RDD → merge explícito), recuperable tras crash.

## Scope

- Fase 0: `nodkray init --global|--project` idempotente, `doctor`, `config get|set`, `status` vacío, `--json/--quiet/--verbose`, exit codes §125, error model §113, logging a `~/.nodkray/logs/`, carga de config CLI > env > project > user > defaults, schema YAML §62.
- Fase 1: DB `~/.nodkray/memory.db`, migraciones, tablas §48–§54 + `task_events`, `workers`, `reviews`, `review_checks`, `project_rules`; FTS5 con triggers; `memory save|search|get|timeline`; proyecto por cwd→repo root; task/events persistidos; `task inspect`; `status` desde SQLite.
- Fase 2: traits `AgentAdapter`/`ExecutionBackend`; adapters Generic + Claude (`-p`); `ConsoleBackend` (spawn, cwd worktree); decision local (score §19, umbrales `st_max: 20`/`odd_max: 60`, >20 → warning + forced ST); contratos JSON §75–§77; FAST RDD (git diff + tests + reglas locales); policy ausente = FAILED/BLOCKED; merge por NodKray, conflicto → CONFLICT; `--yolo`; snapshot de config por sesión.

## Out of scope

- Fases 3–6: ODD task.md/SDD/Spec-Kit, Herdr, paralelismo DAG, validators completos, MCP discovery, remote API, JEV, skills/AGENTS.md, Sentrux.
- Fuente de ArchitekturTemplate/es: no modificar `Docs/` salvo lo pedido.

## Constraints

- Un solo crate binario `nodkray/` (sin workspace multi-crate).
- CLI nunca habla SQL/Git/worker directo: pasa por ApplicationCore → engines.
- Deps: `clap`, `serde`+`serde_yaml`, `thiserror`, `tracing`+`tracing-subscriber`, `directories`, `ulid`, `chrono`, `rusqlite` (bundled, para FTS5).
- Rust estable local; Linux primero; `agent list` con `detect` stub para no-Claude.
- Sin commits más allá de los que el usuario pidió explícitamente (solo git init/branch; commits de fases permitidos para proyecto nuevo — confirmar).

## Acceptance Criteria

- [ ] AC-1: `git init` + rama `main` hechos; crate compila (`cargo test` verde).
- [ ] AC-2: `nodkray init` dos veces produce mismo estado lógico (test de Fase 0).
- [ ] AC-3: `nodkray doctor` no falla el proceso si Herdr/Spec-Kit/Sentrux ausentes (`[--]`).
- [ ] AC-4: `nodkray status --json` es JSON válido; exit codes por categoría §125.
- [ ] AC-5: `nodkray memory save|search|get|timeline` funcionan aislados por proyecto; FTS5 sync por triggers.
- [ ] AC-6: en repo de prueba, `nodkray task "..."` completa ST: classify → worker (Claude o generic) → review → merge; si score > 20 → warning y ejecuta ST forzado.
- [ ] AC-7: matar el proceso a mitad de task: `nodkray status` sigue mostrando la tarea (estado persistido en SQLite).
- [ ] AC-8: check obligatorio ausente → estado FAILED/BLOCKED, nunca pass silencioso; conflicto de merge → CONFLICT, no resuelto automaticamente.

## Plan

- [ ] T-0 git init + rama main + commit inicial de Docs.
- [ ] T-1 Scaffold: Cargo.toml, module tree §6 vacíos, clap, exit codes, error model, config loader (schema §62), logging, project discovery.
- [ ] T-2 Fase 0: `init` global/project idempotente + detección; `doctor`; `config get|set`; `status` vacío; `--json/--quiet/--verbose`; test idempotencia.
- [ ] T-3 Fase 1: SQLite + migraciones + FTS5 + `MemoryRepository`; `memory save|search|get|timeline`; persistencia de tasks/events; `task inspect`; `status` desde DB.
- [ ] T-4 Fase 2: decision local + traits AgentAdapter/ExecutionBackend + Generic/Claude adapters + ConsoleBackend + worktree por tarea + FAST RDD (git/tests/reglas) + policy + merge/CONFLICT + `--yolo` + snapshot config.
- [ ] T-5 Integración E2E en repo de prueba (DoD fase 2): recover test (kill proceso), `--review fast`, exit codes.

## Evidence

- AC-1..AC-8 verificados: 88 tests verdes (66 lib + 14 cli + 8 st), build sin warnings, smoke E2E en repo temporal mock.
- files: `nodkray/` con src/{cli,core,agents,execution,memory,review,spec,mcp,config,installer}, migraciones 001+002, tests/cli.rs, tests/st.rs.

## Progress

Current: done

## Next

Fase 3 (ODD/SDD) es el siguiente paso del plan — requiere aprobación del usuario.

## Notes

- Paths decididos por plan.md (§47 vs §61 conflicto): config global `~/.config/nodkray/config.yaml`; datos `~/.nodkray/`; proyecto `.nodkray/config.yaml`.
- Fase 2 sin Spec-Kit: si score > `st_max`, task se rechaza o fuerza ST con warning (no ODD/SDD aún).
