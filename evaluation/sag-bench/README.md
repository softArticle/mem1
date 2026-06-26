# Same-judge memory-system comparison (mem1 vs mem0 OSS vs SAG)

Apples-to-apples comparison of three memory/retrieval systems on the **same**
conditions, so the numbers isolate retrieval architecture from judge-model and
embedding differences:

- **Same judge**: GPT-5.5 via the JD gateway (`evaluation/metrics/llm_judge.py`, with retry)
- **Same dataset**: LOCOMO `medium` (first 2 conversations, ~304 QAs)
- **Same embedding**: local `all-MiniLM-L6-v2` (384-dim)
- **Same answerer**: identical `ANSWER_PROMPT` + GPT-5.5

## Results (medium llm_score, same GPT-5.5 judge)

| category    | mem1   | mem0 OSS | SAG    |
|-------------|--------|----------|--------|
| **overall** | **0.8369** | 0.5579 | 0.4421 |
| multi_hop   | 0.651  | 0.558    | 0.395  |
| temporal    | 0.937  | 0.064    | 0.095  |
| open_domain | 0.846  | 0.846    | 0.923  |
| single_hop  | 0.860  | 0.798    | 0.597  |

mem1 leads overall; its rule-v2 extraction keeps whole messages (with their date
context), so temporal/single-hop win decisively. SAG's document-RAG strength shows
on open_domain (highest of the three).

## Fairness caveats (read before citing)

This comparison is **not fully fair to mem0 / SAG** — the relative ordering under
identical conditions is meaningful, but the absolute gaps are inflated by setup:

- **temporal collapses for mem0 & SAG (~0.1)**: both extract facts that strip exact
  dates, and neither was given `timestamp`/`reference_date`, so temporal answers come
  out as "today"-ish. mem1 stores whole messages verbatim, preserving dates.
- **SAG is a document-retrieval system** forced onto a conversational-memory benchmark
  (its event-fusion + multi-hop BFS are designed for document chunks).
- **No qwen3-rerank on the gateway**: SAG's reranker degrades to its local lexical
  fallback (`localScoreRerank`), weakening its retrieval.
- Their published scores use their own (stricter) judges and possibly cloud
  services — not comparable to "conversational memory + lenient GPT-5.5 judge".

## Reproduce

### mem1 / mem0
- mem1: run `mem1-server` (rule-v2 default), then `make medium` in `evaluation/`.
- mem0: `python3.11 evaluation/mem0_locomo.py` (configures mem0 OSS with the gateway
  LLM `is_reasoning_model`, local all-MiniLM, qdrant), then judge `mem0_results.json`.

### SAG (Zleap-AI/SAG) — needs source patches
1. `git clone https://github.com/Zleap-AI/SAG`, `npm install`.
2. Patch SAG to 384-dim (it hard-codes 1024):
   - `src/config/env.ts`: `SUPPORTED_EMBEDDING_DIMENSIONS = 384`
   - `migrations/001_init.sql`: all `vector(1024)` → `vector(384)`
   - `migrations/003_*.sql`: dim `1024` → `384`; `migrations/005_*.sql`: check `= 384`
   - `src/ai/llm-client.ts`: remove `temperature: 0.1` (GPT-5.5 rejects non-default temp)
3. Start a local 384-dim OpenAI-compatible embedding endpoint: `python3.11 local_embed_server.py` (:8090).
4. Start pgvector: `docker run -d --name sag-pg -e POSTGRES_USER=sag_lite -e POSTGRES_PASSWORD=sag_lite_pass -e POSTGRES_DB=sag_lite -p 5433:5432 pgvector/pgvector:pg16`
5. SAG `.env`: `EMBEDDING_DIMENSIONS=384`, `EMBEDDING_BASE_URL=http://127.0.0.1:8090/v1`,
   `LLM_BASE_URL=<gateway>`, `LLM_MODEL=GPT-5.5-joybuilder`, `DATABASE_URL=...5433...`.
6. `npm run db:setup` (migrate + seed entity_types — required).
7. Copy `locomo-run.ts` to `SAG/scripts/`, then `npx tsx scripts/locomo-run.ts`.
   - **Critical**: ingest EACH message as its own document (one event per message,
     ~211/speaker). Ingesting a whole speaker as one document fuses ~200 turns into a
     single event and is grossly unfair.
   - Read `r.sections` (original chunks) from search results, not `trace.rerankedEvents`.
8. Judge: `python3 evals.py --input_file results/sag_results.json --output_file sag_metrics.json`.
