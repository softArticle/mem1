#!/usr/bin/env bash
# loop_round.sh — evaluation primitive for the mem1 optimization loop.
#
# Builds mem1-server (release), then runs `make medium` N times against a WIPED
# database each time, capturing the overall LLM-judge score per run. Prints a
# machine-readable summary line for the agent to parse:
#
#     RESULT mean=<m> std=<s> n=<n> scores=<csv> exit=<code>
#
# Usage (from repo root or anywhere):
#     ./evaluation/loop_round.sh [N_RUNS]
# N_RUNS defaults to 1 (single coarse run). Use 3 for variance-controlled confirm.
#
# Env required for LLM answer-gen + judge (OpenAI-compatible gateway):
#     EVAL_LLM_BASE_URL  EVAL_LLM_API_KEY  EVAL_LLM_MODEL
# These may be pre-exported by the caller; defaults below match the verified JD gateway.
set -uo pipefail

N_RUNS="${1:-1}"
REPO_ROOT="${REPO_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$REPO_ROOT"

# --- LLM gateway env (verified working) -------------------------------------
export EVAL_LLM_BASE_URL="${EVAL_LLM_BASE_URL:-http://llm-gw.jd.local/v1}"
export EVAL_LLM_API_KEY="${EVAL_LLM_API_KEY:-aadbdcf5a139423ead71181872a1ea97}"
export EVAL_LLM_MODEL="${EVAL_LLM_MODEL:-GPT-5.5-joybuilder}"

# server runs with cwd = mem1-server/, so default MEM1_DB_PATH lands there.
DB_PATH="${MEM1_DB_PATH:-mem1.db}"
SERVER_DIR="$REPO_ROOT/mem1-server"
SERVER_BIN="$SERVER_DIR/target/release/mem1-server"
HEALTH_URL="http://127.0.0.1:8080/healthz"

log() { echo "[loop_round] $*" >&2; }

fail() {
  log "ERROR: $*"
  pkill -f mem1-server 2>/dev/null || true
  echo "RESULT mean=0.0 std=0.0 n=0 scores= exit=1"
  exit 1
}

# --- Build once (release) ---------------------------------------------------
log "Building mem1-server (release)..."
(cd "$SERVER_DIR" && cargo build --release -q) || fail "cargo build failed"

scores=()
for ((i = 1; i <= N_RUNS; i++)); do
  log "=== run $i / $N_RUNS ==="

  log "Stopping any existing mem1-server..."
  pkill -f mem1-server 2>/dev/null || true
  sleep 2

  # CRITICAL: wipe the persistent RocksDB dir so each run starts clean.
  # Only ever touch the gitignored DB inside the server dir — never git clean.
  log "Wiping DB at $SERVER_DIR/$DB_PATH ..."
  rm -rf "${SERVER_DIR:?}/${DB_PATH:?}"

  log "Starting mem1-server..."
  (cd "$SERVER_DIR" && exec "$SERVER_BIN") &
  SERVER_PID=$!

  # health probe with retries (first run may trigger MiniLM download)
  ok=0
  for _ in $(seq 1 60); do
    if curl -sf -o /dev/null "$HEALTH_URL"; then ok=1; break; fi
    sleep 2
  done
  [ "$ok" = 1 ] || { kill "$SERVER_PID" 2>/dev/null || true; fail "server did not become healthy"; }

  log "Running make medium (add + search + evals + scores)..."
  if ! (cd evaluation && make medium >/dev/null 2>&1); then
    kill "$SERVER_PID" 2>/dev/null || true
    fail "make medium failed on run $i"
  fi

  kill "$SERVER_PID" 2>/dev/null || true
  sleep 1

  s=$(cd evaluation && python3 get_llm_score.py 2>/dev/null)
  if [ -z "$s" ]; then fail "could not read llm_score on run $i"; fi
  log "run $i score=$s"
  scores+=("$s")
done

# --- aggregate (mean/std) via python ----------------------------------------
csv=$(IFS=,; echo "${scores[*]}")
python3 - "$csv" <<'PY'
import sys, statistics
csv = sys.argv[1]
vals = [float(x) for x in csv.split(",") if x.strip()]
mean = statistics.fmean(vals) if vals else 0.0
std = statistics.pstdev(vals) if len(vals) > 1 else 0.0
print(f"RESULT mean={mean:.4f} std={std:.4f} n={len(vals)} scores={csv} exit=0")
PY
