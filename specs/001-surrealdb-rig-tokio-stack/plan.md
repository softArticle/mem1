# Implementation Plan: Memory Service (Rust Server + Python SDK)

**Branch**: `001-surrealdb-rig-tokio-stack` | **Date**: 2026-03-06 | **Spec**: [spec.md](./spec.md)  
**Input**: Feature specification from `specs/001-surrealdb-rig-tokio-stack/spec.md`

**User plan input**: (1) Server (Rust): local single-machine service providing AI memory capability via API (RPC or HTTP). (2) SDK (Python): Python SDK with RAG-friendly integration, reference [mem0](https://github.com/mem0ai/mem0).

## Summary

Deliver a **local-first AI memory service** with a **Rust server** (SurrealDB, Tokio, Rig) exposing memory over an API, and a **Python SDK** that mirrors mem0-style usage (add/search by user, RAG-friendly) so developers can plug memory into RAG and agent workflows. The server runs as a single local process; the SDK talks to it over HTTP (primary) or optional RPC. All storage and embedding run locally by default; external LLM/embedding is opt-in.

## Technical Context

**Language/Version**: Rust (stable toolchain, pinned in repo); Python 3.10+ for SDK  
**Primary Dependencies**: Tokio (async runtime), SurrealDB (storage), Rig (LLM/embeddings); axion/tonic or similar for HTTP/gRPC server; Python: httpx, pydantic  
**Storage**: SurrealDB (embedded or single-node; document + vector + full-text/hybrid)  
**Testing**: `cargo test` (Rust), `pytest` (Python SDK); contract tests for API and SDK  
**Target Platform**: Local single host (Linux, macOS, Windows); server single process, optional embedded DB  
**Project Type**: Backend service (Rust) + client library (Python SDK)  
**Performance Goals**: p95 &lt; 2s for typical add/search; support 10+ concurrent clients without deadlock  
**Constraints**: Local-first (no mandatory cloud); sensitive data encrypted at rest and redacted in logs; API and storage versioned for compatibility  
**Scale/Scope**: Single-node server; Python SDK as primary integration surface for RAG/agents

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Evidence |
|----------|--------|----------|
| I. Rust-First Core | Pass | Server and core memory logic in Rust; Python is SDK consumer only |
| II. Local-First and Privacy by Default | Pass | Server runs locally; storage local (SurrealDB); external LLM/embedding opt-in; encryption/redaction in scope |
| III. Python SDK Parity Contract | Pass | Plan includes Python SDK with parity to server API; mem0-style surface for RAG integration |
| IV. Test and Quality Gates | Pass | Unit + integration tests in Rust; contract tests for API and SDK; CI gates for test/lint/format |
| V. Deterministic Observability and Compatibility | Pass | Structured logs and stable error codes; trace IDs on operations; versioning and migration path for API/storage |

No violations. Complexity table left empty.

**Post–Phase 1 re-check**: All design artifacts (data-model, contracts, quickstart) align with the five principles. SDK contract enforces parity (III); API and data model support traceability and error codes (V). No changes to Constitution Check outcome.

## Project Structure

### Documentation (this feature)

```text
specs/001-surrealdb-rig-tokio-stack/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (API + SDK contracts)
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
# Rust server (single binary, library crates as needed)
mem1-server/             # or crates/server, or src at root
├── src/
│   ├── main.rs
│   ├── api/             # HTTP (and optional gRPC) handlers
│   ├── memory/          # memory service, storage, embedding orchestration
│   ├── storage/         # SurrealDB access
│   └── lib.rs
├── Cargo.toml
└── tests/

# Python SDK (client library)
python/
├── pyproject.toml
├── src/
│   └── mem1/
│       ├── __init__.py
│       ├── client.py    # HTTP client to server
│       ├── memory.py    # mem0-style Memory class (add, search, ...)
│       └── models.py    # request/response models
└── tests/

# Optional: workspace root if multiple Rust crates
# Cargo.toml (workspace), rust-toolchain.toml
```

**Structure Decision**: Rust server in a dedicated directory (e.g. `mem1-server/` or root `src/`); Python SDK in `python/` with package name `mem1`. Single repo; server and SDK share API contracts under `specs/.../contracts/`. Exact crate names and paths to be finalized in tasks.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

(No violations; table omitted.)
