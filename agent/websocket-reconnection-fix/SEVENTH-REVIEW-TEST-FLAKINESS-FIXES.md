# Seventh Review - Test Flakiness Fixes

## Overview

This document details fixes for the seventh code review, which identified **3 test flakiness issues** that could cause false failures in integration tests due to improper message filtering and state handling.

## Findings and Fixes

### Finding 1 (Medium): WebSocket scenario uses HTTP-only assertion step

**Locations**:
- `tests/features/integration/el_proxy.feature:76` - Feature file
- `tests/steps/integration_steps.rs:180` - Step implementation

**Problem**:

The WebSocket reconnection scenario (line 72-78 in feature file) uses the step:
```gherkin
Then I should receive a valid block number response
```

This step implementation checks `world.last_status_code == 200`:
```rust
// BEFORE (line 180-187)
#[then("I should receive a valid block number response")]
async fn verify_block_number_response(world: &mut IntegrationWorld) {
    assert_eq!(
        world.last_status_code,
        Some(200),
        "Expected 200 OK, got {:?}",
        world.last_status_code
    );
```

**Issue**:
- `last_status_code` is only set for HTTP responses (line 176)
- WebSocket responses never set this field
- Test can assert on stale HTTP state from previous scenarios
- False failure if no previous HTTP call occurred

**Fix - Part 1** (lines 180-192): Made status code check optional

```rust
// AFTER
#[then("I should receive a valid block number response")]
async fn verify_block_number_response(world: &mut IntegrationWorld) {
    // For HTTP calls, check status code
    // For WebSocket calls, last_status_code won't be set - skip the check
    if let Some(status) = world.last_status_code {
        assert_eq!(
            status, 200,
            "Expected 200 OK for HTTP response, got {}",
            status
        );
    }

    let body = world.last_response_body.as_ref().expect("No response body");
```

**Fix - Part 2** (lines 1354-1367): Clear stale HTTP state before WebSocket calls

The optional check alone wasn't sufficient - if a previous HTTP call set `last_status_code = Some(404)`, the WebSocket scenario would still check it and fail on stale state.

```rust
// AFTER
#[when("I send eth_blockNumber over WebSocket")]
async fn send_eth_block_number_ws(world: &mut IntegrationWorld) {
    // Clear old response and status code to ensure we're validating the new one
    // This prevents asserting on stale HTTP state when this is a WebSocket call
    world.last_response_body = None;
    world.last_status_code = None;  // ← Added this line

    client_sends_eth_block_number(world).await;

    // Wait briefly and receive the response
    tokio::time::sleep(Duration::from_millis(100)).await;
    client_receives_response_within(world, 5).await;
}
```

**Impact**:
- ✅ Step now works for both HTTP and WebSocket responses
- ✅ No false failures from stale HTTP state (status code cleared before WebSocket calls)
- ✅ Validates response body regardless of transport

---

### Finding 2 (Medium): receive_block_number_response_with_id doesn't skip subscription notifications

**Location**: `tests/steps/integration_steps.rs:1535`

**Problem**:

The step reads exactly one WebSocket message without filtering:
```rust
// BEFORE (lines 1543-1572)
match tokio::time::timeout(Duration::from_secs(5), conn.receiver.next()).await {
    Ok(Some(Ok(WsMessage::Text(text)))) => {
        let json: serde_json::Value =
            serde_json::from_str(&text).expect("Response should be valid JSON");

        let id = json.get("id").expect("Response should have 'id' field");
        // ❌ BUG: If a subscription notification arrives first, this panics!
```

**Issue**:
- In multi-subscription scenarios, subscription notifications can arrive at any time
- If notification arrives before RPC response, test panics: `"Response should have 'id' field"`
- Subscription notifications have `"method": "eth_subscription"`, not `"id"`
- Causes flaky test failures depending on message timing

**Fix** (lines 1538-1594):

```rust
// AFTER
#[then(regex = r"^I should receive block number response with RPC ID (\d+)$")]
async fn receive_block_number_response_with_id(world: &mut IntegrationWorld, rpc_id: u64) {
    if world.ws_connection.is_none() {
        panic!("WebSocket not connected - cannot receive response");
    }

    let conn = world.ws_connection.as_mut().unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);

    // Loop through messages, skipping subscription notifications
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("Timeout waiting for response with RPC ID {}", rpc_id);
        }

        match tokio::time::timeout(remaining, conn.receiver.next()).await {
            Ok(Some(Ok(WsMessage::Text(text)))) => {
                let json: serde_json::Value =
                    serde_json::from_str(&text).expect("Response should be valid JSON");

                // Skip subscription notifications
                if json.get("method").and_then(|m| m.as_str()) == Some("eth_subscription") {
                    eprintln!("  (skipping subscription notification while waiting for RPC ID {})", rpc_id);
                    continue;
                }

                // This should be an RPC response - verify ID
                let id = json.get("id").expect("Response should have 'id' field");

                assert_eq!(
                    id.as_u64(),
                    Some(rpc_id),
                    "Response should have correct RPC ID {}. Got ID: {}",
                    rpc_id,
                    id
                );

                world.last_response_body = Some(text.to_string());
                eprintln!("✓ Received response with correct RPC ID {rpc_id}");
                break;
            }
            // ... error handling
        }
    }
}
```

**Key Changes**:
1. Loop instead of single read
2. Check for `"method": "eth_subscription"` and skip
3. Continue reading until RPC response found or timeout
4. Timeout deadline respects total time limit

**Impact**:
- ✅ Handles interleaved subscription notifications correctly
- ✅ No false failures when notifications arrive first
- ✅ Test passes reliably regardless of message ordering

---

### Finding 3 (Low): receive_confirmation_for_both doesn't filter non-confirmation messages

**Location**: `tests/steps/integration_steps.rs:1444`

**Problem**:

The step reads exactly 2 messages without filtering:
```rust
// BEFORE (lines 1453-1471)
let mut confirmations = 0;

// Wait for 2 confirmation messages
for _ in 0..2 {
    match tokio::time::timeout(Duration::from_secs(5), conn.receiver.next()).await {
        Ok(Some(Ok(WsMessage::Text(text)))) => {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(result) = json.get("result") {
                    if result.is_string() {
                        confirmations += 1;
                        // ❌ BUG: Only reads 2 messages total!
```

**Issue**:
- Loop runs exactly 2 times, reading 2 messages
- If first message is a subscription notification (not confirmation), confirmations = 1
- Loop exits after 2 messages even if both confirmations arrive later
- False failure: `"Should receive confirmation for both subscriptions, got 1 confirmations"`

**Example Failure Scenario**:
1. Send two subscription requests (IDs 100, 101)
2. Message 1: Subscription notification for existing subscription → not counted
3. Message 2: Confirmation for ID 100 → confirmations = 1
4. **Loop exits** (2 messages read)
5. Message 3 (never read): Confirmation for ID 101
6. **Test fails**: `got 1 confirmations` ❌

**Fix** (lines 1446-1510):

```rust
// AFTER
#[then("I receive confirmation for both subscriptions")]
async fn receive_confirmation_for_both(world: &mut IntegrationWorld) {
    if world.ws_connection.is_none() {
        eprintln!("⚠ Skipping - WebSocket not connected");
        return;
    }

    let conn = world.ws_connection.as_mut().unwrap();
    let mut confirmations = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);

    // Keep reading messages until we get 2 confirmations or timeout
    while confirmations < 2 {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!(
                "Timeout waiting for subscription confirmations, got {} confirmations",
                confirmations
            );
        }

        match tokio::time::timeout(remaining, conn.receiver.next()).await {
            Ok(Some(Ok(WsMessage::Text(text)))) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    // Check if this is a subscription confirmation (has "result" with subscription ID string)
                    // Skip subscription notifications (have "method": "eth_subscription")
                    if json.get("method").is_some() {
                        eprintln!("  (skipping subscription notification while waiting for confirmations)");
                        continue;
                    }

                    if let Some(result) = json.get("result") {
                        if result.is_string() {
                            confirmations += 1;
                            eprintln!("✓ Received subscription confirmation {confirmations}/2");
                        }
                    }
                }
            }
            Ok(Some(Ok(msg))) => {
                eprintln!("  (skipping non-text message: {:?})", msg);
                continue;
            }
            // ... error handling
        }
    }

    assert_eq!(
        confirmations, 2,
        "Should receive confirmation for both subscriptions, got {} confirmations",
        confirmations
    );
    eprintln!("✓ Both subscriptions confirmed");
}
```

**Key Changes**:
1. Loop until `confirmations == 2` (not fixed iteration count)
2. Filter out subscription notifications (`json.get("method").is_some()`)
3. Continue reading until both confirmations found
4. Increased timeout to 10 seconds (can read more messages)
5. Better error messages show how many confirmations received

**Impact**:
- ✅ Handles interleaved subscription notifications
- ✅ Reads as many messages as needed to get 2 confirmations
- ✅ No false failures from message ordering
- ✅ Clear diagnostics showing confirmation progress

---

## Common Pattern

All three fixes follow the same pattern for handling WebSocket message streams:

### Before (Fragile)
```rust
// Read exactly N messages, assume they're what we want
for _ in 0..N {
    match read_message() {
        Ok(msg) => {
            // Process assuming msg is expected type
        }
    }
}
```

**Problem**: If unexpected message type arrives, test fails or counts wrong messages

### After (Robust)
```rust
// Read until we get what we need OR timeout
let deadline = now() + timeout;
while !got_what_we_need {
    let remaining = deadline - now();
    match timeout(remaining, read_message()) {
        Ok(msg) => {
            if msg.is_expected_type() {
                process(msg);
            } else {
                continue;  // Skip unexpected messages
            }
        }
    }
}
```

**Benefits**:
- Filter out unexpected message types
- Read as many messages as needed
- Respect total timeout deadline
- Clear about what we're waiting for

---

## Summary of Changes

### Files Modified
1. `tests/steps/integration_steps.rs` - Fixed 3 test step implementations

### Key Changes

**Lines 180-192**: `verify_block_number_response`
- Made status code check optional (only for HTTP responses)
- WebSocket responses skip status code validation

**Lines 1354-1367**: `send_eth_block_number_ws`
- Clear `last_status_code` before WebSocket calls
- Prevents asserting on stale HTTP status from previous scenarios

**Lines 1538-1594**: `receive_block_number_response_with_id`
- Loop through messages instead of single read
- Skip subscription notifications
- Find RPC response with correct ID

**Lines 1446-1510**: `receive_confirmation_for_both`
- Loop until 2 confirmations found (not fixed iteration count)
- Filter out subscription notifications
- Increased timeout to 10 seconds

## Impact on Test Quality

**Before**:
- ❌ Tests could fail due to message ordering
- ❌ Tests could assert on stale HTTP state
- ❌ Subscription notifications caused false failures
- ❌ Fixed iteration counts missed expected messages

**After**:
- ✅ Tests robust to message ordering
- ✅ HTTP/WebSocket responses handled correctly
- ✅ Subscription notifications filtered properly
- ✅ Tests read until expected messages found

## Test Results

```
Unit Tests:
✅ All 91 unit tests pass
✅ All 16 BDD scenarios pass
✅ Clippy clean
✅ Formatting correct
```

---

**Date**: January 23, 2026
**Review Round**: Seventh (Test Flakiness)
**Reviewer**: Independent code review
**Implementer**: Claude Sonnet 4.5
