# Vixy

A Rust proxy that monitors Ethereum Execution Layer (EL) and Consensus Layer (CL) nodes, tracks their health, and routes requests to healthy nodes.

## Features

- Health monitoring for EL nodes (via `eth_getBlockNumber`)
- Health monitoring for CL nodes (via `/eth/v1/node/health` and `/eth/v1/beacon/headers/head`)
- Automatic failover from primary to backup EL nodes
- Rate limiting per node (max consecutive requests, max QPS)
- HTTP proxy for both EL and CL requests
- WebSocket proxy for EL subscriptions
- Prometheus metrics endpoint

## Progress

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Project Setup | Completed |
| 2 | BDD Infrastructure | Completed |
| 3 | Configuration | Completed |
| 4 | State Management | Completed |
| 5 | EL Health Check | Completed |
| 6 | CL Health Check | Completed |
| 7 | Health Monitor | Completed |
| 8 | Proxy Server | Completed |
| 9 | Main Entry Point | Completed |
| 10 | Metrics | Completed |
| 11 | Final Verification | Completed |
| 12 | Enhancements (Optional) | Completed |
| 13 | Write the Story | Completed |

## Quick Start

```bash
# Create config
cp config.example.toml config.toml
# Edit with your node URLs

# Run
cargo run -- --config config.toml
```

## Development

```bash
# Using just (recommended)
just              # Show all available commands
just fmt          # Format code
just clippy       # Run lints
just test         # Run unit tests (TDD)
just test-bdd     # Run BDD tests (cucumber)
just ci           # Run full CI checks
```

## Integration Testing with Kurtosis

Vixy includes comprehensive integration tests that run against a real Ethereum testnet using [Kurtosis](https://docs.kurtosis.com/).

### Prerequisites

- [Kurtosis CLI](https://docs.kurtosis.com/install/) installed
- Docker running

### Running Integration Tests

```bash
# Run all integration tests (starts Kurtosis, runs tests, stops Vixy)
just integration-test

# Or step by step:
just kurtosis-up      # Start 4-node Ethereum testnet
just kurtosis-vixy    # Start Vixy with auto-detected config
just kurtosis-test    # Run integration tests
just kurtosis-down    # Stop testnet
```

### Test Coverage (15 scenarios)

**CL Proxy:**
- Forwards node health requests
- Forwards beacon headers requests
- Forwards node syncing requests
- Fails over when primary CL node is down

**EL Proxy:**
- Forwards eth_blockNumber requests
- Forwards eth_chainId requests
- Handles batch requests
- Fails over when primary node is down
- Uses backup when ALL primary nodes are down
- WebSocket proxy connects and forwards messages

**Health Monitoring:**
- Status endpoint returns all node states
- Health monitor detects node going down
- Health monitor detects node recovering
- Health monitor calculates correct lag
- Prometheus metrics are exposed

### Testnet Configuration

The testnet runs with:
- 4 EL nodes (geth): el-1, el-2 as primary; el-3, el-4 as backup
- 4 CL nodes (lighthouse): cl-1, cl-2, cl-3, cl-4
- Minimal preset with 2-second slot times

See [AGENT.md](./AGENT.md) for detailed implementation plan and architecture.
