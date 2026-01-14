# Vixy development commands

# Default: show available commands
default:
    @just --list

# =============================================================================
# Development
# =============================================================================

# Format code
fmt:
    cargo fmt

# Check formatting without modifying
fmt-check:
    cargo fmt --check

# Run clippy lints
clippy:
    cargo clippy -- -D warnings

# Run unit tests
test:
    cargo test

# Run BDD tests (cucumber, excludes @integration)
test-bdd:
    cargo test --test cucumber

# Run all unit tests (lib + BDD)
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
    @echo "==> Running tests..."
    cargo test
    @echo "==> All CI checks passed!"

# Build the project
build:
    cargo build

# Build release version
build-release:
    cargo build --release

# Run the proxy with config file
run config="config.example.toml":
    cargo run -- --config {{config}}

# Clean build artifacts
clean:
    cargo clean

# =============================================================================
# Kurtosis Integration Tests
# =============================================================================

# Start Kurtosis Ethereum testnet and generate config
kurtosis-up:
    @echo "Starting Kurtosis Ethereum testnet..."
    ./scripts/setup-kurtosis.sh

# Stop and remove Kurtosis testnet
kurtosis-down:
    @echo "Stopping Kurtosis testnet..."
    kurtosis enclave rm -f vixy-testnet || true

# Show Kurtosis enclave status
kurtosis-status:
    kurtosis enclave inspect vixy-testnet

# Regenerate Vixy config from existing Kurtosis enclave
kurtosis-config:
    ./scripts/setup-kurtosis.sh

# Run Vixy with Kurtosis-generated config
kurtosis-vixy:
    @if [ ! -f kurtosis/vixy-kurtosis.toml ]; then \
        echo "Error: kurtosis/vixy-kurtosis.toml not found."; \
        echo "Run 'just kurtosis-up' first to start Kurtosis and generate config."; \
        exit 1; \
    fi
    RUST_LOG=info cargo run -- --config kurtosis/vixy-kurtosis.toml

# Run integration tests (requires Vixy to be running)
kurtosis-test:
    @echo "Running integration tests..."
    @echo "Make sure Vixy is running with 'just kurtosis-vixy' in another terminal"
    cargo test --test integration_cucumber

# Full integration test: start Kurtosis, run Vixy, test, cleanup
integration-test: build-release
    #!/usr/bin/env bash
    set -e

    echo "==> Setting up Kurtosis testnet..."
    ./scripts/setup-kurtosis.sh

    echo "==> Starting Vixy..."
    RUST_LOG=info ./target/release/vixy --config kurtosis/vixy-kurtosis.toml &
    VIXY_PID=$!

    # Wait for Vixy to start
    echo "==> Waiting for Vixy to start..."
    for i in {1..30}; do
        if curl -s http://127.0.0.1:8080/status > /dev/null 2>&1; then
            echo "==> Vixy is ready!"
            break
        fi
        if [ $i -eq 30 ]; then
            echo "Error: Vixy failed to start"
            kill $VIXY_PID 2>/dev/null || true
            exit 1
        fi
        sleep 1
    done

    echo "==> Running integration tests..."
    VIXY_SKIP_INTEGRATION_CHECK=1 cargo test --test integration_cucumber -- --color always
    TEST_RESULT=$?

    echo "==> Stopping Vixy..."
    kill $VIXY_PID 2>/dev/null || true

    if [ $TEST_RESULT -eq 0 ]; then
        echo "==> Integration tests passed!"
    else
        echo "==> Integration tests failed!"
    fi

    exit $TEST_RESULT

# Clean up everything including Kurtosis
clean-all: kurtosis-down clean
    rm -f kurtosis/vixy-kurtosis.toml

# =============================================================================
# Utility Commands
# =============================================================================

# Check Vixy status (requires running instance)
status:
    @curl -s http://127.0.0.1:8080/status | jq . || echo "Vixy not running"

# Check Vixy metrics (requires running instance)
metrics:
    @curl -s http://127.0.0.1:9090/metrics || echo "Metrics not available"

# Quick EL proxy test
test-el:
    @curl -s -X POST http://127.0.0.1:8080/el \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' | jq .

# Quick CL proxy test
test-cl:
    @curl -s http://127.0.0.1:8080/cl/eth/v1/node/health && echo "CL health: OK"
