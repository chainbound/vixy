# Vixy Development Diary

A log of the development journey building Vixy - an Ethereum EL/CL proxy in Rust.

---

## Entry Template

```
### YYYY-MM-DD - Phase X: Title

**What I did:**
- ...

**Challenges faced:**
- ...

**How I solved it:**
- ...

**What I learned:**
- ...

**Mood:** (excited/frustrated/curious/satisfied/etc.)
```

---

## Entries

<!-- Add new entries below this line, newest first -->

### 2026-01-12 - Phase 2: BDD Infrastructure Setup

**What I did:**
- Enhanced tests/cucumber.rs with proper test harness using futures executor
- Created tests/world.rs with VixyWorld struct containing:
  - config: Option<Config> for configuration testing
  - el_nodes/cl_nodes: Vec for node state testing
  - mock_servers: Vec<MockServer> for wiremock integration
  - selected_node, last_response, last_error for step assertions
- Added futures = "0.3" to dev-dependencies (needed for cucumber)
- Verified BDD infrastructure works with `cargo test --test cucumber`

**Challenges faced:**
- Initial cucumber test failed because `futures` crate wasn't in dev-dependencies
- The main dependencies don't automatically get included in test targets

**How I solved it:**
- Added `futures = "0.3"` to dev-dependencies section
- Tests now run successfully (0 features, 0 scenarios - as expected before we add feature files)

**What I learned:**
- Cucumber-rs uses a World struct to maintain state across steps
- The #[derive(World)] macro handles the cucumber integration
- Feature files will be added in Phase 3+ as we implement each component

**Mood:** Satisfied - BDD infrastructure is ready for test-first development!

### 2026-01-12 - Phase 1: Project Setup

**What I did:**
- Fixed Cargo.toml - changed edition from "2024" (invalid) to "2021"
- Added all required dependencies:
  - Runtime/Server: tokio, axum (with WebSocket support)
  - HTTP client: reqwest with rustls-tls
  - Serialization: serde, serde_json, toml
  - Logging: tracing, tracing-subscriber
  - Error handling: thiserror, eyre
  - WebSocket: tokio-tungstenite, futures-util
  - Metrics: prometric
  - CLI: clap
- Added dev-dependencies: cucumber, wiremock, tokio-test
- Created the complete project file structure with stub implementations:
  - src/lib.rs, main.rs, config.rs, state.rs, monitor.rs, metrics.rs
  - src/health/{mod.rs, el.rs, cl.rs}
  - src/proxy/{mod.rs, selection.rs, http.rs, ws.rs}
- Created test infrastructure:
  - tests/cucumber.rs (BDD harness)
  - tests/world.rs (VixyWorld struct)
  - tests/steps/mod.rs
  - tests/features/ directory
- Created justfile with development commands (fmt, clippy, test, test-bdd, ci)
- Created GitHub Actions CI workflow (.github/workflows/ci.yml)
- Created Claude code review workflow (.github/workflows/claude-review.yml)

**Challenges faced:**
- The original Cargo.toml had edition = "2024" which doesn't exist in Rust. Had to change it to "2021".
- Needed to carefully design stub files that expose the right module structure and function signatures without actual implementation.

**How I solved it:**
- Used `unimplemented!()` macros for all stub functions so they compile but will panic if called.
- Carefully structured modules to match the target architecture from AGENT.md.

**What I learned:**
- The project is a comprehensive Ethereum node proxy with health monitoring, failover logic, and metrics.
- TDD workflow means tests first, then implementation - but we need the stubs to compile first.
- Good project structure from the start makes future development smoother.

**Mood:** Excited - the foundation is solid and ready for Phase 2!
