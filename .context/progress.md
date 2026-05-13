## Codebase Patterns
- Add request compatibility is handled with untagged Rust DTO variants; the handler should normalize request variants into one internal representation before shared write logic runs.
- Python SDK high-level methods should preserve API payload shape when the Rust API supports multiple request forms, rather than flattening inputs before sending.

## US-001: Wire extraction into add
- Implemented deterministic Rust fact extraction in `mem1-server/src/memory/extraction.rs` and wired `POST /memories` to store one memory per extracted fact.
- Preserved `ByContent` and `ByMessages` payload compatibility, including optional metadata on message adds.
- Added extraction metadata to stored memories: `source_text`, `source_role`, `source_index`, `language`, and `extractor_version`.
- Added fallback storage of the trimmed original content when extraction returns no facts.
- Updated the Python SDK to send message-list adds as `messages` payloads and to preserve multi-result add responses.
- Added Rust DTO, extraction, and handler tests plus Python SDK fan-out tests.
- Files changed: `.context/progress.md`, `mem1-server/src/api/dto.rs`, `mem1-server/src/api/handlers.rs`, `mem1-server/src/api/middleware.rs`, `mem1-server/src/app_state.rs`, `mem1-server/src/error.rs`, `mem1-server/src/memory/embedding.rs`, `mem1-server/src/memory/extraction.rs`, `mem1-server/src/memory/local_embed.rs`, `mem1-server/src/memory/mod.rs`, `mem1-server/src/memory/model.rs`, `python/src/mem1/client.py`, `python/src/mem1/memory.py`, `python/src/mem1/models.py`, `python/tests/test_memory_api.py`.
- **Learnings for future iterations:**
  - Patterns discovered: handler tests can call Axum handlers directly with a temp SurrealDB store and `Embedder::Off` for deterministic API behavior checks.
  - Gotchas encountered: the repository Rust gate enforces clippy and rustfmt across pre-existing files, so running the gate may require small formatting/clippy cleanup outside the story's primary files.
