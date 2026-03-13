#!/usr/bin/env bash
# Run one optimization round: build mem1-server, start, make medium, print overall llm_score, stop server.
# Usage: from repo root, ./evaluation/run_one_round.sh
# Or:    cd evaluation && ../evaluation/run_one_round.sh  (with REPO_ROOT=..)
set -e
REPO_ROOT="${REPO_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$REPO_ROOT"

echo "[round] Building mem1-server..."
(cd mem1-server && cargo build --release -q)

echo "[round] Stopping any existing mem1-server..."
pkill -f mem1-server 2>/dev/null || true
sleep 2

echo "[round] Starting mem1-server..."
(cd mem1-server && ./target/release/mem1-server) &
SERVER_PID=$!
sleep 4
curl -sf -o /dev/null http://127.0.0.1:8080/healthz || { kill $SERVER_PID 2>/dev/null; exit 1; }

echo "[round] Running make medium (this takes ~35-40 min)..."
(cd evaluation && make medium)

echo "[round] Stopping server..."
kill $SERVER_PID 2>/dev/null || true

SCORE=$(cd evaluation && python3 get_llm_score.py)
echo "[round] overall_llm_score=$SCORE"
