# NodKray flow tests

Use this skill to verify NodKray on **the repository you were asked to test**,
not a dummy scratch. Do not invent a nested folder inside another git tree
without its own `.git`.

If the target must stay clean, copy it literally to a new directory **outside**
the original tree (preserving `.git`) and work only there:

```bash
DEST="${TMPDIR:-/tmp}/nodkray-flow-$(date +%s)"
cp -a /path/to/requested-repo "$DEST"
cd "$DEST"
```

Then `nodkray init --yes` and `nodkray doctor` in that copy. Never create an
empty README-only scratch. Never `cd` into a folder that has no `.git` while
an ancestor repo (for example NodKray itself) would become `project_root`.

Prerequisites: `nodkray doctor` is green for Git and at least one agent. Doctor
must probe contracts (Herdr `run`, `specify version`, `sentrux check`,
`serena --help`, `codegraph version`), not only PATH. Doctor must not fail
because Serena, CodeGraph, Sentrux, Spec-Kit or Herdr are absent — those stay
optional. SDD also needs `specify` on PATH and
`.specify/memory/constitution.md` (init writes a stub if missing).

## Invent local work

Read the repo and invent **three in-tree changes** that make sense there.
Stay inside this checkout. Do not document a CLI that lives in another repo.
Do not ask the agent to leave `project.root` (the task worktree).

## 1. Environment

```bash
nodkray doctor
nodkray mcp status
```

Expect Herdr without `run` to be console fallback, not `[OK]` as a backend.
Expect Spec-Kit `[OK]` only when `specify version` works. Stages are
`/speckit.specify` … `/speckit.converge`, not `specify specify`.

## 2. ST — small change

Pick an existing file and a harmless local edit (comment, typo, docs line).

```bash
nodkray task --workflow st "<that local edit>"
nodkray status --json
nodkray status --all
```

Expect: worker edits the **worktree** (`project.root` = worktree). FAST review
then merge, or `BLOCKED` if a required check is missing (never a silent pass).
`status --json` lists only active tasks; after finish it is `[]` — use
`status --all` or `nodkray task inspect <id>`.

## 3. ODD — medium change

Invent a medium, local change (a small function or a docs section that already
belongs to this repo).

```bash
nodkray task --workflow odd "<that local change>"
```

Expect `.nodkray/tasks/<id>/task.md`. Inspect with `nodkray task inspect <id>`.

## 4. SDD — large change

Requires `.specify/memory/constitution.md` and `specify` on PATH. If `specify`
is missing the command must fail with `SPECKIT_NOT_FOUND` and the task must be
`BLOCKED`, not left in `CLASSIFYING`. Do not force ST as a fallback.

```bash
nodkray task --workflow sdd "<a large local change in this repo>"
```

When Spec-Kit is installed, expect worker stages
`/speckit.specify` → `/speckit.plan` → `/speckit.tasks` → `/speckit.implement`
→ `/speckit.converge`. Not `specify specify`.

## 5. Full integration

In one session, on this copy (or the requested repo):

1. `nodkray task "<typo-level local fix>"` (let classification pick ST)
2. `nodkray status --json` while it is active; `nodkray status --all` after
3. `nodkray review inspect <task_id>`
4. `nodkray worker list`
5. `nodkray memory search` with a token from the task title (decisions are
   searchable; `memory get <decision_id>` works)
6. `nodkray memory timeline`

A missing mandatory review check is `BLOCKED`. Isolation: if the worker
reported `changed_files` or `completed` and the worktree is clean, review
fails (the edit leaked to main). Merge conflicts are reported, never
auto-resolved.

## Cleanup

On a **copy**, `nodkray uninstall --project` removes only NodKray overlay
files. It must not delete `.serena/`, `.codegraph/`, `.sentrux/`, `.specify/`
or `.herdr/` if those already existed. Do not uninstall the original repo
unless the user asked to test in place.
