# Tasks: AI-Driven Iterative Development (External Toolchain)

**Input**: Design documents from `specs/001-ai-iter-dev/`  
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: One integration test for a full iteration and analyzer output shape (per plan). No contract tests or TDD required by spec.

**Organization**: Tasks grouped by user story so each story can be implemented and tested independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story (US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- Toolchain lives in **toolchain/** at repo root (or separate repo). Paths below are relative to repo root: `toolchain/`, `toolchain/runner/`, `toolchain/store/`, `toolchain/tests/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create toolchain directory and Python project layout

- [X] T001 Create toolchain directory structure: toolchain/, toolchain/runner/, toolchain/store/, toolchain/tests/ per plan.md
- [X] T002 Initialize Python project: add toolchain/requirements.txt or toolchain/pyproject.toml with minimal deps (e.g. no extra deps for file/store; optional: pyyaml for config)
- [X] T003 [P] Add toolchain/config.example.yaml or .env.example for MEM1_EVAL_DIR, data dir, retention (default 30 runs / 7 days)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Run entity, file-based store, code-version capture, and config loading so all user stories can use them

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T004 Implement Run and Metrics data structures (id, timestamp, code_version, outcome, failure_step, failure_detail, metrics_ref, log_ref; run_id, overall, by_category, raw_path) in toolchain/store/models.py per data-model.md
- [X] T005 Implement file-based run store: save/load run and metrics as JSON under data dir (e.g. runs/<run_id>/run.json, runs/<run_id>/metrics.json) in toolchain/store/run_store.py
- [X] T006 Implement code version capture: run git rev-parse HEAD and optional git describe from repo root, return code_version and code_version_display in toolchain/runner/code_version.py
- [X] T007 Implement config loading: resolve eval dir (MEM1_EVAL_DIR or default), data dir, retention (default last 30 runs or 7 days) in toolchain/config.py
- [X] T008 Implement retention enforcement: on store write or startup, delete or archive runs beyond configured retention in toolchain/store/run_store.py

**Checkpoint**: Foundation ready – run store, code version, and config available for US1/US2/US3

---

## Phase 3: User Story 1 - Run One Full Iteration and Use Results (Priority: P1) 🎯 MVP

**Goal**: Single CLI command runs eval → collect → analyze and produces run record + suggestions (analyzer may stub or minimal rules).

**Independent Test**: Run `iterate` once; verify a run record exists with code_version and outcome; verify a suggestions file or stdout JSON exists (even if empty or stub).

### Implementation for User Story 1

- [X] T009 [US1] Implement eval runner: subprocess to run make full (or add+search+evals+scores) in evaluation dir, capture stdout/stderr and exit code in toolchain/runner/run_eval.py
- [X] T010 [US1] Implement collect: parse evaluation_metrics.json (and optionally results/mem1_results.json) from evaluation dir, build Metrics payload, save run + metrics to store with run_id and code_version in toolchain/runner/collect.py
- [X] T011 [US1] Implement analyzer stub or minimal rules: input run_id and metrics, output JSON per contracts/analyzer-output.md (suggestions array with id, type, summary; can be empty or one generic suggestion) in toolchain/runner/analyze.py
- [X] T012 [US1] Implement single CLI entry: argparse or click with `iterate` (default), call code_version → run_eval → collect → analyze in order; on eval failure record run with outcome failure and failure_step/failure_detail; write suggestions to stdout or toolchain_data/<run_id>/suggestions.json in toolchain/cli.py
- [X] T013 [US1] Add run outcome logging: log run id, outcome, failure_step if failure, and path to suggestions on success to stderr or log file

**Checkpoint**: User Story 1 complete – one command runs full iteration and produces run + suggestions

---

## Phase 4: User Story 2 - Structured Log and Metric Collection (Priority: P2)

**Goal**: Runs are persisted with metrics; user can list recent runs and compare last two successful runs.

**Independent Test**: Run iterate twice (e.g. two commits); run list-runs and compare; verify two runs visible and compare shows metric diff.

### Implementation for User Story 2

- [X] T014 [US2] Implement list-runs command: read runs from store, sort by timestamp desc, output id, timestamp, code_version, outcome, summary metrics (e.g. overall.llm_score) in toolchain/cli.py
- [X] T015 [US2] Implement compare command: load last two successful runs and their metrics, output diff of overall (and optionally by_category) in toolchain/cli.py or toolchain/runner/compare.py
- [X] T016 [US2] Ensure run record includes metrics_ref and log_ref when outcome is success in toolchain/runner/collect.py

**Checkpoint**: User Stories 1 and 2 – iterate, list-runs, and compare work

---

## Phase 5: User Story 3 - Optimization Suggestions from Analysis (Priority: P3)

**Goal**: Automated analyzer (rules-based) produces concrete suggestions from metrics (e.g. weak categories, low scores, failure patterns).

**Independent Test**: Feed a run with known low category scores; run analyzer; verify at least one suggestion with type weak_category or similar and matching summary.

### Implementation for User Story 3

- [X] T017 [US3] Implement rules-based analyzer: from metrics (overall + by_category), emit suggestions for e.g. weak_category (category with low llm_score or bleu), failure_pattern (many 0-memory results if available from results), regression (compare to previous run if provided) in toolchain/runner/analyze.py
- [X] T018 [US3] Emit analyzer output in exact JSON schema per contracts/analyzer-output.md (run_id, code_version, analyzed_at, suggestions array with id, type, summary, detail, metric_ref, priority) in toolchain/runner/analyze.py
- [X] T019 [US3] Wire analyzer to optional previous run for regression suggestions when two runs exist in toolchain/cli.py or toolchain/runner/analyze.py

**Checkpoint**: All user stories – iterate produces meaningful suggestions; list-runs and compare available

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Test, docs, and retention behavior

- [ ] T020 [P] Add integration test: run one full iteration (mock or real eval dir), assert run record exists with code_version and outcome, assert suggestions JSON has run_id and suggestions array in toolchain/tests/test_one_iteration.py
- [ ] T021 Add README or update quickstart in specs/001-ai-iter-dev/quickstart.md with actual command names and toolchain/ paths
- [ ] T022 Validate quickstart: run iterate from repo root (or documented cwd) and confirm run + suggestions produced per quickstart.md

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies – start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 – BLOCKS all user stories
- **Phase 3 (US1)**: Depends on Phase 2 – MVP
- **Phase 4 (US2)**: Depends on Phase 2; uses run store from Phase 2 and run/collect from Phase 3
- **Phase 5 (US3)**: Depends on Phase 2 and Phase 3 (analyzer consumes metrics from collect)
- **Phase 6 (Polish)**: Depends on Phase 3 at minimum (Phase 4 and 5 optional for basic test)

### User Story Dependencies

- **US1 (P1)**: After Foundational – no dependency on US2/US3
- **US2 (P2)**: After Foundational and US1 (needs store and collect producing run+metrics)
- **US3 (P3)**: After Foundational and US1 (analyzer replaces or extends stub in US1)

### Parallel Opportunities

- T003 can run in parallel with T001/T002
- T020 (test) can run in parallel with T021/T022
- Within Phase 2: T006 and T007 can run in parallel with each other (different files)

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup  
2. Complete Phase 2: Foundational  
3. Complete Phase 3: User Story 1 (iterate one command, stub analyzer)  
4. **STOP and VALIDATE**: Run iterate once; check run record and suggestions output  
5. Optionally add Phase 6 T020 test

### Incremental Delivery

1. Setup + Foundational → run store and code version ready  
2. Add US1 → iterate works end-to-end (MVP)  
3. Add US2 → list-runs and compare  
4. Add US3 → real rules-based suggestions  
5. Polish → test and quickstart validation  

---

## Notes

- Toolchain is **outside** mem1 product code; implement under `toolchain/` at repo root or in a separate repo.
- [P] tasks use different files and have no ordering dependency.
- Each user story is independently testable per spec.
- Spec does not require TDD; one integration test (T020) satisfies plan's test expectation.
