# 为什么 mem1 分数低、空投优化“回归”？——简要分析

## 1. 和 baseline (0.8487) 的差距从哪来？

- **评估流程**：add（LOCOMO）→ 每题对两个 user 各做一次 search → 用 `results` 里的 `content` 拼成 prompt → LLM 生成答案 → 和 gold 比（BLEU/F1/LLM judge）。
- **mem1 实际只用了 `results`**：`mem1_search.py` 里是 `items = resp.get("results", [])`，然后 `str_a = "\n".join(m.get("content", "") for m in mem_a)`。**没有用 `formatted_context`**，所以服务端加的 Zep 式「FACTS + 日期 + ENTITIES」对当前分数没有影响。
- **分数主要取决于检索质量**：谁被召回到 `results`、顺序如何，直接决定给 LLM 的上下文。mem0 的 0.8487 来自它那套检索/索引；mem1 用 RRF + keyword + vector，当前约 0.32，差距很大概率来自**检索阶段**（embedding、索引、排序），而不是「有没有图/时序」。
- **当前约束**：Skill 规定现阶段**只做空投优化、不调 RRF/参数**，所以不能动对分数影响最大的检索参数，导致和 baseline 的 gap 会一直存在，直到允许调参或用上能影响检索的设计。

结论：**低分主要因为检索 pipeline 和 mem0 不同，且评估没有用 formatted_context；空投的“图/时序”在当前数据和评估方式下几乎不参与打分。**

---

## 2. 为什么 round 2 / round 3 会“回归”？

- **数据里没有图/时序信息**：`mem1_add.py` 里是 `memory.add(content, user_id=user_id)`，**没有传 metadata**。所以：
  - 没有 `related_ids` → `expand_with_related` 永远不会多拉任何一条 related，**round 2 的「按 created_at 排序 related」等于没动结果**；
  - 没有 `valid_at`/`invalid_at` → 时序过滤也不起作用。
- **round 3 的“仅当 results 少时才 expand”**：同样因为从没有 related_ids，expand 本来就是 0 条，**改不改条件都不会改变返回的 results**。
- 所以这两轮在**当前 LOCOMO + 当前 add 方式**下，对 `results` 是**逻辑上的 no-op**。分数从 0.3262 → 0.2876 / 0.2918 更可能是**评估方差**（LLM 采样、题目顺序等），而不是这两处空投改动的真实效应。

结论：**在“无 metadata、无 related_ids”的 LOCOMO 下，round 2/3 的改动没有改变检索结果；“回归”更像是方差，而不是设计错误。**

---

## 3. 要让空投真正影响分数，可以怎么做？

1. **让评估用上 formatted_context**  
   在 `mem1_search.py` 里，若 `resp` 里有 `formatted_context`，就把它（或和现有 `str_a`/`str_b` 组合）塞进 prompt，让「FACTS + 日期 + ENTITIES」真正参与生成和打分。

2. **在 add 阶段注入图/时序**  
   例如在 LOCOMO 的 add 里根据对话结构给部分 memory 填 `related_ids` 或 `valid_at`/`invalid_at`（需要设计规则或小模型），这样 expand、排序、时序过滤才会生效，空投优化才有机会在指标上体现。

3. **接受方差，多做几轮取平均**  
   若暂时不改 eval 和 add，同一配置多跑几轮（例如 3～5 轮）取平均 llm_score，再判断“是否回归”会更稳。

---

## 4. 总结表

| 现象 | 主要原因 |
|------|----------|
| mem1 分数远低于 baseline (0.32 vs 0.85) | 检索 pipeline 不同；eval 没用 formatted_context；且当前禁止调 RRF/参数 |
| round 2「排序 related」“回归” | LOCOMO 无 related_ids，该逻辑是 no-op；更可能是评估方差 |
| round 3「仅少结果时 expand」“回归” | 同上，expand 本就为 0，改动仍是 no-op；分数变化更像方差 |

（本文件仅作分析记录，不改变代码行为。）

---

## 5. 直接参考 mem0 的参数（对齐情况）

mem0 官方 LOCOMO 评估（evaluation/run_experiments.py + src/memzero/search.py）中**可确定的参数**如下：

| 参数 | mem0 取值 | mem1 当前 | 说明 |
|------|-----------|-----------|------|
| **top_k / limit** | 30（run_experiments 默认，传给 MemorySearch） | 30（mem1_search.py `top_k=30`，请求时 `limit=30`） | **已对齐** |
| 检索方式 | 平台 API：vector search + 可选 reranker | RRF 融合 keyword + vector（RRF_K=60, RRF_KEYWORD_WEIGHT=1.2） | 架构不同，mem0 无 RRF 可对照 |
| filter_memories / is_graph | 评估里可配置 | 无对应开关 | 可选功能 |

结论：

- **已对齐**：评估时每条 search 的 `limit`/top_k 均为 30，与 mem0 一致。
- **无法直接照抄**：mem0 使用 vector +（可选）reranker，不公开 RRF 常数；mem1 使用 RRF 融合 keyword 与 vector，没有「mem0 的 RRF 参数」可抄。
- 若希望检索行为更接近 mem0，可考虑的**一次性**尝试（需你明确允许调参时再做）：
  - **仅用 vector 分支**（关闭 keyword），看是否更接近 mem0 的纯向量检索；
  - 或保持 hybrid，将 `RRF_KEYWORD_WEIGHT` 改为 1.0，使两路权重相等（mem0 未强调 keyword）。
