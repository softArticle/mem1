#!/usr/bin/env python3
"""Print overall llm_score from evaluation_metrics.json (one line, for scripts)."""
import json
import sys

def main():
    path = "evaluation_metrics.json"
    if len(sys.argv) > 1:
        path = sys.argv[1]
    try:
        with open(path) as f:
            data = json.load(f)
    except FileNotFoundError:
        print("0.0", file=sys.stderr)
        sys.exit(1)
    items = []
    for v in data.values():
        items.extend(v) if isinstance(v, list) else items.append(v)
    if not items:
        print("0.0", file=sys.stderr)
        sys.exit(1)
    s = sum(i.get("llm_score", 0) for i in items) / len(items)
    print(f"{s:.4f}")

if __name__ == "__main__":
    main()
