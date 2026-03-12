"""
File-based run store: save/load run and metrics as JSON under data dir.
Retention: delete runs beyond configured N runs or M days.
"""

import json
import time
import uuid
from pathlib import Path
from typing import Optional

from toolchain.config import get_data_dir, get_retention_days, get_retention_runs
from toolchain.store.models import Metrics, Run


def _runs_dir() -> Path:
    return get_data_dir() / "runs"


def _run_dir(run_id: str) -> Path:
    return _runs_dir() / run_id


def _run_path(run_id: str) -> Path:
    return _run_dir(run_id) / "run.json"


def _metrics_path(run_id: str) -> Path:
    return _run_dir(run_id) / "metrics.json"


def ensure_data_dir() -> Path:
    d = get_data_dir()
    d.mkdir(parents=True, exist_ok=True)
    _runs_dir().mkdir(parents=True, exist_ok=True)
    return d


def new_run_id() -> str:
    return str(uuid.uuid4())


def save_run(run: Run) -> None:
    ensure_data_dir()
    path = _run_path(run.id)
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        json.dump(run.to_dict(), f, indent=2)
    enforce_retention()


def load_run(run_id: str) -> Optional[Run]:
    path = _run_path(run_id)
    if not path.exists():
        return None
    with open(path) as f:
        return Run.from_dict(json.load(f))


def save_metrics(metrics: Metrics) -> None:
    ensure_data_dir()
    path = _metrics_path(metrics.run_id)
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        json.dump(metrics.to_dict(), f, indent=2)


def load_metrics(run_id: str) -> Optional[Metrics]:
    path = _metrics_path(run_id)
    if not path.exists():
        return None
    with open(path) as f:
        return Metrics.from_dict(json.load(f))


def list_run_ids() -> list[str]:
    """List all run ids (directory names under runs/)."""
    r = _runs_dir()
    if not r.exists():
        return []
    return [p.name for p in r.iterdir() if p.is_dir() and (_run_path(p.name)).exists()]


def _run_timestamp_ts(run_id: str) -> Optional[float]:
    run = load_run(run_id)
    if not run:
        return None
    try:
        from datetime import datetime
        dt = datetime.fromisoformat(run.timestamp.replace("Z", "+00:00"))
        return dt.timestamp()
    except Exception:
        return None


def get_run_dir(run_id: str) -> Path:
    """Return the run directory path (e.g. for writing suggestions.json)."""
    return _run_dir(run_id)


def enforce_retention() -> None:
    """Delete or archive runs beyond configured retention (N runs or M days)."""
    ids = list_run_ids()
    if not ids:
        return
    # Sort by timestamp desc (newest first)
    with_ts = [(rid, _run_timestamp_ts(rid)) for rid in ids]
    with_ts = [(rid, ts) for rid, ts in with_ts if ts is not None]
    with_ts.sort(key=lambda x: -x[1])
    keep_runs = get_retention_runs()
    keep_secs = get_retention_days() * 86400
    now = time.time()
    to_remove = []
    for i, (rid, ts) in enumerate(with_ts):
        if i >= keep_runs:
            to_remove.append(rid)
        elif (now - ts) > keep_secs:
            to_remove.append(rid)
    for rid in to_remove:
        _remove_run_dir(rid)


def _remove_run_dir(run_id: str) -> None:
    d = _run_dir(run_id)
    if d.exists():
        for f in d.iterdir():
            f.unlink()
        d.rmdir()
