---
description: Run mem1-server auto-optimization loop until metrics match mem0 baseline (build, start, eval, collect, compare, modify code, stop; repeat). Agent controls server lifecycle.
---

## User input

```text
$ARGUMENTS
```

Consider user input if not empty (e.g. max rounds, custom baseline path).

## Instructions

1. **Read and follow** the mem1 auto-optimize Skill: [.cursor/skills/mem1-auto-optimize/SKILL.md](.cursor/skills/mem1-auto-optimize/SKILL.md). Execute the full workflow: build → start → eval → collect → compare; if not met, **commit** (restore point), **change code**, then **re-eval** → **assess** (revert or strengthen as needed). When max_rounds is large (e.g. 50), use **make medium** per round (first 2 convs, ~300 QAs) so metrics are stable and iterations finish in feasible time; optionally run **make full** once at the end for final score.

2. **Baseline:** Use `evaluation/baselines/mem0_locomo.json` as the target. Pass criterion (initial version): current run’s **overall.llm_score** ≥ baseline **overall.llm_score**.

3. **Optional from $ARGUMENTS:** If the user provides a number, treat it as max rounds (e.g. `5` = stop after 5 rounds even if target not met). If the user provides a path, use it as the baseline file instead of the default.

4. Do not assume mem1-server is already running. Build, start, and stop the server yourself each round as specified in the Skill.

5. **Autonomous execution:** Do not ask the user for decisions. Fix blocking issues by editing code or config. When metrics do not improve after a change, decide yourself: revert (direction wrong) or keep and strengthen (insufficient). The user only receives the final result (target met or report after max rounds).
