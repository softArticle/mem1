# Contract: HTTP API (Rust Server)

**Feature**: 001-surrealdb-rig-tokio-stack  
**Date**: 2026-03-06

## Base

- **Transport**: HTTP/1.1 over TCP (TLS optional for local; recommended for production).
- **Base URL**: Configurable (e.g. `http://127.0.0.1:8080`). All paths below are relative to base.
- **Content type**: `application/json` for request and response bodies.
- **Scoping**: All operations are scoped by `user_id` (path, query, or body as specified per endpoint).

## Endpoints

### Add memories

- **Method**: `POST`
- **Path**: `/memories`
- **Body** (one of):
  - `{ "user_id": "<string>", "messages": [ { "role": "user"|"assistant"|"system", "content": "<string>" }, ... ] }`
  - `{ "user_id": "<string>", "content": "<string>", "metadata": { ... }? }`
- **Response**: `201 Created` + body `{ "results": [ { "id": "<id>", "content": "<string>", "user_id": "<string>", "metadata": {}, "created_at": "<ISO8601>" }, ... ] }`
- **Errors**: `400` invalid input; `500` server/storage/embedding error (stable error code + optional `trace_id`).

### Search memories

- **Method**: `POST` (preferred) or `GET`
- **Path**: `/memories/search`
- **Query (if GET)**: `user_id`, `query`, `limit` (optional, default e.g. 10).
- **Body (if POST)**: `{ "user_id": "<string>", "query": "<string>", "limit": <number>? }`
- **Response**: `200 OK` + `{ "results": [ { "id": "<id>", "content": "<string>", "user_id": "<string>", "metadata": {}, "created_at": "<ISO8601>", "score": <number>? }, ... ] }`
- **Errors**: `400` missing user_id/query; `500` server error.

### Get memory by id

- **Method**: `GET`
- **Path**: `/memories/{id}`
- **Query**: `user_id` (required for scoping).
- **Response**: `200 OK` + single memory object, or `404` if not found / wrong user.

### Delete memory (optional for MVP)

- **Method**: `DELETE`
- **Path**: `/memories/{id}`
- **Query**: `user_id` (required).
- **Response**: `204 No Content` or `404`.

## Headers

- **Request**: `Content-Type: application/json`; optional `Authorization` if auth is added later; optional `X-Trace-Id` for client-provided trace.
- **Response**: `X-Trace-Id` or `trace_id` in body on success/error when available.

## Error body shape

- `{ "code": "<stable_code>", "message": "<human-readable>", "trace_id": "<string>"? }`

Stable codes to be defined in implementation (e.g. `STORAGE_ERROR`, `EMBEDDING_ERROR`, `INVALID_INPUT`).
