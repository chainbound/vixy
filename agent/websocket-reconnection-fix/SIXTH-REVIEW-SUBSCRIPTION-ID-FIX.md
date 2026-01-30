# Sixth Review - Subscription ID Preservation Fix (Issue #2)

## Overview

This document details the fix for **Issue #2: Subscription IDs change after reconnection**, the core production bug that initiated this entire WebSocket reconnection work. Previous rounds fixed subscription replay response forwarding, message queueing, concurrent reconnection guards, and test assertions. This round fixes the final critical bug where subscription IDs changed after reconnection, breaking clients.

## The Bug

**Symptom**: Integration test failure showing subscription ID changed after reconnection:
```
Subscription ID changed after reconnection!
Original: 0xed940a1ae28656838822def6c853b005
Got:      0x499df756125c5a46f1f38aa3a0470c9c
```

**Impact**: In production, clients that subscribed before reconnection would lose their subscription IDs. New events would arrive with different subscription IDs, breaking client code that relied on the original IDs.

## Root Cause Analysis

When Vixy reconnects WebSocket to a new upstream:

1. **Subscription Replay**: Vixy replays client subscriptions to the new upstream
2. **New Upstream IDs**: The new upstream assigns **new** subscription IDs (different from the originals)
3. **Incorrect Mapping**: The code at line 695 called `track_subscribe(params, id, sub_id)` where `sub_id` is the NEW upstream ID
4. **Wrong Behavior**: `track_subscribe()` maps `client_sub_id → client_sub_id`, so it mapped NEW_ID → NEW_ID
5. **Expected Behavior**: Should map NEW_ID → ORIGINAL_ID, preserving client-facing IDs

### Code Flow Before Fix

```rust
// Line 40: Type definition (3-tuple)
type PendingSubscribes = HashMap<String, (Vec<Value>, Option<String>, bool)>;
//                                         params      sub_id     is_replay

// Line 843: Replayed subscription added to pending
pending_subscribes.lock().await.insert(
    id_str,
    (sub.params.clone(), None, true),  // No original_client_sub_id tracked!
);

// Line 691-708: Response handler
if let Some((params, _, is_replay)) = pending.remove(&id_str) {
    if is_replay {
        // BUG: Just suppresses response, doesn't map IDs!
        return Ok(());
    } else {
        // Normal subscription
        tracker.lock().await.track_subscribe(params, id.clone(), sub_id);
        // track_subscribe maps: NEW_ID → NEW_ID (wrong for replays!)
    }
}
```

**Problem**: For replayed subscriptions, we suppress the response (correct), but we don't map the new upstream ID to the original client ID. Clients see the new ID in subsequent subscription notifications.

## The Fix

### Changes Made

**1. Updated `PendingSubscribes` Type (src/proxy/ws.rs:40-45)**

Added fourth field to track original client subscription ID:

```rust
// BEFORE (3-tuple)
type PendingSubscribes = HashMap<String, (Vec<Value>, Option<String>, bool)>;

// AFTER (4-tuple)
type PendingSubscribes = HashMap<String, (Vec<Value>, Option<String>, bool, Option<String>)>;
//                                         params      sub_id     is_replay  original_client_sub_id
```

**2. Updated Normal Subscription Insert (src/proxy/ws.rs:573)**

Normal subscriptions don't have an original ID (they ARE the original):

```rust
// BEFORE
pending_subscribes.lock().await.insert(
    id_str,
    (params_vec, None, false),
);

// AFTER
pending_subscribes.lock().await.insert(
    id_str,
    (params_vec, None, false, None),  // No original ID for new subscriptions
);
```

**3. Updated Replayed Subscription Insert (src/proxy/ws.rs:843-851)**

Replayed subscriptions include the original client subscription ID:

```rust
// BEFORE
pending_subscribes.lock().await.insert(
    id_str,
    (sub.params.clone(), None, true),
);

// AFTER
pending_subscribes.lock().await.insert(
    id_str,
    (
        sub.params.clone(),
        None,
        true,                              // is_replay = true
        Some(sub.client_sub_id.clone()),   // Track original client subscription ID
    ),
);
```

**4. Updated Response Handler (src/proxy/ws.rs:691-719)**

Map new upstream ID to original client ID for replayed subscriptions:

```rust
// BEFORE
if let Some((params, _, is_replay)) = pending.remove(&id_str) {
    if is_replay {
        // BUG: Only suppresses response, doesn't preserve ID!
        return Ok(());
    } else {
        tracker.lock().await.track_subscribe(params, id.clone(), sub_id);
        // Fall through
    }
}

// AFTER
if let Some((params, _, is_replay, original_client_sub_id)) = pending.remove(&id_str) {
    if is_replay {
        // Map NEW upstream ID → ORIGINAL client ID
        if let Some(original_id) = original_client_sub_id {
            tracker.lock().await.map_upstream_id(sub_id, &original_id);
            debug!(
                new_upstream_id = sub_id,
                original_client_id = original_id,
                "Mapped replayed subscription ID"
            );
        }
        return Ok(());
    } else {
        // Normal subscription - track NEW_ID → NEW_ID
        tracker.lock().await.track_subscribe(params, id.clone(), sub_id);
        // Fall through
    }
}
```

**Key Difference**:
- **Normal subscriptions**: `track_subscribe()` maps NEW_ID → NEW_ID (client sees NEW_ID)
- **Replayed subscriptions**: `map_upstream_id()` maps NEW_ID → ORIGINAL_ID (client sees ORIGINAL_ID)

**5. Updated Unit Tests (src/proxy/ws.rs:1104, 1128-1135)**

Updated test code to match new 4-tuple format:

```rust
// Normal subscription test (line 1104)
pending.insert("100".to_string(), (params.clone(), None, false, None));

// Replayed subscription test (lines 1128-1135)
pending.insert(
    "100".to_string(),
    (
        vec![serde_json::json!("newHeads")],
        None,
        true,
        Some("original-id".to_string()),
    ),
);
```

## Verification

### Integration Test Results

The critical test "WebSocket subscription IDs preserved after reconnection" now passes:

```
  Scenario: WebSocket subscription IDs preserved after reconnection
   ✔> Given Vixy is running with integration config
   ✔> And the EL nodes are healthy
   ✔  Given all Kurtosis services are running
   ✔  When I connect to the EL WebSocket endpoint
   ✔  And I subscribe to newHeads and note the subscription ID
   ✔  And I receive at least one block header
   ✔  When the primary EL node is stopped
   ✔  And I wait 6 seconds for health detection
   ✔  Then subscription events should use the same subscription ID

[stderr] Verified subscription ID preserved: 0xa5772b83d71a990b57d282a7e9513fca
```

**Before Fix**: Test failed with assertion error showing different subscription IDs
**After Fix**: Test passes with same subscription ID before and after reconnection

### Test Coverage

The fix includes comprehensive integration tests that verify:
- WebSocket subscription IDs preserved after reconnection
- WebSocket reconnects when primary node becomes unhealthy
- WebSocket switches back to primary when it recovers
- Regular JSON-RPC requests work after reconnection
- Multiple subscriptions preserved after reconnection

## Impact on Production

### Before This Fix

1. Client subscribes to `newHeads`, receives subscription ID `0xabc123`
2. Vixy reconnects to new upstream due to node failure
3. Client receives new events with subscription ID `0xdef456`
4. **Client breaks**: Code expecting `0xabc123` doesn't recognize `0xdef456`
5. User impact: Lost block notifications, broken dapp functionality

### After This Fix

1. Client subscribes to `newHeads`, receives subscription ID `0xabc123`
2. Vixy reconnects to new upstream due to node failure
3. Client receives new events with subscription ID `0xabc123` (preserved!)
4. **Client continues working**: No awareness of reconnection
5. User impact: None - seamless failover

## Summary of All WebSocket Reconnection Fixes

This completes the sixth round of fixes for WebSocket reconnection issues:

### Round 1: Initial Implementation (Phase 0-2)
- Implemented basic reconnection logic
- Added subscription replay
- Added message queueing

### Round 2: First Code Review (3 findings)
- Fixed subscription replay responses forwarded to clients
- Fixed message queueing blocking (spawn background task)
- Fixed queue not cleared on reconnection failure

### Round 3: Second Code Review (5 findings)
- Fixed current_node_name not reverted on failure
- Fixed old upstream metric not cleared
- Fixed BDD steps lacking assertions
- Removed placeholder test
- Fixed documentation accuracy

### Round 4: Third Code Review (3 findings)
- Fixed ALL subscription responses suppressed (added is_replay flag)
- Fixed concurrent reconnections overwriting receiver (added guard)
- Fixed negative integration tests not asserting

### Round 5: Fourth Code Review (5 findings)
- Fixed receive_confirmation_for_both not asserting
- Fixed receive_block_number_response_with_id not asserting
- Fixed metrics_show_primary_connected not verifying state
- Fixed websocket_should_still_work not asserting
- Removed unsupported documentation claims

### Round 6: Fifth Code Review (2 findings)
- Fixed reconnection scenario validating stale response
- Fixed remaining Then steps not asserting

### Round 7 (This Round): Subscription ID Preservation (Issue #2)
- **Fixed subscription IDs changing after reconnection**
- Added original_client_sub_id tracking to PendingSubscribes
- Use map_upstream_id() for replayed subscriptions
- Integration tests confirm subscription IDs preserved

## Files Modified

### Core Implementation
- `src/proxy/ws.rs` - Subscription ID mapping logic
  - Line 45: Updated `PendingSubscribes` type (3-tuple → 4-tuple)
  - Line 573: Updated normal subscription insert
  - Lines 843-851: Updated replayed subscription insert
  - Lines 691-719: Updated response handler to map IDs
  - Lines 1104, 1128-1135: Updated unit tests

### Tests
- `src/proxy/ws.rs` (unit tests) - Updated for new tuple format
- Integration tests - Now passing subscription ID preservation

## Conclusion

**Issue #2 is now FIXED**. Clients can subscribe, Vixy can reconnect to different upstream nodes, and subscription IDs remain stable. This was the original production bug that motivated all the WebSocket reconnection work.

All WebSocket reconnection functionality is now complete and tested:
- ✅ Reconnection to new upstream when current fails
- ✅ Subscription replay to preserve client state
- ✅ Message queueing during reconnection
- ✅ Subscription ID preservation (Issue #2)
- ✅ No duplicate subscription responses
- ✅ Proper state management and cleanup
- ✅ Comprehensive integration test coverage

---

**Date**: January 23, 2026
**Bug**: Issue #2 - Subscription IDs change after reconnection
**Implementer**: Claude Sonnet 4.5
