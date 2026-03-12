"""
Aggregate evaluation_metrics.json: mean BLEU, F1, LLM score per category and overall.
"""

import argparse
import json

try:
    import pandas as pd
    HAS_PANDAS = True
except ImportError:
    HAS_PANDAS = False


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "input_file",
        nargs="?",
        default="evaluation_metrics.json",
        help="Path to evaluation_metrics.json",
    )
    args = parser.parse_args()

    with open(args.input_file, "r") as f:
        data = json.load(f)

    all_items = []
    for k, v in data.items():
        all_items.extend(v)

    if not all_items:
        print("No items in evaluation_metrics.json")
        return

    if HAS_PANDAS:
        df = pd.DataFrame(all_items)
        df["category"] = pd.to_numeric(df["category"], errors="coerce")
        by_cat = df.groupby("category").agg({
            "bleu_score": "mean",
            "f1_score": "mean",
            "llm_score": "mean",
        }).round(4)
        by_cat["count"] = df.groupby("category").size()
        print("Mean scores per category:")
        print(by_cat)
        print("\nOverall mean scores:")
        print(df.agg({"bleu_score": "mean", "f1_score": "mean", "llm_score": "mean"}).round(4))
    else:
        from collections import defaultdict
        by_cat = defaultdict(list)
        for item in all_items:
            c = item.get("category")
            by_cat[c].append(item)
        totals = {"bleu": 0.0, "f1": 0.0, "llm": 0.0}
        n = 0
        print("Mean scores per category:")
        for c in sorted(by_cat.keys(), key=lambda x: (x == "", str(x))):
            items = by_cat[c]
            b = sum(i.get("bleu_score", 0) for i in items) / len(items)
            f = sum(i.get("f1_score", 0) for i in items) / len(items)
            l = sum(i.get("llm_score", 0) for i in items) / len(items)
            print(f"  category {c}: bleu={b:.4f} f1={f:.4f} llm={l:.4f} count={len(items)}")
            for i in items:
                totals["bleu"] += i.get("bleu_score", 0)
                totals["f1"] += i.get("f1_score", 0)
                totals["llm"] += i.get("llm_score", 0)
                n += 1
        if n:
            print("\nOverall mean scores:")
            print(f"  bleu_score={totals['bleu']/n:.4f} f1_score={totals['f1']/n:.4f} llm_score={totals['llm']/n:.4f}")


if __name__ == "__main__":
    main()
