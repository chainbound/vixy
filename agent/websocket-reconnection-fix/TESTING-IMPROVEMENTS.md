# Why Tests Didn't Catch Production Issues - Gap Analysis

## Executive Summary

Our test suite has **critical gaps** that allowed production-breaking bugs to slip through. The tests verify the "happy path" but miss **edge cases that happen in real-world usage**.

**Key Finding**: We test **what the system should do**, but not **what it shouldn't do**.

---

## What Tests DID Cover

### ✅ Unit Tests (72 tests)
- Config parsing
- Health check parsing (hex conversion)
- Node selection logic
- Individual component behavior

**Coverage**: ~85% of functions, but only **happy path scenarios**.

### ✅ Integration Tests (20+ scenarios)
- Basic HTTP proxy forwarding
- Failover when nodes stop
- WebSocket connection establishment
- WebSocket subscription notifications continue after reconnection

**Coverage**: End-to-end happy paths work.

---

## Critical Gaps - Why Production Issues Weren't Caught

### Gap #1: No Test for Regular Requests After Reconnection ❌

**What We Test**:
```gherkin
Scenario: WebSocket reconnects when primary node becomes unhealthy
  When I subscribe to newHeads
  And primary node stops
  Then I should continue receiving block headers  # ✅ Only tests subscriptions
```

**What We DON'T Test**:
```gherkin
Scenario: Regular requests work after WebSocket reconnection
  Given I have active WebSocket connection with subscriptions
  When I send eth_blockNumber request  # Regular call, not subscription
  And primary node stops and recovers
  And reconnection completes
  When I send eth_blockNumber request again
  Then I should receive valid response  # ❌ NOT TESTED
  And I should NOT receive subscription replay responses  # ❌ NOT TESTED
```

**Why This Missed Issue #2**:
- Test only checks subscription notifications continue
- Does NOT verify regular JSON-RPC calls still work
- Does NOT check if client receives unexpected responses
- **Production**: Clients use WebSocket for BOTH subscriptions AND regular calls
- **Test**: Only validates subscriptions

**Evidence from Code** (`tests/steps/integration_steps.rs:969-1008`):
```rust
#[then("I should continue receiving block headers")]
async fn verify_continue_receiving_headers(world: &mut IntegrationWorld) {
    // Only waits for subscription notification
    // NEVER sends regular eth_blockNumber request
    // NEVER checks if replay responses are forwarded
}
```

---

### Gap #2: No Test for Multiple Simultaneous Subscriptions ❌

**What We Test**:
- Single subscription (newHeads)
- Subscription ID preserved after reconnection

**What We DON'T Test**:
```gherkin
Scenario: Multiple subscriptions maintained after reconnection
  Given I have 5 active subscriptions (newHeads, logs, pendingTx, etc)
  When primary node stops and recovers
  Then all 5 subscriptions should continue working
  And I should receive 5 subscription replay responses
  And those replay responses should NOT be forwarded to client
  And subsequent notifications should use correct IDs
```

**Why This Missed Issue #2**:
- Production had 4-5 subscriptions per connection
- Replay sent 4-5 responses
- Test only has 1 subscription → only 1 replay response
- **Impact smaller in test, catastrophic in production**

---

### Gap #3: No Test for RPC ID Reuse ❌

**What We DON'T Test**:
```gherkin
Scenario: Client reuses RPC IDs after reconnection
  Given I subscribed with {"id": 100}
  And reconnection completes
  When I send regular request with {"id": 100}  # Reused ID
  Then I should receive response for my regular request
  And NOT receive the subscription replay response
```

**Why This Missed Issue #2**:
- Real clients (Go, JavaScript) often reuse low RPC IDs
- Test uses sequential IDs, never reuses
- **Production**: ID collision between replay and new request
- **Test**: No collision, no problem observed

---

### Gap #4: No Test for Continuous Operation After Reconnection ❌

**What We Test**:
- Reconnection happens successfully
- One subscription notification received after

**What We DON'T Test**:
```gherkin
Scenario: WebSocket remains stable for extended period after reconnection
  Given reconnection completed
  When I send 1000 requests over 10 minutes
  Then all requests should succeed
  And latency should be normal
  And no connections should break
```

**Why This Missed Issue #2**:
- Test ends shortly after receiving one notification
- **Production**: Connections stayed open for hours
- **Production**: Continuous requests exposed broken state
- **Test**: Short duration hides broken state

---

### Gap #5: No Test for Messages During Reconnection Window ❌

**What We DON'T Test**:
```gherkin
Scenario: Messages sent during reconnection are not lost
  Given WebSocket connection established
  When reconnection starts (trigger manually)
  And I send 100 requests during 2-5 second reconnection window
  Then all 100 requests should eventually receive responses
  And no messages should be silently dropped
```

**Why This Missed Issue #1**:
- Hard to test timing window precisely
- Test doesn't try to send during reconnection
- **Production**: High RPS means many requests hit window
- **Test**: Sequential requests, low chance of collision

---

### Gap #6: No Test for Switching Back to Primary ❌

**What We Test**:
- Failover to backup when primary fails

**What We DON'T Test**:
```gherkin
Scenario: WebSocket switches back to primary when it recovers
  Given connected to backup node (primary is down)
  When primary node recovers and becomes healthy
  And health monitor runs
  Then WebSocket should reconnect to primary
  And metrics should show primary connected
```

**Why This Missed Issue #5**:
- Test stops after verifying failover works
- Doesn't verify recovery behavior
- **Production**: Stayed on backup for 3 hours
- **Test**: Didn't check, so didn't catch

---

### Gap #7: No Load Testing ❌

**What We DON'T Test**:
- High request rate (1000+ req/s)
- Many concurrent WebSocket connections (100+)
- Health checks under load
- Lock contention
- Memory usage over time

**Why This Missed Issue #3**:
- Health check timeout only matters under load
- Single test connection → no lock contention
- **Production**: Multiple connections → write lock starvation
- **Test**: Single connection → no contention observed

---

### Gap #8: No Chaos Engineering ❌

**What We DON'T Test**:
- Random node failures at random times
- Network delays/partitions
- Slow responses (not just failures)
- Partial failures (some methods work, others timeout)

**Why This Missed All Issues**:
- Tests use deterministic scenarios
- Stop node, wait, check result
- **Production**: Nodes become slow before failing
- **Production**: Intermittent issues
- **Test**: Binary up/down, no gradual degradation

---

## Root Cause: Test Design Philosophy Gap

### What We Follow:
✅ **TDD (Test-Driven Development)**:
- Write tests first
- Tests verify requirements
- Good coverage of happy paths

### What We're Missing:
❌ **Property-Based Testing**:
- Test properties, not specific scenarios
- "All messages should eventually be delivered"
- "No unexpected responses should be sent"

❌ **Chaos Engineering**:
- Inject random failures
- Verify system remains stable
- Test resilience, not just functionality

❌ **Adversarial Testing**:
- What could go wrong?
- What happens if client misbehaves?
- Edge cases and race conditions

❌ **Performance/Load Testing**:
- How does it behave under load?
- Does it degrade gracefully?
- Lock contention, memory leaks

---

## Improved Test Plan

### Phase 1: Add Missing Integration Tests (Immediate)

#### Test 1.1: Regular Requests After Reconnection
```gherkin
@integration @websocket @reconnection @critical
Scenario: Regular JSON-RPC requests work after WebSocket reconnection
  Given I connect via WebSocket
  And I subscribe to newHeads
  And I send eth_blockNumber and receive response
  When the primary EL node is stopped
  And I wait for reconnection to complete
  When I send eth_blockNumber request
  Then I should receive valid block number response
  And I should NOT receive any subscription replay responses
  And response time should be < 2 seconds
```

**Implementation** (add to `tests/steps/integration_steps.rs`):
```rust
#[when(regex = r"^I send (eth_\w+) and receive response$")]
async fn send_request_and_wait(world: &mut IntegrationWorld, method: String) {
    let conn = world.ws_connection.as_mut().expect("WebSocket not connected");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": world.next_rpc_id,
        "method": method,
        "params": []
    });

    world.next_rpc_id += 1;
    conn.sender.send(WsMessage::Text(request.to_string())).await.unwrap();

    // Wait for response
    let timeout = Duration::from_secs(5);
    match tokio::time::timeout(timeout, conn.receiver.next()).await {
        Ok(Some(Ok(WsMessage::Text(text)))) => {
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert!(json.get("result").is_some(), "Should have result");
            world.last_response = Some(json);
        }
        _ => panic!("Did not receive response for {}", method),
    }
}

#[then("I should NOT receive any subscription replay responses")]
async fn verify_no_unexpected_responses(world: &mut IntegrationWorld) {
    let conn = world.ws_connection.as_mut().expect("WebSocket not connected");

    // Drain any pending messages for 1 second
    let mut unexpected_responses = vec![];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);

    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(100), conn.receiver.next()).await {
            Ok(Some(Ok(WsMessage::Text(text)))) => {
                let json: serde_json::Value = serde_json::from_str(&text).unwrap();
                // Check if this is a subscription response (has "result" with hex string)
                if let Some(result) = json.get("result") {
                    if result.is_string() && result.as_str().unwrap().starts_with("0x") {
                        // Could be subscription ID - check if we requested this ID
                        unexpected_responses.push(json);
                    }
                }
            }
            _ => break,
        }
    }

    assert!(
        unexpected_responses.is_empty(),
        "Received {} unexpected subscription replay responses: {:?}",
        unexpected_responses.len(),
        unexpected_responses
    );
}
```

---

#### Test 1.2: Multiple Subscriptions After Reconnection
```gherkin
@integration @websocket @reconnection @critical
Scenario: Multiple subscriptions maintained after reconnection
  Given I connect via WebSocket
  When I subscribe to newHeads with ID 100
  And I subscribe to logs with ID 101
  And I subscribe to pendingTransactions with ID 102
  And I receive confirmation for all 3 subscriptions
  When the primary EL node is stopped
  And I wait for reconnection to complete
  Then all 3 subscriptions should still be active
  And I should receive notifications for all 3 types
  And subscription IDs should be preserved (100, 101, 102)

  When I send eth_blockNumber with ID 200
  Then I should receive block number response with ID 200
  And I should NOT receive subscription replay responses (IDs 100-102)
```

---

#### Test 1.3: High Request Rate During Reconnection
```gherkin
@integration @websocket @reconnection @load
Scenario: No messages lost during reconnection window
  Given I connect via WebSocket
  When I start sending 100 requests per second continuously
  And the primary EL node is stopped after 5 seconds
  And reconnection takes 3 seconds
  And I continue sending requests during reconnection
  Then all requests should receive responses within 10 seconds
  And no requests should timeout
  And no "context deadline exceeded" errors
```

---

#### Test 1.4: Switch Back to Primary
```gherkin
@integration @websocket @reconnection @failover
Scenario: WebSocket automatically switches back to primary when recovered
  Given all Kurtosis services are running
  And I connect via WebSocket through Vixy
  When the primary EL node is stopped
  And I wait 6 seconds for failover to backup
  Then metrics should show backup node connected

  When the primary EL node is restarted
  And I wait 6 seconds for health detection
  Then metrics should show primary node connected
  And the WebSocket connection should still work
  And I should receive notifications without interruption
```

**Implementation**:
```rust
#[then("metrics should show {word} node connected")]
async fn verify_metrics_node_connected(world: &mut IntegrationWorld, node_type: String) {
    let metrics_url = format!("http://{}:{}/metrics", world.vixy_host, world.vixy_metrics_port);
    let response = reqwest::get(&metrics_url).await.unwrap();
    let body = response.text().await.unwrap();

    // Parse metrics to find which node is connected
    let mut connected_node = None;
    for line in body.lines() {
        if line.starts_with("vixy_ws_upstream_node_connected{") {
            if line.ends_with(" 1") {
                // Extract node name from label
                if let Some(name_start) = line.find("node=\"") {
                    let name_start = name_start + 6;
                    if let Some(name_end) = line[name_start..].find("\"") {
                        connected_node = Some(line[name_start..name_start + name_end].to_string());
                    }
                }
            }
        }
    }

    let connected = connected_node.expect("No node connected in metrics");

    match node_type.as_str() {
        "primary" => assert!(connected.contains("primary"), "Expected primary node, got {}", connected),
        "backup" => assert!(connected.contains("backup"), "Expected backup node, got {}", connected),
        _ => panic!("Unknown node type: {}", node_type),
    }
}
```

---

### Phase 2: Property-Based Testing (1 week)

Use `proptest` or `quickcheck` to verify properties:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn all_client_messages_receive_responses(
        messages in prop::collection::vec(any::<JsonRpcRequest>(), 1..1000)
    ) {
        // Property: Every request should get exactly one response
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let responses = send_all_via_websocket(messages.clone()).await;
            assert_eq!(messages.len(), responses.len());

            // Verify IDs match
            for (req, resp) in messages.iter().zip(responses.iter()) {
                assert_eq!(req.id, resp.id);
            }
        });
    }

    #[test]
    fn no_unexpected_messages_after_reconnection(
        subscriptions in prop::collection::vec(any::<SubscribeParams>(), 1..10)
    ) {
        // Property: Client should never receive subscription replay responses
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            // Subscribe to multiple things
            for sub in &subscriptions {
                subscribe(sub).await;
            }

            // Force reconnection
            trigger_reconnection().await;

            // Send regular request
            let response = send_request("eth_blockNumber").await;

            // Should ONLY receive our response, not replay responses
            assert_eq!(response.method, "eth_blockNumber");
            assert!(response.result.is_some());
        });
    }
}
```

---

### Phase 3: Chaos Testing (2 weeks)

#### Setup Chaos Framework
```rust
// tests/chaos/mod.rs
pub struct ChaosConfig {
    pub node_failure_probability: f64,  // 0.1 = 10% chance per second
    pub network_delay_ms: Range<u64>,   // 0..1000ms random delay
    pub slow_response_probability: f64, // Some requests take 10x longer
    pub partial_failure_rate: f64,      // Some methods fail, others work
}

pub async fn run_chaos_test(
    duration: Duration,
    config: ChaosConfig,
    workload: impl Fn() -> Future<Output = ()>,
) {
    // Continuously inject faults while running workload
    // Verify system remains stable
}
```

#### Chaos Scenarios
```gherkin
@chaos @extended
Scenario: System remains stable under continuous chaos
  Given Vixy is running with 4 nodes
  When I run chaos test for 1 hour with:
    | node_failures      | 10% per minute |
    | network_delays     | 0-5000ms       |
    | slow_responses     | 20%            |
  And I maintain 100 WebSocket connections
  And I send 1000 requests per second
  Then error rate should be < 1%
  And all WebSocket connections should remain stable
  And no memory leaks should occur
  And response times should recover after chaos stops
```

---

### Phase 4: Load/Soak Testing (2 weeks)

#### Load Test Suite
```bash
# loadtest/websocket_reconnection_load.js (using k6)
import ws from 'k6/ws';
import { check } from 'k6';

export let options = {
  stages: [
    { duration: '5m', target: 100 },  // Ramp up to 100 connections
    { duration: '1h', target: 100 },  // Hold for 1 hour
    { duration: '5m', target: 0 },    // Ramp down
  ],
};

export default function () {
  const url = 'ws://localhost:8080/el/ws';
  const params = { tags: { name: 'WebSocketLoadTest' } };

  ws.connect(url, params, function (socket) {
    // Subscribe to newHeads
    socket.send(JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method: 'eth_subscribe',
      params: ['newHeads'],
    }));

    // Send regular requests every second
    let requestId = 100;
    socket.setInterval(function () {
      socket.send(JSON.stringify({
        jsonrpc: '2.0',
        id: requestId++,
        method: 'eth_blockNumber',
        params: [],
      }));
    }, 1000);

    socket.on('message', function (msg) {
      const data = JSON.parse(msg);

      // Verify we never receive unexpected subscription responses
      if (data.id >= 100) {
        // This should be response to our eth_blockNumber
        check(data, {
          'is valid response': (r) => r.result !== undefined,
          'is not subscription ID': (r) => !r.result?.startsWith('0x'),
        });
      }
    });

    socket.setTimeout(function () {
      socket.close();
    }, 3600000); // 1 hour
  });
}
```

#### Soak Test Goals
- Run for 24 hours
- Force reconnections every 5 minutes
- Monitor metrics:
  - Request success rate (target: >99.9%)
  - Response time P99 (target: <100ms)
  - Memory RSS (should be stable)
  - Connection count (should match active clients)
  - No "context deadline exceeded" errors

---

## Test Coverage Metrics (After Improvements)

### Before
- Unit test coverage: 85% (happy paths only)
- Integration scenarios: 20
- Edge case coverage: ~10%
- Load testing: None
- Chaos testing: None

### After Phase 1
- Unit test coverage: 90% (including edge cases)
- Integration scenarios: 35+
- Edge case coverage: ~60%
- Reconnection scenarios: 10+
- Load testing: Basic
- Chaos testing: None

### After Phase 2-4
- Unit test coverage: 95%
- Property-based tests: 20+
- Integration scenarios: 50+
- Edge case coverage: ~90%
- Load testing: Comprehensive (k6 suite)
- Chaos testing: Continuous (30 min daily)
- Soak testing: Weekly (24 hour runs)

---

## Implementation Timeline

### Week 1 (Immediate - P0)
- Add 4 critical integration tests (1.1-1.4)
- These would have caught Issues #2 and #5
- **Blocks Phase 0 deployment until tests pass**

### Week 2-3 (P1)
- Add property-based tests
- Set up chaos testing framework
- Add load test suite (k6)

### Week 4+ (P2)
- Continuous chaos testing in CI
- Weekly soak tests
- Monthly full chaos runs

---

## CI/CD Integration

### Pre-Merge Checks (Required)
```yaml
# .github/workflows/ci.yml
jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test

  integration-tests:
    runs-on: ubuntu-latest
    steps:
      - name: Start Kurtosis testnet
        run: just kurtosis-up
      - name: Run integration tests
        run: cargo test --test integration_cucumber
      - name: Critical reconnection tests
        run: cargo test --test integration_cucumber --tags @reconnection @critical

  load-tests:
    runs-on: ubuntu-latest
    if: github.event_name == 'pull_request'
    steps:
      - name: Run 5-minute load test
        run: k6 run --duration 5m loadtest/websocket_reconnection_load.js
```

### Nightly Builds (Extended)
```yaml
# .github/workflows/nightly.yml
on:
  schedule:
    - cron: '0 2 * * *'  # 2 AM daily

jobs:
  chaos-test:
    runs-on: ubuntu-latest
    steps:
      - name: Run 30-minute chaos test
        run: cargo test --test chaos -- --ignored

  soak-test:
    runs-on: ubuntu-latest
    if: github.event.schedule == '0 2 * * 0'  # Sunday only
    steps:
      - name: Run 24-hour soak test
        run: k6 run --duration 24h loadtest/soak.js
```

---

## Lessons for Future Development

### 1. Test What Can Go Wrong
- Don't just test happy path
- Think adversarially: "How could this break?"
- Edge cases are where bugs live

### 2. Test Real-World Usage Patterns
- Multiple subscriptions simultaneously
- RPC ID reuse
- High request rates
- Long-lived connections

### 3. Test Failure Recovery
- Not just "failover works"
- But "failover AND recovery work"
- And "system remains stable after recovery"

### 4. Test Under Load
- Single connection hides concurrency bugs
- Lock contention only appears under load
- Memory leaks need time to manifest

### 5. Test Timing Windows
- Reconnection window
- Health check intervals
- Race conditions

### 6. Property-Based Testing
- "All messages delivered" (not "these 3 messages delivered")
- "No unexpected responses" (not "this one response is expected")
- Broader coverage with less code

### 7. Continuous Chaos
- Don't wait for production to find edge cases
- Inject failures regularly in test environments
- Build confidence in resilience

---

## Success Criteria

### Test Suite Quality
- [ ] All issues in AGENT-heavy-load-REVIEWED.md would be caught by tests
- [ ] Can reproduce production incident in test environment
- [ ] CI fails if issues are reintroduced
- [ ] Property tests pass with 10,000+ random inputs
- [ ] Chaos tests run daily without failures
- [ ] Soak tests run weekly for 24 hours without errors

### Coverage Metrics
- [ ] Unit test coverage > 90%
- [ ] Integration scenarios cover all critical paths
- [ ] Edge case coverage > 80%
- [ ] All public APIs have property tests
- [ ] All failure modes have integration tests

### CI/CD Quality
- [ ] PR checks complete in < 15 minutes
- [ ] Nightly builds catch regressions
- [ ] Load tests prevent performance degradation
- [ ] No flaky tests (>99.9% stability)

---

## Conclusion

Our tests were **necessary but not sufficient**. They verified basic functionality but missed:

1. **Edge cases**: RPC ID reuse, multiple subscriptions
2. **Real usage**: Mixed subscription + regular calls
3. **Failure recovery**: Not just failover, but return to primary
4. **Load behavior**: Lock contention, timing windows
5. **Long-term stability**: Memory leaks, broken connections

**The Fix**: Comprehensive test suite following phases 1-4 above.

**Timeline**: 4 weeks to full coverage, but Phase 1 (critical tests) ships in 1 week.

**Philosophy Shift**: From "does it work?" to "what could break it?"
