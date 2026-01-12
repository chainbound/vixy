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
| 1 | Project Setup | Not Started |
| 2 | BDD Infrastructure | Not Started |
| 3 | Configuration | Not Started |
| 4 | State Management | Not Started |
| 5 | EL Health Check | Not Started |
| 6 | CL Health Check | Not Started |
| 7 | Health Monitor | Not Started |
| 8 | Proxy Server | Not Started |
| 9 | Main Entry Point | Not Started |
| 10 | Metrics | Not Started |
| 11 | Final Verification | Not Started |
| 12 | Enhancements (Optional) | Not Started |
| 13 | Write the Story | Not Started |

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

See [AGENT.md](./AGENT.md) for detailed implementation plan and architecture.
