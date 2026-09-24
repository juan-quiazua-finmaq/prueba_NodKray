# ⚡ NodKray

> **Orquestador local para agentes de Inteligencia Artificial.** 🧠  
> NodKray no es otro agente más: es el director de orquesta que decide el flujo de trabajo, aísla el código en ramas limpias, recuerda el contexto de tu proyecto y valida rigurosamente los cambios antes de integrarlos.

```text
       🧑‍💻 Tú
         │
         ▼
🤖 Claude / Codex / Cursor / OpenCode / Pi
         │
         ▼
   ⚡ nodkray task "..."
         │
 ┌───────┴────────────────────────┐
 │ 📐 ST / ODD / SDD              │  (Clasificación según complejidad)
 │ 👷 Worker aislado (Worktree)   │  (Sin alterar tu rama actual)
 │ 🛡️ RDD (Review obligatorio)   │  (Git diff + Tests + Reglas)
 └───────┬────────────────────────┘
         ▼
      🚀 Merge seguro
```

---

### ✨ ¿Por qué NodKray?

- 🔒 **100% Local & Privado:** Funciona con un único binario y SQLite local. Sin cuentas, sin nube obligatoria ni telemetría oculta.
- 🧩 **Agnóstico y Flexible:** Tú eliges qué agente de IA usar (Claude, Cursor, Codex, OpenCode, Pi, o tus propios scripts).
- 🛡️ **RDD (Review-Driven Development):** El código nunca se mezcla a ciegas. Si falla una prueba, un lint o una regla esencial, el cambio se bloquea automáticamente.
- 🌳 **Trabajo Aislado:** Ejecuta a los agentes en *Git worktrees* temporales para que tu espacio de trabajo principal se mantenga siempre intacto mientras el agente programa.

---

### 📋 Requisitos

Solo necesitas contar con tres herramientas básicas:

1. 🐙 **Git:** Para gestionar el repositorio, los worktrees aislados y los merges.
2. ⚡ **[uv](https://docs.astral.sh/uv/):** Gestor de paquetes ultrarrápido para Python, fundamental para ejecutar herramientas del ecosistema.
3. 🤖 **Un Agente de IA:** Cualquiera que esté en tu `PATH` o terminal (Claude Code, Cursor CLI, Codex, OpenCode, Pi o un comando propio).

> 💡 **¿Y las demás herramientas (Spec-Kit, Serena, Herdr, MCPs)?**  
> **¡No necesitas instalarlas a mano!** 🎉 NodKray se encarga automáticamente de detectarlas e instalarlas en caso de que no las encuentre en tu equipo cuando ejecutes `nodkray init`.

---

### 🚀 Instalación Rápida

Instala el binario oficial listo para usar (sin necesidad de tener Rust instalado):

#### 🐧 Linux & 🍎 macOS
```bash
curl -fsSL https://github.com/juan-quiazua-finmaq/prueba_NodKray/releases/latest/download/install.sh | bash
```
*(Si `~/.local/bin` no está en tu `PATH`, agrégalo a tu archivo `~/.bashrc` o `~/.zshrc`)*

#### 🪟 Windows
```powershell
irm https://github.com/juan-quiazua-finmaq/prueba_NodKray/releases/latest/download/install.ps1 | iex
```

*(Opcional: Si prefieres compilar desde el código fuente con Rust: `cargo install --path . --locked`)*

---

### 🏁 Inicio Rápido en 3 Pasos

1. **Entra a tu repositorio:**
   ```bash
   cd tu-proyecto
   ```

2. **Inicializa NodKray en el proyecto:**
   ```bash
   nodkray init
   ```
   *El asistente te preguntará tus preferencias de agentes y aprovisionará automáticamente cualquier herramienta o MCP faltante.*

3. **¡Lanza tu primera tarea!**
   ```bash
   nodkray task "corrige el cálculo de precios y agrega pruebas unitarias"
   ```

---

### ⚙️ ¿Cómo funciona?

NodKray evalúa el alcance de cada tarea y selecciona la estrategia adecuada:

| Flujo | 🎯 Alcance | 📝 ¿Qué hace? |
| :--- | :--- | :--- |
| **ST** *(Small Task)* | Tareas chicas | Arreglos directos y typos. Ejecución rápida y revisión inmediata (**FAST**). |
| **ODD** *(Output-Driven)* | Alcance medio | Diseña un plan estructurado en `.nodkray/tasks/<id>/task.md` antes de implementar. |
| **SDD** *(Spec-Driven)* | Gran impacto | Ciclo profundo con Spec-Kit (`specify` ➔ plan ➔ tareas ➔ implementación ➔ convergencia). |

🛡️ **El candado RDD:** Cuando el worker concluye, NodKray audita el `git diff`, corre la suite de pruebas y verifica tus políticas de código. Si algo no pasa, **el merge se bloquea**. Nada roto llega a producción o a tu rama principal.

---

### 🛠️ Comandos Principales

| Acción | Comando |
| :--- | :--- |
| 🚀 **Ejecutar una tarea** | `nodkray task "describe aquí el cambio"` |
| 🎛️ **Forzar flujo o review** | `nodkray task "..." --workflow st\|odd\|sdd --review fast\|balanced\|deep` |
| ⚡ **Modo desatendido** | `nodkray task --yolo "..."` *(reduce interacciones sin saltarse el review)* |
| 📊 **Consultar estado** | `nodkray status` |
| 🩺 **Diagnóstico del entorno** | `nodkray doctor` |
| 🧠 **Buscar en la memoria** | `nodkray memory search "texto o palabra clave"` |
| 🔍 **Inspeccionar revisiones** | `nodkray review inspect <id>` |
| 🤖 **Ver agentes y workers** | `nodkray agent list` · `nodkray worker list` |
| 🧩 **Estado de servidores MCP** | `nodkray mcp status` |
| 🔄 **Actualizar binario** | `nodkray update` |
| 🗑️ **Desinstalar** | `nodkray uninstall --yes` |

---

### 📁 ¿Dónde se guarda la información?

NodKray mantiene todo aislado para no ensuciar tu repositorio:

- `~/.config/nodkray/config.yaml`: Tu configuración global de usuario.
- `~/.nodkray/memory.db`: Memoria SQLite persistente con búsqueda semántica y de texto completo.
- `.nodkray/`: Configuración del proyecto y worktrees aislados *(ignorado en `.gitignore`)*.
- `skills/nodkray/`: Habilidades que usan tus agentes para comunicarse con NodKray *(ignorado en `.gitignore`)*.

---

### 📄 Licencia

Licencia [MIT](Cargo.toml).
