---
name: mem1-auto-optimize
description: Run the mem1-server optimization loop until metrics match mem0 baseline (build → start → eval → collect → compare → modify server code → stop → repeat). Use when the user asks to "run mem1 auto-optimization", "match mem0", or "iterate until target".
---

# mem1 Auto-Optimize (Server-Side, Agent-Full-Control)

## Scope and authority

- You have **full control of the workspace**: run any shell commands (build, start/stop processes), edit any repo files. The optimization target is **mem1-server** (Rust server); the evaluation pipeline is only used to collect metrics.
- **mem1-server build, start, and stop are all under your control**; do not assume the user has already started the server.

## Autonomous execution — no user decisions

- **You must fix any blocking issue yourself.** If the server fails to start (e.g. panic, missing embed, DB lock), fix the code (e.g. embed download in a thread, clear stale lock, use a different data dir) and retry. If build fails, fix the code. If eval fails (e.g. connection refused, script error), fix the environment or server and re-run. Do **not** ask the user "should I fix X?" or "use sample or full?" — decide and act.
- **Optimization direction is yours.** When metrics are below baseline, decide what to change (retrieval, embedding, search, storage) and apply the code changes; do not ask the user to choose.

## 现阶段优化范围（避免死循环）

- **只优化空投方案，不做参数调节。** 修改代码时只针对**已空投的设计**做优化（例如 Zep 的图上下文：`related_ids`、`expand_with_related`、图结构/组装/排序、metadata 用法等）。**禁止**调节以下参数或等价逻辑：`RRF_K`、`RRF_KEYWORD_WEIGHT`、`fetch_limit_for_rrf`、`significant_terms` 的 term 数量/截断、以及其它与 RRF/检索分支无关的常数。若本轮没有空投，则只允许对现有「图/上下文/空投相关」逻辑做增强或重构，不得改上述参数。
- 原因：参数调节容易在多轮中反复回退、形成死循环；先阶段专注空投方案的改进，待稳定后再考虑参数。
- **The user only gets the result:** at the end, output whether the target was met, total rounds, and final scores. No intermediate "what should I do?" — only final report (or a short per-round summary and then the final report).

## Prerequisites

- Current working directory is the **repo root** (e.g. `mem1`).
- Paths exist: `mem1-server/`, `evaluation/`, and `evaluation/baselines/mem0_locomo.json`.
- No need for the user to start the server; you build and start it each round.

## Step 0. Airdrop 空投（仅当用户提供新方案时执行）

若用户在本轮调用中提供了 **新方案引用**（GitHub 仓库或论文 URL/路径），先执行本步；否则跳过，直接从下面的 One-round steps 的 Step 1 开始。

1. **解析引用**  
   从本次调用的用户输入中识别「新方案」：GitHub 仓库 URL（如 `https://github.com/org/repo` 或 `org/repo`）、论文 URL（如 arxiv、PDF 链接）或本地文件路径（如 `./papers/xxx.pdf`）。若未提供，跳过整节，从 Step 1（Build）开始。

2. **还原点**  
   执行 `git add -A && git commit -m "pre-airdrop"`（若无变更可跳过），以便空投引入错误时可回退。

3. **拉取**  
   - **GitHub**：浅克隆或下载 archive（如 `git clone --depth 1 <url>` 或 `curl -L` 下载 archive.zip），得到本地目录。  
   - **论文**：通过 URL 拉取正文/摘要或读取本地 PDF；若仅能拿到摘要，以摘要+标题为主进行分析。

4. **分析设计**  
   - **仓库**：阅读 README、文档、核心代码（语言不限），提取与 **记忆存储、检索、embedding、记忆模型** 相关的设计（算法、数据结构、配置）。  
   - **论文**：提取方法描述、公式、流程，映射到「存储/检索/embedding」中的可实现点。

5. **引入到 mem1-server（仅服务端）**  
   - 将上述设计 **迁移/改写** 到 mem1-server 的 Rust 实现中；**不修改 evaluation**。  
   - 可涉及：`mem1-server/src/storage/memory.rs`（检索、RRF、keyword/vector 策略）、`mem1-server/src/memory/model.rs`、`embedding.rs`、`local_embed.rs`（模型与嵌入），以及其他 mem1-server 内与存储/检索/embed 相关的模块。  
   - 保持现有 HTTP API（add/search/get/delete）不变；仅改内部实现与数据结构。

6. **提交**  
   `git add -A && git commit -m "airdrop: <简短描述> from <ref>"`。

7. **继续**  
   从下面的 **One-round steps** 的 Step 1（Build）开始执行，后续与当前流程一致（build → start → eval → collect → compare；未达标则 commit 还原点 → 改代码 → re-eval → assess）。

**异常**：引用不可达（私有仓库、死链、无权限）时，在报告中说明并跳过空投，直接进入 Step 1。无法从论文/仓库提取到可实施设计时，同样说明后跳过空投。

## One-round steps

Execute in order each iteration. Keep **metrics from previous run** (or baseline) as **M_prev** for the assessment step.

1. **Build**  
   In `mem1-server/`: run `cargo build --release` (or `cargo build`). If build fails, report and stop or fix the code.

2. **Start**  
   Start mem1-server in the background. **Record the process id (PID)** so you can stop it later.

3. **Wait for server ready**  
   Poll or sleep so the server can bind and accept connections before evaluation runs.

4. **Run evaluation**  
   In `evaluation/`: run evaluation; ensure `MEM1_BASE_URL` points at the running mem1-server.  
   - **Default for iteration:** When max_rounds is specified and large (e.g. ≥10), use **`make medium`** for each round’s eval (and for re-eval after code changes). Medium uses the first 2 conversations from locomo10 (~300 QAs), giving more stable metrics than `make sample` (1 QA) while keeping each round feasible (~20–40 min). Each round still **changes code when not met**, then re-evals and assesses.  
   - **Full eval:** Use `make full` when max_rounds is small or when doing a final confirmation (see Notes).

5. **Collect metrics**  
   Read `evaluation/evaluation_metrics.json`. Parse into **overall** and **by_category** (e.g. `bleu_score`, `f1_score`, `llm_score`). Call this result **M_current**.

6. **Compare to baseline**  
   Load `evaluation/baselines/mem0_locomo.json`. **Pass criterion:** `M_current.overall.llm_score` ≥ baseline `overall.llm_score` (e.g. ≥ 0.8487).

7. **If passed (target met)**  
   Stop mem1-server. Output round number, final scores, and that the run is complete. **End the loop.**

8. **If not passed — optimization and assessment (评估环节)**  
   - **Stop** mem1-server.
   - **Create restore point (必须，否则无法可靠回退):** 在修改任何 mem1-server 代码之前，先提交当前工作区，以便之后可以回退。执行：`git add -A && git commit -m "pre-optimization round N"`（N 为当前轮次）。若当前无变更可提交（working tree clean），可跳过本步，但若上一轮或本轮之前已做过一次「改代码前的提交」，则回退时用该提交即可。
   - Decide **optimization direction** from the gaps. **Edit mem1-server code** (apply one or a small set of changes). 修改须符合「现阶段优化范围」：仅空投方案相关，不调 RRF/ fetch_limit / significant_terms 等参数。
     - **Re-eval:** Rebuild, start server again, run evaluation again (same choice as step 4: medium or full), then **collect new metrics** → call this **M_new**.
   - **Assessment (评估):** Compare **M_new** vs **M_prev** (previous round’s metrics, or M_current from before this round’s change). Use a single primary metric for “improvement”, e.g. `overall.llm_score`.
     - **If M_new shows improvement** (e.g. `M_new.overall.llm_score` > `M_prev.overall.llm_score`): Treat the change as good. Stop server. Set M_prev = M_new. Go back to step 1 for the **next round** (or re-check baseline; if now passed, end).
     - **If M_new shows no improvement or regression** (e.g. `M_new.overall.llm_score` ≤ `M_prev.overall.llm_score`):
       - **You must decide:**  
         - **Optimization direction wrong (方向不对):** **Revert the code** by restoring the pre-edit state. Because you committed before editing (restore point), run: `git reset --hard HEAD~1` (回退到「改代码前」的那一次提交). Then choose a **different** optimization direction, apply a new change, and run **re-eval + assessment** again (from “Edit mem1-server code” in this step). If you did not create a commit before this round’s edit (e.g. no changes were staged), you cannot use `git reset --hard HEAD~1`; then you must manually undo your edits (re-apply the previous file contents from your own record or re-read and revert the same files).
         - **Direction right but insufficient (力度不够):** Keep the current code change, apply a **stronger or follow-up** change, then re-eval and run assessment again.
     - After a revert, do **not** count the reverted attempt as a full round; only count a round when you keep a change and proceed to the next round or exit.
   - Always stop the server before rebuilding or before the next eval.

## Stop conditions

- **Target met:** overall llm_score ≥ baseline overall llm_score (and any additional criteria if baseline is extended).
- **Max rounds:** if a maximum iteration count was specified (e.g. via command args), stop after that many rounds and report final scores.
- **User request:** if the user asks to stop, stop the server and end.

## Output per round and at end

- **Each round:** current overall (and optionally by_category) scores, gap to baseline, and a short summary of changes made to mem1-server (if any).
- **On success:** "Target met", total rounds, final scores.
- **On max rounds or stop:** total rounds, final scores, and whether target was met or not.

## Baseline file

- Path: `evaluation/baselines/mem0_locomo.json`.
- Structure: `overall` with `llm_score` (and optionally `bleu_score`, `f1_score`); `by_category` array with `category`, `llm_score`, and optional `bleu_score`, `f1_score`.
- Initial pass/fail is based on **overall.llm_score**; other fields can be used for reporting or stricter criteria later.

## Notes

- **Revert depends on a restore point:** 每轮在改 mem1-server 代码前必须先 `git add -A && git commit -m "pre-optimization round N"`，这样判断「方向错了」时才能用 `git reset --hard HEAD~1` 回退；否则没有可回退的提交，只能靠手动恢复文件。
- **Medium vs full:** 多轮迭代时默认用 **`make medium`**（前 2 个 conversation，~300 QA），比 sample 更有统计意义；达标或达到 max_rounds 后，可再跑一次 `make full` 作为最终指标并上报该分数。
- Always stop the server before rebuilding or before exiting (no leftover processes).
- If eval fails (e.g. script error, server not reachable), fix the cause yourself (server code, env, or eval script), stop the server if needed, and retry. Do not ask the user; deliver the result or report after max rounds.
