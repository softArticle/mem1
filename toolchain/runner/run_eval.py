"""
Run evaluation: subprocess to make full (or add+search+evals+scores) in evaluation dir.
Capture stdout, stderr, and exit code.
"""

import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

from toolchain.config import get_eval_dir


@dataclass
class EvalResult:
    exit_code: int
    stdout: str
    stderr: str
    latency_ms: float


def run_eval(eval_dir: Path | None = None) -> EvalResult:
    """Run make full in evaluation dir; return exit code, stdout, stderr, latency_ms."""
    ed = eval_dir or get_eval_dir()
    if not ed.is_dir():
        return EvalResult(
            exit_code=-1,
            stdout="",
            stderr=f"Eval dir not found: {ed}",
            latency_ms=0.0,
        )
    start = time.perf_counter()
    try:
        proc = subprocess.run(
            ["make", "full"],
            cwd=ed,
            capture_output=True,
            text=True,
            timeout=3600,
        )
        latency_ms = (time.perf_counter() - start) * 1000
        return EvalResult(
            exit_code=proc.returncode or 0,
            stdout=proc.stdout or "",
            stderr=proc.stderr or "",
            latency_ms=latency_ms,
        )
    except subprocess.TimeoutExpired as e:
        return EvalResult(
            exit_code=-1,
            stdout=e.stdout or "",
            stderr=(e.stderr or "") + "\n[Timeout]",
            latency_ms=(time.perf_counter() - start) * 1000,
        )
    except Exception as e:
        return EvalResult(
            exit_code=-1,
            stdout="",
            stderr=str(e),
            latency_ms=(time.perf_counter() - start) * 1000,
        )
