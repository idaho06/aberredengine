# Aberred Engine task runner. `just` (no args) runs the full gate.

default: check

# Full gate: every test feature combination plus clippy, stops on first failure.
check:
    cargo test
    cargo test --features test-support
    cargo test --no-default-features --features test-support
    cargo clippy --workspace --all-targets
    cargo clippy --workspace --all-targets --no-default-features

# Inner TDD loop: lua-on tests including the headless TestWorld harness.
test-fast:
    cargo test --features test-support

# Line-coverage summary (lua-on run, same command as the 2026-09-30 baseline).
cov:
    cargo llvm-cov --workspace --features test-support --summary-only

# HTML coverage report; prints the path, opens nothing.
cov-html:
    cargo llvm-cov --workspace --features test-support --html
    @echo "report: target/llvm-cov/html/index.html"
