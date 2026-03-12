# mem1-server

Local AI memory service (Rust). HTTP API for adding, searching, getting, and deleting memories with optional vector search.

## Embedding (optional)

**Zero config**: No env vars needed. The server uses in-process embed if `embed_model/` (or `MEM1_LOCAL_EMBED_MODEL_DIR`) contains `model.onnx` and `tokenizer.json`. **If the default path is empty, the server will download the default model** ([all-MiniLM-L6-v2-ONNX](https://huggingface.co/onnx-community/all-MiniLM-L6-v2-ONNX), ~90 MB) from Hugging Face on first run, then load it. If download or load fails, it runs without vectors (keyword search only).

- **Default (local)**: `embed_model/` in the working directory (or `MEM1_LOCAL_EMBED_MODEL_DIR`). Expects:
  - `model.onnx` + `model.onnx_data` (or single `model.onnx`)
  - `tokenizer.json`  
  If missing and using the default path, auto-download is attempted once.
- **Custom path**: `MEM1_LOCAL_EMBED_MODEL_DIR=/path/to/dir` — must contain the above files or startup fails.
- **Disable**: `MEM1_EMBED_PROVIDER=off` — no vectors; keyword match only.
- **OpenAI**: `MEM1_EMBED_PROVIDER=openai` and `OPENAI_API_KEY` (and optionally `MEM1_OPENAI_EMBED_MODEL`).

Optional for local: `MEM1_LOCAL_EMBED_MAX_LENGTH` (default 256).

Local embedding uses **tract** (pure Rust ONNX inference); no native libs, so it builds and runs on macOS, Linux, and Windows without extra setup.
