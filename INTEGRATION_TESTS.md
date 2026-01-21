# Vixy Integration Tests

This document describes how to run integration tests against a real Ethereum testnet using Kurtosis.

## Overview

Vixy has two levels of BDD tests:

1. **Unit Tests** (`just test-bdd`) - Fast, mocked tests that don't require external infrastructure
2. **Integration Tests** (`just kurtosis-test`) - Tests against a real Kurtosis Ethereum testnet

## Quick Start

Run the full integration test workflow with a single command:

```bash
just integration-test
```

This will:
1. **Kurtosis Integration Tests:**
   - Start a Kurtosis Ethereum testnet
   - Generate Vixy configuration
   - Build and start Vixy
   - Run Kurtosis integration tests
2. **WSS Integration Tests:**
   - Restart Vixy with public Holesky WSS endpoints
   - Run WSS/TLS connection tests
   - Report results (failures are non-critical)

**Note:** WSS test failures do not fail the overall test suite. They may fail due to external endpoint unavailability, which is expected.

## Prerequisites

### Kurtosis CLI

Install Kurtosis:

```bash
# macOS
brew install kurtosis-tech/tap/kurtosis-cli

# Linux
echo "deb [trusted=yes] https://apt.fury.io/kurtosis-tech/ /" | sudo tee /etc/apt/sources.list.d/kurtosis.list
sudo apt update
sudo apt install kurtosis-cli
```

### Other Dependencies

- Docker (running)
- jq (for JSON parsing)
- just (command runner)

```bash
# macOS
brew install jq just

# Linux
sudo apt install jq
cargo install just
```

## Step-by-Step Usage

### 1. Start Kurtosis Testnet

```bash
just kurtosis-up
```

This starts a Kurtosis enclave with:
- 3 EL nodes: geth+lighthouse, geth+prysm, nethermind+teku
- 3 CL nodes: lighthouse, prysm, teku
- Fast block times (2 second slots)

The script automatically generates `kurtosis/vixy-kurtosis.toml` with the correct endpoints.

### 2. Start Vixy

In a separate terminal:

```bash
just kurtosis-vixy
```

### 3. Run Integration Tests

In another terminal:

```bash
just kurtosis-test
```

### 4. Cleanup

```bash
just kurtosis-down
```

## Justfile Commands

| Command | Description |
|---------|-------------|
| `just integration-test` | Full workflow (Kurtosis tests + WSS tests) |
| `just kurtosis-up` | Start Kurtosis testnet and generate config |
| `just kurtosis-down` | Stop and remove Kurtosis testnet |
| `just kurtosis-status` | Show Kurtosis enclave status |
| `just kurtosis-vixy` | Run Vixy with Kurtosis config |
| `just kurtosis-test` | Run Kurtosis integration tests only |
| `just test-wss` | Run WSS integration tests only |
| `just clean-all` | Clean everything including Kurtosis |

## Utility Commands

Test individual endpoints while Vixy is running:

```bash
# Check Vixy status
just status

# Test EL proxy
just test-el

# Test CL proxy
just test-cl

# View metrics
just metrics
```

## Kurtosis Network Configuration

The network is configured in `kurtosis/network_params.yaml`:

```yaml
participants:
  - el_type: geth
    cl_type: lighthouse
    count: 1
  - el_type: geth
    cl_type: prysm
    count: 1
  - el_type: nethermind
    cl_type: teku
    count: 1

network_params:
  preset: minimal
  seconds_per_slot: 2
```

## Integration Test Features

Tests are in `tests/features/integration/`:

### WSS Connection Tests (`wss_connection.feature`)
**Note:** These tests use public Holesky WSS endpoints and may fail if endpoints are unavailable.

To run WSS tests:
```bash
# 1. Start Vixy with WSS test config
cargo run --release -- --config config.wss-test.toml

# 2. In another terminal, run WSS tests
cargo test --test integration_cucumber -- --tags @wss
```

Tests:
- Vixy starts without TLS panics (verifies crypto provider initialization)
- WebSocket connects through Vixy to WSS upstream
- WebSocket subscription works over WSS

Configuration file: `config.wss-test.toml` (uses public Holesky endpoints)

### EL Proxy Tests (`el_proxy.feature`)
- Proxy forwards eth_blockNumber request
- Proxy forwards eth_chainId request
- Proxy handles batch requests
- Proxy fails over when primary node is down
- WebSocket proxy connects

### CL Proxy Tests (`cl_proxy.feature`)
- Proxy forwards node health request
- Proxy forwards beacon headers request
- Proxy forwards node syncing request
- Proxy fails over when primary CL node is down

### Health Monitoring Tests (`health_monitoring.feature`)
- Status endpoint returns all node states
- Health monitor detects node going down
- Health monitor detects node recovering
- Health monitor calculates correct lag
- Prometheus metrics are exposed

## Troubleshooting

### Kurtosis enclave already exists

```bash
just kurtosis-down
just kurtosis-up
```

### Vixy not starting

Check if the config was generated:
```bash
cat kurtosis/vixy-kurtosis.toml
```

If empty or missing node entries, regenerate:
```bash
just kurtosis-config
```

### Can't connect to nodes

Check Kurtosis enclave status:
```bash
just kurtosis-status
```

### Port conflicts

If port 8080 or 9090 is in use, modify `kurtosis/vixy-kurtosis.toml` after generation.

## CI Note

Integration tests are **NOT** run in CI. They require:
- Significant resources (multiple containers)
- Time (Kurtosis startup, block production)
- External dependencies (Docker, Kurtosis)

For CI, use unit tests: `just ci`
