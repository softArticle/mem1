"""
Resolve eval dir (MEM1_EVAL_DIR or default), data dir, retention (default 30 runs / 7 days).
"""

import os
from pathlib import Path

# Default: assume toolchain is at repo root and evaluation/ is sibling or under repo
_REPO_ROOT = Path(__file__).resolve().parent.parent


def get_eval_dir() -> Path:
    """Path to mem1 evaluation directory."""
    env = os.environ.get("MEM1_EVAL_DIR")
    if env:
        return Path(env).expanduser().resolve()
    # Default: mem1/evaluation relative to repo root
    candidate = _REPO_ROOT / "evaluation"
    if candidate.is_dir():
        return candidate
    return _REPO_ROOT / "evaluation"


def get_data_dir() -> Path:
    """Where runs and metrics are stored."""
    env = os.environ.get("MEM1_ITER_DATA_DIR")
    if env:
        return Path(env).expanduser().resolve()
    candidate = _REPO_ROOT / "toolchain_data"
    if candidate.exists() or _REPO_ROOT.name == "mem1":
        return candidate
    return Path.home() / ".mem1-iter"


def get_retention_runs() -> int:
    """Max number of runs to keep."""
    env = os.environ.get("MEM1_ITER_RETENTION_RUNS")
    if env:
        return int(env)
    return 30


def get_retention_days() -> int:
    """Max age of runs in days."""
    env = os.environ.get("MEM1_ITER_RETENTION_DAYS")
    if env:
        return int(env)
    return 7
