"""
LLM judge: score generated answer vs gold as CORRECT/WRONG (1/0).
Uses OpenAI-compatible API via httpx (avoids openai lib proxy issues): ARK_API_KEY etc.
Loads evaluation/.env if present.
"""

import json
import os
from pathlib import Path
from typing import Optional

import httpx

# Load evaluation/.env when evals.py imports this module
_env_file = Path(__file__).resolve().parent.parent / ".env"
if _env_file.exists():
    try:
        from dotenv import load_dotenv
        load_dotenv(_env_file)
    except ImportError:
        pass

ACCURACY_PROMPT = """
Your task is to label an answer to a question as 'CORRECT' or 'WRONG'. You will be given:
(1) a question, (2) a gold (ground truth) answer, (3) a generated answer.
Score as CORRECT if the generated answer touches on the same topic/fact as the gold answer.
Be generous: same date in different format, or longer explanation that includes the gold, counts as CORRECT.

Question: {question}
Gold answer: {gold_answer}
Generated answer: {generated_answer}

Reply with a JSON object containing key "label" with value "CORRECT" or "WRONG" only.
"""


def extract_json(text: str) -> str:
    """Extract first JSON object from text."""
    start = text.find("{")
    if start == -1:
        return "{}"
    depth = 0
    for i in range(start, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[start : i + 1]
    return "{}"


def evaluate_llm_judge(
    question: str,
    gold_answer: str,
    generated_answer: str,
    model: Optional[str] = None,
) -> int:
    """Return 1 if CORRECT, 0 if WRONG. Returns 0 if no API key configured."""
    api_key = os.getenv("EVAL_LLM_API_KEY") or os.getenv("OPENAI_API_KEY") or os.getenv("ARK_API_KEY")
    if not api_key:
        return 0
    model = model or os.getenv("MODEL") or os.getenv("EVAL_LLM_MODEL", "gpt-4o-mini")
    base_url = (os.getenv("EVAL_LLM_BASE_URL") or os.getenv("OPENAI_BASE_URL") or "").rstrip("/")
    if not base_url:
        return 0
    try:
        url = f"{base_url}/chat/completions"
        r = httpx.post(
            url,
            headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
            json={
                "model": model,
                "messages": [
                    {
                        "role": "user",
                        "content": ACCURACY_PROMPT.format(
                            question=question,
                            gold_answer=gold_answer,
                            generated_answer=generated_answer,
                        ),
                    }
                ],
            },
            timeout=30.0,
        )
        if r.status_code != 200:
            return 0
        data = r.json()
        content = (data.get("choices") or [{}])[0].get("message", {}).get("content", "{}")
        obj = json.loads(extract_json(content))
        label = obj.get("label", "WRONG").upper()
        return 1 if label == "CORRECT" else 0
    except Exception:
        return 0
