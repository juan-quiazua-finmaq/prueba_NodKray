# NodKray

> **Orquestador ligero, agnóstico de agente y orientado a ejecución de software mediante IA.**
>
> NodKray conecta agentes frontera, agentes trabajadores, metodologías de desarrollo, memoria persistente, herramientas MCP y mecanismos de revisión en una única capa de coordinación. Su propósito no es reemplazar los agentes existentes ni duplicar sus herramientas, sino determinar **qué debe ocurrir, quién debe hacerlo, cómo aislarlo y cómo verificarlo**.

---

# 1. Carta de presentación

NodKray nace para resolver un problema concreto de los entornos modernos de desarrollo asistido por IA:

> Los agentes de código ya son suficientemente capaces para ejecutar tareas complejas, pero cada herramienta administra de forma distinta el contexto, los subagentes, las herramientas, la memoria, las reglas del repositorio y los mecanismos de ejecución.

NodKray introduce una capa pequeña por encima de esas herramientas.

El usuario entrega una tarea a un **agente frontera**. NodKray recibe esa intención, determina el esfuerzo requerido y selecciona uno de tres flujos:

* **ST — Simple Task**
* **ODD — Organic Driven Development**
* **SDD — Spec Driven Development**

Después, el flujo seleccionado puede paralelizar trabajo entre agentes trabajadores utilizando **Herdr o una ejecución directa por consola**, cada uno con aislamiento mediante worktrees cuando corresponda.

Al finalizar, NodKray ejecuta **RDD — Review Driven Development**, cuyo propósito es comprobar que el resultado satisface las reglas del repositorio, el alcance solicitado y las verificaciones configuradas antes de integrar los cambios.

La memoria persistente se mantiene local mediante **SQLite + FTS5** y se consume exclusivamente mediante el CLI de NodKray. No existe una TUI propia en la primera versión.

Las herramientas especializadas existentes —Serena, CodeGraph, Sentrux y otras— permanecen como herramientas externas. NodKray las integra y coordina, pero no las replica.

Spec-Kit constituye la implementación del flujo SDD. Su `constitution.md` se crea una vez por proyecto y posteriormente funciona como fuente de reglas para las ejecuciones SDD. El flujo formal de Spec-Kit sigue la estructura actual de especificación, planificación, tareas, implementación y convergencia.

El resultado es un sistema cuyo núcleo puede permanecer pequeño:

```text
                     ┌────────────────────┐
                     │       USUARIO      │
                     └─────────┬──────────┘
                               │
                               ▼
                    ┌──────────────────────┐
                    │    AGENTE FRONTERA   │
                    │ Cursor / OpenCode /  │
                    │ Claude / Codex / Pi  │
                    └──────────┬───────────┘
                               │
                               ▼
                    ┌──────────────────────┐
                    │       NODKRAY        │
                    │                      │
                    │ Decision             │
                    │ Workflow             │
                    │ Memory               │
                    │ Execution            │
                    │ Review               │
                    └──────────┬───────────┘
                               │
             ┌─────────────────┼─────────────────┐
             │                 │                 │
             ▼                 ▼                 ▼
            ST                ODD               SDD
             │                 │                 │
             │                 │              Spec-Kit
             │                 │                 │
             └─────────────────┼─────────────────┘
                               │
                               ▼
                       Execution Backend
                        /             \
                     Herdr           Console
                        │                │
                        └──────┬─────────┘
                               ▼
                       Worker Agents
                               │
                               ▼
                           Worktrees
                               │
                               ▼
                            RDD Review
                               │
                       ┌───────┴────────┐
                       ▼                ▼
                     PASS              FAIL
                       │                │
                       ▼                ▼
                     Merge          Remediation
```

---

# 2. Objetivos

## 2.1 Objetivo principal

Construir un ejecutable local capaz de:

1. integrar diferentes agentes frontera;
2. seleccionar automáticamente el flujo de trabajo;
3. ejecutar tareas simples, orgánicas o formales;
4. delegar trabajo a agentes trabajadores;
5. paralelizar tareas aisladas;
6. administrar worktrees;
7. conservar memoria persistente del proyecto;
8. integrar herramientas MCP externas;
9. aplicar las reglas del proyecto;
10. ejecutar revisiones automatizadas;
11. integrar resultados aprobados;
12. funcionar sin depender de un proveedor específico de IA.

---

# 3. Principios arquitectónicos

NodKray deberá cumplir los siguientes principios.

## 3.1 Agnosticismo de agente

El core no conocerá internamente detalles específicos de Cursor, OpenCode, Claude, Codex o Pi.

Cada herramienta se implementará mediante un adapter.

```text
                 Agent Interface
                       │
       ┌───────────────┼────────────────┐
       ▼               ▼                ▼
   CursorAdapter   OpenCodeAdapter   CodexAdapter
       │               │                │
       ▼               ▼                ▼
     Cursor          OpenCode          Codex
```

Los adapters serán reemplazables.

---

## 3.2 Roles, no herramientas

La configuración de NodKray se expresará mediante **roles**.

Ejemplo:

```yaml
roles:
  frontier:
    agent: claude

  workers:
    backend:
      agent: codex

    frontend:
      agent: cursor

    reviewer:
      agent: opencode

    documentation:
      agent: pi
```

La responsabilidad pertenece al rol.

El agente concreto es una implementación intercambiable.

---

## 3.3 NodKray no reemplaza herramientas especializadas

NodKray no debe duplicar:

* CodeGraph;
* Serena;
* Sentrux;
* Spec-Kit;
* Herdr;
* capacidades propias de los agentes.

Por ejemplo, CodeGraph ya proporciona un servidor MCP y mantiene `codegraph_explore` como herramienta principal, además de equivalentes CLI para escenarios no-MCP.

NodKray deberá integrarlos cuando estén presentes, no crear wrappers redundantes.

---

## 3.4 Progressive Disclosure

El sistema no deberá entregar todo el contexto disponible a cada agente.

La información se descubrirá bajo demanda:

```text
Estado mínimo
     │
     ▼
Necesidad concreta
     │
     ▼
CLI / MCP
     │
     ▼
Información relevante
```

---

## 3.5 Fuente única de verdad

Cada componente deberá tener una responsabilidad clara:

| Información                    | Fuente de verdad                  |
| ------------------------------ | --------------------------------- |
| Reglas del proyecto SDD        | `.specify/memory/constitution.md` |
| Especificación SDD             | `spec.md`                         |
| Diseño técnico SDD             | `plan.md`                         |
| Tareas SDD                     | `tasks.md`                        |
| Estado persistente NodKray     | SQLite                            |
| Configuración NodKray          | archivo de configuración          |
| Estado Git                     | Git                               |
| Resultado arquitectónico       | RDD                               |
| Reglas arquitectónicas Sentrux | `.sentrux/rules.toml`             |
| Estado de workers              | Execution Backend                 |

---

# 4. Alcance funcional

## 4.1 Incluido

NodKray V1 deberá incluir:

* instalador;
* CLI;
* configuración;
* detección del entorno;
* adapters de agentes;
* selección ST/ODD/SDD;
* workflow ST;
* workflow ODD;
* integración Spec-Kit;
* memoria SQLite + FTS5;
* ejecución mediante Herdr;
* ejecución mediante consola;
* agentes trabajadores;
* worktrees;
* RDD;
* integración opcional de Sentrux;
* descubrimiento de reglas del repositorio;
* integración MCP;
* registro de sesiones;
* soporte de JEV opcional;
* detección y adaptación de Constitution;
* modo `--yolo`;
* ejecución local;
* control remoto mediante una interfaz de control definida por NodKray.

## 4.2 Fuera del alcance V1

No forman parte del núcleo inicial:

* TUI propia para memoria;
* dashboard web obligatorio;
* sustitución de MCP existentes;
* proveedor de IA propio;
* servidor de inferencia propio;
* base de datos remota obligatoria;
* Turso como almacenamiento primario;
* sistema de billing;
* sistema de cuentas;
* marketplace propio;
* interfaz gráfica de escritorio.

---

# 5. Arquitectura general

```text
┌─────────────────────────────────────────────────────────┐
│                         NODKRAY                         │
│                                                         │
│  ┌────────────┐   ┌────────────┐   ┌────────────────┐  │
│  │ CLI        │   │ Config     │   │ Remote Control │  │
│  └─────┬──────┘   └─────┬──────┘   └───────┬────────┘  │
│        │                │                  │           │
│        └────────────────┼──────────────────┘           │
│                         ▼                              │
│                 ┌───────────────┐                      │
│                 │ Application   │                      │
│                 │ Core          │                      │
│                 └───────┬───────┘                      │
│                         │                              │
│      ┌──────────────────┼───────────────────┐          │
│      ▼                  ▼                   ▼          │
│ Decision            Workflow             Memory        │
│ Engine              Engine               Engine        │
│      │                  │                   │          │
│      └──────────────────┼───────────────────┘          │
│                         ▼                              │
│                  Execution Engine                       │
│                         │                              │
│                  ┌──────┴──────┐                       │
│                  ▼             ▼                       │
│                Herdr         Console                   │
│                  │             │                       │
│                  └──────┬──────┘                       │
│                         ▼                              │
│                       Agents                           │
│                                                         │
│  ┌─────────────┐ ┌─────────────┐ ┌──────────────────┐ │
│  │ SDD Adapter │ │ MCP Manager │ │ RDD / Validators │ │
│  └─────────────┘ └─────────────┘ └──────────────────┘ │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

---

# 6. Estructura interna del proyecto

La implementación deberá separar CLI, dominio, infraestructura y adapters.

```text
nodkray/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── init.rs
│   │   ├── task.rs
│   │   ├── memory.rs
│   │   ├── review.rs
│   │   ├── agent.rs
│   │   ├── config.rs
│   │   └── status.rs
│   │
│   ├── core/
│   │   ├── decision/
│   │   ├── workflow/
│   │   ├── roles/
│   │   ├── task/
│   │   └── project/
│   │
│   ├── agents/
│   │   ├── mod.rs
│   │   ├── traits.rs
│   │   ├── cursor.rs
│   │   ├── opencode.rs
│   │   ├── claude.rs
│   │   ├── codex.rs
│   │   └── pi.rs
│   │
│   ├── execution/
│   │   ├── mod.rs
│   │   ├── traits.rs
│   │   ├── herdr.rs
│   │   └── console.rs
│   │
│   ├── memory/
│   │   ├── mod.rs
│   │   ├── sqlite.rs
│   │   ├── search.rs
│   │   └── migrations/
│   │
│   ├── review/
│   │   ├── mod.rs
│   │   ├── engine.rs
│   │   ├── rules.rs
│   │   ├── sentrux.rs
│   │   ├── git.rs
│   │   └── tests.rs
│   │
│   ├── spec/
│   │   ├── mod.rs
│   │   └── speckit.rs
│   │
│   ├── mcp/
│   │   ├── mod.rs
│   │   └── discovery.rs
│   │
│   ├── remote/
│   │   ├── mod.rs
│   │   ├── api.rs
│   │   └── auth.rs
│   │
│   ├── config/
│   │   ├── mod.rs
│   │   └── schema.rs
│   │
│   └── installer/
│       ├── mod.rs
│       ├── agents.rs
│       ├── tools.rs
│       └── project.rs
│
├── migrations/
├── templates/
├── skills/
├── tests/
└── docs/
```

La CLI deberá ser una capa de entrada. Las reglas de negocio no deberán vivir en handlers de comandos.

---

# 7. Modelo de agentes

## 7.1 Interfaz común

Todos los adapters deberán implementar conceptualmente:

```rust
trait AgentAdapter {
    fn id(&self) -> &str;

    fn detect(&self) -> DetectionResult;

    fn capabilities(&self) -> AgentCapabilities;

    fn version(&self) -> Result<String>;

    fn interactive_command(&self, request: AgentRequest)
        -> Result<ProcessSpec>;

    fn non_interactive_command(&self, request: AgentRequest)
        -> Result<ProcessSpec>;

    fn build_environment(&self, context: AgentContext)
        -> Result<Environment>;

    fn parse_result(&self, output: AgentOutput)
        -> Result<AgentResult>;
}
```

---

# 8. Capacidades de los agentes

NodKray no asumirá que todos los agentes soportan exactamente las mismas capacidades.

```yaml
capabilities:
  interactive: true
  headless: true
  json_output: false
  mcp: true
  skills: true
  worktree: true
  system_prompt: true
  stdin: true
  session_resume: true
```

La matriz de capacidades deberá poder consultarse:

```bash
nodkray agent list
nodkray agent inspect codex
```

---

# 9. Adaptadores iniciales

## 9.1 Cursor

Cursor actualmente dispone de Agent CLI, incluyendo uso interactivo y automatización desde terminal. También soporta MCP.

Adapter:

```text
CursorAdapter
 ├── detect
 ├── capabilities
 ├── interactive
 └── headless
```

---

## 9.2 OpenCode

El adapter deberá descubrir la instalación del ejecutable `opencode` y usar su interfaz de consola disponible.

---

## 9.3 Claude

Claude Code opera como herramienta agéntica de terminal y dispone de ejecución no interactiva mediante `-p/--print`.

El adapter deberá utilizar modo no interactivo para workers y modo interactivo para sesiones frontera cuando la configuración así lo indique.

---

## 9.4 Codex

Codex se distribuye como agente de código para terminal y mantiene su implementación CLI dentro del repositorio oficial.

Adapter:

```text
CodexAdapter
 ├── detect
 ├── capabilities
 ├── interactive
 └── headless
```

---

## 9.5 Pi

Pi proporciona un coding agent de terminal con soporte multmodelo y modos interactivo, print/JSON y RPC, por lo que resulta compatible con el concepto de adapter de ejecución de NodKray.

El adapter deberá aprovechar el modo más adecuado según el contexto de ejecución.

---

# 10. Agente frontera

El **frontier agent** recibe la intención del usuario.

Responsabilidades:

1. entender la solicitud;
2. consultar NodKray;
3. participar en la decisión ST/ODD/SDD;
4. delegar;
5. revisar resultados;
6. recibir resultados consolidados;
7. informar al usuario.

El frontier no debe estar codificado dentro de NodKray.

```text
               Agent Frontier
                     │
              nodkray task
                     │
                     ▼
                 NodKray
```

---

# 11. Worker agents

Un worker es una instancia de un agente utilizada para ejecutar una unidad concreta de trabajo.

Cada worker tendrá:

```yaml
worker:
  id: backend-01
  role: backend
  agent: codex
  workspace: /workspace/project
  worktree: .nodkray/worktrees/backend-01
```

Los workers podrán ser:

* seriales;
* paralelos;
* temporales;
* reutilizables durante una sesión.

---

# 12. Asignación por roles

Configuración ejemplo:

```yaml
roles:

  frontier:
    agent: opencode

  workers:

    default:
      agent: codex

    backend:
      agent: codex

    frontend:
      agent: cursor

    reviewer:
      agent: claude

    docs:
      agent: pi

  execution:
    backend: herdr
```

Regla:

> Una tarea debe seleccionar un rol. El rol determina el agente y las capacidades de ejecución.

---

# 13. Ejecución

NodKray implementará una interfaz común:

```rust
trait ExecutionBackend {
    fn create_workspace(
        &self,
        request: WorkspaceRequest
    ) -> Result<Workspace>;

    fn spawn_worker(
        &self,
        request: WorkerRequest
    ) -> Result<WorkerHandle>;

    fn send(
        &self,
        worker: &WorkerHandle,
        input: WorkerInput
    ) -> Result<()>;

    fn status(
        &self,
        worker: &WorkerHandle
    ) -> Result<WorkerStatus>;

    fn stop(
        &self,
        worker: &WorkerHandle
    ) -> Result<()>;
}
```

Implementaciones V1:

```text
ExecutionBackend
      │
      ├── HerdrBackend
      └── ConsoleBackend
```

---

# 14. Herdr

Herdr se utilizará como runtime/multiplexor de agentes, no como reemplazo de los agentes.

El repositorio de Herdr describe explícitamente que mantiene terminales de agentes, permite ejecución persistente, workspaces, múltiples máquinas y trabaja con agentes como Claude Code, Codex, Cursor y OpenCode.

```text
NodKray
   │
   ▼
HerdrBackend
   │
   ├── pane
   ├── agent
   ├── worktree
   └── session
```

---

# 15. Console Backend

Cuando Herdr no esté disponible:

```text
NodKray
   │
   ▼
ConsoleBackend
   │
   ▼
process spawn
   │
   ▼
agent CLI
```

Cada worker será un proceso separado.

El aislamiento de archivos se realizará mediante Git worktrees.

---

# 16. Worktrees

El aislamiento de workers será obligatorio cuando dos tareas puedan modificar código simultáneamente.

```text
repository/
│
├── main
│
├── .nodkray/
│   └── worktrees/
│       ├── worker-backend/
│       ├── worker-frontend/
│       └── worker-review/
```

Reglas:

1. cada worker paralelo tendrá un worktree independiente;
2. un worker no escribirá en el worktree de otro;
3. NodKray conservará la asociación `worker → worktree → task`;
4. la rama principal no recibirá cambios hasta pasar RDD;
5. la integración será explícita.

---

# 17. Modelo de tarea

Toda ejecución deberá convertirse internamente en:

```yaml
task:
  id: task_01J...
  description: ...
  project: ...
  working_directory: ...
  requested_by: ...
  workflow: auto
  role: backend
  priority: normal
  parent_task: null
```

Estados:

```text
PENDING
   │
   ▼
CLASSIFYING
   │
   ▼
PLANNED
   │
   ▼
RUNNING
   │
   ▼
REVIEWING
   │
 ┌─┴─────────┐
 ▼           ▼
PASSED      FAILED
 │           │
 ▼           ▼
MERGED    REMEDIATING
              │
              └──────► REVIEWING
```

---

# 18. Decision Engine

El Decision Engine seleccionará:

```text
ST
ODD
SDD
```

y, de forma independiente:

```text
RDD depth
```

No deberá seleccionar una metodología únicamente por tamaño de texto. Debe considerar características de la tarea.

---

# 19. Modelo de esfuerzo

El esfuerzo se modelará inicialmente mediante una puntuación normalizada:

```text
effort ∈ [0, 100]
```

Variables mínimas:

| Variable             | Descripción                     |
| -------------------- | ------------------------------- |
| scope                | cantidad de áreas afectadas     |
| uncertainty          | incertidumbre                   |
| architectural_impact | impacto arquitectónico          |
| change_volume        | volumen esperado                |
| risk                 | riesgo                          |
| dependencies         | dependencias                    |
| verification_cost    | coste de verificación           |
| user_specificity     | especificidad de requerimientos |

No se utilizará una única métrica para determinar el flujo.

---

# 20. Umbrales de flujo

Valores iniciales configurables:

```yaml
decision:
  thresholds:
    st_max: 20
    odd_max: 60
```

Interpretación:

```text
0 ─────────── 20 ───────────────── 60 ─────────────── 100
│              │                    │                   │
│      ST      │         ODD        │        SDD        │
│              │                    │                   │
└──────────────┴────────────────────┴───────────────────┘
```

Estos valores serán configurables y estarán versionados en la configuración, no hardcodeados como verdad absoluta del dominio.

---

# 21. ST — Simple Task

ST es el flujo de mínima estructura.

Se utilizará cuando:

* el cambio sea localizado;
* exista poca incertidumbre;
* no exista impacto arquitectónico relevante;
* el trabajo pueda realizarse directamente;
* la especificación formal introduciría más overhead que valor.

Flujo:

```text
Usuario
   │
   ▼
Frontier
   │
   ▼
Decision
   │
   ▼
ST
   │
   ▼
Worker / Frontier
   │
   ▼
Review
   │
   ▼
Merge
```

ST no crea:

* `spec.md`;
* `plan.md`;
* `tasks.md`.

---

# 22. ODD — Organic Driven Development

ODD representa una especificación ligera.

Debe existir suficiente estructura para evitar pérdida de intención, sin generar un ciclo SDD completo.

Artefacto mínimo:

```text
.nodkray/tasks/<task-id>/
└── task.md
```

Contenido mínimo:

```markdown
# Task

## Objective

## Context

## Scope

## Constraints

## Expected Result

## Validation
```

Flujo:

```text
User
 │
 ▼
Decision
 │
 ▼
ODD
 │
 ├── task.md
 │
 ▼
Execution
 │
 ▼
Review
```

---

# 23. SDD — Spec Driven Development

SDD será implementado mediante Spec-Kit.

El flujo formal actual de Spec-Kit está orientado a:

```text
constitution
    │
    ▼
specify
    │
    ▼
plan
    │
    ▼
tasks
    │
    ▼
implement
    │
    ▼
converge
```

y puede incorporar etapas adicionales como clarify, checklist y analyze.

NodKray no deberá reimplementar estas etapas.

---

# 24. Constitution

La constitución pertenece al proyecto.

Ubicación:

```text
.specify/
└── memory/
    └── constitution.md
```

La constitución se inicializa una vez por proyecto.

El proceso conceptual será:

```text
nodkray init
      │
      ▼
Detect constitution
      │
 ┌────┴─────┐
 │          │
exists     absent
 │          │
 ▼          ▼
reuse     create
 │          │
 └────┬─────┘
      ▼
project rules
```

Spec-Kit documenta la constitución como una operación de establecimiento de reglas del proyecto y utiliza el archivo posteriormente durante la generación de planes y análisis.

NodKray no deberá recrearla en cada tarea.

---

# 25. Constitución como fuente viva

La constitución será consultada durante SDD.

```text
                    constitution.md
                          │
              ┌───────────┼───────────┐
              ▼           ▼           ▼
            Plan         Tasks       Analyze
              │           │           │
              └───────────┼───────────┘
                          ▼
                       Implement
```

El archivo no será copiado como una nueva versión dentro de cada tarea.

---

# 26. Spec-Kit y NodKray

NodKray incluirá Spec-Kit como dependencia requerida para el flujo SDD.

Responsabilidades:

| NodKray                 | Spec-Kit              |
| ----------------------- | --------------------- |
| decide SDD              | ejecuta SDD           |
| detecta proyecto        | administra artefactos |
| determina cuándo usarlo | specify               |
| coordina agentes        | plan                  |
| gestiona ejecución      | tasks                 |
| gestiona workers        | implement             |
| ejecuta RDD             | converge              |
| conserva configuración  | constitución          |

---

# 27. SDD totalmente dirigido por el agente

NodKray deberá permitir que el agente frontera invoque las etapas de Spec-Kit sin que el usuario tenga que ejecutar manualmente cada comando.

```text
User task
   │
   ▼
Frontier Agent
   │
   ▼
NodKray
   │
   ▼
SDD
   │
   ├── specify
   ├── plan
   ├── tasks
   ├── implement
   └── converge
```

NodKray no debe forzar interacción humana intermedia salvo que el propio workflow encuentre una situación que requiera decisión humana o la configuración del proyecto la exija.

---

# 28. Spec-Kit como dependencia local del proyecto

Debe respetarse cualquier instalación existente.

Reglas:

1. si Spec-Kit ya está instalado, reutilizarlo;
2. si no existe, utilizar la versión administrada por NodKray;
3. no sobrescribir una instalación existente sin instrucción explícita;
4. detectar cambios de versión;
5. registrar la versión utilizada en la sesión.

Spec-Kit dispone actualmente de múltiples integraciones de agentes y una integración genérica para agentes no listados.

---

# 29. RDD — Review Driven Development

RDD es el flujo propio de NodKray para validar resultados.

RDD no es equivalente a una sola herramienta.

```text
                         RDD
                          │
          ┌───────────────┼─────────────────┐
          │               │                 │
          ▼               ▼                 ▼
       Git diff        Tests             Rules
          │               │                 │
          │               │            Sentrux
          │               │                 │
          └───────────────┼─────────────────┘
                          ▼
                       Verdict
```

---

# 30. Objetivo de RDD

RDD responde:

> ¿El trabajo producido satisface las reglas y condiciones necesarias para ser integrado?

No responde:

> ¿El código es bueno en términos abstractos?

Debe existir una relación trazable entre:

```text
Task
   │
   ├── requirements
   ├── constraints
   ├── changes
   └── validations
```

---

# 31. Niveles de RDD

RDD tendrá tres niveles.

## FAST

```text
git diff
tests básicos
reglas disponibles
```

## BALANCED

```text
git diff
tests
lint
reglas
Sentrux si está configurado
```

## DEEP

```text
todo BALANCED
+
análisis adicional
+
revisión estructural
+
review agent
+
convergencia cuando corresponda
```

Configuración:

```yaml
review:
  default_depth: balanced
```

---

# 32. Selección automática del RDD

RDD podrá depender del esfuerzo.

```text
Task effort
     │
     ▼
┌────┼───────────────┐
▼    ▼               ▼
FAST BALANCED       DEEP
```

Ejemplo:

```yaml
review:
  thresholds:
    fast_max: 20
    balanced_max: 60
```

El usuario podrá forzar:

```bash
nodkray task --review fast
nodkray task --review deep
```

---

# 33. Sentrux dentro de RDD

Sentrux no será el RDD.

Sentrux será un **validador opcional de arquitectura y calidad estructural dentro de RDD**.

Sentrux proporciona:

* medición estructural;
* reglas;
* `check`;
* `gate`;
* MCP;
* análisis de dependencias;
* restricciones arquitectónicas.

Por tanto:

```text
RDD
 │
 ├── Git
 ├── Tests
 ├── Rules
 ├── Agent Review
 └── Sentrux [optional]
```

---

# 34. Reglas Sentrux

Cuando exista:

```text
.sentrux/
└── rules.toml
```

NodKray deberá detectarlo.

Ejemplo:

```toml
[constraints]
max_cycles = 0
max_coupling = "B"
max_cc = 25
no_god_files = true
```

Sentrux actualmente utiliza `.sentrux/rules.toml` para restricciones arquitectónicas y ofrece `sentrux check .` con códigos de salida de éxito/fallo apropiados para CI.

---

# 35. Resultado de RDD

RDD deberá producir un objeto normalizado:

```json
{
  "status": "passed",
  "score": null,
  "checks": [
    {
      "id": "git-diff",
      "status": "passed"
    },
    {
      "id": "tests",
      "status": "passed"
    },
    {
      "id": "sentrux",
      "status": "passed"
    }
  ],
  "violations": [],
  "remediation_required": false
}
```

Cuando falle:

```json
{
  "status": "failed",
  "violations": [
    {
      "id": "architecture.boundary",
      "severity": "critical",
      "source": "sentrux",
      "message": "...",
      "path": "..."
    }
  ],
  "remediation_required": true
}
```

---

# 36. Política de aprobación

RDD utilizará una política explícita:

```yaml
review:
  policy:
    test_failure: block
    constitution_violation: block
    sentrux_failure: block
    lint_failure: configurable
    reviewer_failure: block
```

La aprobación no dependerá de una puntuación única.

---

# 37. Integración

Los cambios no deben integrarse directamente después de ser escritos.

```text
Worker
  │
  ▼
Completed
  │
  ▼
RDD
  │
 ┌┴──────────────┐
 ▼               ▼
PASS             FAIL
 │                │
 ▼                ▼
Merge          Remediation
 │                │
 ▼                └──────► RDD
Main
```

---

# 38. Merge

El merge deberá ser realizado por NodKray.

Requisitos:

1. comprobar estado del worktree;
2. comprobar ausencia de modificaciones no registradas que no pertenezcan a la tarea;
3. comprobar branch;
4. ejecutar RDD;
5. integrar;
6. detectar conflicto;
7. marcar resultado.

Estados:

```text
READY_TO_MERGE
MERGING
MERGED
CONFLICT
FAILED
```

---

# 39. Resolución de conflictos

NodKray nunca deberá resolver silenciosamente un conflicto Git.

```text
Conflict
   │
   ▼
BLOCK
   │
   ▼
Frontier Agent
   │
   ▼
Resolution
   │
   ▼
Review
   │
   ▼
Merge
```

---

# 40. MCP

NodKray tratará los MCP como integraciones externas.

No se implementará un CLI sustituto para cada MCP.

```text
                  Agent
                    │
      ┌─────────────┼─────────────┐
      ▼             ▼             ▼
   Serena        CodeGraph      Sentrux
      │             │             │
      └─────────────┼─────────────┘
                    ▼
                  MCP
```

---

# 41. Serena

Serena será tratado como servidor MCP/agent integration.

NodKray deberá:

1. detectar si está instalado;
2. detectar si está configurado;
3. preservar configuración existente;
4. incorporar configuración únicamente cuando el usuario la solicite o cuando la instalación inicial así lo establezca;
5. no duplicar sus herramientas.

---

# 42. CodeGraph

CodeGraph ya expone integración MCP y equivalentes CLI; su enfoque actual utiliza `codegraph_explore` como interfaz principal simplificada para exploración de código, mientras las herramientas más especializadas pueden permanecer disponibles por CLI.

NodKray deberá dejar esa responsabilidad en CodeGraph.

---

# 43. Sentrux MCP

Cuando Sentrux esté habilitado:

```text
Agent
  │
  ▼
Sentrux MCP
  │
  ▼
architectural checks
```

NodKray lo tratará como otra fuente de validación.

---

# 44. CLI NodKray

El CLI es una de las interfaces principales del sistema.

Debe permitir:

```bash
nodkray init
nodkray doctor
nodkray status

nodkray task
nodkray task run
nodkray task inspect
nodkray task cancel

nodkray agent list
nodkray agent inspect

nodkray worker list
nodkray worker inspect

nodkray memory save
nodkray memory search
nodkray memory get
nodkray memory timeline

nodkray review
nodkray review inspect

nodkray config get
nodkray config set

nodkray mcp list
nodkray mcp status

nodkray project inspect
```

---

# 45. Principio del CLI

El CLI deberá reducir trabajo cognitivo del agente.

Un agente no debería necesitar:

```text
grep
find
cat
sqlite3
git log
git branch
...
```

para realizar operaciones que NodKray pueda encapsular de forma determinista.

Ejemplo:

```bash
nodkray memory search "decision authentication"
```

en lugar de hacer que el agente descubra manualmente la estructura de la base.

---

# 46. Memoria persistente

La memoria será:

```text
Local
Persistent
Project-aware
Indexed
Searchable
CLI-only
```

La V1 utilizará **SQLite + FTS5**.

Engram documenta precisamente SQLite + FTS5 como almacenamiento local para memoria persistente y separación del almacenamiento respecto de la superficie del agente.

---

# 47. Estructura de memoria

```text
~/.nodkray/
├── config.toml
├── memory.db
├── logs/
└── cache/
```

Por proyecto:

```text
project
  │
  ├── repository path
  ├── git remote
  ├── project identity
  └── memory entries
```

---

# 48. Modelo SQLite

Tablas mínimas:

```sql
projects
sessions
tasks
task_events
observations
decisions
memories
workers
reviews
review_checks
project_rules
```

---

# 49. `projects`

```sql
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    git_remote TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

---

# 50. `sessions`

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    frontier_agent TEXT,
    workflow TEXT,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    status TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);
```

---

# 51. `tasks`

```sql
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    session_id TEXT,
    parent_task_id TEXT,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    workflow TEXT NOT NULL,
    effort INTEGER,
    role TEXT,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

---

# 52. `observations`

```sql
CREATE TABLE observations (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    session_id TEXT,
    task_id TEXT,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

---

# 53. `decisions`

```sql
CREATE TABLE decisions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    task_id TEXT,
    question TEXT NOT NULL,
    decision TEXT NOT NULL,
    rationale TEXT,
    created_at TEXT NOT NULL
);
```

---

# 54. `memories`

```sql
CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    source TEXT,
    importance INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

---

# 55. FTS5

La búsqueda de memoria se implementará con FTS5.

```text
                query
                  │
                  ▼
              FTS5 index
                  │
                  ▼
             candidates
                  │
                  ▼
             ranking
                  │
                  ▼
              results
```

Ejemplo conceptual:

```sql
CREATE VIRTUAL TABLE memory_fts
USING fts5(
    title,
    content,
    content='memories',
    content_rowid='rowid'
);
```

La implementación final deberá utilizar triggers o estrategia equivalente para mantener el índice sincronizado.

---

# 56. Memoria: qué guardar

La memoria no debe convertirse en un transcript.

Debe conservar información reutilizable:

```text
decisiones
convenciones
restricciones
arquitectura
bugs relevantes
soluciones
preferencias del proyecto
resultados importantes
```

No debe guardarse automáticamente cada mensaje del agente.

---

# 57. Protocolo de memoria

Antes de realizar una tarea relacionada con conocimiento histórico:

```text
task
 │
 ▼
memory search
 │
 ├── previous decisions
 ├── project conventions
 └── relevant observations
```

Después de finalizar:

```text
result
 │
 ▼
extract significant knowledge
 │
 ▼
memory save
```

---

# 58. CLI de memoria

```bash
nodkray memory search "database architecture"

nodkray memory save \
  --type decision \
  --title "SQLite local source of truth" \
  --content "..."

nodkray memory get <id>

nodkray memory timeline <project>
```

---

# 59. SQLite vs Turso

## Decisión

**SQLite es el almacenamiento primario de V1.**

Turso no será requerido.

Arquitectura futura compatible:

```text
NodKray
   │
   ▼
MemoryRepository
   │
   ├── SQLite
   │
   └── Remote Sync [future]
           │
           ▼
          Turso
```

Turso podrá utilizarse posteriormente como capa remota/sincronización, pero no será requisito del runtime local.

---

# 60. Razón arquitectónica

NodKray debe funcionar:

* offline;
* sin cuenta;
* sin credenciales remotas;
* sin infraestructura externa;
* desde un único binario.

SQLite satisface estas propiedades y coincide con la arquitectura local-first utilizada por sistemas de memoria de agentes.

---

# 61. Configuración

Archivo principal:

```text
.nodkray/config.yaml
```

Configuración global:

```text
~/.config/nodkray/config.yaml
```

Prioridad:

```text
CLI arguments
      │
      ▼
project config
      │
      ▼
user config
      │
      ▼
defaults
```

---

# 62. Configuración completa inicial

```yaml
version: 1

project:
  name: my-project

agents:

  frontier:
    provider: opencode

  workers:
    default:
      provider: codex

    backend:
      provider: codex

    frontend:
      provider: cursor

    reviewer:
      provider: claude

    docs:
      provider: pi

execution:
  backend: herdr

  worktrees:
    enabled: true

decision:
  provider: local

  thresholds:
    st_max: 20
    odd_max: 60

review:
  default_depth: balanced

  policy:
    test_failure: block
    constitution_violation: block
    sentrux_failure: block

integrations:
  speckit:
    enabled: true

  sentrux:
    enabled: true
    required: false

  serena:
    enabled: true

  codegraph:
    enabled: true

memory:
  backend: sqlite
  path: ~/.nodkray/memory.db

remote:
  enabled: false

security:
  yolo: false
```

---

# 63. Instalación

NodKray será distribuido como binario Rust.

Objetivo:

```bash
nodkray init
```

La instalación deberá detectar:

```text
OS
architecture
Git
Spec-Kit
agents
Herdr
MCP
project rules
```

---

# 64. `nodkray init`

Flujo:

```text
                 nodkray init
                      │
          ┌───────────┴────────────┐
          ▼                        ▼
    detect environment       detect project
          │                        │
          ▼                        ▼
       agents                 constitution
          │                        │
          ▼                        ▼
        tools                  project rules
          │                        │
          └────────────┬───────────┘
                       ▼
                    configure
                       │
                       ▼
                  save config
```

---

# 65. Scope de instalación

NodKray deberá permitir:

```bash
nodkray init --global
nodkray init --project
```

Global:

```text
~/.config/nodkray/
```

Proyecto:

```text
project/
└── .nodkray/
```

---

# 66. Detección de reglas

NodKray deberá buscar, respetando precedencia y existencia:

```text
CONSTITUTION.md
CONSTITUTION.MD
.specify/memory/constitution.md
README.md
AGENTS.md
CLAUDE.md
AGENTS.local.md
.sentrux/rules.toml
```

La lista debe ser configurable/extensible.

---

# 67. Principio de reglas locales

Las reglas del repositorio son **locales al proyecto**.

No deberán convertirse en una política global silenciosa.

```text
Global NodKray
      │
      └── defaults

Project
      │
      └── explicit rules
```

Cuando exista una regla local:

```text
project rules > generic defaults
```

---

# 68. Constitution existente

Cuando exista `.specify/memory/constitution.md`:

```text
detect
  │
  ▼
reuse
```

Nunca deberá:

```text
overwrite
regenerate
duplicate
```

sin una operación explícita de modificación.

---

# 69. Integraciones existentes

Si el proyecto ya posee:

* Spec-Kit;
* MCP;
* configuración del agente;
* Sentrux;
* Serena;
* CodeGraph;

NodKray deberá detectarlo y respetar la configuración existente.

Esto es obligatorio para evitar que el instalador rompa entornos ya configurados.

---

# 70. `doctor`

Comando:

```bash
nodkray doctor
```

Debe comprobar:

```text
✓ NodKray
✓ Git
✓ SQLite
✓ agent frontier
✓ worker agents
✓ Herdr
✓ Spec-Kit
✓ Serena
✓ CodeGraph
✓ Sentrux
✓ project configuration
```

Salida:

```text
NodKray Doctor

[OK] Git
[OK] SQLite
[OK] OpenCode
[OK] Codex
[OK] Herdr
[OK] Spec-Kit
[--] Sentrux disabled
[OK] Project rules
```

---

# 71. Modo YOLO

NodKray podrá ejecutarse:

```bash
nodkray task --yolo "..."
```

`--yolo` no deberá modificar el flujo lógico.

Solo modifica el modo de ejecución/autorización de los workers.

```text
normal
  │
  ▼
agent permissions

yolo
  │
  ▼
reduced-interaction execution
```

---

# 72. JEV

JEV será una integración opcional.

No es una dependencia del core.

```text
Decision Engine
      │
      ├── Local reasoning
      │
      └── JEV adapter
```

Configuración:

```yaml
decision:
  provider: local
```

o:

```yaml
decision:
  provider: jev

  api_key_env: JEV_API_KEY
```

---

# 73. Decisión con JEV

Cuando esté habilitado:

```text
Task
 │
 ▼
JEV
 │
 ├── effort
 ├── uncertainty
 ├── workflow
 └── review depth
```

NodKray deberá validar que la respuesta de JEV cumpla el schema esperado.

Nunca deberá aceptar texto arbitrario como decisión interna.

---

# 74. Decisión sin JEV

Cuando JEV no esté configurado:

```text
Task
 │
 ▼
Frontier / local reasoning
 │
 ▼
Decision schema
```

El sistema debe funcionar completamente sin JEV.

---

# 75. Decision Schema

```json
{
  "workflow": "ODD",
  "effort": 47,
  "review_depth": "balanced",
  "reasons": [
    "multiple files",
    "moderate uncertainty",
    "no architectural redesign"
  ]
}
```

---

# 76. Agent Contract

La información enviada al agente deberá tener estructura estable.

```json
{
  "task_id": "task_123",
  "project": {
    "root": "/repo"
  },
  "workflow": {
    "type": "ODD"
  },
  "role": {
    "name": "backend"
  },
  "constraints": [],
  "memory_context": [],
  "instructions": "...",
  "validation": {
    "required": true
  }
}
```

---

# 77. Output Contract

Un worker debe terminar con:

```json
{
  "status": "completed",
  "summary": "...",
  "changed_files": [
    "src/foo.rs"
  ],
  "tests_run": [
    "cargo test"
  ],
  "notes": [],
  "blocking_issues": []
}
```

Esto evita que NodKray tenga que interpretar únicamente texto humano.

---

# 78. Estado de ejecución

Cada worker deberá emitir eventos:

```text
CREATED
STARTED
WORKING
BLOCKED
COMPLETED
FAILED
CANCELLED
```

Ejemplo:

```text
worker.created
worker.started
worker.progress
worker.blocked
worker.completed
review.started
review.completed
```

Los eventos deben persistirse en SQLite.

---

# 79. Paralelización

La paralelización será definida por el workflow.

Ejemplo:

```text
Task
 │
 ▼
Planner
 │
 ├──────────┬─────────────┐
 ▼          ▼             ▼
Backend   Frontend      Docs
 │          │             │
 ▼          ▼             ▼
WT-1       WT-2          WT-3
 │          │             │
 └──────────┼─────────────┘
            ▼
          RDD
```

---

# 80. Dependencias entre workers

No se deberán ejecutar tareas dependientes simultáneamente.

Ejemplo:

```text
A
│
├── B
│
└── C

B + C
  │
  ▼
D
```

NodKray debe construir un DAG de tareas.

---

# 81. DAG de ejecución

Representación interna:

```yaml
tasks:
  - id: backend
    depends_on: []

  - id: frontend
    depends_on: []

  - id: integration
    depends_on:
      - backend
      - frontend
```

---

# 82. Worker Reviewer

El reviewer será un rol independiente.

Puede utilizar otro agente:

```yaml
roles:
  workers:
    reviewer:
      agent: claude
```

El reviewer no debe trabajar sobre el mismo worktree que el worker revisado cuando sea necesario preservar independencia.

---

# 83. Review Agent

El review agent recibirá:

```text
task specification
project rules
changed files
git diff
test results
Sentrux results
memory context if relevant
```

No deberá recibir toda la conversación histórica.

---

# 84. RDD y Constitución

Si la tarea se ejecutó mediante SDD:

```text
constitution
       │
       ▼
specification
       │
       ▼
implementation
       │
       ▼
RDD
```

RDD deberá verificar explícitamente violaciones de principios de la constitución cuando puedan determinarse de forma objetiva.

---

# 85. RDD y ODD/ST

ST y ODD también pasan por RDD.

La diferencia es el volumen de artefactos, no la ausencia total de revisión.

```text
ST  → FAST
ODD → BALANCED
SDD → BALANCED/DEEP
```

Los defaults podrán configurarse.

---

# 86. Integración remota

NodKray deberá separar:

```text
Core
Control API
Remote Client
```

```text
               PHONE
                 │
                 ▼
        ┌─────────────────┐
        │ Remote Client   │
        └────────┬────────┘
                 │
          authenticated
                 │
                 ▼
        ┌─────────────────┐
        │ NodKray Control │
        │ API             │
        └────────┬────────┘
                 │
                 ▼
             NodKray Core
```

La interfaz remota deberá permitir como mínimo:

```text
status
task creation
task status
worker status
review status
logs/events
cancel task
```

No se permitirá ejecutar operaciones administrativas sensibles sin autenticación/autorización.

---

# 87. Control remoto

La capa remota deberá ser:

* opcional;
* deshabilitada por defecto;
* autenticada;
* cifrada cuando abandone localhost;
* independiente del core de decisión.

Configuración:

```yaml
remote:
  enabled: false
  bind: 127.0.0.1:8787
```

La implementación de transporte podrá utilizar HTTP/JSON para comandos y WebSocket/SSE para eventos.

---

# 88. Seguridad

NodKray controla procesos que pueden modificar código y ejecutar comandos.

Por ello:

```text
Security boundary
      │
      ├── agent process
      ├── shell
      ├── Git
      ├── MCP
      └── remote API
```

No se deberán almacenar claves directamente en SQLite.

Las credenciales deberán permanecer en:

* variables de entorno;
* mecanismos de autenticación del agente;
* sistemas seguros disponibles en el entorno.

---

# 89. Modo seguro

Por defecto:

```yaml
security:
  yolo: false
```

El sistema deberá solicitar intervención cuando una operación requiera interacción y el backend/agente no pueda realizarla de manera segura y automática.

---

# 90. Observabilidad

NodKray deberá mantener logs estructurados.

```text
~/.nodkray/logs/
├── nodkray.log
├── sessions/
└── workers/
```

Cada evento deberá contener:

```json
{
  "timestamp": "...",
  "session_id": "...",
  "task_id": "...",
  "worker_id": "...",
  "event": "worker.completed",
  "level": "info"
}
```

---

# 91. Logs y memoria

Los logs no son memoria.

```text
LOG
 ↓
debug / forensic

MEMORY
 ↓
reusable knowledge
```

No se deberá utilizar el log completo como contexto de una sesión.

---

# 92. Skills

NodKray podrá distribuir skills pequeñas para agentes.

Una skill deberá:

* tener alcance reducido;
* utilizar instrucciones concretas;
* evitar documentación redundante;
* explicar únicamente cuándo y cómo utilizar NodKray;
* derivar consultas complejas al CLI.

Ejemplo:

```text
skills/
└── nodkray/
    ├── memory.md
    ├── task.md
    ├── review.md
    └── orchestration.md
```

---

# 93. AGENTS.md

NodKray podrá generar un bloque específico para el agente.

Ejemplo conceptual:

```markdown
## NodKray

Use `nodkray memory search` before revisiting project decisions.

Use `nodkray task` for delegated work.

Do not modify another worker's worktree.

Use `nodkray review` before requesting merge.
```

La inserción deberá ser:

* delimitada;
* idempotente;
* no destructiva.

---

# 94. MCP y contexto

Las herramientas MCP deberán cargarse según necesidad.

NodKray no deberá proporcionar automáticamente todas las herramientas posibles a cada worker.

```text
worker role
    │
    ▼
required capabilities
    │
    ▼
MCP selection
```

---

# 95. Mapeo de herramientas por rol

Ejemplo:

```yaml
roles:

  workers:
    backend:
      agent: codex
      mcp:
        - codegraph
        - serena

    reviewer:
      agent: claude
      mcp:
        - sentrux
```

La configuración permitirá `all`, `none` o una lista explícita.

---

# 96. Descubrimiento MCP

NodKray deberá detectar:

```text
configuration
binary
server
availability
```

y registrar:

```json
{
  "name": "codegraph",
  "installed": true,
  "configured": true,
  "available": true
}
```

---

# 97. Adopción de proyecto existente

NodKray debe soportar un repositorio que ya tenga código.

```text
existing repo
     │
     ▼
nodkray init
     │
     ├── detect rules
     ├── detect Spec-Kit
     ├── detect MCP
     ├── detect agents
     └── create NodKray config
```

No se debe exigir iniciar el proyecto desde cero.

---

# 98. Idempotencia

Los siguientes comandos deberán ser idempotentes cuando no se especifique lo contrario:

```bash
nodkray init
nodkray doctor
nodkray install
```

Ejecutar dos veces no debe:

* duplicar configuraciones;
* duplicar skills;
* sobrescribir reglas;
* crear múltiples MCP;
* romper el proyecto.

---

# 99. Compatibilidad multiplataforma

V1 deberá soportar:

```text
Windows
Linux
macOS
```

Con:

```text
x86_64
aarch64
```

El núcleo Rust deberá evitar dependencias innecesariamente específicas de plataforma.

---

# 100. Requisitos funcionales

## FR-001 — Inicialización

El sistema deberá permitir inicializar NodKray global o localmente.

## FR-002 — Detección de agentes

Deberá detectar agentes instalados compatibles.

## FR-003 — Configuración de roles

Deberá permitir asignar agentes a roles.

## FR-004 — Selección de workflow

Deberá determinar ST, ODD o SDD.

## FR-005 — JEV opcional

Deberá poder realizar la decisión mediante JEV cuando esté configurado.

## FR-006 — Decisión local

Deberá continuar funcionando sin JEV.

## FR-007 — ST

Deberá ejecutar tareas simples sin artefactos SDD.

## FR-008 — ODD

Deberá crear y utilizar un artefacto orgánico mínimo.

## FR-009 — SDD

Deberá delegar el workflow formal a Spec-Kit.

## FR-010 — Constitution

Deberá detectar, crear y reutilizar la constitución del proyecto.

## FR-011 — Spec-Kit existente

Deberá respetar una instalación existente.

## FR-012 — Workers

Deberá crear workers mediante roles.

## FR-013 — Paralelización

Deberá paralelizar tareas independientes.

## FR-014 — Worktrees

Deberá aislar workers mediante worktrees.

## FR-015 — Herdr

Deberá ejecutar workers mediante Herdr cuando esté configurado.

## FR-016 — Console

Deberá ejecutar workers directamente cuando Herdr no esté disponible o seleccionado.

## FR-017 — MCP

Deberá detectar y configurar integraciones MCP sin duplicarlas.

## FR-018 — Serena

Deberá soportar Serena como integración externa.

## FR-019 — CodeGraph

Deberá soportar CodeGraph como integración externa.

## FR-020 — Sentrux

Deberá soportar Sentrux como checker opcional.

## FR-021 — RDD

Deberá ejecutar el proceso de revisión.

## FR-022 — Checks

Deberá integrar Git, tests, reglas y herramientas externas.

## FR-023 — Verdict

Deberá producir un veredicto estructurado.

## FR-024 — Merge

Deberá integrar cambios aprobados.

## FR-025 — Conflictos

Deberá bloquear integraciones conflictivas.

## FR-026 — Memoria

Deberá almacenar memoria persistente.

## FR-027 — Búsqueda

Deberá buscar memoria mediante índice FTS5.

## FR-028 — Timeline

Deberá mostrar historial mediante CLI.

## FR-029 — Eventos

Deberá registrar eventos de ejecución.

## FR-030 — Skills

Deberá poder instalar sus skills.

## FR-031 — AGENTS.md

Deberá poder generar integración de instrucciones.

## FR-032 — Doctor

Deberá verificar dependencias.

## FR-033 — YOLO

Deberá permitir modo YOLO.

## FR-034 — Remote control

Deberá exponer un control remoto opcional.

## FR-035 — Persistencia de tareas

Deberá recuperar el estado de tareas interrumpidas.

## FR-036 — Cancelación

Deberá permitir cancelar una ejecución.

## FR-037 — Inspección

Deberá permitir inspeccionar tareas, workers y reviews.

## FR-038 — Configuración

Deberá permitir modificación de configuración desde CLI.

---

# 101. Requisitos no funcionales

## NFR-001 — Local-first

La ejecución principal no deberá requerir un servicio remoto.

## NFR-002 — Offline

ST, ODD, SDD local, memoria y RDD básico deberán poder ejecutarse offline cuando las herramientas utilizadas lo permitan.

## NFR-003 — Rendimiento

La inicialización del core deberá ser ligera y no cargar contexto completo del proyecto.

## NFR-004 — Bajo consumo de contexto

NodKray deberá favorecer operaciones deterministas mediante CLI antes que entregar grandes bloques de contexto a los agentes.

## NFR-005 — Determinismo

Las decisiones internas no deberán depender de texto libre cuando pueda utilizarse un schema estructurado.

## NFR-006 — Idempotencia

La configuración deberá ser idempotente.

## NFR-007 — Recuperabilidad

Una ejecución interrumpida deberá dejar estado persistente suficiente para inspección y recuperación.

## NFR-008 — Observabilidad

Todo proceso deberá ser auditable mediante eventos y logs.

## NFR-009 — Portabilidad

V1 deberá funcionar en Windows, Linux y macOS.

## NFR-010 — Seguridad

Las credenciales no deberán almacenarse en memoria persistente de NodKray.

## NFR-011 — Aislamiento

Workers paralelos deberán disponer de workspace aislado.

## NFR-012 — Extensibilidad

Agregar un agente no deberá requerir modificar el motor de decisión.

## NFR-013 — Extensibilidad de ejecución

Agregar un backend de ejecución no deberá requerir modificar los workflows.

## NFR-014 — Compatibilidad

NodKray deberá respetar configuraciones existentes.

## NFR-015 — Trazabilidad

Toda tarea deberá poder relacionarse con:

```text
session
workflow
worker
worktree
review
merge
```

---

# 102. Contratos principales

## AgentAdapter

```text
detect
capabilities
version
interactive_command
non_interactive_command
environment
parse_result
```

## ExecutionBackend

```text
create_workspace
spawn_worker
send
status
stop
destroy
```

## MemoryRepository

```text
project
session
save_memory
search_memory
get_memory
timeline
save_decision
save_observation
```

## ReviewEngine

```text
review
run_check
collect_results
evaluate_policy
verdict
```

## WorkflowEngine

```text
classify
initialize
execute
review
finalize
```

---

# 103. Diagrama de clases conceptual

```text
                ┌───────────────────┐
                │   WorkflowEngine  │
                └─────────┬─────────┘
                          │
              ┌───────────┼────────────┐
              ▼           ▼            ▼
             ST          ODD          SDD
                                      │
                                      ▼
                               SpecKitAdapter


┌────────────────┐
│ DecisionEngine │
└───────┬────────┘
        │
   ┌────┴─────┐
   ▼          ▼
 Local       JEV


┌──────────────────┐
│ ExecutionEngine  │
└────────┬─────────┘
         │
   ┌─────┴──────┐
   ▼            ▼
 Herdr        Console


┌──────────────────┐
│ ReviewEngine     │
└────────┬─────────┘
         │
  ┌──────┼────────┬────────┐
  ▼      ▼        ▼        ▼
 Git   Tests   Sentrux   Agent
```

---

# 104. Flujo completo

```text
USER
 │
 ▼
FRONTIER AGENT
 │
 ▼
nodkray task
 │
 ▼
Load project
 │
 ├── Constitution
 ├── Rules
 ├── Config
 ├── Memory
 └── Integrations
 │
 ▼
DECISION
 │
 ├── ST
 ├── ODD
 └── SDD
 │
 ▼
PLAN / DELEGATE
 │
 ├── worker A
 ├── worker B
 └── worker C
 │
 ▼
EXECUTION
 │
 ├── Herdr
 └── Console
 │
 ▼
WORKTREES
 │
 ▼
IMPLEMENTATION
 │
 ▼
RDD
 │
 ├── Tests
 ├── Git
 ├── Rules
 ├── Sentrux
 └── Reviewer
 │
 ▼
VERDICT
 │
 ├── PASS
 │    │
 │    ▼
 │   MERGE
 │
 └── FAIL
      │
      ▼
   REMEDIATION
      │
      └──────────────► RDD
```

---

# 105. Flujo de ST completo

```text
Task
 │
 ▼
Decision
 │
 ▼
ST
 │
 ▼
Agent
 │
 ▼
Changes
 │
 ▼
FAST RDD
 │
 ├── PASS → Merge
 └── FAIL → Remediation
```

---

# 106. Flujo de ODD completo

```text
Task
 │
 ▼
Decision
 │
 ▼
ODD
 │
 ▼
task.md
 │
 ▼
Agent
 │
 ▼
Changes
 │
 ▼
BALANCED RDD
 │
 ├── PASS → Merge
 └── FAIL → Remediation
```

---

# 107. Flujo de SDD completo

```text
Task
 │
 ▼
Decision
 │
 ▼
SDD
 │
 ▼
Spec-Kit
 │
 ├── specify
 ├── clarify [optional]
 ├── plan
 ├── checklist [optional]
 ├── tasks
 ├── analyze [optional]
 ├── implement
 └── converge
 │
 ▼
RDD
 │
 ├── PASS → Merge
 └── FAIL → Remediation
```

Spec-Kit define actualmente el ciclo base `specify → plan → tasks → implement → converge`, con constitución una vez por proyecto y etapas adicionales disponibles como quality gates.

---

# 108. Flujo de paralelización

```text
                    Task
                      │
                      ▼
                   Planner
                      │
           ┌──────────┼──────────┐
           ▼          ▼          ▼
       Backend     Frontend     Docs
           │          │          │
       Worker A   Worker B    Worker C
           │          │          │
       WT-A       WT-B        WT-C
           │          │          │
           └──────────┼──────────┘
                      ▼
                    RDD
                      │
                      ▼
                    Merge
```

---

# 109. Estructura de una sesión

```text
session/
├── metadata
├── decision
├── task
├── workers
│   ├── worker-a
│   ├── worker-b
│   └── worker-c
├── review
└── result
```

La representación física de esta sesión podrá residir principalmente en SQLite; el filesystem se utilizará únicamente donde sea necesario para ejecución, worktrees y artefactos.

---

# 110. Estado persistente

La ejecución no dependerá exclusivamente de memoria del proceso.

Después de:

```text
process crash
terminal close
network interruption
agent failure
```

NodKray deberá conservar:

```text
task
worker
workflow
status
events
review
```

La recuperación exacta del proceso del agente dependerá de las capacidades del adapter y del backend de ejecución.

---

# 111. Versionado

La configuración deberá tener:

```yaml
version: 1
```

La base de datos utilizará migraciones.

```text
migration 001
migration 002
migration 003
...
```

Nunca se deberán modificar silenciosamente columnas en producción sin migración.

---

# 112. Identificadores

Los identificadores deberán ser globalmente únicos.

Formato recomendado:

```text
task_<ulid>
session_<ulid>
worker_<ulid>
review_<ulid>
memory_<ulid>
decision_<ulid>
```

ULID permite ordenamiento temporal y evita colisiones.

---

# 113. Errores

Los errores internos deberán clasificarse:

```text
configuration
dependency
agent
execution
git
memory
review
network
permission
user_input
internal
```

Ejemplo:

```json
{
  "code": "AGENT_NOT_FOUND",
  "category": "agent",
  "message": "Codex executable was not found",
  "recoverable": true
}
```

---

# 114. Política de errores

NodKray nunca debe transformar silenciosamente:

```text
agent failure → successful task
```

ni:

```text
review unavailable → passed review
```

Un check obligatorio no disponible debe producir:

```text
BLOCKED
```

o:

```text
FAILED
```

según el contexto.

---

# 115. Tests de NodKray

El proyecto deberá tener:

```text
unit tests
integration tests
adapter tests
workflow tests
database tests
CLI tests
review tests
```

---

# 116. Test de adapters

Cada adapter debe probar:

```text
detect
version
capabilities
command construction
argument escaping
environment
result parsing
failure handling
```

---

# 117. Test de workflows

Casos mínimos:

```text
ST selected
ODD selected
SDD selected
JEV enabled
JEV disabled
RDD fast
RDD balanced
RDD deep
Herdr enabled
Console fallback
```

---

# 118. Test de aislamiento

Debe probarse:

```text
worker A cannot write worker B worktree
worker B cannot write worker A worktree
merge only affects main
```

---

# 119. Test de memoria

Debe probar:

```text
save
search
ranking
project isolation
timeline
update
deletion
migration
FTS rebuild
```

---

# 120. Test de idempotencia

Ejecutar dos veces:

```bash
nodkray init
nodkray init
```

debe generar el mismo estado lógico.

---

# 121. Test de recuperación

Simular:

```text
process termination
agent crash
Herdr disconnect
network loss
```

y comprobar que:

```bash
nodkray status
```

continúe mostrando correctamente la sesión.

---

# 122. Test de integración con Spec-Kit

Debe comprobar:

```text
existing constitution reused
existing Spec-Kit preserved
specify invocation
plan invocation
tasks invocation
implement invocation
converge invocation
```

---

# 123. Test de RDD

Debe probar:

```text
tests pass
tests fail
rules pass
rules fail
sentrux pass
sentrux fail
review pass
review fail
policy block
policy allow
```

---

# 124. CLI UX

Los comandos deberán tener:

* salida legible;
* código de salida adecuado;
* `--json`;
* `--quiet`;
* `--verbose`.

Ejemplo:

```bash
nodkray status --json
```

deberá producir exclusivamente JSON válido.

---

# 125. Exit codes

Se utilizará una convención estable:

```text
0   success
1   general failure
2   invalid usage
3   configuration error
4   dependency error
5   agent error
6   execution error
7   review failure
8   blocked
9   conflict
10  authentication error
```

---

# 126. Configuración por entorno

Las variables sensibles y overrides temporales podrán utilizar:

```text
NODKRAY_*
```

Ejemplo:

```bash
NODKRAY_AGENT_FRONTIER=claude
NODKRAY_EXECUTION_BACKEND=console
NODKRAY_REVIEW_DEPTH=deep
```

Las variables de entorno no deberán sobrescribir permanentemente archivos de configuración.

---

# 127. Versionado del proyecto

NodKray deberá registrar:

```text
nodkray version
agent versions
speckit version
herdr version
sentrux version
```

durante la sesión cuando estén disponibles.

Esto permite reproducibilidad.

---

# 128. Compatibilidad futura

Los adapters deberán permitir agregar:

```text
Gemini
Kiro
Copilot
Custom agents
Local agents
```

sin modificar:

```text
DecisionEngine
WorkflowEngine
ReviewEngine
MemoryEngine
```

---

# 129. Agente genérico

Debe existir un adapter `generic` para agentes no soportados directamente.

Configuración:

```yaml
agents:
  custom:
    command: my-agent
    args:
      - "--headless"
```

Interfaz mínima:

```text
stdin
stdout
exit code
```

---

# 130. Backend de ejecución genérico

Debe existir una capa común para:

```text
Herdr
Console
Future backend
```

No se permitirá que los workflows llamen directamente comandos de Herdr.

Incorrecto:

```text
ODD → herdr ...
```

Correcto:

```text
ODD → ExecutionBackend → HerdrBackend
```

---

# 131. Separación de responsabilidades

```text
CLI
 │
 ▼
Application
 │
 ├── Decision
 ├── Workflow
 ├── Execution
 ├── Memory
 └── Review
```

Nunca:

```text
CLI
 └── SQL + Git + Agent + Review + parsing
```

---

# 132. Rendimiento y tokens

La arquitectura se diseñará alrededor de la reducción de:

```text
LLM context
LLM rediscovery
unnecessary MCP calls
duplicated instructions
duplicated history
```

La estrategia principal será:

```text
deterministic CLI
       +
local indexes
       +
structured artifacts
       +
on-demand context
```

---

# 133. Política de contexto de memoria

Resultados de búsqueda deberán ser pequeños.

Ejemplo:

```json
{
  "results": [
    {
      "id": "memory_01",
      "title": "SQLite is source of truth",
      "type": "decision",
      "score": 0.93,
      "preview": "..."
    }
  ]
}
```

El contenido completo deberá recuperarse posteriormente.

Esto evita insertar grandes observaciones innecesariamente.

---

# 134. Política de contexto de CodeGraph

CodeGraph ya está orientado a proporcionar respuestas de exploración en una llamada y reducir búsquedas repetitivas; NodKray deberá respetar esa característica en lugar de introducir una capa de consulta redundante.

---

# 135. Política de contexto de Serena

Serena deberá actuar como herramienta de manipulación/exploración cuando el agent la tenga configurada.

NodKray administra disponibilidad y configuración, no reimplementa su semántica.

---

# 136. Política de contexto de Sentrux

Sentrux proporciona información estructural y reglas arquitectónicas; NodKray únicamente debe incorporar sus resultados al modelo de RDD.

---

# 137. Flujo de instalación completo

```text
Download
   │
   ▼
Install binary
   │
   ▼
nodkray init
   │
   ├── detect OS
   ├── detect Git
   ├── detect agents
   ├── detect Herdr
   ├── detect Spec-Kit
   ├── detect MCP
   ├── detect rules
   └── create config
   │
   ▼
nodkray doctor
   │
   ▼
Ready
```

---

# 138. Primer uso

```bash
cd my-project
nodkray init
nodkray doctor
```

Posteriormente el usuario puede continuar utilizando su agente frontera:

```text
Cursor
OpenCode
Claude
Codex
Pi
```

y el agente puede invocar:

```bash
nodkray task "..."
```

---

# 139. Ejemplo conceptual de ejecución

Usuario:

```text
"Agregue autenticación JWT al backend"
```

El agente frontera ejecuta:

```bash
nodkray task "Agregue autenticación JWT al backend"
```

NodKray:

```text
1. detect project
2. load constitution
3. load relevant memory
4. classify
5. choose SDD
6. invoke Spec-Kit
7. decompose tasks
8. assign backend worker
9. create worktree
10. execute
11. review
12. merge
```

---

# 140. Ejemplo de respuesta estructurada

```json
{
  "task_id": "task_01J...",
  "workflow": "SDD",
  "effort": 78,
  "workers": [
    {
      "role": "backend",
      "agent": "codex",
      "status": "completed"
    }
  ],
  "review": {
    "depth": "deep",
    "status": "passed"
  },
  "merge": {
    "status": "merged"
  }
}
```

---

# 141. Proyecto sin Sentrux

```text
RDD
 │
 ├── Git
 ├── Tests
 └── Reviewer
```

Sentrux no es obligatorio.

---

# 142. Proyecto con Sentrux

```text
RDD
 │
 ├── Git
 ├── Tests
 ├── Reviewer
 └── Sentrux
```

Su fallo podrá bloquear RDD según `review.policy`.

---

# 143. Proyecto sin Herdr

```text
ExecutionEngine
      │
      ▼
ConsoleBackend
      │
      ▼
Agent process
```

No debe bloquear la instalación.

---

# 144. Proyecto sin JEV

```text
DecisionEngine
      │
      ▼
Local reasoning
```

No debe bloquear la instalación.

---

# 145. Proyecto con JEV

```text
DecisionEngine
      │
      ▼
JEV adapter
      │
      ▼
structured decision
```

---

# 146. Proyecto sin Spec-Kit

Si la tarea selecciona ST u ODD:

```text
no Spec-Kit required
```

Si selecciona SDD:

```text
NodKray
   │
   ▼
provision/locate Spec-Kit
   │
   ▼
SDD
```

---

# 147. Proyecto existente con Spec-Kit

```text
existing .specify/
       │
       ▼
detect
       │
       ▼
reuse
```

No se reconstruye el proyecto desde cero.

---

# 148. Proyecto existente con Constitución

```text
.specify/memory/constitution.md
                 │
                 ▼
          Runtime source
```

Las operaciones futuras utilizarán ese archivo.

---

# 149. Contrato de instalación de integración

Cada integración deberá describirse mediante:

```yaml
integration:
  id: codegraph
  type: mcp
  executable: codegraph

  detect:
    command: codegraph
    args:
      - "--version"

  configure:
    supported_agents:
      - cursor
      - opencode
      - claude
      - codex
```

---

# 150. Registro de adapters

El sistema utilizará registro:

```text
AgentRegistry
 ├── cursor
 ├── opencode
 ├── claude
 ├── codex
 ├── pi
 └── generic
```

Agregar un adapter no requiere editar el Decision Engine.

---

# 151. Registro de backends

```text
ExecutionRegistry
 ├── herdr
 └── console
```

---

# 152. Registro de validators

```text
ReviewRegistry
 ├── git
 ├── tests
 ├── lint
 ├── sentrux
 └── agent-review
```

---

# 153. Registro de decisiones

```text
DecisionProviderRegistry
 ├── local
 └── jev
```

---

# 154. Registro de workflows

```text
WorkflowRegistry
 ├── st
 ├── odd
 └── sdd
```

---

# 155. Esquema conceptual completo

```text
                    ┌───────────────┐
                    │ Agent Registry│
                    └───────┬───────┘
                            │
                    ┌───────▼───────┐
                    │ Decision      │
                    └───────┬───────┘
                            │
              ┌─────────────┼─────────────┐
              ▼             ▼             ▼
             ST            ODD            SDD
                                          │
                                     Spec-Kit
                                          │
              └─────────────┬────────────┘
                            ▼
                   Execution Registry
                       │          │
                       ▼          ▼
                     Herdr      Console
                       │          │
                       └────┬─────┘
                            ▼
                         Workers
                            │
                            ▼
                           RDD
                    ┌───────┼────────┐
                    ▼       ▼        ▼
                   Git    Tests   Sentrux
                    │       │        │
                    └───────┼────────┘
                            ▼
                         Verdict
                            │
                            ▼
                           Git
```

---

# 156. Principios de desarrollo del propio NodKray

El propio repositorio de NodKray deberá tener una constitución que establezca como mínimo:

```text
I. Architecture
II. Testability
III. Deterministic Core
IV. Agent Agnosticism
V. Local-First Persistence
VI. Security
VII. Observability
VIII. Compatibility
```

Esta constitución será la constitución del **proyecto NodKray**, diferente de las constituciones de los repositorios sobre los que NodKray opera.

---

# 157. Diferencia entre constituciones

```text
NodKray repository
└── .specify/memory/constitution.md
    → reglas para construir NodKray


Target repository
└── .specify/memory/constitution.md
    → reglas para construir el proyecto objetivo
```

NodKray nunca debe confundirlas.

---

# 158. Scope de memoria

Memoria global:

```text
~/.nodkray/memory.db
```

Puede contener múltiples proyectos:

```text
memory
 ├── project A
 ├── project B
 └── project C
```

Cada consulta deberá resolverse con el contexto del proyecto actual.

---

# 159. Resolución del proyecto

El proyecto se identifica principalmente mediante:

```text
current working directory
        │
        ▼
nearest repository root
        │
        ▼
project identity
```

Git remote y ruta pueden utilizarse como atributos de identificación complementarios.

---

# 160. Evitar contaminación entre proyectos

Una búsqueda:

```bash
nodkray memory search "database"
```

deberá limitarse al proyecto actual por defecto.

Para buscar globalmente:

```bash
nodkray memory search --global "database"
```

---

# 161. Privacidad

La memoria será local por defecto.

No se enviará automáticamente a:

```text
JEV
LLM
Turso
remote API
```

La información solo podrá salir del host mediante una configuración explícita.

---

# 162. Auditoría

Cada decisión importante deberá poder relacionarse con:

```text
session
task
provider
decision
workflow
```

Ejemplo:

```text
task_123
 ├── decision_01
 ├── session_10
 ├── worker_01
 └── review_09
```

---

# 163. Reproducibilidad

Una sesión deberá poder reconstruirse a partir de:

```text
project
config snapshot
task
workflow
agents
versions
events
review
```

---

# 164. Snapshot de configuración

Al comenzar una sesión se almacenará una instantánea lógica de:

```text
frontier agent
worker roles
execution backend
workflow
review policy
enabled integrations
```

Cambios posteriores en la configuración no deben alterar retrospectivamente una sesión histórica.

---

# 165. Política de cambios de configuración

Los cambios en configuración afectan:

```text
future tasks
```

No deben modificar:

```text
completed sessions
```

---

# 166. Diagnóstico de incompatibilidades

Si una configuración solicita:

```yaml
execution:
  backend: herdr
```

pero Herdr no está instalado:

```text
status = unavailable
```

NodKray deberá ofrecer explícitamente:

```text
continue with console backend
```

cuando la configuración lo permita.

---

# 167. Integración con terminal

Toda operación crítica deberá poder ejecutarse desde terminal.

No debe existir una operación V1 que solo sea posible desde la interfaz remota.

---

# 168. Control remoto como cliente secundario

El control remoto no debe contener lógica de negocio.

```text
Phone
 │
 ▼
Remote API
 │
 ▼
Application Core
```

No:

```text
Phone
 │
 └── own orchestration logic
```

---

# 169. Arquitectura del binario

NodKray debe poder distribuirse como:

```text
single executable
```

con dependencias externas únicamente para:

```text
configured agents
Git
optional Herdr
optional MCP
optional Spec-Kit runtime
```

---

# 170. Filosofía de instalación

El objetivo es que:

```bash
nodkray init
```

sea suficiente para adoptar NodKray sobre un repositorio existente.

El usuario no deberá tener que editar manualmente múltiples archivos para el caso básico.

---

# 171. Flujo de adopción

```text
Existing repository
        │
        ▼
nodkray init
        │
        ▼
Detect
        │
        ▼
Configure
        │
        ▼
nodkray doctor
        │
        ▼
Ready
```

---

# 172. Checklist de implementación

## Core

* [ ] Rust workspace
* [ ] CLI
* [ ] configuración
* [ ] logging
* [ ] error model
* [ ] registries
* [ ] project discovery

## Agents

* [ ] Agent trait
* [ ] Cursor adapter
* [ ] OpenCode adapter
* [ ] Claude adapter
* [ ] Codex adapter
* [ ] Pi adapter
* [ ] Generic adapter

## Decision

* [ ] effort model
* [ ] local decision provider
* [ ] JEV provider
* [ ] workflow selection
* [ ] review depth selection

## Workflows

* [ ] ST
* [ ] ODD
* [ ] SDD
* [ ] Spec-Kit adapter
* [ ] constitution detection

## Execution

* [ ] Execution trait
* [ ] Herdr backend
* [ ] Console backend
* [ ] worker lifecycle
* [ ] worktrees
* [ ] DAG scheduling

## Memory

* [ ] SQLite
* [ ] migrations
* [ ] FTS5
* [ ] search
* [ ] timeline
* [ ] project isolation

## RDD

* [ ] Review engine
* [ ] Git validator
* [ ] Test validator
* [ ] rule validator
* [ ] Sentrux validator
* [ ] reviewer agent
* [ ] verdict engine
* [ ] remediation loop
* [ ] merge engine

## Integration

* [ ] Serena discovery
* [ ] CodeGraph discovery
* [ ] Sentrux discovery
* [ ] MCP configuration
* [ ] Skills installation
* [ ] AGENTS.md integration

## Remote

* [ ] Control API
* [ ] Authentication
* [ ] event stream
* [ ] task control
* [ ] worker status
* [ ] review status

---

# 173. Definition of Done de NodKray V1

NodKray V1 se considerará funcional cuando pueda realizar:

```text
1. instalarse con un binario;
2. inicializar un repositorio existente;
3. detectar al menos un agente;
4. configurar frontier y workers;
5. crear una tarea;
6. seleccionar ST/ODD/SDD;
7. ejecutar ST;
8. ejecutar ODD;
9. ejecutar SDD mediante Spec-Kit;
10. crear/reutilizar Constitution;
11. paralelizar workers;
12. crear worktrees;
13. utilizar Herdr;
14. utilizar ConsoleBackend;
15. almacenar memoria;
16. buscar memoria;
17. ejecutar RDD;
18. ejecutar Sentrux opcionalmente;
19. bloquear violaciones configuradas;
20. integrar cambios aprobados;
21. recuperar estado de una sesión;
22. operar completamente desde CLI;
23. operar sin JEV;
24. funcionar sin Herdr mediante ConsoleBackend;
25. no romper instalaciones existentes.
```

---

# 174. Contrato final del sistema

La arquitectura completa puede resumirse en:

```text
                        USER
                         │
                         ▼
                  FRONTIER AGENT
                         │
                         ▼
                     NODKRAY
                         │
        ┌────────────────┼────────────────┐
        │                │                │
        ▼                ▼                ▼
     DECISION          MEMORY          CONFIG
        │                │                │
        ▼                │                │
   ST / ODD / SDD       SQLite            │
        │                │                │
        │                ▼                │
        │               CLI               │
        │                                 │
        ▼                                 │
      WORKFLOW                            │
        │                                 │
        ▼                                 │
   AGENT ROLES                            │
        │                                 │
        ▼                                 │
 EXECUTION BACKEND                        │
    │          │                          │
    ▼          ▼                          │
  HERDR      CONSOLE                      │
    │          │                          │
    └────┬─────┘                          │
         ▼                                │
      WORKERS                             │
         │                                │
         ▼                                │
      WORKTREES                           │
         │                                │
         ▼                                │
        RDD ◄────────────── Rules ────────┘
         │
   ┌─────┼──────────┐
   ▼     ▼          ▼
  Git   Tests     Sentrux
   │     │          │
   └─────┼──────────┘
         ▼
      VERDICT
         │
    ┌────┴─────┐
    ▼          ▼
  MERGE      BLOCK
    │          │
    ▼          ▼
 PROJECT    REMEDIATION
```

---

# 175. Decisiones arquitectónicas cerradas

| Decisión                     | Estado                                                   |
| ---------------------------- | -------------------------------------------------------- |
| Core en Rust                 | **Cerrado**                                              |
| CLI como interfaz primaria   | **Cerrado**                                              |
| TUI propia de memoria        | **No incluir en V1**                                     |
| Memoria                      | **SQLite + FTS5**                                        |
| Turso                        | **No como backend primario; preparado para futuro sync** |
| Adapter por agente frontera  | **Cerrado**                                              |
| Agente frontera configurable | **Cerrado**                                              |
| Workers por rol              | **Cerrado**                                              |
| Cursor                       | **Soportado mediante adapter**                           |
| OpenCode                     | **Soportado mediante adapter**                           |
| Claude                       | **Soportado mediante adapter**                           |
| Codex                        | **Soportado mediante adapter**                           |
| Pi                           | **Soportado mediante adapter**                           |
| Generic agent                | **Soportado**                                            |
| Herdr                        | **Backend de ejecución opcional**                        |
| Consola                      | **Backend de ejecución alternativo**                     |
| Worktrees                    | **Mecanismo de aislamiento**                             |
| ST                           | **Workflow NodKray**                                     |
| ODD                          | **Workflow NodKray**                                     |
| SDD                          | **Delegado a Spec-Kit**                                  |
| Constitution                 | **Una por proyecto, reutilizable**                       |
| Spec-Kit                     | **Instalado/provisionado por NodKray**                   |
| MCP wrappers                 | **No duplicar MCP existentes**                           |
| Serena                       | **Integración externa**                                  |
| CodeGraph                    | **Integración externa**                                  |
| Sentrux                      | **Checker opcional de RDD**                              |
| RDD                          | **Workflow propio de NodKray**                           |
| JEV                          | **Proveedor de decisión opcional**                       |
| JEV sin configurar           | **Fallback local**                                       |
| YOLO                         | **Flag de ejecución**                                    |
| AGENTS.md                    | **Integración opcional/idempotente**                     |
| Remote control               | **Opcional y separado del core**                         |
| Offline-first                | **Sí**                                                   |
| Cloud obligatorio            | **No**                                                   |

---

# 176. Resultado arquitectónico

NodKray no pretende convertirse en otro agente de código.

Su posición en el ecosistema es deliberadamente distinta:

```text
                  MODELS
                    │
                    ▼
                 AGENTS
                    │
        ┌───────────┼───────────┐
        ▼           ▼           ▼
      Cursor      Claude      Codex
        │           │           │
        └───────────┼───────────┘
                    ▼
                  NODKRAY
                    │
       ┌────────────┼────────────┐
       ▼            ▼            ▼
   WORKFLOW      EXECUTION      REVIEW
       │            │            │
       ▼            ▼            ▼
   Spec-Kit       Herdr       Sentrux
       │            │            │
       └────────────┼────────────┘
                    ▼
                  Git
                    │
                    ▼
                PROJECT
```

La responsabilidad fundamental de NodKray es entonces:

> **decidir, coordinar, aislar, recordar y verificar.**

No sustituye al agente.

No sustituye al MCP.

No sustituye a Spec-Kit.

No sustituye al runtime de ejecución.

Los conecta mediante interfaces pequeñas y estables.

Ese límite constituye la principal decisión arquitectónica del sistema.
