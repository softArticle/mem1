"""
Collect: parse evaluation_metrics.json (and optionally results/mem1_results.json) from
evaluation dir, build Metrics payload, save run + metrics to store with run_id and code_version.
"""

import json
from pathlib import Path

from toolchain.config import get_eval_dir
from toolchain.store import Metrics, Run, save_metrics, save_run
from toolchain.store.run_store import get_run_dir


def _parse_evaluation_metrics(eval_dir: Path) -> tuple[dict | None, list | None, str | None]:
    """Parse evaluation_metrics.json. Returns (overall, by_category, raw_path)."""
    path = eval_dir / "evaluation_metrics.json"
    if not path.exists():
        return None, None, None
    with open(path) as f:
        data = json.load(f)
    # data is keyed by category (e.g. "0","1",...); each value is list of items with bleu_score, f1_score, llm_score
    all_items = []
    for k, v in data.items():
        all_items.extend(v)
    if not all_items:
        return None, None, str(path)
    # Overall means
    n = len(all_items)
    overall = {
        "bleu_score": sum(i.get("bleu_score", 0) for i in all_items) / n,
        "f1_score": sum(i.get("f1_score", 0) for i in all_items) / n,
        "llm_score": sum(i.get("llm_score", 0) for i in all_items) / n,
        "count": n,
    }
    # By category
    from collections import defaultdict
    by_cat = defaultdict(list)
    for item in all_items:
        c = str(item.get("category", ""))
        by_cat[c].append(item)
    by_category = []
    for c in sorted(by_cat.keys(), key=lambda x: (x == "", x)):
        items = by_cat[c]
        by_category.append({
            "category": c,
            "count": len(items),
            "bleu_score": sum(i.get("bleu_score", 0) for i in items) / len(items),
            "f1_score": sum(i.get("f1_score", 0) for i in items) / len(items),
            "llm_score": sum(i.get("llm_score", 0) for i in items) / len(items),
        })
    return overall, by_category, str(path)


def collect_and_save(
    run_id: str,
    code_version: str,
    timestamp: str,
    outcome: str,
    failure_step: str | None = None,
    failure_detail: str | None = None,
    latency_ms: float | None = None,
    eval_dir: Path | None = None,
) -> Run:
    """Build run and (if success) metrics from eval dir; save both; set metrics_ref and log_ref when success."""
    ed = eval_dir or get_eval_dir()
    metrics_ref = None
    log_ref = None
    if outcome == "success":
        overall, by_category, raw_path = _parse_evaluation_metrics(ed)
        metrics_ref = f"runs/{run_id}/metrics.json"
        log_ref = f"runs/{run_id}/metrics.json"  # same dir; could add run.log later
        metrics = Metrics(
            run_id=run_id,
            overall=overall,
            by_category=by_category,
            latency_ms=latency_ms,
            raw_path=raw_path,
        )
        save_metrics(metrics)
    run = Run(
        id=run_id,
        timestamp=timestamp,
        code_version=code_version,
        outcome=outcome,
        failure_step=failure_step,
        failure_detail=failure_detail,
        metrics_ref=metrics_ref,
        log_ref=log_ref,
    )
    save_run(run)
    return run
