#!/usr/bin/env python3
"""Translate a LOCOMO dataset file to Chinese for diagnostic eval.

Builds a Chinese variant of the LOCOMO JSON while preserving structure exactly:
- Translates each session turn's `text` and each QA's `question` / `answer`.
- Keeps speaker names (Caroline/Mel/...), dia_id, *_date_time, category, evidence
  and all summary fields unchanged — translating names would add noise and break
  user_id construction ({speaker}_{idx}), and the point is to test how Chinese
  *content* affects retrieval (esp. the capitalization-based graph entity
  heuristic), not to localize names.

Date-like answers (e.g. "7 May 2023") are left untranslated so temporal scoring
stays comparable.

Usage:
  EVAL_LLM_API_KEY=... EVAL_LLM_BASE_URL=... MODEL=... \
    python3 translate_to_zh.py dataset/medium_locomo.json dataset/medium_locomo_zh.json
"""
import json
import os
import re
import sys
import time

import httpx

API_KEY = os.getenv("EVAL_LLM_API_KEY") or os.getenv("OPENAI_API_KEY")
BASE_URL = (os.getenv("EVAL_LLM_BASE_URL") or os.getenv("OPENAI_BASE_URL") or "").rstrip("/")
MODEL = os.getenv("MODEL") or os.getenv("EVAL_LLM_MODEL", "gpt-4o-mini")

SYSTEM = (
    "You are a professional EN->ZH translator for a conversational dataset. "
    "Translate each numbered English string into natural, colloquial Simplified Chinese. "
    "Rules: keep person names in their original English form (e.g. Caroline, Mel); "
    "keep dates, numbers, and proper nouns faithful; do NOT translate a string that is "
    "purely a date/number (return it unchanged); preserve meaning exactly, no additions. "
    "Return ONLY a JSON object {\"items\": [\"<zh1>\", \"<zh2>\", ...]} with the same count and order."
)

DATE_ONLY = re.compile(r"^[\d\s\-/:.,]*(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec|am|pm|\d)[\w\s\-/:.,]*$", re.I)


def translate_batch(strings):
    """Translate a list of strings; returns a same-length list. Empty/date-only pass through."""
    idx_to_translate = [i for i, s in enumerate(strings) if s.strip() and not _is_date_only(s)]
    if not idx_to_translate:
        return list(strings)
    payload_items = [strings[i] for i in idx_to_translate]
    numbered = "\n".join(f"{j+1}. {s}" for j, s in enumerate(payload_items))
    body = {
        "model": MODEL,
        "messages": [
            {"role": "system", "content": SYSTEM},
            {"role": "user", "content": f"Translate these {len(payload_items)} strings:\n{numbered}"},
        ],
    }
    out = None
    for attempt in range(4):
        try:
            r = httpx.post(
                f"{BASE_URL}/chat/completions",
                headers={"Authorization": f"Bearer {API_KEY}", "Content-Type": "application/json"},
                json=body,
                timeout=120.0,
            )
            if r.status_code != 200:
                time.sleep(1.5 * (attempt + 1))
                continue
            content = r.json()["choices"][0]["message"]["content"]
            obj = json.loads(_extract_json(content))
            items = obj.get("items", [])
            if len(items) == len(payload_items):
                out = items
                break
            time.sleep(1.5 * (attempt + 1))
        except Exception:
            time.sleep(1.5 * (attempt + 1))
    if out is None:
        # Fail-open: keep originals for this batch rather than dropping content.
        print(f"  [warn] batch translate failed, keeping {len(payload_items)} originals", file=sys.stderr)
        return list(strings)
    result = list(strings)
    for j, i in enumerate(idx_to_translate):
        result[i] = out[j]
    return result


def _is_date_only(s):
    return bool(DATE_ONLY.match(s.strip())) and not re.search(r"[a-z]{4,}", s.lower().replace("may", ""))


def _extract_json(text):
    text = text.strip()
    if text.startswith("```"):
        text = re.sub(r"^```(json)?", "", text).rsplit("```", 1)[0]
    start, end = text.find("{"), text.rfind("}")
    return text[start : end + 1] if start >= 0 and end > start else text


def main():
    src, dst = sys.argv[1], sys.argv[2]
    data = json.load(open(src))
    for ci, conv in enumerate(data):
        c = conv["conversation"]
        # Collect + translate all turn texts per conversation (batched by session).
        for key in list(c.keys()):
            if key.startswith("session") and isinstance(c[key], list):
                turns = c[key]
                texts = [t.get("text", "") for t in turns]
                zh = translate_batch(texts)
                for t, z in zip(turns, zh):
                    t["text"] = z
                print(f"conv{ci}: {key} ({len(turns)} turns) done")
        # Translate QA questions + answers.
        qa = conv.get("qa", [])
        questions = [q.get("question", "") for q in qa]
        answers = [str(q.get("answer", "")) for q in qa]
        zq = translate_batch(questions)
        za = translate_batch(answers)
        for q, zqi, zai in zip(qa, zq, za):
            q["question"] = zqi
            if "answer" in q:
                q["answer"] = zai
        print(f"conv{ci}: qa ({len(qa)}) done")
    json.dump(data, open(dst, "w"), ensure_ascii=False, indent=2)
    print(f"wrote {dst}")


if __name__ == "__main__":
    main()
