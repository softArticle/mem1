# Tasks: Memory Service (Rust Server + Python SDK)

**Input**: Design documents from `specs/001-surrealdb-rig-tokio-stack/`  
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Constitution IV requires unit tests (Rust), integration tests (memory flows), and contract tests (API + SDK). Test tasks are included below.

**Organization**: Tasks are grouped by user story (US1 → US2 → US3), then Python SDK parity, then Polish.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1, US2, US3 for user story phases; no label for Setup, Foundational, SDK, Polish
- Include exact file paths in descriptions

## Path Conventions

- **Rust server**: `mem1-server/` (src/, tests/)
- **Python SDK**: `python/` (src/mem1/, tests/)
- Paths below use these roots per plan.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [x] T001 Create project structure per plan: mem1-server/src with api/, memory/, storage/ subdirs; mem1-server/tests; python/src/mem1; python/tests
- [x] T002 Initialize Rust crate: mem1-server/Cargo.toml with tokio, surrealdb, serde, uuid, axum (or equivalent); add rust-toolchain.toml at repo root or in mem1-server
- [x] T003 [P] Initialize Python package: python/pyproject.toml with package name mem1 and deps (httpx, pydantic); python/src/mem1/__init__.py
- [x] T004 [P] Configure Rust lint/format: .rustfmt.toml and clippy in mem1-server; add cargo test and format check to CI or justfile

**Checkpoint**: Repo layout and dependencies ready

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T005 Implement SurrealDB connection and bootstrap (embedded or file backend) in mem1-server/src/storage/mod.rs or mem1-server/src/storage/db.rs
- [x] T006 Define error type and stable error codes (INVALID_INPUT, STORAGE_ERROR, EMBEDDING_ERROR) in mem1-server/src/error.rs
- [x] T007 [P] Add structured logging (tracing) and X-Trace-Id request/response support in mem1-server/src/api/middleware.rs or mem1-server/src/main.rs

**Checkpoint**: Foundation ready - user story implementation can begin

---

## Phase 3: User Story 1 - Unified Storage for Memory Data (Priority: P1) 🎯 MVP

**Goal**: Single storage engine (SurrealDB) with memory records, vector and full-text search, local/embedded deployment.

**Independent Test**: Run SurrealDB (embedded or local), add memories via storage layer, retrieve by id and by search (full-text and vector); no HTTP required for this checkpoint.

### Implementation for User Story 1

- [x] T008 [P] [US1] Create Memory struct (id, content, user_id, embedding, metadata, created_at, updated_at) in mem1-server/src/memory/model.rs per data-model.md
- [x] T009 [US1] Define SurrealDB schema for memories table with vector index on embedding and full-text index on content in mem1-server (init script or migration)
- [x] T010 [US1] Implement storage layer: add memory, get by id (scoped by user_id), search (full-text and vector) in mem1-server/src/storage/memory.rs
- [ ] T011 [US1] Unit tests for storage (add, get, search) in mem1-server/tests/storage_test.rs or mem1-server/src/storage/tests.rs
- [ ] T012 [US1] Integration test: bootstrap SurrealDB, run add then get then search in mem1-server/tests/integration/storage.rs

**Checkpoint**: User Story 1 complete - storage layer works independently; can add/get/search memories without HTTP

---

## Phase 4: User Story 2 - Async Service Runtime + API (Priority: P2)

**Goal**: Tokio-based HTTP server exposing POST /memories, POST /memories/search, GET /memories/:id, DELETE /memories/:id with trace_id and stable error codes.

**Independent Test**: Start server, send concurrent add and search requests via HTTP; verify responses and no deadlock.

### Implementation for User Story 2

- [ ] T013 [US2] Add axum (or chosen HTTP) server and route registration in mem1-server/src/main.rs and mem1-server/src/api/mod.rs
- [ ] T014 [US2] Implement POST /memories handler (body: user_id + messages or user_id + content + metadata) and POST /memories/search in mem1-server/src/api/handlers.rs, wired to storage
- [ ] T015 [US2] Implement GET /memories/:id and DELETE /memories/:id with user_id query in mem1-server/src/api/handlers.rs
- [ ] T016 [US2] Add request/response DTOs (serde) and validation (user_id, content required; content length limit) in mem1-server/src/api/dto.rs or handlers
- [ ] T017 [US2] Return error body { code, message, trace_id } and X-Trace-Id header on responses in mem1-server/src/api/
- [x] T013 [US2] Add axum (or chosen HTTP) server and route registration in mem1-server/src/main.rs and mem1-server/src/api/mod.rs
- [x] T014 [US2] Implement POST /memories handler (body: user_id + messages or user_id + content + metadata) and POST /memories/search in mem1-server/src/api/handlers.rs, wired to storage
- [x] T015 [US2] Implement GET /memories/:id and DELETE /memories/:id with user_id query in mem1-server/src/api/handlers.rs
- [x] T016 [US2] Add request/response DTOs (serde) and validation (user_id, content required; content length limit) in mem1-server/src/api/dto.rs or handlers
- [x] T017 [US2] Return error body { code, message, trace_id } and X-Trace-Id header on responses in mem1-server/src/api/
- [ ] T018 [US2] Contract test for HTTP API: POST add, POST search, GET by id (and optional DELETE) in mem1-server/tests/contract/api_http.rs or python/tests calling server
- [ ] T019 [US2] Integration test: start server, concurrent add and search over HTTP in mem1-server/tests/integration/api.rs

**Checkpoint**: User Story 2 complete - server exposes API; clients can add/search via HTTP

---

## Phase 5: User Story 3 - LLM and RAG Toolchain Integration (Priority: P3)

**Goal**: Rig-based embedding on add; vector similarity in search; configurable provider.

**Independent Test**: Add memory with embedding enabled, run semantic search; verify results and error handling for provider failures.

### Implementation for User Story 3

- [x] T020 [US3] Add Rig dependency and embedding client/config (e.g. env or config file for provider/model) in mem1-server/Cargo.toml and mem1-server/src/memory/embedding.rs
- [x] T021 [US3] On add: optionally compute embedding via Rig and store vector in memory record in mem1-server/src/memory/service.rs or handlers
- [x] T022 [US3] On search: when query embedding is available, run vector similarity and combine with full-text (hybrid) in mem1-server/src/storage/memory.rs or memory service
- [ ] T023 [US3] Integration test: add with embedding, semantic search (mock or real Rig provider) in mem1-server/tests/integration/embedding.rs

**Checkpoint**: User Story 3 complete - end-to-end add with embedding and semantic/hybrid search

---

## Phase 6: Python SDK (Parity with Server API)

**Goal**: mem0-style Memory class (add, search, get, delete) calling server over HTTP per contracts/sdk-python.md.

**Independent Test**: Start server, use Python SDK to add and search; verify response shapes and errors.

- [x] T024 [P] Create HTTP client (httpx, base_url, post/get/delete, error parsing) in python/src/mem1/client.py
- [x] T025 [P] Create request/response models (pydantic) for add/search/get in python/src/mem1/models.py
- [x] T026 Implement Memory class (add(messages|str, user_id), search(query, user_id, limit), get, delete) in python/src/mem1/memory.py
- [ ] T027 Contract test for Python SDK: add and search against running server or mock in python/tests/test_memory_contract.py
- [ ] T028 Integration test: run server, call Memory.add and Memory.search from Python in python/tests/test_integration.py

**Checkpoint**: Python SDK parity - add, search (and get/delete) work from Python

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: CI, docs, quickstart validation

- [ ] T029 [P] Add CI workflow: cargo test and cargo clippy in mem1-server; pytest in python (e.g. .github/workflows/ci.yml or justfile)
- [ ] T030 Run quickstart.md validation: start server, run Python snippet from quickstart, verify add and search
- [ ] T031 [P] Add README for mem1-server and python (or single repo README) with build/run and link to specs/001-surrealdb-rig-tokio-stack/quickstart.md

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup - BLOCKS all user stories
- **US1 (Phase 3)**: Depends on Foundational - storage only
- **US2 (Phase 4)**: Depends on US1 (storage layer) - adds HTTP and concurrency
- **US3 (Phase 5)**: Depends on US2 (API) and US1 (storage) - adds embeddings
- **Python SDK (Phase 6)**: Depends on US2 (API contract stable) - can start after US2
- **Polish (Phase 7)**: Depends on US1–US3 and SDK complete

### User Story Dependencies

- **US1**: After Foundational only - no other story
- **US2**: After US1 (needs storage to wire handlers)
- **US3**: After US1 and US2 (needs storage and API to add embedding path)

### Within Each Phase

- Models/structs before services; services before handlers
- Tests can be written alongside implementation (constitution: tests required)

### Parallel Opportunities

- T003 and T004 (Setup); T024 and T025 (SDK client and models); T029 and T031 (CI and README)
- Within US1: T008 (Memory struct) is [P] vs T009/T010

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup  
2. Complete Phase 2: Foundational  
3. Complete Phase 3: User Story 1 (storage only)  
4. **STOP and VALIDATE**: Run storage integration test  
5. Then proceed to US2 for HTTP API and end-to-end demo

### Incremental Delivery

1. Setup + Foundational → Foundation ready  
2. US1 → Storage working (MVP for “write/read without server”)  
3. US2 → HTTP API → Deploy/demo server + curl  
4. US3 → Embeddings → Semantic search  
5. SDK → Python parity → RAG integration demo  
6. Polish → CI and quickstart

### Suggested MVP Scope

- **Minimum**: Phase 1 + Phase 2 + Phase 3 (US1). Delivers: Rust project, SurrealDB storage, add/get/search at storage layer (no HTTP yet).  
- **First deployable**: Phase 1–4 (through US2). Delivers: local server with POST/GET /memories and /memories/search; test with curl or simple client.  
- **Full spec**: All phases through Phase 7.

---

## Notes

- [P] tasks = different files, no dependencies on other tasks in same phase
- [US1/US2/US3] maps task to spec user story for traceability
- Constitution IV: unit tests (Rust), integration tests (memory flows), contract tests (API + SDK) are required
- Commit after each task or logical group; re-run tests after each phase
