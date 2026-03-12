# Quickstart: AI-Driven Iterative Development Toolchain

**Feature**: 001-ai-iter-dev  
**Date**: 2026-03-09

## Purpose

Run the loop **optimize code → eval → collect → analyze → (you modify code) → next iteration** with one command. The toolchain runs outside the mem1 repo and invokes mem1 evaluation, stores each run with a code version, and produces machine-readable optimization suggestions (e.g. JSON) for you or an AI assistant to use.

## Prerequisites

- Mem1 evaluation runnable (e.g. `cd mem1/evaluation && make full` works).
- Git available; target repo (mem1) is a git repo so code version can be captured.
- Toolchain installed or available (e.g. `toolchain` CLI or `make iterate` from a wrapper repo).

## Run one iteration

1. From the **target repo root** (e.g. mem1) or the directory that contains it:
   ```bash
   python -m toolchain iterate
   ```
   or, if the toolchain is exposed via Make:
   ```bash
   make iterate
   ```

2. The toolchain will:
   - Capture current git commit (e.g. `git rev-parse HEAD`).
   - Run evaluation (e.g. `make full` in `evaluation/`).
   - Collect metrics and logs and store them with a run id and code version.
   - Run the automated analyzer (rules or AI) on the run data.
   - Emit suggestions in machine-readable form (e.g. `suggestions.json` or stdout).

3. On success: use the suggestions to decide code changes; commit and run `iterate` again for the next iteration. On failure: check run outcome (e.g. `failure_step`, `failure_detail`) and fix before re-running.

## List and compare runs

- **List recent runs**: `python -m toolchain list-runs` (or `make list-runs`) to see run id, timestamp, code version, outcome, and summary metrics.
- **Compare last two runs**: `python -m toolchain compare` (or `make compare`) to see whether metrics improved or regressed.

## Configuration

- **Eval path**: Set `MEM1_EVAL_DIR` (or equivalent) to the path to `mem1/evaluation/` if not auto-detected.
- **Data dir**: Where runs are stored (default e.g. `~/.mem1-iter` or `toolchain_data/`); override via config or env.
- **Retention**: Default last 30 runs or 7 days; override in config if needed.

## Output for AI assistant

The analyzer output (e.g. `suggestions.json`) is JSON so that an AI coding assistant can read it and propose or prioritize code changes. Each suggestion has at least `id`, `type`, and `summary`; see [contracts/analyzer-output.md](./contracts/analyzer-output.md).
