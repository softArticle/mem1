---
description: Run mem1-server auto-optimization loop until metrics match mem0 baseline (build, start, eval, collect, compare, modify code, stop; repeat). Agent controls server lifecycle.
---

## User input

```text
$ARGUMENTS
```

Consider user input if not empty (e.g. max rounds, custom baseline path).

## Instructions

1. **Read and follow** the mem1 auto-optimize Skill: [.cursor/skills/mem1-auto-optimize/SKILL.md](.cursor/skills/mem1-auto-optimize/SKILL.md). **If the user input contains a 新方案引用** (GitHub repo URL or paper URL/path): first run **Step 0. Airdrop** from the Skill (fetch reference, analyze design, introduce into mem1-server only, commit), then execute the full workflow below. Otherwise, start directly at build.

2. Execute the full workflow: build → start → eval → collect → compare; if not met, **commit** (restore point), **change code**, then **re-eval** → **assess** (revert or strengthen as needed). When max_rounds is large (e.g. 50), use **make medium** per round (first 2 convs, ~300 QAs) so metrics are stable and iterations finish in feasible time; optionally run **make full** once at the end for final score.

3. **Baseline:** Use `evaluation/baselines/mem0_locomo.json` as the target. Pass criterion (initial version): current run’s **overall.llm_score** ≥ baseline **overall.llm_score**.

4. **Optional from $ARGUMENTS:** Parse user input for: (a) **max rounds** — a number (e.g. `5`, `20 轮`) means stop after that many rounds even if target not met; (b) **baseline path** — if the user provides a file path to a baseline JSON, use it instead of the default; (c) **新方案引用** — a GitHub repo URL (e.g. `https://github.com/org/repo` or `org/repo`), paper URL (e.g. arxiv, PDF link), or local path (e.g. `./papers/xxx.pdf`). When present, the reference triggers **Step 0. Airdrop** before the loop; the number (if any) is max_rounds. User can supply both, e.g. `20 轮 https://github.com/mem0ai/mem0`.

5. Do not assume mem1-server is already running. Build, start, and stop the server yourself each round as specified in the Skill.

6. **Autonomous execution:** Do not ask the user for decisions. Fix blocking issues by editing code or config. When metrics do not improve after a change, decide yourself: revert (direction wrong) or keep and strengthen (insufficient). The user only receives the final result (target met or report after max rounds).
