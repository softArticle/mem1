"""
Run mem1 evaluation experiments: add memories from dataset, then search + LLM answer.
Compatible with LOCOMO dataset format (same as mem0 evaluation).
Run from evaluation/ directory: python run_experiments.py --method add
"""

import argparse
import os
import sys
from pathlib import Path

# Load evaluation/.env so ARK_API_KEY, EVAL_LLM_BASE_URL, EVAL_LLM_MODEL 等生效
_EVAL_DIR = Path(__file__).resolve().parent
_env_file = _EVAL_DIR / ".env"
if _env_file.exists():
    import dotenv
    dotenv.load_dotenv(_env_file)

# Run from evaluation/ so src and metrics resolve
if str(_EVAL_DIR) not in sys.path:
    sys.path.insert(0, str(_EVAL_DIR))

from src.mem1_add import process_all_conversations
from src.mem1_search import Mem1Search


def main() -> None:
    parser = argparse.ArgumentParser(description="Run mem1 memory experiments")
    parser.add_argument(
        "--method",
        choices=["add", "search"],
        default="add",
        help="add: load dataset and add memories; search: run search + LLM answer per question",
    )
    parser.add_argument(
        "--data_path",
        type=str,
        default="dataset/locomo10.json",
        help="Path to LOCOMO-format JSON",
    )
    parser.add_argument(
        "--output_folder",
        type=str,
        default="results",
        help="Output folder for search results JSON",
    )
    parser.add_argument(
        "--top_k",
        type=int,
        default=30,
        help="Number of memories to retrieve per search",
    )
    parser.add_argument(
        "--base_url",
        type=str,
        default=os.getenv("MEM1_BASE_URL", "http://127.0.0.1:8080"),
        help="mem1-server base URL",
    )
    args = parser.parse_args()

    if args.method == "add":
        process_all_conversations(args.data_path, base_url=args.base_url)
    elif args.method == "search":
        output_path = os.path.join(args.output_folder, "mem1_results.json")
        runner = Mem1Search(
            output_path=output_path,
            top_k=args.top_k,
            base_url=args.base_url,
        )
        runner.process_data_file(args.data_path)


if __name__ == "__main__":
    main()
