"""
Compute evaluation metrics (BLEU, F1, optional LLM judge) on mem1 search results.
Input: JSON from run_experiments.py --method search (format: { "0": [ {question, answer, response, category}, ... ], ... }).
Output: same structure with bleu_score, f1_score, llm_score added; save to evaluation_metrics.json.
"""

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path

# Run from evaluation/ so metrics resolve
_EVAL_DIR = Path(__file__).resolve().parent
if str(_EVAL_DIR) not in sys.path:
    sys.path.insert(0, str(_EVAL_DIR))

from metrics.llm_judge import evaluate_llm_judge
from metrics.utils import calculate_bleu_scores, calculate_metrics


def process_item(item_data: tuple) -> dict:
    k, v = item_data
    local = defaultdict(list)
    for item in v:
        gt = str(item.get("answer", ""))
        pred = str(item.get("response", ""))
        category = str(item.get("category", ""))
        question = str(item.get("question", ""))

        if category == "5":
            continue

        metrics = calculate_metrics(pred, gt)
        bleu = calculate_bleu_scores(pred, gt)
        llm_score = evaluate_llm_judge(question, gt, pred)

        local[k].append({
            "question": question,
            "answer": gt,
            "response": pred,
            "category": category,
            "bleu_score": bleu.get("bleu1", 0.0),
            "f1_score": metrics["f1"],
            "llm_score": llm_score,
        })
    return local


def main() -> None:
    parser = argparse.ArgumentParser(description="Evaluate mem1 results")
    parser.add_argument(
        "--input_file",
        type=str,
        default="results/mem1_results.json",
        help="Path to search results JSON",
    )
    parser.add_argument(
        "--output_file",
        type=str,
        default="evaluation_metrics.json",
        help="Path to save metrics JSON",
    )
    args = parser.parse_args()

    with open(args.input_file, "r") as f:
        data = json.load(f)

    results = defaultdict(list)
    for item_data in data.items():
        local = process_item(item_data)
        for k, items in local.items():
            results[k].extend(items)

    with open(args.output_file, "w") as f:
        json.dump(dict(results), f, indent=2)

    print(f"Results saved to {args.output_file}")


if __name__ == "__main__":
    main()
