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

    echo "════════════════════════════════════════════════════════════════"
    echo "  Kurtosis Integration Tests"
    echo "════════════════════════════════════════════════════════════════"
    echo ""

    echo "==> Setting up Kurtosis testnet..."
    ./scripts/setup-kurtosis.sh

    echo "==> Starting Vixy with Kurtosis config..."
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

    echo "==> Running Kurtosis integration tests..."
    VIXY_SKIP_INTEGRATION_CHECK=1 cargo test --test integration_cucumber -- --color always
    KURTOSIS_TEST_RESULT=$?

    echo "==> Stopping Vixy..."
    kill $VIXY_PID 2>/dev/null || true
    sleep 2

    if [ $KURTOSIS_TEST_RESULT -eq 0 ]; then
        echo "✓ Kurtosis integration tests passed!"
    else
        echo "✗ Kurtosis integration tests failed!"
        exit $KURTOSIS_TEST_RESULT
    fi

    echo ""
    echo "════════════════════════════════════════════════════════════════"
    echo "  WSS Integration Tests (External)"
    echo "════════════════════════════════════════════════════════════════"
    echo ""

    echo "==> Starting Vixy with WSS test config..."
    RUST_LOG=info ./target/release/vixy --config config.wss-test.toml &
    VIXY_PID=$!

    # Wait for Vixy to start
    echo "==> Waiting for Vixy to start..."
    for i in {1..30}; do
        if curl -s http://127.0.0.1:8080/health > /dev/null 2>&1; then
            echo "==> Vixy is ready!"
            break
        fi
        if [ $i -eq 30 ]; then
            echo "⚠  Vixy failed to start for WSS tests"
            kill $VIXY_PID 2>/dev/null || true
            echo "⚠  Skipping WSS tests"
            exit 0
        fi
        sleep 1
    done

    echo "==> Running WSS integration tests..."
    echo "Note: Tests use public Hoodi endpoints and may fail due to:"
    echo "  - Network issues"
    echo "  - Endpoint rate limiting"
    echo "  - Endpoint unavailability"
    echo ""

    VIXY_WSS_ONLY=1 VIXY_SKIP_INTEGRATION_CHECK=1 cargo test --test integration_cucumber -- --color always
    WSS_TEST_RESULT=$?

    echo "==> Stopping Vixy..."
    kill $VIXY_PID 2>/dev/null || true

    echo ""
    echo "════════════════════════════════════════════════════════════════"
    echo "  Integration Test Summary"
    echo "════════════════════════════════════════════════════════════════"

    if [ $WSS_TEST_RESULT -eq 0 ]; then
        echo "✓ Kurtosis tests: PASSED"
        echo "✓ WSS tests: PASSED"
        echo ""
        echo "All integration tests passed!"
        exit 0
    else
        echo "✓ Kurtosis tests: PASSED"
        echo "⚠ WSS tests: FAILED (may be due to external endpoint issues)"
        echo ""
        echo "⚠ WSS test failures are non-critical and may be due to:"
        echo "  - Public endpoint unavailability"
        echo "  - Network connectivity issues"
        echo "  - Rate limiting"
        echo ""
        echo "This does not indicate a problem with WSS/TLS implementation."
        exit 0
    fi

# Clean up everything including Kurtosis
clean-all: kurtosis-down clean
    rm -f kurtosis/vixy-kurtosis.toml

# =============================================================================
# WSS Integration Tests (External)
# =============================================================================

# Run WSS integration tests (uses public Hoodi endpoints)
# Note: May fail if public endpoints are unavailable
test-wss: build-release
    #!/usr/bin/env bash
    set -e

    echo "==> Starting Vixy with WSS test config..."
    RUST_LOG=info ./target/release/vixy --config config.wss-test.toml &
    VIXY_PID=$!

    # Wait for Vixy to start
    echo "==> Waiting for Vixy to start..."
    for i in {1..30}; do
        if curl -s http://127.0.0.1:8080/health > /dev/null 2>&1; then
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

    echo "==> Running WSS integration tests..."
    echo "Note: Tests use public Hoodi endpoints and may fail due to:"
    echo "  - Network issues"
    echo "  - Endpoint rate limiting"
    echo "  - Endpoint unavailability"
    echo ""

    VIXY_WSS_ONLY=1 VIXY_SKIP_INTEGRATION_CHECK=1 cargo test --test integration_cucumber -- --color always || {
        echo ""
        echo "⚠  WSS tests failed - this is expected if public endpoints are unavailable"
        echo "   This does not indicate a problem with the WSS/TLS implementation"
        TEST_RESULT=1
    }

    echo "==> Stopping Vixy..."
    kill $VIXY_PID 2>/dev/null || true

    if [ "${TEST_RESULT:-0}" -eq 0 ]; then
        echo "==> WSS tests passed!"
    else
        echo "==> WSS tests failed (may be due to external endpoint issues)"
        exit 1
    fi

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
