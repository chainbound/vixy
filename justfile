# Vixy development commands

# Default: show available commands
default:
    @just --list

# Format code
fmt:
    cargo fmt

# Check formatting without modifying
fmt-check:
    cargo fmt --check

# Run clippy lints
clippy:
    cargo clippy -- -D warnings

# Run unit tests (TDD)
test:
    cargo test

# Run BDD tests (cucumber)
test-bdd:
    cargo test --test cucumber

# Run all tests (TDD + BDD)
test-all:
    cargo test
    cargo test --test cucumber

# Run full CI checks (fmt, clippy, tests)
ci:
    @echo "Running CI checks..."
    @echo "==> Checking formatting..."
    cargo fmt --check
    @echo "==> Running clippy..."
    cargo clippy -- -D warnings
    @echo "==> Running unit tests..."
    cargo test
    @echo "==> Running BDD tests..."
    cargo test --test cucumber
    @echo "==> All CI checks passed!"

# Build the project
build:
    cargo build

# Build release version
build-release:
    cargo build --release

# Run the proxy
run config="config.toml":
    cargo run -- --config {{config}}

# Clean build artifacts
clean:
    cargo clean
