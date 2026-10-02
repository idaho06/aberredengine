# Aberred Engine task runner. `just` (no args) runs the full gate.

# just defaults to `sh` on every platform; use PowerShell on Windows so no Git Bash is needed.
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

default: check

# Full gate: rustfmt, every test feature combination, clippy (incl. tracy), guide snippets, then doc links; stops on first failure.
check: && check-guide doc-links
    cargo fmt --all --check
    cargo test
    cargo test --features test-support
    cargo test --no-default-features --features test-support
    cargo clippy --workspace --all-targets
    cargo clippy --workspace --all-targets --no-default-features
    cargo clippy --workspace --lib --bins --features tracy  # no test code is tracy-gated

# Every rustdoc warning is fatal, in both feature configs: intra-doc links must
# resolve (links into `aberred-lua` break only when it isn't compiled), must not
# point at private items, and must not carry redundant explicit targets. The
# exported parameter sets RUSTDOCFLAGS for this recipe on every platform's shell.
doc-links $RUSTDOCFLAGS="-D warnings":
    cargo doc --workspace --no-deps
    cargo doc --workspace --no-deps --no-default-features

# Compiles every RUST-GAME-GUIDE.md snippet (guide-check/, outside the workspace) as a
# downstream game, without and with Lua. It reuses the workspace lockfile and target dir.
# Clippy findings there are guide issues and don't fail the gate.
check-guide:
    cargo fetch
    cp Cargo.lock guide-check/Cargo.lock
    cargo check --manifest-path guide-check/Cargo.toml --target-dir target
    cargo check --manifest-path guide-check/Cargo.toml --target-dir target --features lua
    cargo clippy --manifest-path guide-check/Cargo.toml --target-dir target
    cargo clippy --manifest-path guide-check/Cargo.toml --target-dir target --features lua

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
