# Data Model: Memory Service

**Feature**: 001-surrealdb-rig-tokio-stack  
**Date**: 2026-03-06

## Entities

### Memory (memory record)

A single unit of stored AI memory: a fact, observation, or interaction that can be retrieved by identity or by semantic/full-text search.

| Attribute | Type | Rules |
|-----------|------|--------|
| `id` | string (UUID or record id) | Required; unique; assigned by server if not provided |
| `content` | string | Required; the memory text (e.g. extracted from messages or user input) |
| `user_id` | string | Required; scopes memory to a user (or agent/session) for multi-tenant isolation |
| `embedding` | float[] (vector) | Optional at input; server computes and stores for semantic search |
| `metadata` | object | Optional; key-value for filtering or display (e.g. source, created_by) |
| `created_at` | datetime | Set by server; ISO 8601 |
| `updated_at` | datetime | Set by server on create/update |

**Validation**:

- `content`: non-empty string, max length bounded (e.g. 64 KiB) to avoid abuse.
- `user_id`: non-empty string, format not enforced (opaque identifier).
- `metadata`: shallow key-value; size limit to be defined in implementation.

**State**: Created → (optional Update) → optionally Deleted. No explicit state machine; soft-delete optional later.

### Add input (messages or text)

When adding memories, the client may send:

- **Messages**: list of `{ "role": "user"|"assistant"|"system", "content": "..." }`; server extracts or summarizes into one or more memory records (implementation may use LLM/summarization via Rig).
- **Text**: single string; stored as one memory record with optional metadata.

Server generates `id`, `created_at`, `updated_at`, and optionally `embedding` (if embedding is enabled).

### Search result item

Returned by search endpoints; subset of Memory plus score when relevant:

- `id`, `content`, `user_id`, `metadata`, `created_at`
- `score` (optional): similarity or relevance score when doing vector or hybrid search

---

## Relationships

- **User → Memories**: One-to-many. All queries (add/search/get/delete) are scoped by `user_id` so that tenants do not see each other’s data.
- No required graph relations between memories for MVP; SurrealDB can later support graph edges if we add “memory links” or similar.

---

## Storage (SurrealDB)

- **Table (or equivalent)**: `memories` with fields above. Vector index on `embedding` for similarity search; full-text index on `content` for hybrid search.
- **Scoping**: All queries MUST filter by `user_id` (and optionally by `metadata` filters) to enforce isolation.
- **Local-first**: Default deployment uses SurrealDB embedded or single-node; no mandatory remote DB.

---

## Traceability (Constitution V)

- Every write (add/update) and read (search/get) SHOULD include a **trace_id** (or request_id) in response headers or body for reproducibility and support.
- Error responses MUST include a stable **error code** and optional trace_id.
