# Data Model: AI-Driven Iterative Development Toolchain

**Feature**: 001-ai-iterative-development  
**Date**: 2026-03-09

## Entities

### Run (iteration run)

A single execution of the iteration loop (eval → collect → analyze).

| Field | Type | Description |
|-------|------|-------------|
| id | string | Unique run identifier (e.g. UUID or timestamp-based). |
| timestamp | string (ISO 8601) | When the run started. |
| code_version | string | Git commit hash (e.g. from `git rev-parse HEAD`). |
| code_version_display | string (optional) | Human-readable version (e.g. `git describe --tags`). |
| outcome | enum | `success` \| `failure`. |
| failure_step | string (optional) | If outcome is `failure`, which step failed (e.g. `eval`, `collect`, `analyze`). |
| failure_detail | string (optional) | Brief error message or log excerpt. |
| metrics_ref | string (optional) | Path or key to persisted metrics for this run. |
| log_ref | string (optional) | Path or key to persisted logs for this run. |

**Validation**: `id` and `timestamp` and `code_version` and `outcome` are required. At least one of `metrics_ref` or `log_ref` should be set when `outcome` is `success`.

**Lifecycle**: Created when an iteration starts; immutable after the run completes (no updates to outcome or metrics after close).

---

### Metrics

Numeric or structured results from evaluation, associated with a run.

| Field | Type | Description |
|-------|------|-------------|
| run_id | string | References Run.id. |
| overall | object (optional) | E.g. `bleu_score`, `f1_score`, `llm_score` (from evaluation_metrics.json). |
| by_category | array (optional) | Per-category scores (e.g. category 1–4 with bleu, f1, llm, count). |
| latency_ms | number (optional) | Total eval duration in milliseconds. |
| raw_path | string (optional) | Path to raw evaluation output (e.g. evaluation_metrics.json) for this run. |

**Validation**: `run_id` is required. At least one of `overall`, `by_category`, or `raw_path` should be present for a successful run.

**Relationship**: One-to-one with Run (one Metrics record per Run when outcome is success and metrics were collected).

---

### Analysis / optimization suggestion

A single finding or recommendation produced by the automated analyzer, emitted in machine-readable form (e.g. JSON).

| Field | Type | Description |
|-------|------|-------------|
| id | string | Unique suggestion id within the analysis run. |
| type | string | E.g. `regression`, `weak_category`, `bottleneck`, `failure_pattern`. |
| summary | string | Short human-readable summary. |
| detail | string (optional) | Longer description or evidence. |
| metric_ref | string (optional) | Reference to metric or run (e.g. run_id, category, metric name). |
| priority | string (optional) | `high` \| `medium` \| `low`. |

**Validation**: `id`, `type`, and `summary` are required. `type` should be one of a closed set (extensible later).

**Relationship**: Many suggestions per analysis run; analysis run is associated with one or more runs (e.g. current run only, or current + previous for comparison).

---

## Retention and storage

- **Default retention**: Keep at most the last 30 runs or runs within the last 7 days, whichever is more restrictive (delete or archive older runs).
- **Configurable**: Retention policy (N runs, M days, or both) can be overridden via config or env.
- **Storage**: File-based (e.g. one JSON file per run under `runs/<run_id>/run.json`, `runs/<run_id>/metrics.json`); or single SQLite DB with `runs`, `metrics`, and `suggestions` tables. Exact layout is an implementation choice; the fields above define the logical model.
