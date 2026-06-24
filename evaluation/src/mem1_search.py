"""
Run mem1 search for each question in the dataset, then use an LLM to generate
an answer from retrieved memories. Output format matches mem0 for evals (question, answer, response, category).
"""

import json
import os
import sys
import time
from pathlib import Path

_REPO_ROOT = Path(__file__).resolve().parent.parent.parent
_PYTHON_SRC = _REPO_ROOT / "python" / "src"
if _PYTHON_SRC.is_dir() and str(_PYTHON_SRC) not in sys.path:
    sys.path.insert(0, str(_PYTHON_SRC))

import httpx
from jinja2 import Template
from tqdm import tqdm

from mem1 import Memory

# Prompt: answer from two users' memories (no graph)
ANSWER_PROMPT = """
You are an intelligent memory assistant. Use only the provided memories to answer the question.
Resolve relative dates such as yesterday, last week, last Saturday, last year, and next month using the memory Date shown in the context. Answer the inferred event date, month, or year; do not answer with the memory Date unless the event happened on that date.

Memories for user {{speaker_1_user_id}}:
{{speaker_1_memories}}

Memories for user {{speaker_2_user_id}}:
{{speaker_2_memories}}

Question: {{question}}

Answer in 5-6 words or less:
"""


def memory_context_from_response(resp: dict) -> str:
    formatted_context = (resp.get("formatted_context") or "").strip()
    if formatted_context:
        return formatted_context
    items = resp.get("results", [])
    return "\n".join(m.get("content", "") for m in items)


def load_data(file_path: str) -> list:
    with open(file_path, "r") as f:
        return json.load(f)


def build_answer_prompt(
    speaker_1_user_id: str,
    speaker_2_user_id: str,
    speaker_1_memories: str,
    speaker_2_memories: str,
    question: str,
) -> str:
    template = Template(ANSWER_PROMPT)
    return template.render(
        speaker_1_user_id=speaker_1_user_id,
        speaker_2_user_id=speaker_2_user_id,
        speaker_1_memories=speaker_1_memories,
        speaker_2_memories=speaker_2_memories,
        question=question,
    )


class Mem1Search:
    def __init__(
        self,
        output_path: str = "results/mem1_results.json",
        top_k: int = 30,
        base_url: str = "http://127.0.0.1:8080",
    ):
        self.memory = Memory(base_url=base_url)
        self.top_k = top_k
        self.output_path = output_path
        self.results = {}
        # OpenAI-compatible LLM: use httpx to avoid openai lib proxy/version issues (e.g. Ark)
        self._llm_api_key = os.getenv("EVAL_LLM_API_KEY") or os.getenv("OPENAI_API_KEY") or os.getenv("ARK_API_KEY")
        self._llm_base_url = (os.getenv("EVAL_LLM_BASE_URL") or os.getenv("OPENAI_BASE_URL") or "").rstrip("/")
        self._llm_model = os.getenv("MODEL") or os.getenv("EVAL_LLM_MODEL", "gpt-4o-mini")
        self._llm_ok = bool(self._llm_api_key and self._llm_base_url)

    def search_memory(self, user_id: str, query: str) -> tuple[list[dict], str, float]:
        start = time.time()
        resp = self.memory.search(query, user_id=user_id, limit=self.top_k)
        elapsed = time.time() - start
        items = resp.get("results", [])
        context = memory_context_from_response(resp)
        return items, context, elapsed

    def answer_question(
        self,
        speaker_a_user_id: str,
        speaker_b_user_id: str,
        question: str,
    ) -> tuple[str, list, list, float, float, float]:
        mem_a, context_a, time_a = self.search_memory(speaker_a_user_id, question)
        mem_b, context_b, time_b = self.search_memory(speaker_b_user_id, question)
        if not mem_a and not mem_b:
            import warnings
            warnings.warn(
                f"Search returned 0 memories for both users (q={question!r}). "
                "Check that add ran against the same mem1-server and that embedding or keyword index is enabled."
            )

        speaker_1_id = speaker_a_user_id.split("_")[0]
        speaker_2_id = speaker_b_user_id.split("_")[0]

        prompt = build_answer_prompt(
            speaker_1_user_id=speaker_1_id,
            speaker_2_user_id=speaker_2_id,
            speaker_1_memories=context_a or "(none)",
            speaker_2_memories=context_b or "(none)",
            question=question,
        )

        response_time = 0.0
        if self._llm_ok:
            t0 = time.time()
            url = f"{self._llm_base_url}/chat/completions"
            payload = {
                "model": self._llm_model,
                "messages": [{"role": "user", "content": prompt}],
            }
            # Retry transient gateway failures so a flaky call doesn't become a
            # permanent "(LLM error)" that scores 0 and adds noise to the metric.
            response_text = "(LLM error: no attempt)"
            for attempt in range(4):
                try:
                    r = httpx.post(
                        url,
                        headers={"Authorization": f"Bearer {self._llm_api_key}", "Content-Type": "application/json"},
                        json=payload,
                        timeout=60.0,
                    )
                    if r.status_code == 200:
                        data = r.json()
                        response_text = (data.get("choices") or [{}])[0].get("message", {}).get("content", "") or ""
                        break
                    response_text = f"(LLM error {r.status_code})"
                except Exception as e:
                    response_text = f"(LLM error: {e})"
                time.sleep(1.5 * (attempt + 1))
            response_time = time.time() - t0
        else:
            response_text = "(OPENAI_API_KEY not set; skip LLM answer)"

        return response_text, mem_a, mem_b, time_a, time_b, response_time

    def process_question(
        self,
        qa_item: dict,
        speaker_a_user_id: str,
        speaker_b_user_id: str,
    ) -> dict:
        question = qa_item.get("question", "")
        answer = qa_item.get("answer", "")
        category = qa_item.get("category", -1)

        response, mem_a, mem_b, t_a, t_b, t_resp = self.answer_question(
            speaker_a_user_id, speaker_b_user_id, question
        )

        return {
            "question": question,
            "answer": answer,
            "category": category,
            "response": response,
            "evidence": qa_item.get("evidence", []),
            "speaker_1_memories": [{"content": m.get("content", ""), "score": m.get("score")} for m in mem_a],
            "speaker_2_memories": [{"content": m.get("content", ""), "score": m.get("score")} for m in mem_b],
            "speaker_1_memory_time": t_a,
            "speaker_2_memory_time": t_b,
            "response_time": t_resp,
        }

    def process_data_file(self, file_path: str) -> None:
        data = load_data(file_path)
        Path(self.output_path).parent.mkdir(parents=True, exist_ok=True)

        for idx, item in enumerate(tqdm(data, desc="Conversations")):
            conv = item.get("conversation", {})
            qa = item.get("qa", [])
            speaker_a = conv.get("speaker_a", "A")
            speaker_b = conv.get("speaker_b", "B")
            speaker_a_user_id = f"{speaker_a}_{idx}"
            speaker_b_user_id = f"{speaker_b}_{idx}"

            self.results[str(idx)] = []
            for qa_item in tqdm(qa, desc=f"QA conv {idx}", leave=False):
                rec = self.process_question(qa_item, speaker_a_user_id, speaker_b_user_id)
                self.results[str(idx)].append(rec)

            with open(self.output_path, "w") as f:
                json.dump(self.results, f, indent=2)

        print(f"Results saved to {self.output_path}")


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("data_path", nargs="?", default="dataset/locomo10.json")
    parser.add_argument("--output", default="results/mem1_results.json")
    parser.add_argument("--top_k", type=int, default=30)
    parser.add_argument("--base_url", default="http://127.0.0.1:8080")
    args = parser.parse_args()

    runner = Mem1Search(output_path=args.output, top_k=args.top_k, base_url=args.base_url)
    runner.process_data_file(args.data_path)
