"""
Run mem0 (OSS) on the same LOCOMO medium subset, same gateway LLM judge/answerer,
same local all-MiniLM embedder as mem1 — to get an apples-to-apples comparison.
Outputs results in the same shape as mem1_results.json for evals.py to judge.
"""
import os, sys, json, time, shutil, warnings
warnings.filterwarnings("ignore")
os.environ["TOKENIZERS_PARALLELISM"] = "false"

_EVAL_DIR = os.path.dirname(os.path.abspath(__file__))
# load eval .env for gateway creds
envf = os.path.join(_EVAL_DIR, ".env")
if os.path.exists(envf):
    for line in open(envf):
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            k, v = line.split("=", 1)
            os.environ.setdefault(k, v)

API_KEY = os.environ["EVAL_LLM_API_KEY"]
BASE_URL = os.environ["EVAL_LLM_BASE_URL"].rstrip("/")
MODEL = os.environ.get("MODEL", "GPT-5.5-joybuilder")
os.environ["OPENAI_API_KEY"] = API_KEY
os.environ["OPENAI_BASE_URL"] = BASE_URL

import httpx
from tqdm import tqdm
from mem0 import Memory

ANSWER_PROMPT = """You are an intelligent memory assistant. Use only the provided memories to answer the question.
Resolve relative dates such as yesterday, last week, last Saturday, last year, and next month using the memory Date shown in the context. Answer the inferred event date, month, or year; do not answer with the memory Date unless the event happened on that date.

Memories for user {s1}:
{m1}

Memories for user {s2}:
{m2}

Question: {q}

Answer in 5-6 words or less:"""


def build_memory(qdrant_path):
    shutil.rmtree(qdrant_path, ignore_errors=True)
    config = {
        "llm": {"provider": "openai", "config": {"model": MODEL, "is_reasoning_model": True, "openai_base_url": BASE_URL, "api_key": API_KEY}},
        "embedder": {"provider": "huggingface", "config": {"model": "sentence-transformers/all-MiniLM-L6-v2"}},
        "vector_store": {"provider": "qdrant", "config": {"collection_name": "locomo", "path": qdrant_path, "on_disk": False, "embedding_model_dims": 384}},
    }
    return Memory.from_config(config)


def answer(q, s1, m1, s2, m2):
    prompt = ANSWER_PROMPT.format(s1=s1, m1=m1 or "(none)", s2=s2, m2=m2 or "(none)", q=q)
    for attempt in range(4):
        try:
            r = httpx.post(f"{BASE_URL}/chat/completions",
                headers={"Authorization": f"Bearer {API_KEY}", "Content-Type": "application/json"},
                json={"model": MODEL, "messages": [{"role": "user", "content": prompt}]}, timeout=60.0)
            if r.status_code == 200:
                return (r.json().get("choices") or [{}])[0].get("message", {}).get("content", "") or ""
        except Exception:
            pass
        time.sleep(1.5 * (attempt + 1))
    return "(LLM error)"


def speaker_messages(conv):
    sa, sb = conv["speaker_a"], conv["speaker_b"]
    ma, mb = [], []
    for key in conv:
        if key in ("speaker_a", "speaker_b") or "date" in key or "timestamp" in key:
            continue
        chats = conv.get(key, [])
        if not isinstance(chats, list):
            continue
        for chat in chats:
            sp, txt = chat.get("speaker", ""), chat.get("text", "").strip()
            if not txt:
                continue
            msg = f"{sp}: {txt}"
            (ma if sp == sa else mb if sp == sb else []).append(msg) if sp in (sa, sb) else None
    return sa, sb, ma, mb


def main():
    data = json.load(open(os.path.join(_EVAL_DIR, "dataset/medium_locomo.json")))
    mem = build_memory("/tmp/mem0_qdrant_locomo")
    # ADD
    for idx, item in enumerate(data):
        conv = item["conversation"]
        sa, sb, ma, mb = speaker_messages(conv)
        for uid, msgs, name in [(f"{sa}_{idx}", ma, sa), (f"{sb}_{idx}", mb, sb)]:
            for m in tqdm(msgs, desc=f"add {name}_{idx}", leave=False):
                try:
                    mem.add(m, user_id=uid)
                except Exception as e:
                    print("add err:", str(e)[:80])
    print("ADD done.")
    # SEARCH + ANSWER
    out = {}
    for idx, item in enumerate(data):
        conv = item["conversation"]
        sa, sb = conv["speaker_a"], conv["speaker_b"]
        ua, ub = f"{sa}_{idx}", f"{sb}_{idx}"
        qas = item.get("qa", [])
        conv_out = []
        for qa in tqdm(qas, desc=f"QA conv {idx}", leave=False):
            q = qa.get("question", "")
            gold = qa.get("answer", "")
            cat = qa.get("category", -1)
            if not q:
                continue
            try:
                ra = mem.search(q, filters={"user_id": ua}, limit=30)
                rb = mem.search(q, filters={"user_id": ub}, limit=30)
                la = [x.get("memory", "") for x in (ra.get("results") if isinstance(ra, dict) else ra) or []]
                lb = [x.get("memory", "") for x in (rb.get("results") if isinstance(rb, dict) else rb) or []]
            except Exception as e:
                la, lb = [], []
                print("search err:", str(e)[:80])
            resp = answer(q, sa, "\n".join(la), sb, "\n".join(lb))
            conv_out.append({"question": q, "answer": str(gold), "category": str(cat), "response": resp,
                             "speaker_1_memories": [{"content": c} for c in la],
                             "speaker_2_memories": [{"content": c} for c in lb]})
        out[str(idx)] = conv_out
    json.dump(out, open(os.path.join(_EVAL_DIR, "results/mem0_results.json"), "w"), indent=2)
    print("SEARCH done -> results/mem0_results.json")


if __name__ == "__main__":
    main()
