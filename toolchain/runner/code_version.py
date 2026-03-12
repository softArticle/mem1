"""
Capture code version from git: rev-parse HEAD and optional git describe.
"""

import subprocess
from pathlib import Path
from typing import Optional


def get_code_version(repo_root: Optional[Path] = None) -> tuple[str, Optional[str]]:
    """
    Run git rev-parse HEAD and optional git describe from repo root.
    Returns (code_version, code_version_display).
    """
    root = repo_root or Path.cwd()
    if not (root / ".git").exists():
        return ("unknown", None)

    try:
        rev = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=5,
        )
        code_version = rev.stdout.strip() if rev.returncode == 0 else "unknown"
    except Exception:
        code_version = "unknown"

    try:
        desc = subprocess.run(
            ["git", "describe", "--tags", "--always"],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=5,
        )
        code_version_display = desc.stdout.strip() if desc.returncode == 0 else None
    except Exception:
        code_version_display = None

    return (code_version, code_version_display)
