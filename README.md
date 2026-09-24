# NodKray

Orquestador local para agentes de código. NodKray no es otro agente: decide el flujo, aísla el trabajo, recuerda el proyecto y revisa antes de integrar.

```text
tú  →  Cursor / Claude / Codex / OpenCode / Pi
              ↓
          nodkray task "…"
              ↓
     ST · ODD · SDD   →   worker   →   RDD   →   merge
```

- Local-first: un binario + SQLite. Sin cuenta, sin nube obligatoria.
- Agnóstico: el agente es un adapter. El rol es lo que importa.
- Review Driven Development: un check obligatorio ausente **bloquea**. Nunca pasa en silencio.

## Requisitos

| Plataforma | Arquitecturas | Notas |
| --- | --- | --- |
| Linux | `x86_64`, `aarch64` | glibc (binario `unknown-linux-gnu`) |
| macOS | Intel y Apple Silicon | no hay build para iOS; NodKray es un CLI de escritorio |
| Windows | `x86_64` | PowerShell 5+ para `install.ps1` |

**Obligatorios para el path básico**

| Cosa | Para qué |
| --- | --- |
| Git | worktrees + merge |
| Un agente en el `PATH` | Claude, Codex, Cursor, OpenCode, Pi, o un comando generic |

**Opcionales** (el instalador los ofrece si faltan)

| Cosa | Para qué |
| --- | --- |
| `uv` | instala Spec-Kit y Serena |
| Spec-Kit (`specify`) | flujo SDD |
| Herdr | multiplexor de workers (si no está, NodKray usa Console) |
| Serena / CodeGraph / Sentrux | MCP y validación |

`nodkray doctor` marca los opcionales como `[--]`. No tumba la instalación.

## Instalación

La forma oficial es bajar el binario que publica cada release. No hace falta Rust.

### Linux / macOS

```bash
curl -fsSL https://github.com/juan-quiazua-finmaq/prueba_NodKray/releases/latest/download/install.sh | bash
```

El script detecta OS y arquitectura, instala `nodkray` en `~/.local/bin` y deja intactos MCP, Spec-Kit y Herdr.

Si `~/.local/bin` no está en el `PATH`:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

Repo o versión concretos:

```bash
NODKRAY_REPO=juan-quiazua-finmaq/prueba_NodKray NODKRAY_VERSION=v0.2.0 bash scripts/install.sh
```

### Windows

En PowerShell:

```powershell
irm https://github.com/juan-quiazua-finmaq/prueba_NodKray/releases/latest/download/install.ps1 | iex
```

O baja `nodkray-x86_64-pc-windows-msvc.zip` desde Releases y pon `nodkray.exe` en un directorio del `PATH`.

### Desde el código (desarrollo)

```bash
git clone https://github.com/juan-quiazua-finmaq/prueba_NodKray.git
cd NodKray
cargo install --path . --locked
```

Necesitas Rust stable (`rustup`).

Comprobar:

```bash
nodkray --version
```

## Primer uso

```bash
cd tu-repo
git status          # NodKray trabaja sobre un repo Git

nodkray init        # pregunta roles, ofrece MCP/Spec-Kit/Herdr si faltan
nodkray doctor
```

En un TTY, `init` pregunta:

1. alcance: global, proyecto o ambos
2. qué agente es frontera y cuál cubre cada worker (`default`, `backend`, `frontend`, `reviewer`, `docs`)
3. si instala MCP que no encuentre (Serena, CodeGraph, Sentrux)
4. si Spec-Kit no está: advierte que SDD no correrá y ofrece el último release de `github/spec-kit`
5. si también quieres Herdr

`init --yes` acepta defaults y no instala herramientas. `init --provision` las instala sin preguntar.

Si ya existía `AGENTS.md` u otras reglas, NodKray **añade** su bloque y no borra tu texto. Lo que escribe en el repo (`.nodkray/`, `skills/nodkray/`, índices MCP) queda en `.gitignore`.

Luego:

```bash
nodkray task "arregla el typo del README"
nodkray status --json
```

## Uso

NodKray clasifica cada pedido:

| Flujo | Cuándo | Qué crea |
| --- | --- | --- |
| **ST** | Cambio chico | Nada extra. Review FAST. |
| **ODD** | Alcance medio | `.nodkray/tasks/<id>/task.md` |
| **SDD** | Impacto grande | Spec-Kit (`specify` → plan → tasks → implement → converge) |

Umbrales por defecto (configurables): `0 — ST — 20 — ODD — 60 — SDD — 100`.

El worker corre aislado (Console, o Herdr si está). **RDD** (git + tests + reglas; lint/Sentrux en balanced/deep) decide el merge. Un check obligatorio ausente bloquea.

Para que un agente frontera ejercite los tres flujos y el ciclo completo, usa la skill `skills/nodkray/test.md` (se instala con `init`; no se sube al git del proyecto).

## Comandos

| Quieres… | Comando |
| --- | --- |
| Instalar config en este repo | `nodkray init` / `init --yes` / `init --provision` / `init --reconfigure` |
| Diagnosticar | `nodkray doctor` |
| Actualizar el binario | `nodkray update` |
| Quitar NodKray | `nodkray uninstall --yes` |
| Quitar también overlay del repo | `nodkray uninstall --project --yes` |
| Ejecutar una tarea | `nodkray task "describe el cambio"` |
| Forzar flujo o review | `nodkray task --workflow st\|odd\|sdd` · `--review fast\|balanced\|deep` |
| Menos prompts del worker | `nodkray task --yolo "…"` (`--yolo` no salta el review) |
| Inspeccionar / cancelar | `nodkray task inspect task_<ulid>` · `nodkray task cancel task_<ulid>` |
| Estado | `nodkray status --json` |
| Memoria | `nodkray memory search "…"` · `get` · `save` · `timeline` · `repair` |
| Review | `nodkray review --depth fast` · `review inspect <id>` |
| Workers / agentes / MCP | `nodkray worker list` · `agent list` · `mcp status` |
| Config | `nodkray config get KEY` · `config set KEY VALUE` |
| Skills / AGENTS.md | `nodkray skills` · `nodkray skills --agents-md` |
| Proyecto | `nodkray project inspect` |
| API local (apagada por defecto) | `NODKRAY_REMOTE_TOKEN=… nodkray serve --bind 127.0.0.1:8787` |

`uninstall` borra el binario, `~/.config/nodkray` y `~/.nodkray`. Con `--project` también quita `.nodkray/`, `skills/nodkray/` y los bloques que NodKray añadió a `AGENTS.md` / `.gitignore`.

**No borra** `.serena/`, `.codegraph/`, `.sentrux/`, `.specify/`, `.herdr/`, ni binarios MCP / Spec-Kit / Herdr, aunque NodKray los haya detectado o instalado. Esas configs son tuyas.

Los scripts `install.sh --uninstall` e `install.ps1 -Uninstall` solo quitan el binario.

## Dónde vive la config

| Qué | Dónde | ¿Se sube al git del proyecto? |
| --- | --- | --- |
| Config de usuario | `~/.config/nodkray/config.yaml` | no |
| Memoria y logs | `~/.nodkray/memory.db` · `~/.nodkray/logs/` | no |
| Config del repo | `.nodkray/config.yaml` | no (gitignore) |
| Skills NodKray | `skills/nodkray/` | no (gitignore) |
| Worktrees | `.nodkray/worktrees/` | no |
| Bloque en `AGENTS.md` | solo si el archivo ya existía queda a tu criterio | el archivo no se ignora si ya lo versionabas |

Prioridad: flags CLI → `NODKRAY_*` → proyecto → usuario → defaults.

```bash
export NODKRAY_EXECUTION_BACKEND=console
export NODKRAY_AGENT_FRONTIER=claude
export NODKRAY_REVIEW_DEPTH=fast
export NODKRAY_GENERIC_COMMAND="$HOME/bin/mi-worker"
export NODKRAY_REPO=owner/NodKray
```

Si `execution.backend` es `herdr` y Herdr no está, NodKray cae a Console (`fallback_console: true`).

Si SQLite no abre (lock, WAL, archivo corrupto), NodKray reintenta, hace backup y recrea. `nodkray memory repair` y `nodkray doctor` usan la misma ruta.

## Licencia

MIT. Ver `Cargo.toml`.
