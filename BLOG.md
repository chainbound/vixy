# Building Vixy: An Ethereum Proxy with AI-Assisted Development

*A story of building production-grade software with TDD, BDD, and an AI pair programmer*

---

## The Challenge

Building infrastructure software for Ethereum is not trivial. Validators, stakers, and application developers all depend on reliable access to both the Execution Layer (EL) and Consensus Layer (CL). When nodes go down or fall behind, requests fail. When requests fail, users suffer.

Vixy was born from a simple need: **route requests to healthy nodes, automatically**.

But this blog isn't just about what Vixy does—it's about *how* it was built. In a single day, using Test-Driven Development (TDD), Behavior-Driven Development (BDD), and an AI assistant (Claude), we went from an empty repository to a fully functional, well-tested Ethereum proxy.

---

## The Blueprint: AGENT.md

Every successful project starts with a plan. Before writing a single line of code, we created `AGENT.md`—a comprehensive specification that served as both documentation and task list.

The plan broke development into 13 phases:

1. **Project Setup** - Dependencies, file structure, CI/CD
2. **BDD Infrastructure** - Cucumber test harness
3. **Configuration** - TOML parsing with validation
4. **State Management** - Thread-safe node state tracking
5. **EL Health Check** - JSON-RPC block number monitoring
6. **CL Health Check** - Beacon API health and slot monitoring
7. **Health Monitor** - Background health checking loop
8. **Proxy Server** - HTTP and WebSocket request forwarding
9. **Main Entry Point** - CLI and server initialization
10. **Metrics** - Prometheus endpoint
11. **Final Verification** - CI validation
12. **Enhancements** - Status endpoint, configuration options
13. **Write the Story** - This blog post

Each phase had clear deliverables, test requirements, and acceptance criteria. The AI could follow this blueprint autonomously, making decisions within defined boundaries.

---

## The TDD Rhythm: RED → GREEN → REFACTOR

We followed strict TDD throughout:

### Phase 5: EL Health Check (A Case Study)

**RED Phase** - First, we wrote 17 tests that defined the expected behavior:

```rust
#[test]
fn test_parse_hex_block_number_with_prefix() {
    let result = parse_hex_block_number("0x10d4f");
    assert_eq!(result.unwrap(), 68943);
}

#[tokio::test]
async fn test_check_el_node_success() {
    let mock_server = MockServer::start().await;
    // ... mock eth_blockNumber response
    let block_number = check_el_node(&mock_server.uri()).await;
    assert_eq!(block_number.unwrap(), 68943);
}
```

Running `cargo test el` showed 17 failures. Perfect—that's exactly what we wanted.

**GREEN Phase** - Then we implemented just enough code to make tests pass:

```rust
pub fn parse_hex_block_number(hex: &str) -> Result<u64> {
    let hex_str = hex.strip_prefix("0x").unwrap_or(hex);
    u64::from_str_radix(hex_str, 16)
        .wrap_err_with(|| format!("invalid hex number: {hex}"))
}
```

One by one, tests went green. The rhythm was addictive.

**REFACTOR Phase** - With passing tests as our safety net, we cleaned up code without fear.

---

## The Hardships: What Went Wrong

Building software is never smooth. Here are the challenges we faced:

### The Unreachable Node Bug

Early in development, we had a subtle bug: unreachable nodes were being marked as healthy.

**The Problem:** When a node couldn't be reached, it had `block_number = 0`. The chain head was also `0` (no nodes responding). So the lag calculation was `0 - 0 = 0`, which was within the threshold.

**The Fix:** We added a `check_ok` field to track whether the health check succeeded:

```rust
// Before: is_healthy = lag <= max_lag
// After:  is_healthy = check_ok && lag <= max_lag
```

TDD caught this bug immediately. The test `test_el_node_marked_unhealthy_on_connection_failure` failed, showing us the edge case before it could reach production.

### The axum 0.8 Route Syntax Change

We hit a compiler error when setting up routes:

```
error: invalid route syntax `*path`
```

axum 0.8 changed wildcard syntax from `*path` to `{*path}`. A quick documentation check revealed the fix. This is the kind of "boring" bug that AI handles well—pattern matching against known issues.

### The WebSocket Type Mismatch

tokio-tungstenite and axum use different types for WebSocket messages. What looked like the same type was actually incompatible:

```rust
// tungstenite uses its own Utf8Bytes
// axum uses its own Utf8Bytes
// They're not the same type!

// Fix: explicit conversion
Message::Text(text.as_str().into())
```

Three hours of human debugging compressed into three minutes of AI analysis.

---

## BDD: Speaking the Language of Users

Beyond unit tests, we used Cucumber for Behavior-Driven Development:

```gherkin
Feature: EL (Execution Layer) Health Check

  Scenario: Healthy EL node within lag threshold
    Given an EL node at block 1000
    And the EL chain head is at block 1002
    And the max EL lag is 5 blocks
    When the health check runs
    Then the EL node should be marked as healthy
    And the EL node lag should be 2 blocks
```

These scenarios served as living documentation. Anyone could read them and understand what Vixy does, without diving into code.

**Final BDD Results:**
- 3 features (config, EL health, CL health)
- 16 scenarios
- 83 steps
- All passing

---

## The Numbers

By the end of development:

| Metric | Count |
|--------|-------|
| Unit Tests | 72 |
| BDD Scenarios | 16 |
| BDD Steps | 83 |
| Lines of Rust | ~2,500 |
| Commits | 15+ |
| Development Time | ~8 hours |

All tests pass. All CI checks pass. The code is formatted, linted, and ready for production.

---

## What We Built

Vixy is a production-ready Ethereum proxy with:

- **Health Monitoring** - Tracks block numbers (EL) and slots (CL)
- **Automatic Failover** - Routes to backup nodes when primaries fail
- **HTTP Proxy** - `/el` for JSON-RPC, `/cl/*` for Beacon API
- **WebSocket Proxy** - `/el/ws` for subscriptions
- **Status Endpoint** - `/status` returns JSON with all node states
- **Metrics** - `/metrics` for Prometheus

### Quick Start

```bash
# Create config
cp config.example.toml config.toml

# Run
cargo run -- --config config.toml

# Test EL proxy
curl -X POST http://localhost:8080/el \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Check status
curl http://localhost:8080/status
```

---

## Lessons Learned

### 1. TDD is Not Optional

Every bug we caught early was a bug we didn't debug in production. The 72 unit tests aren't overhead—they're insurance.

### 2. AI Excels at Pattern Matching

The AI handled:
- Boilerplate code generation
- Error message interpretation
- Documentation lookups
- Repetitive test writing

Humans (or human-AI collaboration) handled:
- Architecture decisions
- Edge case identification
- "Does this make sense?" questions

### 3. Good Specifications Enable Autonomy

`AGENT.md` was the key. With clear phases, acceptance criteria, and examples, the AI could work independently. Vague instructions produce vague results.

### 4. The Diary Matters

`DIARY.md` captured the journey—not just what was built, but *how*. Every challenge, every fix, every learning. This blog exists because that documentation exists.

---

## The Future

Vixy is functional, but there's always more to do:

- [ ] Round-robin load balancing
- [ ] Actual retry logic (infrastructure is in place)
- [ ] TLS/HTTPS support
- [ ] CL WebSocket support (events API)
- [ ] Kubernetes deployment manifests

The foundation is solid. Extensions can be added incrementally, each with their own TDD cycle.

---

## Conclusion

Building Vixy demonstrated that AI-assisted development isn't about replacing programmers—it's about amplifying them. The AI wrote tests, implemented functions, and debugged issues. But it did so within a framework designed by humans, following principles established over decades of software engineering.

TDD, BDD, CI/CD, incremental commits—these aren't old-fashioned practices made obsolete by AI. They're the guardrails that make AI development reliable.

The future of programming is collaboration: humans defining *what* and *why*, AI executing *how*, and tests ensuring *correctness*.

Vixy is proof that this future works.

---

*Built with Rust, tested with Cucumber, assisted by Claude, powered by coffee and curiosity.*

**Repository:** [github.com/your-repo/vixy](https://github.com/your-repo/vixy)

**License:** MIT
