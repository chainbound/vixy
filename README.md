# Vixy

A high-performance Ethereum node proxy built in Rust that monitors node health and automatically routes traffic to healthy endpoints.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.86%2B-orange.svg)](https://www.rust-lang.org/)

---

## Overview

Vixy is a transparent proxy that sits between your application and Ethereum nodes (both Execution Layer and Consensus Layer). It continuously monitors node health, tracks synchronization status, and intelligently routes requests to healthy nodes with automatic failover.

**Key Capabilities:**
- **Health Monitoring**: Continuous health checks for EL and CL nodes
- **Automatic Failover**: Seamless routing to backup nodes when primary nodes fail
- **WebSocket Support**: Proxies WebSocket connections with subscription replay on reconnection
- **Metrics & Observability**: Comprehensive Prometheus metrics and Grafana dashboards
- **HTTP & WebSocket Proxying**: Support for both REST APIs and WebSocket subscriptions

## Quick Start

### Using Docker

```bash
# Pull the latest image
docker pull ghcr.io/chainbound/vixy:latest

# Create a configuration file
cp config.example.toml config.toml
# Edit config.toml with your node URLs

# Run Vixy
docker run -v $(pwd)/config.toml:/app/config.toml \
  -p 8080:8080 -p 9090:9090 \
  ghcr.io/chainbound/vixy:latest
```

### Building from Source

```bash
# Clone the repository
git clone https://github.com/chainbound/vixy.git
cd vixy

# Build and run
cargo run --release -- --config config.toml
```

## Configuration

Create a `config.toml` file based on `config.example.toml`:

```toml
[server]
listen = "127.0.0.1:8080"

[metrics]
enabled = true
listen = "127.0.0.1:9090"

[[el_nodes]]
name = "geth-primary"
url = "http://geth-1:8545"
ws_url = "ws://geth-1:8546"
tier = "primary"

[[cl_nodes]]
name = "lighthouse-1"
url = "http://lighthouse-1:5052"
```

See [config.example.toml](config.example.toml) for all available options.

## API Endpoints

Vixy exposes the following HTTP endpoints:

### Proxy Endpoints

#### Execution Layer (EL)

**POST /el**
- Proxies JSON-RPC requests to healthy EL nodes (uses JSON-RPC protocol, not REST)
- Supports multiple primary nodes (round-robin load balancing)
- Supports multiple backup nodes (used when all primary nodes fail)
- Automatic failover: primary → backup tier on failure
- Supports batch requests
- Content-Type: `application/json`

Example:
```bash
curl -X POST http://localhost:8080/el \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_blockNumber",
    "params": [],
    "id": 1
  }'
```

**GET /el/ws**
- WebSocket proxy for EL subscriptions (HTTP upgrade to WebSocket)
- Connects to healthy EL nodes from primary tier first
- Automatic reconnection with subscription replay on node failure
- Health-aware upstream switching (primary → backup on failure)
- Supports `eth_subscribe` and `eth_unsubscribe`

Example:
```javascript
const ws = new WebSocket('ws://localhost:8080/el/ws');
ws.send(JSON.stringify({
  jsonrpc: '2.0',
  method: 'eth_subscribe',
  params: ['newHeads'],
  id: 1
}));
```

#### Consensus Layer (CL)

**ANY /cl/{path}**
- Proxies all HTTP methods (GET, POST, etc.) to healthy CL nodes (uses REST API)
- Supports multiple CL nodes (round-robin load balancing)
- Forwards all paths under `/cl/` to beacon node API endpoints
- Automatic failover to next healthy node on failure

Example:
```bash
# Get beacon chain head
curl http://localhost:8080/cl/eth/v1/beacon/headers/head

# Check node health
curl http://localhost:8080/cl/eth/v1/node/health

# Get node syncing status
curl http://localhost:8080/cl/eth/v1/node/syncing
```

### Monitoring Endpoints

**GET /health**
- Simple health check for the proxy itself
- Returns: `OK` (200 status)
- Useful for load balancer health checks

Example:
```bash
curl http://localhost:8080/health
```

**GET /status**
- Detailed JSON status of all monitored nodes
- Shows health state, block/slot numbers, and lag
- Content-Type: `application/json`

Example:
```bash
curl http://localhost:8080/status | jq .
```

Response format:
```json
{
  "el_nodes": {
    "geth-primary": {
      "healthy": true,
      "block_number": 12345678,
      "lag": 0,
      "tier": "primary"
    }
  },
  "cl_nodes": {
    "lighthouse-1": {
      "healthy": true,
      "slot": 9876543,
      "lag": 1
    }
  },
  "el_failover_active": false
}
```

**GET /metrics**
- Prometheus metrics endpoint
- Only available if `metrics.enabled = true` in config
- Can be on main port or separate port (see `metrics.port`)

Example:
```bash
curl http://localhost:9090/metrics
```

See [grafana/README.md](grafana/README.md) for full metrics documentation.

## Documentation

### User Guide
- [Configuration Guide](config.example.toml) - Complete configuration reference
- [Monitoring with Grafana](grafana/README.md) - Setup Grafana dashboards
- [Integration Testing](INTEGRATION_TESTS.md) - Running integration tests with Kurtosis

### Architecture
- [Agent Design](AGENT.md) - Implementation details and architecture
- [Development Diary](DIARY.md) - Development log and design decisions
- [Blog Post](BLOG.md) - Deep dive into Vixy's features

### API Documentation
- API docs available at: https://docs.rs/vixy (coming soon)
- Metrics reference: [grafana/README.md](grafana/README.md)

## Development

### Prerequisites
- Rust 1.86 or later (for `let_chains` support)
- Docker (for integration tests)
- [Kurtosis](https://docs.kurtosis.com/install/) (for testnet setup)
- [just](https://github.com/casey/just) (optional, for task automation)

### Development Workflow

```bash
# Format code
cargo fmt

# Run lints
cargo clippy -- -D warnings

# Run unit tests
cargo test

# Run BDD tests
cargo test --test cucumber

# Full CI check
just ci
```

### Integration Testing

Vixy includes comprehensive integration tests using [Kurtosis](https://docs.kurtosis.com/) to spin up a local Ethereum testnet:

```bash
# Setup and run integration tests
just integration-test

# Or step-by-step
just kurtosis-up      # Start testnet
just kurtosis-vixy    # Run Vixy
just kurtosis-test    # Run tests
just kurtosis-down    # Cleanup
```

See [INTEGRATION_TESTS.md](INTEGRATION_TESTS.md) for detailed testing documentation.

## Monitoring

Vixy exposes Prometheus metrics on `/metrics` (default port 9090). A pre-configured Grafana dashboard is available in the [grafana/](grafana/) directory.

**Dashboard Features:**
- EL/CL node health and lag monitoring
- Request rates and latency (P50/P95/P99)
- WebSocket connection tracking
- Failover events and status

See [grafana/README.md](grafana/README.md) for setup instructions.

## Contributing

We welcome contributions! Here's how to get started:

### Reporting Issues

- **Bug Reports**: Use the [issue tracker](https://github.com/chainbound/vixy/issues) with a clear description and reproduction steps
- **Feature Requests**: Open an issue describing the use case and proposed solution
- **Security Issues**: Please report security vulnerabilities privately to security@chainbound.io

### Development Process

1. **Fork the repository** and create a feature branch
   ```bash
   git checkout -b feat/your-feature-name
   ```

2. **Make your changes** following our coding standards:
   - Run `cargo fmt` before committing
   - Ensure `cargo clippy -- -D warnings` passes
   - Add tests for new functionality
   - Update documentation as needed

3. **Test thoroughly**:
   ```bash
   # Run full test suite
   just ci

   # Run integration tests
   just integration-test
   ```

4. **Commit with clear messages**:
   ```bash
   git commit -m "feat: add support for node prioritization"
   ```

   Follow [Conventional Commits](https://www.conventionalcommits.org/) format:
   - `feat:` - New features
   - `fix:` - Bug fixes
   - `docs:` - Documentation changes
   - `refactor:` - Code refactoring
   - `test:` - Test additions/changes
   - `chore:` - Maintenance tasks

5. **Submit a Pull Request**:
   - Provide a clear description of the changes
   - Reference any related issues
   - Ensure CI passes

### Code Review Process

- All submissions require review before merging
- Maintainers will provide feedback within a few days
- Address review comments and update your PR
- Once approved, a maintainer will merge your PR

### Development Tools

We use these tools to maintain code quality:

- **rustfmt**: Code formatting (`cargo fmt`)
- **clippy**: Linting (`cargo clippy`)
- **cucumber**: BDD testing (`cargo test --test cucumber`)
- **just**: Task automation (`just --list`)

Run the full CI suite locally before submitting:
```bash
just ci
```

### Getting Help

- **Discord**: Join our community (coming soon)
- **Discussions**: Use [GitHub Discussions](https://github.com/chainbound/vixy/discussions)
- **Documentation**: Check [AGENT.md](AGENT.md) for architecture details

## Roadmap

Future enhancements we're considering:

- [ ] Dynamic node discovery and registration
- [ ] Advanced load balancing strategies
- [ ] Rate limiting per application/API key
- [ ] gRPC support for CL nodes
- [ ] WebAssembly plugin system
- [ ] Multi-region node distribution

Have ideas? Open an issue or discussion!

## License

This project is licensed under the **MIT License** - see the [LICENSE](LICENSE) file for details.

### Third-Party Licenses

Vixy depends on various open-source libraries. Their licenses can be found in their respective repositories:

- [tokio](https://github.com/tokio-rs/tokio) - MIT License
- [axum](https://github.com/tokio-rs/axum) - MIT License
- [reqwest](https://github.com/seanmonstar/reqwest) - MIT/Apache-2.0
- [prometheus](https://github.com/tikv/rust-prometheus) - Apache-2.0

## Acknowledgments

Built with ❤️ by [Chainbound](https://chainbound.io)

Special thanks to:
- The Ethereum community for the robust infrastructure
- All contributors who have helped improve Vixy
- The Rust community for excellent tooling and libraries

---

**Need Help?** Check our [documentation](AGENT.md) or open an [issue](https://github.com/chainbound/vixy/issues).
