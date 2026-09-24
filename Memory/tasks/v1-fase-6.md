# NodKray V1 — Fase 6 (main)

## Owner

Agente en `/home/juan-manuel-quiazua/Documentos/NodKray` (`main`).

## Hecho

- Control API opcional: `nodkray serve`, bind loopback, token `NODKRAY_REMOTE_TOKEN`, HTTP/JSON + SSE
- Endpoints: `/status`, `POST /tasks`, `GET /tasks/:id`, `/workers`, `/reviews`, `POST /tasks/:id/cancel`, `/events`, `/events/stream`
- JEV `DecisionProvider` con validación de schema §75 y fallback local
- Snapshot de sesión = config + versions (nodkray, agentes, speckit, herdr, sentrux)
- Constitución propia en `.specify/memory/constitution.md` (distinta de la de targets)

## Notas

- `serve` habilita la API solo para el proceso; no escribe `remote.enabled`
- Credenciales solo en env; nunca en SQLite
- Cliente remoto = HTTP; cero lógica de negocio
