# 🐙 NodKray

**Orquestador local para agentes de código.**  
NodKray no es otro agente: decide el flujo, aísla el trabajo, recuerda el proyecto y revisa antes de integrar.

```text
tú  →  Cursor / Claude / Codex / OpenCode / Pi
              ↓
          nodkray task "…"
              ↓
     ST · ODD · SDD   →   worker   →   RDD   →   merge
```

- 🏠 Local-first: un binario + SQLite. Sin cuenta, sin nube obligatoria.
- 🔌 Agnóstico: el agente es un adapter. El rol es lo que importa.
- ✅ Review Driven Development: un check obligatorio ausente **bloquea**. Nunca pasa en silencio.

---

## 📦 Instalar

La forma oficial es **bajar el binario que GitHub Actions compila** en cada release. No hace falta tener Rust.

### 1. Script (Linux / macOS)

```bash
curl -fsSL https://github.com/juan-quiazua-finmaq/prueba_NodKray/releases/latest/download/install.sh | bash
```

El script:

1. detecta OS y arquitectura (`x86_64` / `aarch64`)
2. descarga el artifact del **último Release**
3. instala `nodkray` en `~/.local/bin`

Si `~/.local/bin` no está en el `PATH`:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

Repo distinto o tag concreto:

```bash
NODKRAY_REPO=juan-quiazua-finmaq/prueba_NodKray NODKRAY_VERSION=v0.1.1 bash scripts/install.sh
```

### 2. Windows

En [Releases](../../releases) baja `nodkray-x86_64-pc-windows-msvc.zip`, extrae `nodkray.exe` y ponlo en un directorio del `PATH`.

### 3. Desde el código (desarrollo)

```bash
git clone https://github.com/juan-quiazua-finmaq/prueba_NodKray.git
cd NodKray
cargo install --path . --locked
```

Necesitas **Rust stable** (`rustup`). El crate pide `rust-version = 1.98`.

Comprobar:

```bash
nodkray --version
```

---

## 🚀 Primer uso (5 minutos)

```bash
cd tu-repo
git status          # NodKray trabaja sobre un repo Git

nodkray init        # config global + .nodkray/ del proyecto
nodkray doctor      # qué agentes y tools hay en el PATH
nodkray skills --agents-md   # skills + bloque en AGENTS.md (opcional)
```

`init` es **idempotente**: la segunda vez no pisa reglas, constitución ni `AGENTS.md`.

Luego, desde tu agente frontera (o la terminal):

```bash
nodkray task "arregla el typo del README"
nodkray status --json
```

Flujo que corre:

1. clasifica **ST / ODD / SDD**
2. lanza un worker (Console, o Herdr si está instalado)
3. **RDD** (git + tests + reglas; lint/Sentrux en balanced/deep)
4. merge a la rama actual si el review pasa

---

## 🧭 Cómo usarlo

### Tareas

| Quieres… | Comando |
| --- | --- |
| Ejecutar | `nodkray task "describe el cambio"` |
| Forzar flujo | `nodkray task --workflow st\|odd\|sdd "…"` |
| Forzar review | `nodkray task --review fast\|balanced\|deep "…"` |
| Menos prompts del worker | `nodkray task --yolo "…"` |
| Inspeccionar | `nodkray task inspect task_<ulid>` |
| Cancelar | `nodkray task cancel task_<ulid>` |

`--yolo` solo relaja permisos del worker. **No** salta el review ni el merge.

### Memoria (SQLite + FTS5)

```bash
nodkray memory search "autenticación"
nodkray memory get memory_<ulid>
nodkray memory save --type decision --title "SQLite es la fuente" --content "…"
nodkray memory timeline
```

La búsqueda es **del proyecto actual**. Para cruzar proyectos: `--global`.

### Review y workers

```bash
nodkray review --depth fast
nodkray review inspect task_<ulid>    # o review_<ulid>
nodkray worker list
nodkray worker inspect worker_<ulid>
```

### Agentes, MCP, config

```bash
nodkray agent list
nodkray agent inspect claude
nodkray mcp list
nodkray mcp status
nodkray config get execution.backend
nodkray config set execution.backend console
nodkray project inspect
```

### Control remoto (opcional, apagado por defecto)

```bash
export NODKRAY_REMOTE_TOKEN="un-token-largo"
nodkray serve --bind 127.0.0.1:8787
```

Solo loopback. El token vive en el entorno, **nunca** en SQLite.

---

## 🧠 Flujos

| Flujo | Cuándo | Qué crea |
| --- | --- | --- |
| ⚡ **ST** | Cambio chico, poca duda | Nada extra. Review FAST. |
| 🌿 **ODD** | Alcance medio | `.nodkray/tasks/<id>/task.md` |
| 📐 **SDD** | Impacto grande / arquitectura | Spec-Kit (`specify` → plan → tasks → implement → converge) |

NodKray **no reimplementa** Spec-Kit, Herdr, Serena, CodeGraph ni Sentrux. Los detecta y los usa si están.

Umbrales por defecto (configurables):

```text
0 ── ST ── 20 ── ODD ── 60 ── SDD ── 100
```

---

## ⚙️ Dónde vive la config

| Qué | Dónde |
| --- | --- |
| Config de usuario | `~/.config/nodkray/config.yaml` |
| Memoria y logs | `~/.nodkray/memory.db` · `~/.nodkray/logs/` |
| Config del repo | `.nodkray/config.yaml` |
| Worktrees | `.nodkray/worktrees/` |

Prioridad: flags CLI → `NODKRAY_*` → proyecto → usuario → defaults.

Variables útiles:

```bash
export NODKRAY_EXECUTION_BACKEND=console
export NODKRAY_AGENT_FRONTIER=claude
export NODKRAY_REVIEW_DEPTH=fast
export NODKRAY_GENERIC_COMMAND="$HOME/bin/mi-worker"
```

Si `execution.backend` es `herdr` y Herdr no está, NodKray cae a **Console** (`fallback_console: true`).

---

## 🧰 Dependencias

**Obligatorias para el path básico**

| Cosa | Para qué |
| --- | --- |
| Git | worktrees + merge |
| Un agente en el `PATH` | Claude, Codex, Cursor, OpenCode, Pi… o un comando generic |

**Opcionales**

| Cosa | Para qué |
| --- | --- |
| Herdr | multiplexor de workers |
| Spec-Kit (`specify`) | flujo SDD |
| Sentrux / Serena / CodeGraph | validación o MCP |

`nodkray doctor` marca los opcionales como `[--]`. No tumba la instalación.

---

## 🏭 GitHub Actions (cómo se instala de verdad)

Al subir el repo:

1. **CI** (`.github/workflows/ci.yml`) — en cada push/PR corre `cargo test --locked` en la raíz.
2. **Release** (`.github/workflows/release.yml`) — al publicar un tag `v*` (o disparar el workflow a mano) compila:

   | Target | Runner |
   | --- | --- |
   | `x86_64-unknown-linux-gnu` | Ubuntu |
   | `aarch64-unknown-linux-gnu` | Ubuntu ARM |
   | `x86_64-apple-darwin` | macOS 15 Intel (`macos-15-intel`) |
   | `aarch64-apple-darwin` | macOS 15 Apple Silicon |
   | `x86_64-pc-windows-msvc` | Windows |

   Sube los `.tar.gz` / `.zip` y un `install.sh` con el repo ya rellenado.

Publicar la primera versión:

```bash
git tag v0.1.1
git push origin v0.1.1
```

En GitHub → **Actions** → espera el job **Release** → **Releases** tendrá los binarios.

---

## 📚 Más detalle

| Doc | Contenido |
| --- | --- |
| [skills/nodkray/](skills/nodkray/) | Skills cortas para el agente frontera |

---

## 🪪 Licencia

MIT. Ver `Cargo.toml`.
