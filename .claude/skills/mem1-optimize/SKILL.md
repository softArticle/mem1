---
name: mem1-optimize
description: Run the mem1-server recall-optimization loop against the LOCOMO benchmark — build, start server, run eval, compare to the current-best floor, change server code, keep-or-revert, repeat. Use when the user asks to "optimize mem1", "冲分", "run the optimization loop", "iterate until target", or hands over a new paper/repo to try (airdrop).
---

# mem1 Optimize (recall loop, agent-full-control)

Autonomous optimization of **mem1-server** recall on LOCOMO. You control the whole workspace (build/run/edit); the eval pipeline only collects metrics. Decide and act — do not ask the user "should I fix X?" or "sample or full?".

## The floor (the number that matters)

- **Current best (default config): medium llm_score = 0.8369.** This is the floor: any change that lands **below 0.8369 must be reverted**.
- The old skill compared against "mem0 paper 0.8487" — that is **wrong / stale**. Under the *same* GPT-5.5 judge + same medium data, mem0 OSS scores only ~0.56 and SAG ~0.44; mem1 already beats them. 0.8487 was mem0's own (stricter) judge, not comparable. Optimize against the **0.8369 floor**, not 0.8487.
- **Eval jitter is ±0.02** even with retry. A delta under ~0.02 is noise, not a win — need a larger delta or multiple rounds to trust it.

## What has already been tried (do NOT repeat — all verified dead)

Read [[mem1-recall-optimization-findings]] in memory first. Confirmed dead ends, with scores:
- **Cross-encoder rerank** (ms-marco 0.8069, bge-reranker 0.8283) — both below floor. Reranking can't fix recall misses.
- **LLM multi-query rewrite** (0.8112) — hurt multi_hop. Widening recall coverage systematically hurts multi/single/temporal.
- **Global MMR, stemming, context re-ranking** — all net-negative "此消彼长" (open gains, temporal loses).
- **Core lesson:** LOCOMO's bottleneck is "correct fact ranked into the top-k window", NOT recall width or rerank order. Widening-recall and rerank families are dead. The remaining gap (~0.01 on English) is mostly in the **answering LLM** (eval-side), not retrieval.
- **Do NOT** re-try reranking or recall-widening unless it's a genuinely new mechanism.

## Config knobs (all env, already tuned to sweet spots)

rule-v2 extraction + 3-way RRF (keyword+vector+graph) + protected-prefix MMR. Sweet spots: `MEM1_MMR_LAMBDA=0.85`, `MEM1_MMR_PROTECT=limit/2`, `MEM1_RERANK_POOL_EXTRA=30` (pool=60), `MEM1_RRF_K=60`, top_k=30. These were swept — don't re-sweep without a reason.

## Multilingual (Qwen3 embedding)

Non-English recall needs `MEM1_EMBED_PROVIDER=qwen3` + `MEM1_EMBED_DIM=1024` + a Chinese eval set (`evaluation/dataset/medium_locomo_zh.json`, `MEM1_EVAL_LANG=zh`). Default all-MiniLM collapses on Chinese (0.39 → 0.79 with Qwen3). See [[mem1-eval-loop-setup]].

## Step 0. Airdrop (only when the user gives a new paper/repo reference)

If the user provides a GitHub repo or paper URL/path: (1) `git commit` a restore point; (2) fetch it (shallow clone / download / read PDF); (3) extract the storage/retrieval/embedding design; (4) port it into **mem1-server only** (never touch evaluation), keeping the HTTP API stable; (5) `git commit -m "airdrop: <what> from <ref>"`; (6) proceed to the loop. If the reference is unreachable or has no implementable design, note it and skip. Otherwise skip Step 0 entirely.

## One-round loop

1. **Build:** `cargo build --release` (workspace root, or `-p mem1-server`). Fix code if it fails.
2. **Start:** run the **root** binary `./target/release/mem1-server` in background, record PID. ⚠️ Never run `mem1-server/target/...` — that path may hold a stale pre-workspace binary. Fresh `MEM1_DB_PATH` if extraction changed.
3. **Wait ready:** poll `/healthz`.
4. **Eval:** in `evaluation/`, `make medium` (2 convs, ~304 QA). Ensure `MEM1_BASE_URL` points at the server. Env `.env` has the GPT-5.5 gateway (never send `temperature` — GPT-5.5 rejects it).
5. **Collect:** read `evaluation/evaluation_metrics.json` → overall + by_category llm_score. Call it M_current.
6. **Compare to floor 0.8369.** If ≥ floor AND improved: keep, update floor, next round. If < floor: revert.
7. **Change + keep-or-revert:** before editing, `git commit -m "pre-opt round N"` (restore point). Edit mem1-server. Re-eval. If M_new > M_prev (beyond ±0.02 noise): keep, next round. If ≤: `git reset --hard HEAD~1` and try a different direction, OR if direction seems right but weak, strengthen it. Always check by_category — overall can hide a temporal regression.
8. Always stop the server before rebuild/exit. No leftover processes.

## Output

Per round: overall + gap to floor + what changed. At end: whether floor was beaten, total rounds, final scores. No mid-loop questions.
