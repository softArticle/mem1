"""
Single CLI entry: iterate (default), list-runs, compare.
iterate: code_version → run_eval → collect → analyze; on eval failure record run with outcome failure.
"""

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

from toolchain.config import get_eval_dir
from toolchain.runner import get_code_version
from toolchain.runner.analyze import analyze, write_suggestions
from toolchain.runner.collect import collect_and_save
from toolchain.runner.run_eval import run_eval
from toolchain.store import new_run_id
from toolchain.store.run_store import get_run_dir


def _log(msg: str) -> None:
    print(msg, file=sys.stderr)


def cmd_iterate() -> int:
    repo_root = Path.cwd()
    eval_dir = get_eval_dir()
    code_version, code_version_display = get_code_version(repo_root)
    run_id = new_run_id()
    timestamp = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")

    _log(f"run_id={run_id} code_version={code_version}")
    result = run_eval(eval_dir)
    if result.exit_code != 0:
        _log(f"Eval failed: exit_code={result.exit_code}")
        _log(result.stderr[:500] if result.stderr else result.stdout[:500])
        collect_and_save(
            run_id=run_id,
            code_version=code_version,
            timestamp=timestamp,
            outcome="failure",
            failure_step="eval",
            failure_detail=result.stderr[:500] or str(result.exit_code),
            eval_dir=eval_dir,
        )
        _log(f"Run recorded: outcome=failure failure_step=eval")
        return 1

    collect_and_save(
        run_id=run_id,
        code_version=code_version,
        timestamp=timestamp,
        outcome="success",
        latency_ms=result.latency_ms,
        eval_dir=eval_dir,
    )
    # Optional previous run for regression suggestions
    from toolchain.store import list_run_ids, load_run
    all_ids = [rid for rid in list_run_ids() if rid != run_id]
    runs_with_ts = [(rid, load_run(rid)) for rid in all_ids]
    runs_with_ts = [(rid, r) for rid, r in runs_with_ts if r and r.outcome == "success"]
    runs_with_ts.sort(key=lambda x: x[1].timestamp, reverse=True)
    previous_run_id = runs_with_ts[0][0] if runs_with_ts else None
    payload = analyze(run_id, code_version, previous_run_id=previous_run_id)
    out_dir = get_run_dir(run_id)
    suggestions_path = out_dir / "suggestions.json"
    write_suggestions(payload, suggestions_path)
    _log(f"Run recorded: outcome=success suggestions={suggestions_path}")
    print(json.dumps(payload, indent=2))
    return 0


def cmd_list_runs() -> int:
    from toolchain.store import load_run, list_run_ids
    ids = list_run_ids()
    runs = []
    for rid in ids:
        r = load_run(rid)
        if r:
            runs.append(r)
    runs.sort(key=lambda r: r.timestamp, reverse=True)
    for r in runs[:30]:
        summary = ""
        if r.metrics_ref:
            from toolchain.store import load_metrics
            m = load_metrics(r.id)
            if m and m.overall:
                summary = f" llm={m.overall.get('llm_score')} bleu={m.overall.get('bleu_score')} f1={m.overall.get('f1_score')}"
        print(f"{r.id} {r.timestamp} {r.code_version[:8]} {r.outcome}{summary}")
    return 0


def cmd_compare() -> int:
    from toolchain.store import load_metrics, load_run, list_run_ids
    ids = list_run_ids()
    runs = [(rid, load_run(rid)) for rid in ids]
    runs = [(rid, r) for rid, r in runs if r and r.outcome == "success"]
    runs.sort(key=lambda x: x[1].timestamp, reverse=True)
    if len(runs) < 2:
        _log("Need at least two successful runs to compare")
        return 1
    (id1, r1), (id2, r2) = runs[0], runs[1]
    m1, m2 = load_metrics(id1), load_metrics(id2)
    if not m1 or not m2 or not m1.overall or not m2.overall:
        _log("Missing metrics for one or both runs")
        return 1
    print(f"Run 1: {id1} {r1.code_version[:8]} {r1.timestamp}")
    print(f"Run 2: {id2} {r2.code_version[:8]} {r2.timestamp}")
    for k in ("bleu_score", "f1_score", "llm_score"):
        v1 = m1.overall.get(k)
        v2 = m2.overall.get(k)
        if v1 is not None and v2 is not None:
            diff = v2 - v1
            print(f"  {k}: {v1} -> {v2} (diff={diff:+.4f})")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="AI-driven iterative development toolchain")
    parser.add_argument(
        "command",
        nargs="?",
        default="iterate",
        choices=["iterate", "list-runs", "compare"],
        help="Command to run",
    )
    args = parser.parse_args()
    if args.command == "iterate":
        return cmd_iterate()
    if args.command == "list-runs":
        return cmd_list_runs()
    if args.command == "compare":
        return cmd_compare()
    return 0


if __name__ == "__main__":
    sys.exit(main())
