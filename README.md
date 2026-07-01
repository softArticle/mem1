# mem1

A local-first AI memory service, written in Rust. mem1 stores conversational
memories and retrieves them by meaning — a drop-in memory backend for AI agents
and assistants, with no external services required for its core path.

It began as a Rust rewrite of [mem0](https://github.com/mem0ai/mem0) and, under
an identical same-judge evaluation on the LOCOMO long-term-conversation
benchmark, its retrieval outperforms mem0's open-source build.

## Highlights

- **Local-first, embedded inference.** Embeddings run in-process via pure-Rust
  ONNX (tract) or candle — no Python, no native libs, no API keys on the default
  path. Ships an auto-downloading `all-MiniLM-L6-v2` (384-dim) embedder.
- **Hybrid retrieval.** Keyword (full-text) + vector (HNSW cosine) + entity-graph
  candidates, fused with Reciprocal Rank Fusion, then diversified with a
  protected-prefix MMR pass. All knobs are env-tunable.
- **Two front-ends.** An HTTP API (axum) and a stdio **MCP server** so
  Claude Code, Codex, OpenCode, Pi, and other MCP clients can use mem1 as their
  long-term memory backend.
- **Multilingual.** Optional in-process **Qwen3-Embedding-0.6B** (via candle)
  lifts non-English recall substantially (on a Chinese eval set, 0.39 → 0.79).
- **Storage.** Embedded SurrealDB (RocksDB) for documents, full-text, vectors,
  and the entity graph — a single local file, no database to run.

## Workspace layout

| Crate / dir | What it is |
| --- | --- |
| [`mem1-server/`](mem1-server/README.md) | The Rust memory service + HTTP API |
| [`mem1-mcp/`](mem1-mcp/README.md) | stdio MCP server (embeds the store in-process) |
| `python/` | Python client SDK |
| `evaluation/` | LOCOMO benchmark harness (metrics, runners, datasets) |

## Quick start

### HTTP server

```bash
cargo build --release -p mem1-server
./target/release/mem1-server          # listens on 127.0.0.1:8080
```

```bash
# Store a memory
curl -X POST localhost:8080/memories \
  -H 'Content-Type: application/json' \
  -d '{"user_id":"alice","messages":[{"role":"user","content":"Alice prefers tea over coffee."}]}'

# Search
curl -X POST localhost:8080/memories/search \
  -H 'Content-Type: application/json' \
  -d '{"user_id":"alice","query":"what does alice drink?"}'
```

On first run the server auto-downloads the default embedding model (~90 MB); set
`MEM1_EMBED_PROVIDER=off` for keyword-only search with no download.

### MCP server (Claude Code / Codex / OpenCode / Pi)

```bash
cargo build --release -p mem1-mcp
claude mcp add -t stdio -s user mem1 \
  -e MEM1_DB_PATH=/Users/you/.mem1/mem1.db \
  -- /ABS/PATH/to/mem1/target/release/mem1-mcp
```

See [`mem1-mcp/README.md`](mem1-mcp/README.md) for Codex / OpenCode / Pi config.

## HTTP API

| Method | Path | Purpose |
| --- | --- | --- |
| `POST` | `/memories` | Add a memory (`content` or `messages`) |
| `POST` | `/memories/search` | Semantic search (returns matches + assembled context) |
| `GET` | `/memories` | List a user's memories |
| `GET/PATCH/DELETE` | `/memories/:id` | Get / update / delete one memory |
| `GET` | `/memories/:id/history` | Change history |
| `GET` | `/users` · `POST` `/reset` · `DELETE` `/memories` | Users / reset / bulk delete |
| `GET` | `/healthz` | Liveness |

## Configuration

All behaviour is environment-driven; sensible defaults need no config. Key knobs:

| Env | Default | Purpose |
| --- | --- | --- |
| `MEM1_BIND` | `127.0.0.1:8080` | HTTP bind address |
| `MEM1_DB_PATH` | `mem1.db` | SurrealDB/RocksDB store path (use absolute) |
| `MEM1_EMBED_PROVIDER` | `local` | `local` (all-MiniLM) · `qwen3` · `openai` · `off` |
| `MEM1_EMBED_DIM` | `384` | HNSW dimension — set `1024` for `qwen3` |
| `MEM1_MMR_LAMBDA` / `MEM1_MMR_PROTECT` / `MEM1_RERANK_POOL_EXTRA` | tuned | retrieval diversity/pool |
| `MEM1_EXTRACT_PROVIDER` | rule-v2 | set `openai` for LLM fact extraction |

See the per-crate READMEs for the full list.

## Development

```bash
just check          # fmt --check + clippy -D warnings + tests (Rust) + python tests
cargo test --release
```

## Benchmark

The `evaluation/` harness runs the LOCOMO benchmark. Under one identical judge,
embedding, dataset, and answering model, mem1 scores medium llm_score **0.8369**
vs mem0 OSS ~0.56 and SAG ~0.44 — the same-judge relative ordering is the fair
comparison (absolute numbers depend on the judge). See `evaluation/`.

## License

[Apache-2.0](LICENSE).
