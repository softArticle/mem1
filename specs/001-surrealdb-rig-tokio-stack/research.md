# Research: Memory Service API and SDK Design

**Feature**: 001-surrealdb-rig-tokio-stack  
**Date**: 2026-03-06

## 1. Server API: HTTP vs RPC

**Decision**: Provide **HTTP REST** as the primary and first-delivered API. Optionally add **gRPC** later for performance-sensitive or streaming use cases.

**Rationale**:

- **HTTP REST**: Easy to consume from any language, simple to debug (curl, browser), aligns with mem0’s REST-style usage and common RAG/agent integrations. Constitution prefers “transport-neutral contracts (e.g., HTTP/gRPC + schema)”; REST + JSON schema or OpenAPI satisfies this.
- **gRPC**: Better for high-throughput, streaming, and strong typing; can be added once REST is stable and if metrics justify it.

**Alternatives considered**:

- HTTP-only: Keeps stack minimal; we may add gRPC later without breaking REST clients.
- gRPC-first: Higher barrier for Python/RAG users and tooling; deferred.
- Custom RPC (e.g. custom protocol): No standard tooling; rejected.

**Implementation note**: Server will expose at least: `POST /memories` (add), `GET /memories/search` (or `POST /memories/search` with body), `GET /memories/{id}`, optional `DELETE /memories/{id}`. Auth and `user_id`/scoping in request (header or body). See [contracts](./contracts/).

---

## 2. Python SDK: mem0-Style RAG Integration

**Decision**: Python SDK exposes a **mem0-like** surface: a `Memory` (or equivalent) object with `add(...)` and `search(...)` keyed by `user_id`, plus minimal config (server URL, optional API key). RAG and agent code can “add” messages or text and “search” by query with limit; results are lists of memory items (e.g. `{ "memory": "...", "id": "...", "metadata": {...} }`).

**Rationale**:

- [mem0](https://github.com/mem0ai/mem0) is widely used for AI memory and RAG; matching its mental model (add/search, user_id, simple results) reduces friction for adoption.
- Our backend is Rust + local server; the SDK is a thin client over HTTP. Parity is at the *API capability* level (add, search, get, delete) and *usage pattern* (user-scoped, limit, query), not implementation detail.

**Alternatives considered**:

- Fully custom SDK: Would work but miss familiarity of mem0-style API; rejected for first release.
- Exact mem0 API clone: We keep similar method names and semantics (add, search, user_id, limit) but request/response shapes can differ slightly to match our server schema; documented in contracts.

**Key surface (to be fixed in contracts)**:

- `Memory(base_url=..., ...)`  
- `memory.add(messages | text, user_id=..., ...)` → server `POST /memories` (or batch endpoint).  
- `memory.search(query=..., user_id=..., limit=...)` → server search endpoint; returns list of memory items.  
- Optional: `memory.get(id)`, `memory.delete(id)` for parity with server.

---

## 3. SurrealDB + Rig Integration (recap)

**Decision**: SurrealDB for all persistent and vector storage; Rig for embeddings (and optional completion). No separate vector DB; SurrealDB’s vector and full-text capabilities cover the spec.

**Rationale**: Spec and assumptions already fix this; research only confirms single storage engine and one LLM/embedding stack (Rig) for consistency and simpler ops.

**Alternatives considered**: Separate vector DB (e.g. Qdrant) would add a component; SurrealDB’s unified model is preferred for local-first and single-process deployment.

---

## 4. Resolved “NEEDS CLARIFICATION” Items

- **API transport**: HTTP REST first; gRPC optional later (no NEEDS CLARIFICATION in spec; plan resolves).
- **SDK style**: mem0-style add/search and user_id scoping (user plan input + research).
- **Server scope**: Local single-machine server (user plan input); no distributed requirement for MVP.

All Phase 0 unknowns are resolved; Phase 1 can proceed.
