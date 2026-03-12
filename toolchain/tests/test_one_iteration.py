"""
Integration test: one full iteration (mock or real eval dir).
Assert run record exists with code_version and outcome; assert suggestions JSON has run_id and suggestions array.
"""

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))


def test_iterate_creates_run_record_with_code_version_and_outcome():
    """Run iterate with a fake eval dir (make exits 1); verify run record exists with code_version and outcome."""
    with tempfile.TemporaryDirectory() as tmp:
        data_dir = Path(tmp) / "data"
        data_dir.mkdir()
        fake_eval = Path(tmp) / "eval"
        fake_eval.mkdir()
        (fake_eval / "Makefile").write_text("all:\n\t@exit 1\n")
        env = os.environ.copy()
        env["MEM1_ITER_DATA_DIR"] = str(data_dir)
        env["MEM1_EVAL_DIR"] = str(fake_eval)

        result = subprocess.run(
            [sys.executable, "-m", "toolchain.cli", "iterate"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            env=env,
            timeout=15,
        )
        assert result.returncode != 0  # eval failed
        assert "run_id=" in (result.stderr or "") or "Run recorded" in (result.stderr or "")

        runs_dir = data_dir / "runs"
        assert runs_dir.exists()
        run_dirs = [d for d in runs_dir.iterdir() if d.is_dir()]
        assert len(run_dirs) >= 1
        run_json = run_dirs[0] / "run.json"
        assert run_json.exists()
        with open(run_json) as f:
            run_data = json.load(f)
        assert run_data.get("code_version") is not None
        assert run_data.get("outcome") in ("success", "failure")


def test_analyzer_output_has_run_id_and_suggestions_array():
    """Call analyze() and verify output has run_id and suggestions array with id, type, summary."""
    from toolchain.store import Run, Metrics, save_run, save_metrics, new_run_id
    from toolchain.runner.analyze import analyze
    from datetime import datetime, timezone

    with tempfile.TemporaryDirectory() as tmp:
        data_dir = Path(tmp)
        env = os.environ.copy()
        env["MEM1_ITER_DATA_DIR"] = str(data_dir)
        os.environ["MEM1_ITER_DATA_DIR"] = str(data_dir)

        run_id = new_run_id()
        runs_dir = data_dir / "runs" / run_id
        runs_dir.mkdir(parents=True)
        ts = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
        save_run(Run(id=run_id, timestamp=ts, code_version="abc123", outcome="success", metrics_ref="m", log_ref="l"))
        save_metrics(Metrics(run_id=run_id, overall={"llm_score": 0.5, "bleu_score": 0.4, "f1_score": 0.6}))

        payload = analyze(run_id, "abc123")
        assert "run_id" in payload
        assert payload["run_id"] == run_id
        assert "suggestions" in payload
        assert isinstance(payload["suggestions"], list)
        for s in payload["suggestions"]:
            assert "id" in s and "type" in s and "summary" in s
