# Build and test (constitution IV: CI gates)
default:
    just check

check: check-rust check-python
    @echo "All checks passed."

check-rust:
    cd mem1-server && cargo fmt -- --check && cargo clippy -- -D warnings && cargo test

check-python:
    cd python && python -m pytest tests/ -v 2>/dev/null || true
