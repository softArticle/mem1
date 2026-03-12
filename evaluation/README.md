# mem1 Evaluation

Evaluation pipeline for mem1, aligned with [mem0 evaluation](https://github.com/mem0ai/mem0/tree/main/evaluation): add memories from a LOCOMO-format dataset, run search + LLM answer, then compute BLEU, F1, and optional LLM judge scores.

## Dataset

Use the same **LOCOMO** dataset as mem0:

- **自动下载**（推荐）：在 `evaluation/` 下执行 `make download-locomo`，用 `curl` 从 [snap-research/locomo](https://github.com/snap-research/locomo) 的 GitHub raw 拉取 `locomo10.json` 到 `dataset/`（无需 gdown/Google Drive）。
- **手动**：从 [snap-research/locomo data](https://github.com/snap-research/locomo/tree/main/data) 下载 `locomo10.json`，放到 `evaluation/dataset/`。

格式：每项包含 `conversation`（`speaker_a`、`speaker_b`，及 `chat_1` 等键，值为 `{speaker, text}` 列表）和 `qa`（`{question, answer, category, evidence}` 列表）。本地快速测试可用 `dataset/sample_locomo.json`，无需下载完整数据。

## Setup

- **mem1-server**: run from repo root, e.g. `cd mem1-server && cargo run`. For meaningful retrieval (and non-zero scores), use the same server for both add and search, and enable embedding (e.g. put `embed_model/` with ONNX + tokenizer, or set `MEM1_EMBED_PROVIDER=openai`) so vector search can return memories; otherwise search may return 0 results and the LLM will answer "No relevant information.", giving BLEU/F1 = 0.
- **Python**: from repo root, `cd python && pip install -e .` (or set `PYTHONPATH` to `python/src`).
- **Evaluation deps** (from `evaluation/`):
  ```bash
  pip install -r requirements.txt
  ```
- For **LLM answer + LLM judge**: set an API key and optionally base URL and model. Any OpenAI-compatible endpoint works (e.g. 火山引擎方舟 Ark):
  - **推荐**：在 `evaluation/` 下复制 `cp .env.example .env`，填入 `ARK_API_KEY` 和 `EVAL_LLM_MODEL`（火山控制台创建的推理接入点 ID，形如 `ep-xxxxxx`）。脚本会自动加载 `.env`，无需每次 `export`。
  - 环境变量（与 `.env` 二选一或同时用）：`OPENAI_API_KEY` 或 `ARK_API_KEY`（或 `EVAL_LLM_API_KEY`）；可选 `EVAL_LLM_BASE_URL`、`EVAL_LLM_MODEL`。
  - **火山方舟（ark-code-latest）** 已写在 `.env.example`：`EVAL_LLM_BASE_URL=https://ark.cn-beijing.volces.com/api/coding/v3`，`EVAL_LLM_MODEL=ark-code-latest`。

## Commands

在 `evaluation/` 目录下执行。

**完整 LOCOMO 一条龙**（缺数据时会先执行 `make download-locomo` 再跑）：

```bash
# 确保 mem1-server 已启动（建议开 embedding）
make full
```

即：若无 `dataset/locomo10.json` 则先下载 → add → search → evals → scores。

**分步执行**（或指定数据文件）：

```bash
make add                          # 默认 dataset/locomo10.json
make search
make evals
make scores
# 或指定数据： make add DATA=dataset/locomo10.json
```

**小样本快速跑**（不需下载完整数据）：

```bash
make sample
```

可选环境变量：`MEM1_BASE_URL`（默认 `http://127.0.0.1:8080`）。

## Metrics

- **BLEU-1**: n-gram overlap (via `nltk`).
- **F1**: token-level F1 between prediction and gold.
- **LLM score**: 1 if an LLM judge labels the answer as CORRECT, 0 otherwise (requires `OPENAI_API_KEY`).

## Project layout

```
evaluation/
├── dataset/           # locomo10.json (or sample_locomo.json)
├── results/           # mem1_results.json, evaluation_metrics.json
├── src/
│   ├── mem1_add.py    # add memories from dataset
│   └── mem1_search.py # search + LLM answer per question
├── metrics/
│   ├── utils.py       # BLEU, F1
│   └── llm_judge.py   # LLM judge
├── run_experiments.py # add | search
├── evals.py           # compute metrics
├── generate_scores.py # aggregate scores
└── README.md
```
