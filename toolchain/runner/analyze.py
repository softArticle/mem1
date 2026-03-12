"""
Analyzer: input run_id and metrics, output JSON per contracts/analyzer-output.md.
Rules-based: weak_category, failure_pattern, regression (when previous run provided).
"""

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from toolchain.store import load_metrics, load_run

# Thresholds for suggestions
WEAK_LLM_THRESHOLD = 0.5
WEAK_BLEU_THRESHOLD = 0.3
REGRESSION_THRESHOLD = 0.05  # 5% drop is a regression


def _suggestion(
    id_suffix: str,
    type_: str,
    summary: str,
    detail: str | None = None,
    metric_ref: str | None = None,
    priority: str = "medium",
) -> dict[str, Any]:
    return {
        "id": f"suggestion-{id_suffix}",
        "type": type_,
        "summary": summary,
        "detail": detail,
        "metric_ref": metric_ref,
        "priority": priority,
    }


def _weak_category_suggestions(metrics: Any) -> list[dict[str, Any]]:
    out = []
    if not metrics.by_category:
        return out
    for i, cat in enumerate(metrics.by_category):
        c = cat.get("category", str(i))
        llm = cat.get("llm_score")
        bleu = cat.get("bleu_score")
        if llm is not None and llm < WEAK_LLM_THRESHOLD:
            out.append(_suggestion(
                f"weak-llm-{c}",
                "weak_category",
                f"Category {c} has low LLM score ({llm:.3f}); consider improving retrieval or answers.",
                f"llm_score={llm:.3f} (threshold {WEAK_LLM_THRESHOLD})",
                f"category={c}",
                "high" if llm < 0.3 else "medium",
            ))
        if bleu is not None and bleu < WEAK_BLEU_THRESHOLD:
            out.append(_suggestion(
                f"weak-bleu-{c}",
                "weak_category",
                f"Category {c} has low BLEU ({bleu:.3f}); consider improving answer overlap.",
                f"bleu_score={bleu:.3f} (threshold {WEAK_BLEU_THRESHOLD})",
                f"category={c}",
                "medium",
            ))
    return out


def _regression_suggestions(
    run_id: str,
    metrics: Any,
    previous_metrics: Any,
) -> list[dict[str, Any]]:
    out = []
    if not previous_metrics or not previous_metrics.overall or not metrics.overall:
        return out
    for key in ("llm_score", "bleu_score", "f1_score"):
        prev = previous_metrics.overall.get(key)
        curr = metrics.overall.get(key)
        if prev is None or curr is None:
            continue
        if prev <= 0:
            continue
        drop = (prev - curr) / prev
        if drop >= REGRESSION_THRESHOLD:
            out.append(_suggestion(
                f"regression-{key}",
                "regression",
                f"{key} dropped by {drop*100:.1f}% vs previous run.",
                f"Previous {key}={prev:.4f}, current={curr:.4f}",
                run_id,
                "high" if drop >= 0.1 else "medium",
            ))
    return out


def analyze(run_id: str, code_version: str, previous_run_id: str | None = None) -> dict[str, Any]:
    """
    Produce analyzer output per analyzer-output.md.
    Returns dict with run_id, code_version, analyzed_at, suggestions (list with id, type, summary, detail, metric_ref, priority).
    """
    run = load_run(run_id)
    metrics = load_metrics(run_id)
    previous_metrics = load_metrics(previous_run_id) if previous_run_id else None
    analyzed_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    suggestions: list[dict[str, Any]] = []

    if metrics:
        suggestions.extend(_weak_category_suggestions(metrics))
        suggestions.extend(_regression_suggestions(run_id, metrics, previous_metrics))
    if not suggestions and metrics and (metrics.overall or metrics.by_category):
        suggestions.append(_suggestion(
            "review",
            "other",
            "Review overall and per-category scores; consider improving weak categories.",
            None,
            run_id,
            "medium",
        ))

    return {
        "run_id": run_id,
        "code_version": code_version,
        "analyzed_at": analyzed_at,
        "suggestions": suggestions,
    }


def write_suggestions(payload: dict[str, Any], path: Path) -> None:
    """Write analyzer output JSON to path."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        json.dump(payload, f, indent=2)
