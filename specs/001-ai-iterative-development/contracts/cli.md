# CLI Contract: Iteration Toolchain

**Feature**: 001-ai-iterative-development  
**Date**: 2026-03-09

## Entry point

- **Single command**: One CLI entry runs the full iteration (eval → collect → analyze). Examples: `make iterate`, `python -m toolchain iterate`, or `./iterate.sh`.
- **Working directory**: When invoked, the toolchain MUST assume the current working directory is the **target repo root** (e.g. mem1) or a directory that contains both the target repo and the evaluation dir (e.g. `mem1/evaluation/`). Code version (git) is captured from the target repo.

## Commands (minimum)

| Command | Description | Exit code |
|---------|-------------|-----------|
| `iterate` (default) | Run one full iteration: run eval, collect metrics and logs, run analyzer, write suggestions (e.g. to stdout or file). | 0 on success; non-zero on failure (eval failure, collect failure, or analyzer failure). |
| `list-runs` (optional) | List recent runs (id, timestamp, code_version, outcome, summary metrics). | 0. |
| `compare` (optional) | Compare last two successful runs (e.g. diff of overall metrics). | 0 if two runs exist; non-zero if not. |

## Environment / configuration

- **Eval path**: Path to evaluation directory (e.g. `mem1/evaluation/`) or repo root; configurable via env (e.g. `MEM1_EVAL_DIR`) or config file.
- **Data dir**: Where runs and metrics are stored (e.g. `~/.mem1-iter` or `./toolchain_data`); configurable.
- **Retention**: Default last 30 runs or 7 days; overridable via config.

## Outputs

- **Run outcome**: On success, run id and path to suggestions (or suggestions on stdout). On failure, run id (if created), outcome `failure`, and failure_step + failure_detail (to stderr or log).
- **Suggestions**: Machine-readable format per [analyzer-output.md](./analyzer-output.md) (e.g. written to `suggestions.json` or stdout).
