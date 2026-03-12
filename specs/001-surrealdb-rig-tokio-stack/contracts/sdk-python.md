# Contract: Python SDK (mem0-style)

**Feature**: 001-surrealdb-rig-tokio-stack  
**Date**: 2026-03-06

## Purpose

Provide a Python client that mirrors [mem0](https://github.com/mem0ai/mem0)-style usage for RAG and agent integration: instantiate a memory object, add messages or text (keyed by user_id), and search by query. The SDK calls the Rust server over HTTP.

## Public surface

### Class: `Memory` (or `Mem1` to avoid name clash with mem0)

**Constructor**:

- `Memory(base_url: str = "http://127.0.0.1:8080", api_key: str | None = None, ...)`
- `base_url`: server base URL (no trailing slash).
- `api_key`: optional; if server requires auth, send in header (e.g. `Authorization: Bearer <api_key>`).

### Methods

- **add**
  - Signature: `add(messages: list[dict] | str, user_id: str = "default_user", **kwargs) -> dict`
  - If `messages` is a list of `{"role": "...", "content": "..."}`, POST to server add endpoint (messages form).
  - If `messages` is a string, POST as single content with optional `metadata=kwargs`.
  - Returns: dict with `results` list of created memory items (id, content, user_id, metadata, created_at).

- **search**
  - Signature: `search(query: str, user_id: str = "default_user", limit: int = 10, **kwargs) -> dict`
  - POST (or GET) to server search endpoint.
  - Returns: dict with `results` list of memory items (id, content, user_id, metadata, created_at, optional score).

- **get** (optional for MVP)
  - Signature: `get(memory_id: str, user_id: str = "default_user") -> dict | None`
  - GET single memory by id; returns item or None.

- **delete** (optional for MVP)
  - Signature: `delete(memory_id: str, user_id: str = "default_user") -> bool`
  - DELETE single memory; returns True on success.

## Response shapes (aligned with server)

- Add response: `{ "results": [ { "id", "content", "user_id", "metadata", "created_at" }, ... ] }`
- Search response: `{ "results": [ { "id", "content", "user_id", "metadata", "created_at", "score"? }, ... ] }`
- Errors: raise a client exception with `code`, `message`, and optional `trace_id` from server error body.

## Dependency

- HTTP client: `httpx` (sync or async as needed). No direct SurrealDB or Rig dependency in SDK; all logic on server.

## Parity with Constitution III

Every server API capability exposed to clients (add, search, get, delete) MUST have an equivalent Python SDK method or documented workaround (e.g. raw request helper) so that Python users do not need to call HTTP by hand for standard flows.
