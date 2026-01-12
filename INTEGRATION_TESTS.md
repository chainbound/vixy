# Vixy Integration Tests

This document describes how to run integration tests against real Ethereum infrastructure.

## Overview

Vixy has two levels of BDD tests:

1. **Unit Tests** (`cargo test --test cucumber`) - Fast, mocked tests that don't require external infrastructure
2. **Integration Tests** (`cargo test --test integration_cucumber`) - Tests against real Docker/Kurtosis infrastructure

## Quick Start with Docker Compose

The simplest way to run integration tests:

```bash
# One-liner to run everything
./scripts/run-integration-tests.sh
```

Or manually:

```bash
# 1. Start Docker infrastructure
cd docker && docker-compose up -d

# 2. Start Vixy (in another terminal)
cargo run -- --config docker/vixy-integration.toml

# 3. Run integration tests (in another terminal)
cargo test --test integration_cucumber
```

## Docker Compose Setup

The Docker Compose setup provides:
- 2x Geth nodes in dev mode (instant mining)
- 2x Mock CL nodes (nginx serving Beacon API responses)

### Files

- `docker/docker-compose.yaml` - Docker Compose configuration
- `docker/vixy-integration.toml` - Vixy config for Docker setup
- `docker/cl-mock/nginx.conf` - Mock CL configuration

### Ports

| Service | HTTP Port | WebSocket Port |
|---------|-----------|----------------|
| geth-primary | 8545 | 8546 |
| geth-secondary | 8555 | 8556 |
| cl-mock-primary | 5052 | - |
| cl-mock-secondary | 5053 | - |

### Commands

```bash
# Start infrastructure
cd docker && docker-compose up -d

# Check status
docker-compose ps

# View logs
docker-compose logs -f

# Stop infrastructure
docker-compose down
```

## Kurtosis Setup (Full Ethereum Network)

For more realistic testing with actual consensus, use Kurtosis:

```bash
# 1. Install Kurtosis CLI
brew install kurtosis-tech/tap/kurtosis-cli  # macOS

# 2. Run setup script
./scripts/setup-kurtosis.sh

# 3. Start Vixy with generated config
cargo run -- --config kurtosis/vixy-kurtosis.toml

# 4. Run integration tests
cargo test --test integration_cucumber
```

### Kurtosis Network

The Kurtosis setup creates a full Ethereum testnet with:
- Geth + Lighthouse (primary)
- Geth + Prysm (secondary)
- Nethermind + Teku (tertiary)

Configuration: `kurtosis/network_params.yaml`

### Kurtosis Commands

```bash
# List enclaves
kurtosis enclave ls

# Inspect enclave
kurtosis enclave inspect vixy-testnet

# Get service logs
kurtosis service logs vixy-testnet el-1-geth-lighthouse

# Stop enclave
kurtosis enclave rm vixy-testnet
```

## Integration Test Features

Integration tests are in `tests/features/integration/`:

### EL Proxy Tests (`el_proxy.feature`)
- Proxy forwards eth_blockNumber request
- Proxy forwards eth_chainId request
- Proxy handles batch requests
- Proxy fails over when primary node is down
- WebSocket proxy connects and forwards messages

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

## Running Specific Tests

```bash
# Run all integration tests
cargo test --test integration_cucumber

# Run with specific tags (if supported)
cargo test --test integration_cucumber -- --tags @el

# Run with verbose output
cargo test --test integration_cucumber -- --color always
```

## Troubleshooting

### Vixy not running
```
Integration tests require running infrastructure!
```
Solution: Start Vixy with `cargo run -- --config docker/vixy-integration.toml`

### Docker containers not healthy
```bash
# Check container status
docker ps -a

# View container logs
docker logs vixy-geth-primary
```

### Port conflicts
If default ports are in use, modify `docker/docker-compose.yaml` and `docker/vixy-integration.toml`.

### Kurtosis issues
```bash
# Clean up failed enclave
kurtosis enclave rm -f vixy-testnet

# Check Kurtosis engine
kurtosis engine status
kurtosis engine restart
```

## CI/CD Note

Integration tests are **NOT** run in CI by design. They require:
- Significant resources (multiple containers)
- Time (container startup, block production)
- External dependencies (Docker, optionally Kurtosis)

For CI, use the unit-level BDD tests: `cargo test --test cucumber`
