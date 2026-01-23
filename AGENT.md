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

[[el.primary]]
name = "geth-2"
http_url = "http://localhost:8547"
ws_url = "ws://localhost:8548"

# Backup EL nodes - only used when ALL primary nodes are unavailable
[[el.backup]]
name = "alchemy-1"
http_url = "https://eth-mainnet.g.alchemy.com/v2/xxx"
ws_url = "wss://eth-mainnet.g.alchemy.com/v2/xxx"

[[el.backup]]
name = "infura-1"
http_url = "https://mainnet.infura.io/v3/xxx"
ws_url = "wss://mainnet.infura.io/ws/v3/xxx"

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
2. If NO primary node is available (all unhealthy), use backup nodes
3. Health monitoring runs on BOTH primary and backup nodes continuously
4. When a primary node becomes available again, switch back to primary

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
> - `fix(proxy): handle failover edge case when all primary nodes down`

> **DIARY.md**: Create and maintain a `DIARY.md` file as a development log. Update it whenever you:
> - Complete a task or phase (what was done, what was learned)
> - Encounter hardships or blockers (what went wrong, how it was resolved)
> - Make important decisions (why a certain approach was chosen)
> - Discover something interesting or unexpected
>
> This log will be used to create a documentary of the development journey. Write in first person, be honest about struggles, and capture the human (or AI) side of building software.

### Phase 1: Project Setup
- [x] Add dependencies to Cargo.toml:
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
- [x] Add dev-dependencies to Cargo.toml:
  - `cucumber` (BDD testing framework)
  - `wiremock` (mock HTTP server for testing)
  - `tokio-test` (async test utilities)
- [x] Create minimal stub files so tests can compile (but fail):
  - `src/lib.rs` (expose modules)
  - `src/config.rs` (empty structs)
  - `src/state.rs` (empty structs)
  - `src/health/mod.rs`, `el.rs`, `cl.rs` (empty functions)
  - `src/proxy/mod.rs`, `selection.rs`, `http.rs`, `ws.rs` (empty functions)
  - `src/monitor.rs` (empty function)
  - `src/metrics.rs` (empty struct)
- [x] Create `justfile` for common development commands:
  - `fmt` - format code
  - `fmt-check` - check formatting without modifying
  - `clippy` - run clippy lints
  - `test` - run unit tests (TDD)
  - `test-bdd` - run BDD tests (cucumber)
  - `test-all` - run both TDD and BDD tests
  - `ci` - run full CI checks (fmt-check, clippy, test-all)
- [x] Create GitHub Actions CI workflow `.github/workflows/ci.yml`:
  - Trigger on push/PR to main branch
  - Jobs: format check, clippy, unit tests (TDD), BDD tests (cucumber), build
  - Use rust caching for faster CI runs
- [x] Integrate Claude code review in CI `.github/workflows/claude-review.yml`:
  - Trigger on pull requests
  - Use Claude to review code changes and provide feedback
  - Post review comments on the PR

### Phase 2: Setup BDD Infrastructure

> Setup the test infrastructure that will be used throughout development.

- [x] Setup BDD test infrastructure:
  - [x] Create `tests/cucumber.rs` as test harness
  - [x] Create `tests/world.rs` with `VixyWorld` struct:
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
  - [x] Configure `[[test]]` in Cargo.toml for cucumber
  - [x] Create `tests/steps/mod.rs` for step definitions

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

- [x] Write BDD feature `tests/features/config.feature` (scenarios for valid config, missing fields, etc.)
- [x] Write step definitions in `tests/steps/config_steps.rs`
- [x] Write unit tests in `src/config.rs` `#[cfg(test)]` module:
  - [x] `test_parse_valid_config`
  - [x] `test_parse_config_missing_el_fails`
  - [x] `test_parse_config_missing_cl_fails`
  - [x] `test_parse_config_invalid_url_fails`
  - [x] `test_default_values_applied`
- [x] Create stub structs/functions so tests compile (use `unimplemented!()`)
- [x] Run `cargo test config` - verify tests FAIL (RED ✗)

#### 3.2 Implement (GREEN)
- [x] Implement `src/config.rs`:
  - [x] `Config` struct with `Global`, `El`, `Vec<Cl>`
  - [x] `Global` struct with `max_el_lag_blocks`, `max_cl_lag_slots`, `health_check_interval_ms`
  - [x] `El` struct with `primary: Vec<ElNode>`, `backup: Vec<ElNode>`
  - [x] `ElNode` struct with `name`, `http_url`, `ws_url`
  - [x] `Cl` struct with `name`, `url`
  - [x] `Config::load(path)` and `Config::from_str(s)` to parse TOML
- [x] Run `cargo test config` - should PASS (GREEN ✓)

---

### Phase 4: State Management

#### 4.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 4.2

- [x] Write unit tests in `src/state.rs` `#[cfg(test)]` module:
  - [x] `test_el_node_state_from_config`
  - [x] `test_cl_node_state_from_config`
  - [x] `test_app_state_initialization`
  - [x] `test_initial_health_is_false`
- [x] Create stub structs so tests compile
- [x] Run `cargo test state` - verify tests FAIL (RED ✗)

#### 4.2 Implement (GREEN)
- [x] Implement `src/state.rs`:
  - [x] `ElNodeState` struct (name, urls, block_number, is_healthy, lag)
  - [x] `ClNodeState` struct (name, url, slot, health_ok, is_healthy, lag)
  - [x] `AppState` struct (Arc<RwLock<Vec<...>>> for nodes, AtomicU64 for chain heads, AtomicBool for failover)
- [x] Run `cargo test state` - should PASS (GREEN ✓)

---

### Phase 5: EL Health Check

#### 5.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 5.2

- [x] Write BDD feature `tests/features/el_health.feature` (healthy node, lagging node, unreachable node)
- [x] Write step definitions in `tests/steps/el_health_steps.rs`
- [x] Write unit tests in `src/health/el.rs` `#[cfg(test)]` module:
  - [x] `test_parse_hex_block_number` (with and without 0x prefix)
  - [x] `test_parse_hex_block_number_invalid`
  - [x] `test_check_el_node_success` (use wiremock)
  - [x] `test_check_el_node_timeout`
  - [x] `test_check_el_node_invalid_response`
  - [x] `test_calculate_el_lag`
  - [x] `test_el_node_healthy_within_lag`
  - [x] `test_el_node_unhealthy_exceeds_lag`
  - [x] `test_update_chain_head_finds_max`
- [x] Create stub functions so tests compile
- [x] Run `cargo test el` - verify tests FAIL (RED ✗)

#### 5.2 Implement (GREEN)
- [x] Implement `src/health/mod.rs` (module definition)
- [x] Implement `src/health/el.rs`:
  - [x] `parse_hex_block_number(hex: &str) -> Result<u64>`
  - [x] `check_el_node(url: &str) -> Result<u64>`
  - [x] `update_el_chain_head(nodes: &[ElNodeState]) -> u64`
  - [x] `calculate_el_health(node: &mut ElNodeState, chain_head: u64, max_lag: u64)`
- [x] Run `cargo test el` - should PASS (GREEN ✓)

---

### Phase 6: CL Health Check

#### 6.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 6.2

- [x] Write BDD feature `tests/features/cl_health.feature` (healthy, health endpoint fails, lagging)
- [x] Write step definitions in `tests/steps/cl_health_steps.rs`
- [x] Write unit tests in `src/health/cl.rs` `#[cfg(test)]` module:
  - [x] `test_check_cl_health_returns_true_on_200`
  - [x] `test_check_cl_health_returns_false_on_503`
  - [x] `test_check_cl_slot_parses_json`
  - [x] `test_check_cl_slot_invalid_json`
  - [x] `test_calculate_cl_lag`
  - [x] `test_cl_node_unhealthy_when_health_fails`
  - [x] `test_cl_node_unhealthy_when_lagging`
  - [x] `test_cl_node_healthy_when_both_pass`
- [x] Create stub functions
- [x] Run `cargo test cl` - verify tests FAIL (RED ✗)

#### 6.2 Implement (GREEN)
- [x] Implement `src/health/cl.rs`:
  - [x] `check_cl_health(url: &str) -> Result<bool>`
  - [x] `check_cl_slot(url: &str) -> Result<u64>`
  - [x] `check_cl_node(url: &str) -> Result<(bool, u64)>`
  - [x] `update_cl_chain_head(nodes: &[ClNodeState]) -> u64`
  - [x] `calculate_cl_health(node: &mut ClNodeState, chain_head: u64, max_lag: u64)`
- [x] Run `cargo test cl` - should PASS (GREEN ✓)

---

### Phase 7: Health Monitor

#### 7.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 7.2

- [x] Write unit tests in `src/monitor.rs` `#[cfg(test)]` module:
  - [x] `test_monitor_updates_el_node_state`
  - [x] `test_monitor_updates_cl_node_state`
  - [x] `test_monitor_calculates_chain_head`
  - [x] `test_monitor_sets_failover_flag`
  - [x] `test_monitor_clears_failover_when_primary_recovers`
  - [x] `test_monitor_runs_at_configured_interval`
- [x] Create stub functions
- [x] Run `cargo test monitor` - verify tests FAIL (RED ✗)

#### 7.2 Implement (GREEN)
- [x] Implement `src/monitor.rs`:
  - [x] `run_health_monitor(state: AppState)` - async loop checking all nodes
  - [x] Update chain heads, recalculate health, manage failover flag, log changes
- [x] Run `cargo test monitor` - should PASS (GREEN ✓)

---

### Phase 8: Proxy Server

#### 8.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 8.2

- [x] Write BDD features:
  - [x] `tests/features/el_failover.feature` (primary preference, backup failover, recovery)
  - [x] `tests/features/proxy_http.feature` (forward requests, 503 on no healthy)
  - [x] `tests/features/proxy_ws.feature` (establish connection, bidirectional, reconnect)
- [x] Write step definitions in `tests/steps/` (failover, proxy)
- [x] Write unit tests for `src/proxy/selection.rs`:
  - [x] `test_select_healthy_node_from_list`
  - [x] `test_select_skips_unhealthy_nodes`
  - [x] `test_select_primary_before_backup`
  - [x] `test_select_backup_when_no_primary_available`
  - [x] `test_select_returns_none_when_all_unavailable`
- [x] Write unit tests for `src/proxy/http.rs`:
  - [x] `test_el_proxy_forwards_request`
  - [x] `test_el_proxy_returns_503_no_healthy_nodes`
  - [x] `test_cl_proxy_forwards_get_request`
  - [x] `test_cl_proxy_preserves_path`
  - [x] `test_proxy_timeout_returns_504`
- [x] Write unit tests for `src/proxy/ws.rs`:
  - [x] `test_ws_upgrade_success`
  - [x] `test_ws_message_forwarded_upstream`
  - [x] `test_ws_message_forwarded_downstream`
  - [x] `test_ws_client_disconnect_closes_upstream`
  - [x] `test_ws_no_healthy_node_returns_503`
- [x] Create stub functions
- [x] Run `cargo test proxy` - verify tests FAIL (RED ✗)

#### 8.2 Implement (GREEN)
- [x] Implement `src/proxy/mod.rs`
- [x] Implement `src/proxy/selection.rs` (node selection with failover)
- [x] Implement `src/proxy/http.rs` (EL and CL HTTP handlers)
- [x] Implement `src/proxy/ws.rs` (WebSocket upgrade and bidirectional piping)
- [x] Run `cargo test proxy` - should PASS (GREEN ✓)
- [x] Run `cargo test --test cucumber` - BDD tests should PASS ✓

---

### Phase 9: Main Entry Point

- [x] Implement `src/main.rs`:
  - [x] Parse CLI args for config path
  - [x] Load config, initialize AppState and metrics
  - [x] Spawn health monitor, start axum server with routes + `/metrics`
  - [x] Add graceful shutdown handling
- [x] Run `cargo build` - should compile ✓

---

### Phase 10: Metrics

#### 10.1 Write Tests FIRST (RED)
> **IMPORTANT**: Finish writing ALL tests with real assertions before moving to 10.2

- [x] Write unit tests in `src/metrics.rs` `#[cfg(test)]` module:
  - [x] `test_metrics_initialization`
  - [x] `test_el_request_counter_increments`
  - [x] `test_gauge_updates`
- [x] Create stub struct
- [x] Run `cargo test metrics` - verify tests FAIL (RED ✗)

#### 10.2 Implement (GREEN)
- [x] Implement `src/metrics.rs`:
  - [x] Define `VixyMetrics` struct using `#[metrics(scope = "vixy")]`:
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
  - [x] Create static instance: `static METRICS: LazyLock<VixyMetrics> = ...`
  - [x] Implement helper functions to record metrics throughout the codebase
  - [x] Add `/metrics` endpoint using prometric's HTTP exporter
- [x] Integrate metrics into health monitor:
  - [x] Update `el_block_number`, `el_lag`, `el_healthy` gauges on each check
  - [x] Update `cl_slot`, `cl_lag`, `cl_healthy` gauges on each check
  - [x] Increment `el_failovers` counter on primary→backup switch
- [x] Integrate metrics into proxy:
  - [x] Increment `el_requests` / `cl_requests` on each request
  - [x] Record `el_request_duration` / `cl_request_duration` histograms
  - [x] Update `ws_connections` gauge on connect/disconnect
  - [x] Increment `ws_messages` counter on each message
- [x] Run `cargo test metrics` - metrics tests should now PASS ✓

### Phase 11: Final Verification
- [x] Run `just ci` (or manually: `cargo fmt --check && cargo clippy -- -D warnings && cargo test && cargo test --test cucumber`)
  - [x] `cargo fmt --check` - code is formatted
  - [x] `cargo clippy -- -D warnings` - no warnings
  - [x] `cargo test` - ALL unit tests should PASS ✓
  - [x] `cargo test --test cucumber` - ALL BDD tests should PASS ✓
- [x] Verify GitHub Actions CI passes on push/PR

### Phase 12: Enhancements (Optional)
- [x] Add `/status` endpoint to view all node health states as JSON
- [ ] Implement round-robin or least-connections load balancing
- [ ] Add retry logic for failed proxy requests (try next healthy node)
- [x] Add request timeout configuration
- [ ] Add TLS/HTTPS support
- [ ] Add CL WebSocket support (CL events API)

### Phase 13: Write the Story
- [x] Create `BLOG.md` - a blog post telling the story of building Vixy with an AI Agent
  - Use `DIARY.md` as the primary resource for content
  - Highlight the engineering practices applied:
    - TDD (Test-Driven Development) - tests first, then implementation
    - BDD (Behavior-Driven Development) - cucumber scenarios for acceptance tests
    - CI/CD - automated checks on every push/PR
    - Small incremental commits - frequent, focused, well-documented changes
    - Good documentation - AGENT.md as the blueprint, README.md for users
  - Emphasize the speed and precision of AI-assisted development
  - Include specific examples of challenges overcome (from DIARY.md)
  - Reflect on what worked well and what could be improved
  - Make it engaging - this is a story, not just a technical report

### Phase 14: Integration Testing with Kurtosis
- [x] Set up Kurtosis integration test infrastructure:
  - [x] Create `kurtosis/network_params.yaml` - 4-node Ethereum testnet config
  - [x] Create `scripts/setup-kurtosis.sh` - Auto-detects nodes, generates Vixy config
  - [x] Add justfile commands: `kurtosis-up`, `kurtosis-down`, `kurtosis-vixy`, `integration-test`
- [x] Create integration test scenarios (`tests/features/integration/`):
  - [x] `cl_proxy.feature` - CL proxy forwarding and failover (4 scenarios)
  - [x] `el_proxy.feature` - EL proxy forwarding, failover, backup failover, WebSocket (6 scenarios)
  - [x] `health_monitoring.feature` - Status, detection, recovery, lag, metrics (5 scenarios)
- [x] Implement integration step definitions (`tests/steps/integration_steps.rs`):
  - [x] Kurtosis service start/stop helpers
  - [x] HTTP request steps for EL JSON-RPC and CL Beacon API
  - [x] Health polling and status verification
  - [x] Backup failover test (stop ALL primaries, verify backups work)
- [x] Fix bugs found by integration tests:
  - [x] HTTP proxy Content-Type header forwarding
  - [x] Accept 2xx status codes (Lighthouse returns 206 when syncing)

**Test Configuration:**
```yaml
# 4-node testnet: 2 primary + 2 backup EL nodes
participants:
  - el_type: geth
    cl_type: lighthouse
    count: 4

network_params:
  preset: minimal
  seconds_per_slot: 2
```

**Running Integration Tests:**
```bash
just integration-test  # Full cycle: setup, test, cleanup
```

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
    ├── selection.rs  # Node selection logic (health + failover)
    ├── http.rs       # HTTP proxy for EL and CL
    └── ws.rs         # WebSocket proxy for EL (eth_subscribe support)

tests/
├── cucumber.rs       # BDD test harness entry point
├── world.rs          # Test world state struct
├── features/         # Gherkin feature files
│   ├── config.feature
│   ├── el_health.feature
│   ├── cl_health.feature
│   ├── el_failover.feature
│   ├── proxy_http.feature
│   └── proxy_ws.feature
└── steps/            # Step definitions
    ├── mod.rs
    ├── config_steps.rs
    ├── el_health_steps.rs
    ├── cl_health_steps.rs
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

---

## File Organization for AI Sessions

**Important**: When working on new features or fixes that generate significant documentation, organize files properly:

### Session-Specific Files → `agent/<session-name>/`

All documentation, analysis, and artifacts from an AI-assisted development session should go into a dedicated folder:

```
agent/
  └── <session-name>/          # e.g., "websocket-reconnection-fix"
      ├── README.md            # Session overview and summary
      ├── <ANALYSIS>.md        # Root cause analysis, investigation
      ├── <FIX-PLAN>.md        # Implementation plan
      ├── <IMPROVEMENTS>.md    # Testing/design improvements
      └── ...                  # Other session-specific docs
```

### Files That Stay in Root

- **AGENT.md** (this file) - Core development guide
- **DIARY.md** - Ongoing development diary (all sessions)
- **README.md** - Project documentation
- **BLOG.md** - Project blog posts and stories (general, not session-specific)
- **Cargo.toml**, **Justfile**, etc. - Configuration files

### Example Structure

```
vixy/
├── AGENT.md                              # ← Core guide (stays in root)
├── DIARY.md                              # ← Development log (stays in root)
├── README.md                             # ← Project docs (stays in root)
├── BLOG.md                               # ← Blog posts (stays in root)
├── agent/                                # ← Session artifacts folder
│   └── websocket-reconnection-fix/      # ← Example session
│       ├── README.md                    # Session summary
│       ├── WEBSOCKET-RECONNECTION-FIX.md
│       ├── TESTING-IMPROVEMENTS.md
│       └── INTEGRATION_TESTS.md
└── src/                                  # ← Source code
```

### Benefits

1. **Clean Root Directory**: Project essentials remain visible
2. **Organized History**: Each AI session is self-contained
3. **Easy Reference**: Find all artifacts from a specific fix/feature
4. **No Clutter**: Session-specific docs don't pollute the root

### When Starting a New Session

1. Create folder: `agent/<descriptive-session-name>/`
2. Add session README.md explaining the goal
3. Place all analysis, fixes, and documentation in that folder
4. Update DIARY.md with references to the session folder
5. Keep root clean!

---
