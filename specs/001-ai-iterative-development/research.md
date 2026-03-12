# Research: AI-Driven Iterative Development Toolchain

**Feature**: 001-ai-iterative-development  
**Date**: 2026-03-09

## 1. Toolchain language and orchestration

**Decision**: Python 3.10+ as primary language for the toolchain (orchestration, store, analyzer), with optional shell wrapper for a single CLI command.

**Rationale**: Mem1 evaluation is already Python + Makefile; reusing Python allows direct reuse of evaluation scripts, same env (e.g. `evaluation/`), and simple subprocess or in-process calls. A single `make iterate` or `python -m toolchain.iterate` keeps the “one command” requirement. Shell-only could work but is harder to test and to emit structured JSON from the analyzer.

**Alternatives considered**: Pure shell (harder to maintain and test); Node (extra runtime); Rust (overkill for personal orchestration and would duplicate eval invocation).

---

## 2. Run and metrics storage

**Decision**: File-based storage under a dedicated data directory (e.g. `~/.mem1-iter/runs/` or `./toolchain_data/runs/`). Each run is one directory or one JSON file (run metadata + metrics); retention enforced by deleting or archiving runs beyond the default (e.g. last 30 runs or 7 days).

**Rationale**: No server; single-user; easy to inspect and backup; configurable path. SQLite is an alternative if querying “last N runs” or “runs by commit” becomes important—can be added later without changing the contract (run id, timestamp, code_version, outcome, metrics).

**Alternatives considered**: SQLite from day one (simpler to start with files); remote DB (out of scope for local toolchain).

---

## 3. Code version capture

**Decision**: Capture git commit hash (and optionally branch or tag) at the start of each iteration by running `git rev-parse HEAD` (and optionally `git describe --tags`) from the target repo (mem1). Store as `code_version` (e.g. `abc123`) and optional `code_version_display` (e.g. `v0.1.0-2-gabc123`).

**Rationale**: Unambiguous before/after comparison; works with any git-based workflow; no extra tooling.

**Alternatives considered**: Git tag only (less granular); no version (rejected by spec).

---

## 4. Analyzer output format (machine-readable)

**Decision**: Emit a single JSON artifact per analysis run (e.g. `suggestions.json` or stdout). Schema: list of suggestions; each suggestion has at least: `id`, `type` (e.g. regression | weak_category | bottleneck | failure_pattern), `summary` (short human-readable), `detail` (optional), `metric_ref` (optional: which metric or run it refers to), `priority` (optional: high | medium | low).

**Rationale**: Enables AI assistant or other tools to consume suggestions programmatically; still human-readable when pretty-printed; extensible (add fields later).

**Alternatives considered**: Multiple files per suggestion (more complex); CSV (less structured); free-form text only (rejected by spec).

---

## 5. Integration with mem1 evaluation

**Decision**: Toolchain invokes the existing evaluation pipeline (e.g. `make full` or `make add && make search && make evals && make scores`) in the mem1 repo’s `evaluation/` directory via subprocess, with env (e.g. `MEM1_BASE_URL`, `OPENAI_API_KEY`) inherited or set by the toolchain. Parse evaluation outputs (e.g. `evaluation_metrics.json`, `results/mem1_results.json`) to populate run metrics and to feed the analyzer.

**Rationale**: No changes to mem1 evaluation code required; toolchain is a wrapper that adds run tracking, code version, and analyzer. Same as current manual “run make full, then look at numbers.”

**Alternatives considered**: Embedding eval inside the toolchain (duplication); calling a remote API for eval (out of scope for local toolchain).
