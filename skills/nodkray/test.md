# NodKray flow tests

Use this skill to verify NodKray in a **scratch git repo**, not in a customer's
tree. Do not commit the files NodKray writes (they are gitignored).

Prerequisites: `nodkray doctor` is green for Git and at least one agent. SDD
also needs `specify` on `PATH`.

## 1. Environment

```bash
nodkray doctor
nodkray mcp status
```

Doctor must not fail because Serena, CodeGraph, Sentrux, Spec-Kit or Herdr are
absent. Those are optional.

## 2. ST — small change

```bash
nodkray task --workflow st "add a harmless comment in README"
nodkray status --json
```

Expect a short cycle: worker → FAST review → merge (or BLOCKED if a required
check is missing — that is correct, never a silent pass).

## 3. ODD — medium change

```bash
nodkray task --workflow odd "document the public CLI in README"
```

Expect `.nodkray/tasks/<id>/task.md`. Inspect with `nodkray task inspect <id>`.

## 4. SDD — large change

```bash
nodkray task --workflow sdd "propose a new execution backend"
```

If `specify` is missing the command must fail with `SPECKIT_NOT_FOUND`. Do not
force ST as a fallback. When Spec-Kit is installed, expect the specify → plan →
tasks → implement → converge stages.

## 5. Full integration

In one session:

1. `nodkray task "typo-level README fix"` (let classification pick ST)
2. `nodkray status --json`
3. `nodkray review inspect <task_id>`
4. `nodkray worker list`
5. `nodkray memory search "README"`
6. `nodkray memory timeline`

A missing mandatory review check is `BLOCKED`. Merge conflicts are reported,
never auto-resolved.

## Cleanup

`nodkray uninstall --project` removes only NodKray overlay files. It must not
delete `.serena/`, `.codegraph/`, `.sentrux/`, `.specify/` or `.herdr/` if those
already existed.
