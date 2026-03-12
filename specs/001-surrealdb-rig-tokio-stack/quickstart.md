# Quickstart: Memory Service (Rust Server + Python SDK)

**Feature**: 001-surrealdb-rig-tokio-stack  
**Date**: 2026-03-06

## Prerequisites

- Rust (stable), Cargo  
- Python 3.10+  
- SurrealDB is **embedded** in mem1-server (no separate DB process)

## 1. Start the server (Rust)

From repository root:

```bash
cd mem1-server
cargo run
```

Default: server listens on `http://127.0.0.1:8080`, and stores data in the local directory `mem1.db` (RocksDB, embedded in-process).

Optional: set `MEM1_DB_PATH` to use a different path (e.g. `MEM1_DB_PATH=/data/mem1.db cargo run`).

To enable semantic (vector) search via Rig + OpenAI embeddings:

```bash
MEM1_EMBED_PROVIDER=openai OPENAI_API_KEY=... cargo run
```

## 2. Install the Python SDK

```bash
cd python
pip install -e .
# or: pip install mem1   # when published
```

## 3. Use memory in Python (mem0-style)

```python
from mem1 import Memory

memory = Memory(base_url="http://127.0.0.1:8080")

# Add from conversation messages
messages = [
    {"role": "user", "content": "I prefer dark mode and use Python for ML."},
    {"role": "assistant", "content": "Noted. I'll remember your preference and that you use Python for ML."},
]
memory.add(messages, user_id="alice")

# Or add raw text
memory.add("Alice's favorite editor is VS Code.", user_id="alice")

# Search (RAG-style)
results = memory.search(query="What does Alice prefer?", user_id="alice", limit=5)
for r in results["results"]:
    print(r["content"], r.get("score"))
```

## 4. RAG integration pattern

Use search results as context for your LLM:

```python
def answer_with_memory(question: str, user_id: str = "default_user") -> str:
    memories = memory.search(query=question, user_id=user_id, limit=3)
    context = "\n".join(m["content"] for m in memories["results"])
    # Build prompt with context and call your LLM (OpenAI, local, etc.)
    # Then optionally: memory.add([...messages...], user_id=user_id)
    return response
```

## 5. Verify

- **Server**: `curl -X POST http://127.0.0.1:8080/memories -H "Content-Type: application/json" -d '{"user_id":"test","content":"Hello memory"}'`
- **SDK**: Run the snippet above and confirm add returns `results` and search returns relevant items.

## Next steps

- Configure embedding/LLM (Rig) in the server for semantic search.
- Add auth (API key or other) if exposing beyond localhost.
- See [contracts](./contracts/) for full API and SDK contracts.
