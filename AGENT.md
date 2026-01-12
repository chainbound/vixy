# Vixy
Vibing the Ethereum EL and CL Proxy

A Rust proxy that monitors Ethereum Execution Layer (EL) and Consensus Layer (CL) nodes, tracks their health, and routes requests to healthy nodes.

## Architecture Overview

```
                    +-------------------+
                    |   TOML Config     |
                    | (nodes, settings) |
                    +--------+----------+
                             |
              +--------------+--------------+
              |                             |
     +--------v--------+          +--------v--------+
     |  EL Health Mon  |          |  CL Health Mon  |
     | eth_getBlockNum |          | /eth/v1/node/   |
     | track chain head|          | health + headers|
     +--------+--------+          +--------+--------+
              |                             |
              v                             v
     +--------+--------+          +--------+--------+
     |  EL Node Pool   |          |  CL Node Pool   |
     | healthy/lagging |          | healthy/lagging |
     | rate-limited    |          |                 |
     +--------+--------+          +--------+--------+
              |                             |
              +--------------+--------------+
                             |
                    +--------v--------+
                    |  Proxy Server   |
                    | EL HTTP: /el/*  |
                    | EL WS:   /el/ws |
                    | CL HTTP: /cl/*  |
                    +-----------------+
```

## Configuration

Example `config.toml`:
```toml
[global]
max_el_lag_blocks = 5
max_cl_lag_slots = 3
health_check_interval_ms = 1000

# Primary EL nodes - used first
[[el.primary]]
name = "geth-1"
http_url = "http://localhost:8545"
ws_url = "ws://localhost:8546"
max_consecutive = 150
max_per_second = 100

[[el.primary]]
name = "geth-2"
http_url = "http://localhost:8547"
ws_url = "ws://localhost:8548"
max_consecutive = 150
max_per_second = 100

# Backup EL nodes - only used when ALL primary nodes are unavailable
[[el.backup]]
name = "alchemy-1"
http_url = "https://eth-mainnet.g.alchemy.com/v2/xxx"
ws_url = "wss://eth-mainnet.g.alchemy.com/v2/xxx"
max_consecutive = 100
max_per_second = 25

[[el.backup]]
name = "infura-1"
http_url = "https://mainnet.infura.io/v3/xxx"
ws_url = "wss://mainnet.infura.io/ws/v3/xxx"
max_consecutive = 100
max_per_second = 25

[[cl]]
name = "lighthouse-1"
url = "http://localhost:5052"

[[cl]]
name = "prysm-1"
url = "http://localhost:5053"
```

## Health Check Logic

### EL (Execution Layer)
1. Call `eth_getBlockNumber` via JSON-RPC → returns block number in hex (e.g., `"0x10d4f"`)
2. Parse hex to u64
3. Track the highest block number across all EL nodes = "chain head"
4. Calculate lag for each node: `chain_head - node_block_number`
5. Node is **unhealthy** if `lag > MAX_EL_LAG_BLOCKS`

### EL Primary/Backup Failover
EL nodes are split into two lists:
- **Primary**: Preferred nodes, used under normal operation
- **Backup**: Fallback nodes, only used when ALL primary nodes are unavailable

Failover logic:
1. Try to select from primary nodes first
2. If NO primary node is available (all unhealthy or rate-limited), use backup nodes
3. Health monitoring runs on BOTH primary and backup nodes continuously
4. When a primary node becomes available again, switch back to primary

### EL Rate Limiting
Each EL node (both primary and backup) has rate limiting to prevent overloading:
- `max_consecutive`: Max times to use this node in a row before rotating to another
- `max_per_second`: Max queries per second (QPS) allowed

A node is **unavailable** for selection if:
- `consecutive_count >= max_consecutive` (must rotate to another node)
- `requests_this_second >= max_per_second` (rate limit hit)

When a different node is selected, reset `consecutive_count` for all nodes.
Reset `requests_this_second` every second using a background timer or sliding window.

### CL (Consensus Layer)
1. Call `GET /eth/v1/node/health` → must return HTTP 200
2. Call `GET /eth/v1/beacon/headers/head` → extract slot from `/data/header/message/slot`
3. Track the highest slot across all CL nodes = "chain head"
4. Calculate lag for each node: `chain_head_slot - node_slot`
5. Node is **unhealthy** if health != 200 OR `lag > MAX_CL_LAG_SLOTS`

---

## TODO

> **TDD Workflow**: Tests are written FIRST, then implementation makes them pass.
> ```
> Phase 1: Setup → Phase 2: Write Tests (RED) → Phase 3-10: Implement (GREEN) → Refactor
> ```

> **IMPORTANT**: After completing each phase, update the Progress table in `README.md` to reflect the current status (Not Started → In Progress → Completed).

> **GIT COMMITS**: Commit often with verbose, descriptive messages. Each logical change should be its own commit. Examples:
> - `feat(config): add TOML config parsing with Global and ElNode structs`
> - `test(el_health): add unit tests for hex block number parsing`
> - `feat(health/el): implement eth_getBlockNumber health check`
> - `fix(proxy): handle rate limit edge case when all nodes exhausted`

### Phase 1: Project Setup
- [ ] Fix Cargo.toml edition from "2024" to "2021"
- [ ] Add dependencies to Cargo.toml:
  - `tokio` (async runtime with full features)
  - `axum` (HTTP server for proxy)
  - `reqwest` (HTTP client for health checks)
  - `serde` + `serde_json` (JSON parsing)
  - `toml` (config parsing)
  - `tracing` + `tracing-subscriber` (logging)
  - `eyre` or `thiserror` (error handling)
  - `tokio-tungstenite` (WebSocket client for EL WS proxy)
  - `futures-util` (for stream handling with WebSocket)
  - `prometric` (Prometheus metrics - https://github.com/chainbound/prometric)
- [ ] Add dev-dependencies to Cargo.toml:
  - `cucumber` (BDD testing framework)
  - `wiremock` (mock HTTP server for testing)
  - `tokio-test` (async test utilities)
- [ ] Create minimal stub files so tests can compile (but fail):
  - `src/lib.rs` (expose modules)
  - `src/config.rs` (empty structs)
  - `src/state.rs` (empty structs)
  - `src/health/mod.rs`, `el.rs`, `cl.rs` (empty functions)
  - `src/proxy/mod.rs`, `selection.rs`, `http.rs`, `ws.rs` (empty functions)
  - `src/monitor.rs` (empty function)
  - `src/metrics.rs` (empty struct)
- [ ] Create `justfile` for common development commands:
  - `fmt` - format code
  - `fmt-check` - check formatting without modifying
  - `clippy` - run clippy lints
  - `test` - run unit tests (TDD)
  - `test-bdd` - run BDD tests (cucumber)
  - `test-all` - run both TDD and BDD tests
  - `ci` - run full CI checks (fmt-check, clippy, test-all)
- [ ] Create GitHub Actions CI workflow `.github/workflows/ci.yml`:
  - Trigger on push/PR to main branch
  - Jobs: format check, clippy, unit tests (TDD), BDD tests (cucumber), build
  - Use rust caching for faster CI runs
- [ ] Integrate Claude code review in CI `.github/workflows/claude-review.yml`:
  - Trigger on pull requests
  - Use Claude to review code changes and provide feedback
  - Post review comments on the PR

### Phase 2: Setup BDD Infrastructure

> Setup the test infrastructure that will be used throughout development.

- [ ] Setup BDD test infrastructure:
  - [ ] Create `tests/cucumber.rs` as test harness
  - [ ] Create `tests/world.rs` with `VixyWorld` struct:
    ```rust
    #[derive(Debug, Default, World)]
    pub struct VixyWorld {
        pub config: Option<Config>,
        pub el: Vec<ElNodeState>,
        pub cl: Vec<ClNodeState>,
        pub mock_servers: Vec<MockServer>,
        pub selected_node: Option<String>,
        pub last_response: Option<Response>,
        pub last_error: Option<String>,
    }
    ```
  - [ ] Configure `[[test]]` in Cargo.toml for cucumber
  - [ ] Create `tests/steps/mod.rs` for step definitions

**Testing Philosophy:**
```
┌─────────────────────────────────────────────────────────────┐
│                    Testing Pyramid                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│                    ┌───────────┐                            │
│                    │    BDD    │  ← Integration/Acceptance  │
│                    │ (cucumber)│    (few, slow, valuable)   │
│                    └───────────┘                            │
│               ┌─────────────────────┐                       │
│               │        TDD          │  ← Unit Tests         │
│               │   (cargo test)      │    (many, fast, cheap)│
│               └─────────────────────┘                       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Phase 3-11: TDD Implementation Cycles

> Each phase follows the TDD cycle:
> 1. **Write Tests** - Write actual test code (not todo!), create stubs so it compiles
> 2. **RED** - Run tests, verify they fail (no implementation yet)
> 3. **GREEN** - Implement code to make tests pass
> 4. **REFACTOR** - Clean up while keeping tests green
>
> Tests can be improved/modified during implementation as long as they remain robust.

### Phase 3: Configuration

#### 3.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 3.2

- [ ] Write BDD feature `tests/features/config.feature` (scenarios for valid config, missing fields, etc.)
- [ ] Write step definitions in `tests/steps/config_steps.rs`
- [ ] Write unit tests in `src/config.rs` `#[cfg(test)]` module:
  - [ ] `test_parse_valid_config`
  - [ ] `test_parse_config_missing_el_fails`
  - [ ] `test_parse_config_missing_cl_fails`
  - [ ] `test_parse_config_invalid_url_fails`
  - [ ] `test_default_values_applied`
- [ ] Create stub structs/functions so tests compile (use `unimplemented!()`)
- [ ] Run `cargo test config` - verify tests FAIL (RED ✗)

#### 3.2 Implement (GREEN)
- [ ] Implement `src/config.rs`:
  - [ ] `Config` struct with `Global`, `El`, `Vec<Cl>`
  - [ ] `Global` struct with `max_el_lag_blocks`, `max_cl_lag_slots`, `health_check_interval_ms`
  - [ ] `El` struct with `primary: Vec<ElNode>`, `backup: Vec<ElNode>`
  - [ ] `ElNode` struct with `name`, `http_url`, `ws_url`, `max_consecutive`, `max_per_second`
  - [ ] `Cl` struct with `name`, `url`
  - [ ] `Config::load(path)` and `Config::from_str(s)` to parse TOML
- [ ] Run `cargo test config` - should PASS (GREEN ✓)

---

### Phase 4: State Management

#### 4.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 4.2

- [ ] Write unit tests in `src/state.rs` `#[cfg(test)]` module:
  - [ ] `test_el_node_state_from_config`
  - [ ] `test_cl_node_state_from_config`
  - [ ] `test_app_state_initialization`
  - [ ] `test_initial_health_is_false`
- [ ] Create stub structs so tests compile
- [ ] Run `cargo test state` - verify tests FAIL (RED ✗)

#### 4.2 Implement (GREEN)
- [ ] Implement `src/state.rs`:
  - [ ] `ElNodeState` struct (name, urls, block_number, is_healthy, lag, rate limit fields)
  - [ ] `ClNodeState` struct (name, url, slot, health_ok, is_healthy, lag)
  - [ ] `AppState` struct (Arc<RwLock<Vec<...>>> for nodes, AtomicU64 for chain heads, AtomicBool for failover)
- [ ] Run `cargo test state` - should PASS (GREEN ✓)

---

### Phase 5: EL Health Check

#### 5.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 5.2

- [ ] Write BDD feature `tests/features/el_health.feature` (healthy node, lagging node, unreachable node)
- [ ] Write step definitions in `tests/steps/el_health_steps.rs`
- [ ] Write unit tests in `src/health/el.rs` `#[cfg(test)]` module:
  - [ ] `test_parse_hex_block_number` (with and without 0x prefix)
  - [ ] `test_parse_hex_block_number_invalid`
  - [ ] `test_check_el_node_success` (use wiremock)
  - [ ] `test_check_el_node_timeout`
  - [ ] `test_check_el_node_invalid_response`
  - [ ] `test_calculate_el_lag`
  - [ ] `test_el_node_healthy_within_lag`
  - [ ] `test_el_node_unhealthy_exceeds_lag`
  - [ ] `test_update_chain_head_finds_max`
- [ ] Create stub functions so tests compile
- [ ] Run `cargo test el` - verify tests FAIL (RED ✗)

#### 5.2 Implement (GREEN)
- [ ] Implement `src/health/mod.rs` (module definition)
- [ ] Implement `src/health/el.rs`:
  - [ ] `parse_hex_block_number(hex: &str) -> Result<u64>`
  - [ ] `check_el_node(url: &str) -> Result<u64>`
  - [ ] `update_el_chain_head(nodes: &[ElNodeState]) -> u64`
  - [ ] `calculate_el_health(node: &mut ElNodeState, chain_head: u64, max_lag: u64)`
- [ ] Run `cargo test el` - should PASS (GREEN ✓)

---

### Phase 6: CL Health Check

#### 6.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 6.2

- [ ] Write BDD feature `tests/features/cl_health.feature` (healthy, health endpoint fails, lagging)
- [ ] Write step definitions in `tests/steps/cl_health_steps.rs`
- [ ] Write unit tests in `src/health/cl.rs` `#[cfg(test)]` module:
  - [ ] `test_check_cl_health_returns_true_on_200`
  - [ ] `test_check_cl_health_returns_false_on_503`
  - [ ] `test_check_cl_slot_parses_json`
  - [ ] `test_check_cl_slot_invalid_json`
  - [ ] `test_calculate_cl_lag`
  - [ ] `test_cl_node_unhealthy_when_health_fails`
  - [ ] `test_cl_node_unhealthy_when_lagging`
  - [ ] `test_cl_node_healthy_when_both_pass`
- [ ] Create stub functions
- [ ] Run `cargo test cl` - verify tests FAIL (RED ✗)

#### 6.2 Implement (GREEN)
- [ ] Implement `src/health/cl.rs`:
  - [ ] `check_cl_health(url: &str) -> Result<bool>`
  - [ ] `check_cl_slot(url: &str) -> Result<u64>`
  - [ ] `check_cl_node(url: &str) -> Result<(bool, u64)>`
  - [ ] `update_cl_chain_head(nodes: &[ClNodeState]) -> u64`
  - [ ] `calculate_cl_health(node: &mut ClNodeState, chain_head: u64, max_lag: u64)`
- [ ] Run `cargo test cl` - should PASS (GREEN ✓)

---

### Phase 7: Health Monitor

#### 7.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 7.2

- [ ] Write unit tests in `src/monitor.rs` `#[cfg(test)]` module:
  - [ ] `test_monitor_updates_el_node_state`
  - [ ] `test_monitor_updates_cl_node_state`
  - [ ] `test_monitor_calculates_chain_head`
  - [ ] `test_monitor_sets_failover_flag`
  - [ ] `test_monitor_clears_failover_when_primary_recovers`
  - [ ] `test_monitor_runs_at_configured_interval`
- [ ] Create stub functions
- [ ] Run `cargo test monitor` - verify tests FAIL (RED ✗)

#### 7.2 Implement (GREEN)
- [ ] Implement `src/monitor.rs`:
  - [ ] `run_health_monitor(state: AppState)` - async loop checking all nodes
  - [ ] Update chain heads, recalculate health, manage failover flag, log changes
- [ ] Run `cargo test monitor` - should PASS (GREEN ✓)

---

### Phase 8: Proxy Server

#### 8.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 8.2

- [ ] Write BDD features:
  - [ ] `tests/features/el_rate_limit.feature` (max_consecutive, max_per_second, reset)
  - [ ] `tests/features/el_failover.feature` (primary preference, backup failover, recovery)
  - [ ] `tests/features/proxy_http.feature` (forward requests, 503 on no healthy)
  - [ ] `tests/features/proxy_ws.feature` (establish connection, bidirectional, reconnect)
- [ ] Write step definitions in `tests/steps/` (rate_limit, failover, proxy)
- [ ] Write unit tests for `src/proxy/selection.rs`:
  - [ ] `test_select_healthy_node_from_list`
  - [ ] `test_select_skips_unhealthy_nodes`
  - [ ] `test_select_skips_rate_limited_nodes`
  - [ ] `test_select_skips_max_consecutive_nodes`
  - [ ] `test_select_primary_before_backup`
  - [ ] `test_select_backup_when_no_primary_available`
  - [ ] `test_select_returns_none_when_all_unavailable`
  - [ ] `test_consecutive_count_increments`
  - [ ] `test_consecutive_count_resets_on_switch`
  - [ ] `test_qps_counter_increments`
  - [ ] `test_qps_counter_resets_after_second`
- [ ] Write unit tests for `src/proxy/http.rs`:
  - [ ] `test_el_proxy_forwards_request`
  - [ ] `test_el_proxy_returns_503_no_healthy_nodes`
  - [ ] `test_cl_proxy_forwards_get_request`
  - [ ] `test_cl_proxy_preserves_path`
  - [ ] `test_proxy_timeout_returns_504`
- [ ] Write unit tests for `src/proxy/ws.rs`:
  - [ ] `test_ws_upgrade_success`
  - [ ] `test_ws_message_forwarded_upstream`
  - [ ] `test_ws_message_forwarded_downstream`
  - [ ] `test_ws_client_disconnect_closes_upstream`
  - [ ] `test_ws_no_healthy_node_returns_503`
- [ ] Create stub functions
- [ ] Run `cargo test proxy` - verify tests FAIL (RED ✗)

#### 8.2 Implement (GREEN)
- [ ] Implement `src/proxy/mod.rs`
- [ ] Implement `src/proxy/selection.rs` (node selection with rate limiting + failover)
- [ ] Implement `src/proxy/http.rs` (EL and CL HTTP handlers)
- [ ] Implement `src/proxy/ws.rs` (WebSocket upgrade and bidirectional piping)
- [ ] Run `cargo test proxy` - should PASS (GREEN ✓)
- [ ] Run `cargo test --test cucumber` - BDD tests should PASS ✓

---

### Phase 9: Main Entry Point

- [ ] Implement `src/main.rs`:
  - [ ] Parse CLI args for config path
  - [ ] Load config, initialize AppState and metrics
  - [ ] Spawn health monitor, start axum server with routes + `/metrics`
  - [ ] Add graceful shutdown handling
- [ ] Run `cargo build` - should compile ✓

---

### Phase 10: Metrics

#### 10.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 10.2

- [ ] Write unit tests in `src/metrics.rs` `#[cfg(test)]` module:
  - [ ] `test_metrics_initialization`
  - [ ] `test_el_request_counter_increments`
  - [ ] `test_gauge_updates`
- [ ] Create stub struct
- [ ] Run `cargo test metrics` - verify tests FAIL (RED ✗)

#### 10.2 Implement (GREEN)
- [ ] Implement `src/metrics.rs`:
  - [ ] Define `VixyMetrics` struct using `#[metrics(scope = "vixy")]`:
    ```rust
    #[metrics(scope = "vixy")]
    pub struct VixyMetrics {
        // EL metrics
        #[metric(rename = "el_requests_total", labels = ["node", "tier"])]
        el_requests: Counter,  // tier = "primary" | "backup"

        #[metric(rename = "el_request_duration_seconds", labels = ["node", "tier"])]
        el_request_duration: Histogram,

        #[metric(rename = "el_node_block_number", labels = ["node", "tier"])]
        el_block_number: Gauge,

        #[metric(rename = "el_node_lag_blocks", labels = ["node", "tier"])]
        el_lag: Gauge,

        #[metric(rename = "el_node_healthy", labels = ["node", "tier"])]
        el_healthy: Gauge,  // 1 = healthy, 0 = unhealthy

        #[metric(rename = "el_failover_total")]
        el_failovers: Counter,  // times switched to backup

        // CL metrics
        #[metric(rename = "cl_requests_total", labels = ["node"])]
        cl_requests: Counter,

        #[metric(rename = "cl_request_duration_seconds", labels = ["node"])]
        cl_request_duration: Histogram,

        #[metric(rename = "cl_node_slot", labels = ["node"])]
        cl_slot: Gauge,

        #[metric(rename = "cl_node_lag_slots", labels = ["node"])]
        cl_lag: Gauge,

        #[metric(rename = "cl_node_healthy", labels = ["node"])]
        cl_healthy: Gauge,

        // WebSocket metrics
        #[metric(rename = "ws_connections_active")]
        ws_connections: Gauge,

        #[metric(rename = "ws_messages_total", labels = ["direction"])]
        ws_messages: Counter,  // direction = "upstream" | "downstream"
    }
    ```
  - [ ] Create static instance: `static METRICS: LazyLock<VixyMetrics> = ...`
  - [ ] Implement helper functions to record metrics throughout the codebase
  - [ ] Add `/metrics` endpoint using prometric's HTTP exporter
- [ ] Integrate metrics into health monitor:
  - [ ] Update `el_block_number`, `el_lag`, `el_healthy` gauges on each check
  - [ ] Update `cl_slot`, `cl_lag`, `cl_healthy` gauges on each check
  - [ ] Increment `el_failovers` counter on primary→backup switch
- [ ] Integrate metrics into proxy:
  - [ ] Increment `el_requests` / `cl_requests` on each request
  - [ ] Record `el_request_duration` / `cl_request_duration` histograms
  - [ ] Update `ws_connections` gauge on connect/disconnect
  - [ ] Increment `ws_messages` counter on each message
- [ ] Run `cargo test metrics` - metrics tests should now PASS ✓

### Phase 11: Final Verification
- [ ] Run `just ci` (or manually: `cargo fmt --check && cargo clippy -- -D warnings && cargo test && cargo test --test cucumber`)
  - [ ] `cargo fmt --check` - code is formatted
  - [ ] `cargo clippy -- -D warnings` - no warnings
  - [ ] `cargo test` - ALL unit tests should PASS ✓
  - [ ] `cargo test --test cucumber` - ALL BDD tests should PASS ✓
- [ ] Verify GitHub Actions CI passes on push/PR

### Phase 12: Enhancements (Optional)
- [ ] Add `/status` endpoint to view all node health states as JSON
- [ ] Implement round-robin or least-connections load balancing
- [ ] Add retry logic for failed proxy requests (try next healthy node)
- [ ] Add request timeout configuration
- [ ] Add TLS/HTTPS support
- [ ] Add CL WebSocket support (CL events API)

---

## File Structure (Target)

```
src/
├── main.rs           # Entry point
├── config.rs         # TOML config parsing
├── state.rs          # Shared state (EL/CL node states)
├── metrics.rs        # Prometheus metrics using prometric
├── monitor.rs        # Background health check loop
├── health/
│   ├── mod.rs        # Module exports
│   ├── el.rs         # EL health check (eth_getBlockNumber)
│   └── cl.rs         # CL health check (node/health + headers/head)
└── proxy/
    ├── mod.rs        # Module exports
    ├── selection.rs  # Node selection logic (health + rate limiting)
    ├── http.rs       # HTTP proxy for EL and CL
    └── ws.rs         # WebSocket proxy for EL (eth_subscribe support)

tests/
├── cucumber.rs       # BDD test harness entry point
├── world.rs          # Test world state struct
├── features/         # Gherkin feature files
│   ├── config.feature
│   ├── el_health.feature
│   ├── cl_health.feature
│   ├── el_rate_limit.feature
│   ├── el_failover.feature
│   ├── proxy_http.feature
│   └── proxy_ws.feature
└── steps/            # Step definitions
    ├── mod.rs
    ├── config_steps.rs
    ├── el_health_steps.rs
    ├── cl_health_steps.rs
    ├── rate_limit_steps.rs
    ├── failover_steps.rs
    └── proxy_steps.rs
```

## Quick Start (After Implementation)

```bash
# Create config
cp config.example.toml config.toml
# Edit with your node URLs

# Run
cargo run -- --config config.toml

# Test EL HTTP proxy
curl -X POST http://localhost:8080/el \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Test EL WebSocket proxy (using websocat or similar)
websocat ws://localhost:8080/el/ws
# Then send: {"jsonrpc":"2.0","method":"eth_subscribe","params":["newHeads"],"id":1}

# Test CL proxy
curl http://localhost:8080/cl/eth/v1/beacon/headers/head

# Check Prometheus metrics
curl http://localhost:8080/metrics
```

## Development Commands

```bash
# Using just (recommended)
just              # Show all available commands
just fmt          # Format code
just clippy       # Run lints
just test         # Run unit tests (TDD)
just test-bdd     # Run BDD tests (cucumber)
just ci           # Run full CI checks

# Using cargo directly
cargo test                    # Unit tests (TDD)
cargo test --test cucumber    # BDD tests
cargo fmt --check             # Check formatting
cargo clippy -- -D warnings   # Run lints
```
