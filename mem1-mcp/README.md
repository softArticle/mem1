# mem1-mcp

A **stdio [MCP](https://modelcontextprotocol.io) server** for [mem1](../mem1-server). It embeds the mem1 store **in-process** (no HTTP server needed) and exposes mem1's memory operations as MCP tools, so any MCP-capable agent — **Claude Code, Codex, OpenCode, Pi**, and others — can use mem1 as its long-term memory backend.

The MCP tools call the same `*_svc` service layer as the HTTP API, so storing and retrieving through MCP behaves identically to the HTTP server (same extraction, embedding, RRF fusion, and reranking pipeline).

## Tools

| Tool | What it does |
| --- | --- |
| `add_memory` | Store a memory (`content` string or `messages` turns). |
| `search_memory` | Semantic search; returns matches + a prompt-ready `formatted_context`. |
| `list_memories` | List a user's memories (paginated). |
| `get_memory` | Fetch one memory by id. |
| `update_memory` | Update a memory's content (re-embeds). |
| `delete_memory` | Delete one memory. |
| `delete_all_memories` | Delete all memories for a user (destructive). |
| `memory_history` | Change history of one memory. |
| `list_users` | All user ids with stored memories. |

Every tool takes a `user_id` — use a stable id per user/project so memories accumulate in one place.

## Build

```bash
cargo build --release -p mem1-mcp
# binary at: target/release/mem1-mcp
```

## Configuration (environment)

The server is configured entirely via env vars. **Because an MCP client launches the binary with an arbitrary working directory, always use absolute paths** — otherwise the database and model files get created wherever the client happens to start.

| Env | Purpose | Recommended |
| --- | --- | --- |
| `MEM1_DB_PATH` | SurrealDB/RocksDB store path | absolute, e.g. `/Users/you/.mem1/mem1.db` |
| `MEM1_LOCAL_EMBED_MODEL_DIR` | local embedding model dir (auto-downloads if missing) | absolute, e.g. `/Users/you/.mem1/embed_model` |
| `MEM1_EMBED_PROVIDER` | `local` (default, in-process all-MiniLM) / `openai` / `off` | `local` |
| `MEM1_RERANK_MODEL_DIR` | embedded cross-encoder dir (only if `MEM1_RERANK_PROVIDER=crossencoder`) | absolute |

Logs go to **stderr**; stdout carries only the MCP JSON-RPC stream.

## Client setup

In every example below, replace `/ABS/PATH/to/mem1` with the absolute path to this repo, and pick a stable `MEM1_DB_PATH`.

### Claude Code

```bash
claude mcp add -t stdio -s user mem1 \
  -e MEM1_DB_PATH=/Users/you/.mem1/mem1.db \
  -e MEM1_LOCAL_EMBED_MODEL_DIR=/Users/you/.mem1/embed_model \
  -- /ABS/PATH/to/mem1/target/release/mem1-mcp
```

Or add to `.mcp.json` (project) / `~/.claude.json` (global):

```json
{
  "mcpServers": {
    "mem1": {
      "type": "stdio",
      "command": "/ABS/PATH/to/mem1/target/release/mem1-mcp",
      "env": {
        "MEM1_DB_PATH": "/Users/you/.mem1/mem1.db",
        "MEM1_LOCAL_EMBED_MODEL_DIR": "/Users/you/.mem1/embed_model"
      }
    }
  }
}
```

Verify with `/mcp` inside Claude Code — `mem1` should show as connected.

### Codex CLI

Add to `~/.codex/config.toml`:

```toml
[mcp_servers.mem1]
command = "/ABS/PATH/to/mem1/target/release/mem1-mcp"

[mcp_servers.mem1.env]
MEM1_DB_PATH = "/Users/you/.mem1/mem1.db"
MEM1_LOCAL_EMBED_MODEL_DIR = "/Users/you/.mem1/embed_model"
```

### OpenCode

Add to `~/.config/opencode/opencode.json`:

```json
{
  "mcp": {
    "mem1": {
      "type": "local",
      "command": ["/ABS/PATH/to/mem1/target/release/mem1-mcp"],
      "environment": {
        "MEM1_DB_PATH": "/Users/you/.mem1/mem1.db",
        "MEM1_LOCAL_EMBED_MODEL_DIR": "/Users/you/.mem1/embed_model"
      }
    }
  }
}
```

Then `OpenCode: Reload Config` and check the `mem1` tools are available.

### Pi

Pi consumes stdio MCP servers like the others. In Pi's MCP config, register a stdio server pointing at the binary:

```json
{
  "mcpServers": {
    "mem1": {
      "command": "/ABS/PATH/to/mem1/target/release/mem1-mcp",
      "env": {
        "MEM1_DB_PATH": "/Users/you/.mem1/mem1.db",
        "MEM1_LOCAL_EMBED_MODEL_DIR": "/Users/you/.mem1/embed_model"
      }
    }
  }
}
```

## Smoke test (no client needed)

Pipe newline-delimited JSON-RPC into the binary:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"add_memory","arguments":{"user_id":"alice","content":"Alice prefers tea over coffee."}}}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"search_memory","arguments":{"user_id":"alice","query":"what does alice drink"}}}' \
  | MEM1_DB_PATH=/tmp/mem1-mcp-smoke.db ./target/release/mem1-mcp
```

You should see the stored fact echoed back from `search_memory` along with a `formatted_context` block.
