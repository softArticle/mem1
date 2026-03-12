#!/usr/bin/env python3
"""Quick test: Ark API with model ark-code-latest (OpenAI-compatible)."""
import json
import os
import sys
from pathlib import Path

# Load evaluation/.env if present
_dir = Path(__file__).resolve().parent
_env = _dir / ".env"
if _env.exists():
    try:
        from dotenv import load_dotenv
        load_dotenv(_env)
    except ImportError:
        pass

api_key = os.environ.get("ARK_API_KEY")
if not api_key:
    print("ARK_API_KEY not set. Set it or add to evaluation/.env", file=sys.stderr)
    sys.exit(1)

import httpx

base_url = os.environ.get("EVAL_LLM_BASE_URL", "https://ark.cn-beijing.volces.com/api/coding/v3")
model = "ark-code-latest"
url = f"{base_url}/chat/completions"

try:
    r = httpx.post(
        url,
        headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
        json={
            "model": model,
            "messages": [{"role": "user", "content": "Say hello in one word."}],
            "temperature": 0,
        },
        timeout=30.0,
    )
    if r.status_code != 200:
        print(f"model={model!r} -> HTTP {r.status_code}: {r.text[:500]!r}", file=sys.stderr)
        sys.exit(1)
    data = r.json()
    text = (data.get("choices") or [{}])[0].get("message", {}).get("content", "")
    print(f"model={model!r} -> ok: {text!r}")
except Exception as e:
    print(f"model={model!r} -> error: {e}", file=sys.stderr)
    sys.exit(1)
