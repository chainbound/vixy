# Vixy Heavy Load Issues - Comprehensive Review & Fix Plan

## Executive Summary

After production incident analysis, we identified that Vixy has **fundamental design flaws** that cause cascading failures under load. These aren't just bugs - they violate core distributed systems principles.

**Key Finding**: A well-designed proxy should NEVER break client connections during backend node failures. Vixy currently does.

---

## Design Principles Violated

### Principle #1: Transparent Proxying
**Violated**: Clients should never know the proxy is switching upstreams.

**Current Behavior**:
- Subscription replay responses forwarded to clients
- Clients receive unexpected JSON-RPC responses
- Client state machines break
- Connection becomes permanently unusable

**Should Be**: Reconnection is completely invisible to clients.

---

### Principle #2: Graceful Degradation
**Violated**: System should degrade gracefully under load, not catastrophically fail.

**Current Behavior**:
- Health checks without timeouts → minutes-long write locks
- WebSocket reconnection breaks ALL active connections
- No circuit breaker, no backpressure
- Cascading failures

**Should Be**: Slow nodes timeout quickly, proxy remains responsive.

---

### Principle #3: No Message Loss
**Violated**: Proxy must never silently drop messages.

**Current Behavior**:
- Messages sent during reconnection window (2-5s) are lost
- No queueing, no retry, no error to client
- Client waits indefinitely → timeout

**Should Be**: Messages queued during reconnection, replayed after.

---

### Principle #4: Lock-Free Hot Path
**Violated**: Critical path should avoid holding locks during I/O.

**Current Behavior**:
- Health monitor holds write lock during HTTP requests
- No timeout → can block for minutes
- WebSocket health monitor starves waiting for read lock
- Delayed failover detection

**Should Be**: Health checks run without holding state locks.

---

### Principle #5: Smart Routing
**Violated**: Proxy should use best available backend.

**Current Behavior**:
- Health monitor only reconnects when current node fails
- Never switches back to primary after recovery
- All traffic stuck on backup forever

**Should Be**: Always use primary when available, auto-rebalance.

---

## Root Cause Analysis - Confirmed Issues

### ✅ Issue #2: Subscription Replay Responses Break Clients (CONFIRMED ROOT CAUSE)

**Evidence**:
- Timeline: 4 reconnections → 3 hours of continuous timeouts
- Subscriptions dropped from 4-5 to 1 (others broke)
- Half the clients eventually gave up and reconnected fresh

**What Happens**:
```
1. Client has 4 subscriptions active (IDs: 100, 101, 102, 103)
2. Reconnection triggered
3. Vixy replays subscriptions with original IDs to new upstream
4. New upstream responds: {"id": 100, "result": "0xNEW_SUB_ID"}
5. Vixy forwards this to client (not in pending_subscribes)
6. Client wasn't expecting this (completed subscription hours ago)
7. Client's JSON-RPC state machine breaks
8. All future requests timeout
9. Connection becomes "zombie" - open but broken
10. Client eventually gives up, disconnects, reconnects fresh
```

**Why This Violates Principle #1**:
- Client sees internal Vixy operations (subscription replay)
- Breaks JSON-RPC protocol assumptions (unexpected responses)
- Not transparent

---

### ✅ Issue #5: Never Switches Back to Primary (CONFIRMED)

**Evidence**:
- Metrics show only backup connected
- Primary recovered 3 hours ago
- failover_active = false (primaries healthy)
- Still stuck on backup

**Code Analysis**:
```rust
// src/proxy/ws.rs:166
if !is_node_healthy(&state, &node_name).await {
    // Only checks if CURRENT node unhealthy
    // Never checks if BETTER node available!
}
```

**Why This Violates Principle #5**:
- Backup nodes often have rate limits, lower quality
- Should prefer primary when available
- No auto-rebalancing

---

### ❓ Issue #1: Messages Lost During Reconnection (LIKELY, NOT CONFIRMED)

**Current Status**: Not observed in production, but code clearly has this bug.

**Why We Didn't See It**:
- Reconnection window is only 2-5 seconds
- Issue #2 broke connections permanently AFTER reconnection
- Issue #2's impact masked Issue #1

**But It's Still a Bug**: Under high RPS, many messages hit the reconnection window.

---

### ❓ Issue #3: Health Checks Block Without Timeout (POSSIBLE, NOT CONFIRMED)

**Evidence Needed**:
- Check health check cycle duration in metrics
- Look for slow health check logs

**Likely Scenario**:
- Nodes were genuinely slow/unhealthy for extended period
- Health checks may have been slow, but not indefinitely blocked
- This is more of a performance issue than a breaking bug

**Still Needs Fixing**: Prevents other issues from cascading.

---

### ❌ Issue #4: WebSocket Monitor Starvation (NOT AN ISSUE)

**Analysis**: This is a CONSEQUENCE of Issue #3, not a root cause itself.

**Verdict**: Don't need a separate fix, fixing Issue #3 resolves this.

---

## Revised Issue Priority

### 🔴 CRITICAL (Breaks Production)
1. **Issue #2**: Subscription replay responses break clients
   - **Impact**: 3 hours of continuous failures, clients broken until restart
   - **Fix Complexity**: Medium (add pending_subscribes parameter)
   - **Priority**: P0 - MUST FIX IMMEDIATELY

2. **Issue #5**: Never switches back to primary
   - **Impact**: All traffic stuck on backup indefinitely
   - **Fix Complexity**: Low (add better node check)
   - **Priority**: P0 - FIX WITH ISSUE #2

### 🟠 HIGH (Prevents Cascading Failures)
3. **Issue #1**: Messages lost during reconnection
   - **Impact**: Sporadic timeouts during reconnection window
   - **Fix Complexity**: High (need queueing mechanism)
   - **Priority**: P1 - FIX AFTER P0 ISSUES

4. **Issue #3**: Health checks block without timeout
   - **Impact**: Slow failover, lock contention
   - **Fix Complexity**: Low (add timeout to HTTP client)
   - **Priority**: P1 - FIX WITH ISSUE #1

### 🟡 MEDIUM (Future Optimization)
5. **Health Check Concurrency**: Run checks in parallel
   - **Impact**: Faster health check cycles
   - **Fix Complexity**: Medium
   - **Priority**: P2 - AFTER P0 AND P1

---

## Comprehensive Fix Plan

### Phase 0: Immediate Hotfix (P0 Issues)

**Goal**: Stop breaking client connections during reconnection.

**Issues Fixed**: #2, #5

**Timeline**: Ship within 24 hours

#### Step 0.1: Fix Subscription Replay (Issue #2)

**Test First (TDD)**:
```rust
#[tokio::test]
async fn test_subscription_replay_responses_not_forwarded_to_client() {
    // Setup: client with active subscription
    // Trigger reconnection
    // Verify: replay responses consumed internally, not forwarded
    // Verify: client's regular requests still work
}

#[tokio::test]
async fn test_subscription_notifications_work_after_reconnection() {
    // Setup: client subscribed to newHeads
    // Trigger reconnection
    // Verify: subscription notifications continue
    // Verify: subscription IDs correctly mapped
}
```

**Implementation**:
```rust
// Update reconnect_upstream signature
async fn reconnect_upstream(
    ws_url: &str,
    tracker: &Arc<Mutex<SubscriptionTracker>>,
    _old_sender: &Arc<Mutex<UpstreamSender>>,
    pending_subscribes: &Arc<Mutex<PendingSubscribes>>,  // ← ADD
) -> Result<(UpstreamReceiver, UpstreamSender), String> {
    let (new_ws, _) = connect_async(ws_url).await?;
    let (mut new_sender, new_receiver) = new_ws.split();

    // Clear old mappings
    let mut tracker_guard = tracker.lock().await;
    tracker_guard.clear_upstream_mappings();

    // Get subscriptions to replay
    let subscriptions: Vec<_> = tracker_guard
        .get_all_subscriptions()
        .iter()
        .map(|s| (*s).clone())
        .collect();
    drop(tracker_guard);

    // Replay subscriptions
    for sub in subscriptions {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": sub.rpc_id,
            "method": "eth_subscribe",
            "params": sub.params
        });

        // ✅ ADD TO PENDING BEFORE SENDING
        let id_str = sub.rpc_id.to_string();
        pending_subscribes.lock().await.insert(
            id_str,
            (sub.params.clone(), None)
        );

        new_sender.send(TungsteniteMessage::Text(request.to_string().into())).await?;

        debug!(
            client_sub_id = %sub.client_sub_id,
            "Replayed subscription request"
        );
    }

    Ok((new_receiver, new_sender))
}

// Update call site in run_proxy_loop (line 370)
match reconnect_upstream(
    &reconnect_info.ws_url,
    &tracker,
    &upstream_sender,
    &pending_subscribes,  // ← ADD
).await {
    // ... rest unchanged
}
```

**Verification**:
- Unit tests pass
- Integration test: subscribe, trigger reconnection, verify no broken state
- Manual test: run for 1 hour with forced reconnections every 5 min

---

#### Step 0.2: Fix Never Switches Back to Primary (Issue #5)

**Test First (TDD)**:
```rust
#[tokio::test]
async fn test_ws_switches_back_to_primary_when_recovered() {
    // Setup: primary unhealthy, connected to backup
    // Make primary healthy
    // Wait for health monitor cycle
    // Verify: switched back to primary
}

#[tokio::test]
async fn test_ws_prefers_primary_over_backup() {
    // Setup: both primary and backup healthy
    // Verify: always uses primary
}
```

**Implementation**:
```rust
// Update health_monitor in src/proxy/ws.rs:153
async fn health_monitor(
    state: Arc<AppState>,
    current_node_name: Arc<Mutex<String>>,
    reconnect_tx: mpsc::Sender<ReconnectInfo>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));

    loop {
        interval.tick().await;

        let node_name = current_node_name.lock().await.clone();
        let current_healthy = is_node_healthy(&state, &node_name).await;

        // ✅ Check if better node available
        if let Some((best_name, best_url)) = select_healthy_node(&state).await {
            // Reconnect if:
            // 1. Current node is unhealthy, OR
            // 2. Better node available (e.g., primary when on backup)
            if !current_healthy || best_name != node_name {
                if best_name != node_name {
                    info!(
                        current_node = %node_name,
                        best_node = %best_name,
                        reason = if !current_healthy { "current_unhealthy" } else { "better_available" },
                        "Switching WebSocket upstream"
                    );

                    if reconnect_tx.send(ReconnectInfo {
                        node_name: best_name,
                        ws_url: best_url,
                    }).await.is_err() {
                        break;
                    }
                }
            }
        } else if !current_healthy {
            warn!("Current WebSocket node unhealthy but no healthy nodes available");
        }
    }
}
```

**Verification**:
- Unit tests pass
- Integration test: failover to backup, recover primary, verify switch back
- Metrics show correct node transitions

---

#### Step 0.3: Integration Testing

**Test Scenarios**:
1. **Reconnection doesn't break clients**:
   - 100 clients, 5 subscriptions each
   - Force reconnection every 30 seconds
   - Send 1000 req/s continuous
   - Verify: 0 timeouts after reconnection

2. **Switch back to primary**:
   - Start with backup
   - Make primary healthy
   - Verify: switches within 2 seconds
   - Verify: metrics updated

3. **Multiple reconnections**:
   - Run for 1 hour
   - Force 20 reconnections
   - Verify: all subscriptions maintained
   - Verify: no broken connections

**Acceptance Criteria**:
- Zero "context deadline exceeded" after reconnection
- All subscriptions maintain correct IDs
- Auto-switch to primary when available
- No memory leaks (check RSS)

---

### Phase 1: Prevent Message Loss (P1 Issues)

**Goal**: Ensure zero message loss during reconnection.

**Issues Fixed**: #1, #3

**Timeline**: Ship within 1 week after Phase 0

#### Step 1.1: Add Health Check Timeouts (Issue #3)

**Why First**: Prevents long lock holds, makes Phase 1.2 easier.

**Test First**:
```rust
#[tokio::test]
async fn test_health_check_respects_timeout() {
    // Mock server that never responds
    // Call check_el_node with 5s timeout
    // Verify: returns error after ~5s, not hanging
}
```

**Implementation**:
```rust
// Update src/health/el.rs
pub async fn check_el_node(url: &str, timeout_ms: u64) -> Result<u64> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| eyre!("Failed to build HTTP client: {e}"))?;

    // ... rest unchanged
}

// Update src/config.rs - add to Global
pub struct Global {
    pub max_el_lag_blocks: u64,
    pub max_cl_lag_slots: u64,
    pub health_check_interval_ms: u64,
    pub health_check_timeout_ms: u64,  // ← ADD, default 5000
    pub proxy_timeout_ms: u64,
    pub max_retries: u32,
}

// Update src/monitor.rs - pass timeout
for node in el_nodes.iter_mut() {
    match el::check_el_node(&node.http_url, state.health_check_timeout_ms).await {
        // ... rest unchanged
    }
}
```

**Impact**: Health check cycle completes in <10s even with slow nodes.

---

#### Step 1.2: Queue Messages During Reconnection (Issue #1)

**Design Decision**: Queue vs Block vs Atomic Swap

**Option A: Message Queueing** (RECOMMENDED)
- Queue client messages during reconnection
- Replay after reconnect completes
- Pros: No blocking, preserves order
- Cons: Need queue size limit

**Option B: Block Client Messages**
- Pause reading from client during reconnection
- Pros: Simple, no queue needed
- Cons: Head-of-line blocking, TCP backpressure

**Option C: Atomic Sender Swap**
- Use `ArcSwap<UpstreamSender>`
- Pros: No coordination needed
- Cons: Complex, may still lose messages in flight

**Chosen: Option A (Queueing)**

**Test First**:
```rust
#[tokio::test]
async fn test_messages_queued_during_reconnection() {
    // Send 100 messages
    // Trigger reconnection (takes 2s)
    // Continue sending during reconnection
    // Verify: all messages delivered, none lost
    // Verify: responses received in correct order
}

#[tokio::test]
async fn test_queue_size_limit_enforced() {
    // Fill queue to limit
    // Send one more message
    // Verify: error or oldest message dropped (configurable)
}
```

**Implementation**:
```rust
// Add to run_proxy_loop
let reconnecting = Arc::new(AtomicBool::new(false));
let message_queue: Arc<Mutex<VecDeque<TungsteniteMessage>>> =
    Arc::new(Mutex::new(VecDeque::new()));
let max_queue_size = 1000; // Configurable

// Update handle_client_message
async fn handle_client_message(
    msg: Message,
    upstream_sender: &Arc<Mutex<UpstreamSender>>,
    tracker: &Arc<Mutex<SubscriptionTracker>>,
    pending_subscribes: &Arc<Mutex<PendingSubscribes>>,
    reconnecting: &Arc<AtomicBool>,
    message_queue: &Arc<Mutex<VecDeque<TungsteniteMessage>>>,
    max_queue_size: usize,
) -> Result<(), bool> {
    match msg {
        Message::Text(text) => {
            // ... existing parsing logic for tracking subs ...

            let upstream_msg = TungsteniteMessage::Text(text.to_string().into());

            // Check if reconnecting
            if reconnecting.load(Ordering::SeqCst) {
                let mut queue = message_queue.lock().await;
                if queue.len() < max_queue_size {
                    queue.push_back(upstream_msg);
                    debug!("Queued message during reconnection");
                    return Ok(());
                } else {
                    warn!("Message queue full during reconnection");
                    return Err(false); // Or drop oldest and enqueue new
                }
            }

            // Normal forwarding
            if upstream_sender.lock().await.send(upstream_msg).await.is_err() {
                return Err(false);
            }
        }
        // ... handle other message types ...
    }
    Ok(())
}

// Update reconnection handler in run_proxy_loop
Some(reconnect_info) = reconnect_rx.recv() => {
    // Set reconnecting flag
    reconnecting.store(true, Ordering::SeqCst);

    info!("Reconnecting WebSocket upstream");

    match reconnect_upstream(...).await {
        Ok((new_receiver, new_sender)) => {
            // Update sender
            *upstream_sender.lock().await = new_sender;

            // Drain queue
            let mut queue = message_queue.lock().await;
            while let Some(msg) = queue.pop_front() {
                if upstream_sender.lock().await.send(msg).await.is_err() {
                    error!("Failed to send queued message after reconnection");
                    break;
                }
            }
            drop(queue);

            // Clear reconnecting flag
            reconnecting.store(false, Ordering::SeqCst);

            info!(queued_messages = queue_size, "WebSocket reconnection successful, replayed queue");
        }
        Err(e) => {
            // Reconnection failed, keep old connection
            reconnecting.store(false, Ordering::SeqCst);
            error!(error = %e, "Failed to reconnect");
        }
    }
}
```

**Metrics**:
```rust
vixy_ws_messages_queued_total - counter
vixy_ws_queue_size_current - gauge
vixy_ws_queue_full_total - counter (dropped messages)
```

**Verification**:
- Load test: 5000 req/s, force reconnection every 30s
- Verify: zero message loss
- Verify: queue drains quickly after reconnect
- Verify: queue never exceeds limit

---

### Phase 2: Performance Optimization (P2)

**Goal**: Reduce health check latency, improve failover speed.

**Timeline**: Ship within 2 weeks after Phase 1

#### Step 2.1: Concurrent Health Checks

**Current**: Sequential checks hold write lock

**New Design**: Concurrent checks, lock-free reads

```rust
pub async fn check_all_el_nodes(state: &Arc<AppState>) -> bool {
    // Read URLs without holding lock
    let nodes_to_check: Vec<(String, String)> = {
        let el_nodes = state.el_nodes.read().await;
        el_nodes.iter()
            .map(|n| (n.name.clone(), n.http_url.clone()))
            .collect()
    };

    // Check all nodes concurrently
    let mut tasks = vec![];
    for (name, url) in nodes_to_check {
        let timeout = state.health_check_timeout_ms;
        tasks.push(tokio::spawn(async move {
            (name, el::check_el_node(&url, timeout).await)
        }));
    }

    // Collect results
    let mut results = vec![];
    for task in tasks {
        if let Ok(result) = task.await {
            results.push(result);
        }
    }

    // Update state with write lock (fast, no I/O)
    let mut el_nodes = state.el_nodes.write().await;
    for (name, result) in results {
        if let Some(node) = el_nodes.iter_mut().find(|n| n.name == name) {
            match result {
                Ok(block_number) => {
                    node.block_number = block_number;
                    node.check_ok = true;
                }
                Err(_) => {
                    node.check_ok = false;
                }
            }
        }
    }
    drop(el_nodes); // Release lock

    // Calculate health (with write lock again, but fast)
    let chain_head = {
        let el_nodes = state.el_nodes.read().await;
        el::update_el_chain_head(&el_nodes)
    };

    let mut el_nodes = state.el_nodes.write().await;
    let mut any_primary_healthy = false;
    for node in el_nodes.iter_mut() {
        el::calculate_el_health(node, chain_head, state.max_el_lag);
        if node.is_primary && node.is_healthy {
            any_primary_healthy = true;
        }
    }

    any_primary_healthy
}
```

**Impact**:
- 4 nodes × 5s timeout: 20s sequential → 5s concurrent
- Write lock held for <100ms instead of 20s
- WebSocket health monitor no longer starves

---

## Testing Strategy

### Unit Tests (TDD)
- Write tests FIRST for each fix
- Verify tests fail before implementation
- Verify tests pass after implementation

### Integration Tests
- Kurtosis testnet with 4 nodes
- Load generator: 1000-5000 req/s
- Network chaos: random delays, disconnects
- Scenarios:
  1. All primary nodes fail → failover to backup
  2. Primary recovers → switch back
  3. Continuous reconnections (every 30s for 1hr)
  4. 100 clients with subscriptions

### Soak Tests
- Run for 24 hours in staging
- Metrics monitoring:
  - Zero "context deadline exceeded" after reconnection
  - Queue size stays <10 during reconnection
  - Health check cycles <10s
  - Memory stable (no leaks)

---

## Rollout Plan

### Phase 0 Deployment (Hotfix)
1. **Code Review**: 2 engineers review fix
2. **Testing**: Run integration tests 3 times
3. **Canary**: Deploy to 10% traffic
4. **Monitor**: 24 hours, watch error rates
5. **Rollout**: 25% → 50% → 100% over 3 days

### Phase 1 Deployment
1. **Staging**: 1 week soak test
2. **Canary**: Deploy to 10% traffic
3. **Monitor**: 48 hours, watch queue metrics
4. **Rollout**: Gradual over 1 week

### Phase 2 Deployment
1. **Benchmark**: Compare health check latency before/after
2. **Staging**: 2 week soak test
3. **Canary**: Deploy to 10% traffic
4. **Rollout**: Gradual over 2 weeks

---

## Success Metrics

### Before Fixes (Current)
- Request failure rate: ~5% during/after reconnection
- Subscriptions broken: ~40% after reconnection
- Time stuck on backup: Indefinite
- Health check cycles: 30-180s
- Messages lost during reconnection: Unknown (silently dropped)

### After Phase 0 (Target)
- Request failure rate: <0.01%
- Subscriptions broken: <0.1%
- Time stuck on backup: <2 seconds after primary recovery
- Reconnection impact: Invisible to clients

### After Phase 1 (Target)
- Messages lost during reconnection: 0
- Health check cycles: <10s
- Lock contention: Eliminated

### After Phase 2 (Target)
- Health check cycles: <5s (concurrent)
- Failover detection: <2s
- Memory usage: Stable over 7 days

---

## Implementation Order Summary

1. **Phase 0** (24 hours):
   - Fix subscription replay (Issue #2)
   - Fix switch back to primary (Issue #5)
   - **Ship immediately**

2. **Phase 1** (1 week):
   - Add health check timeouts (Issue #3)
   - Add message queueing (Issue #1)
   - **Ship after Phase 0 proven stable**

3. **Phase 2** (2 weeks):
   - Concurrent health checks
   - **Ship after Phase 1 proven stable**

Total timeline: ~3-4 weeks from start to full deployment.

---

## References

- Production incident timeline
- Metrics dashboard
- DIARY.md for detailed investigation log
- AGENT.md for TDD workflow

---

## Open Questions

1. **Queue size limit**: 1000 messages reasonable? Calculate based on RPS × reconnection time.
2. **Queue overflow behavior**: Drop oldest, drop newest, or return error to client?
3. **Metrics retention**: How long to keep detailed reconnection metrics?
4. **Alerting**: What thresholds trigger pages?

---

## Lessons Learned

1. **Design for Failure**: Assume backends will fail, design for transparency
2. **Lock Hygiene**: Never hold locks during I/O operations
3. **Test Under Load**: Unit tests aren't enough, need chaos engineering
4. **Metrics Matter**: Good metrics would have caught Issue #5 immediately
5. **Simplicity**: Complex reconnection logic → more bugs

---

## Future Improvements (Post-P2)

1. **Circuit Breaker**: Stop trying unhealthy nodes for exponential backoff period
2. **Connection Pooling**: Reuse upstream connections across clients
3. **Request Hedging**: Send duplicate requests to multiple upstreams, use fastest
4. **Smart Load Balancing**: Consider node latency, not just health
5. **Subscription Deduplication**: If multiple clients subscribe to same thing, share upstream subscription

These are NICE TO HAVE, not required for production stability.
