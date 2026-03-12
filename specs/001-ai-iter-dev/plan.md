# Implementation Plan: AI-Driven Iterative Development (External Toolchain)

**Branch**: `001-ai-iter-dev` | **Date**: 2026-03-09 | **Spec**: [spec.md](./spec.md)  
**Input**: Feature specification from `specs/001-ai-iter-dev/spec.md`  
**User context**: 构建主项目外，auto code工具链， 优化代码 => eval评估 => 根据评估结果继续优化代码

## Summary

An **external AI coding toolchain** (outside the mem1 repo) that runs the loop: **optimize code → run evaluation → collect logs/metrics → automated analysis (rules or AI) → machine-readable suggestions (e.g. JSON)** so the developer (or an AI assistant) can apply changes and run the next iteration. Each run is tied to a code version (e.g. git commit). Trigger: single CLI command. Default retention: e.g. last 30 runs or 7 days. The toolchain targets mem1 (e.g. LOCOMO eval) but is personal tooling and not open-sourced.

## Technical Context

**Language/Version**: Flexible; recommended Python 3.10+ or shell + Python for orchestration (toolchain is outside mem1; not bound by mem1's Rust-first constitution).  
**Primary Dependencies**: Invokes existing mem1 evaluation (Makefile / Python in `evaluation/`); optional HTTP client for external eval; optional LLM client if analyzer uses AI.  
**Storage**: File-based or simple local store (e.g. JSON/SQLite in a toolchain data dir); retention default last 30 runs or 7 days, configurable.  
**Testing**: Script/CLI tests (e.g. pytest or shell) that run one iteration and assert run record + analyzer output shape.  
**Target Platform**: Developer machine (macOS/Linux); runs in same environment as mem1 (e.g. `make full`).  
**Project Type**: CLI / orchestration tooling (separate repo or dedicated directory outside mem1).  
**Performance Goals**: One iteration completes in a reasonable time (driven by eval duration, e.g. minutes for LOCOMO full).  
**Constraints**: Must not modify mem1 source tree except by user; toolchain reads eval outputs and (optionally) git state.  
**Scale/Scope**: Single user; tens of runs; retention bounded by default.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

This feature is an **external AI coding toolchain**, not part of the mem1 product or repository. The Memory Service Constitution (Rust-first, Python SDK parity, test gates, etc.) applies to the **memory service (mem1)** only. The toolchain is personal development tooling used to iterate on mem1 (optimize code → eval → collect → analyze → suggest).

| Principle | Applicability | Status |
|-----------|---------------|--------|
| I. Rust-First Core | N/A – toolchain is not the memory service | Skip |
| II. Local-First and Privacy | Toolchain runs locally; no requirement to change | Pass |
| III. Python SDK Parity | N/A – toolchain is not the mem1 API | Skip |
| IV. Test and Quality Gates | Toolchain should have tests for one iteration and output shape | Pass (plan tests) |
| V. Deterministic Observability | Toolchain should log run id, outcome, failure step | Pass (plan logging) |

**Result**: No constitution violations. Toolchain is out of scope for Rust-first and SDK parity; local-first and test/observability are applied as good practice.

## Project Structure

### Documentation (this feature)

```text
specs/001-ai-iter-dev/
├── plan.md              # This file
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── contracts/           # Phase 1 (CLI and analyzer output)
└── tasks.md             # Phase 2 (/speckit.tasks)
```

### Source Code (toolchain – outside mem1 repo or in a dedicated sibling dir)

Toolchain lives **outside** the mem1 repository (or in a separate top-level dir that does not ship with mem1). Example layout:

```text
toolchain/                    # or separate repo
├── cli.py or iterate.sh      # Single entry: run one iteration
├── runner/                   # Orchestration: run eval, collect, analyze
│   ├── run_eval.py           # Invoke mem1 evaluation (e.g. make full or subprocess)
│   ├── collect.py           # Persist metrics + run metadata + code version
│   └── analyze.py            # Rules or AI → JSON suggestions
├── store/                    # Run storage (file-based or SQLite)
│   └── runs/
├── config.yaml or .env      # Eval path, retention, analyzer options
└── tests/
    └── test_one_iteration.py
```

**Structure Decision**: Single CLI entry point; runner modules for eval, collect, analyze; file-based or SQLite store under a data dir; tests for one full iteration and analyzer output schema.

## Complexity Tracking

Not applicable – no constitution violations to justify.
