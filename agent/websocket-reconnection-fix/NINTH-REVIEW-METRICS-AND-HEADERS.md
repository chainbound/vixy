# Ninth Review - Metrics Double-Counting and HTTP Header Forwarding

## Overview

This document details fixes for the ninth code review, which identified **2 medium-severity findings** related to metric accuracy and HTTP proxy functionality.

## Findings and Fixes

### Finding 1 (Medium): WebSocket replayed subscriptions increment ws_subscriptions metric

**Location**: `src/proxy/ws.rs:707`

**Problem**:

When subscriptions are replayed after WebSocket reconnection, the code incremented the `ws_subscriptions` metric:

```rust
// BEFORE (line 707)
if is_replay {
    // Map new upstream ID to original client ID
    if let Some(original_id) = original_client_sub_id {
        tracker.lock().await.map_upstream_id(sub_id, &original_id);
        debug!(...);
    }
    VixyMetrics::inc_ws_subscriptions();  // ❌ BUG: Double counts!
    return Ok(());
}
```

**Issue**:
- Replayed subscriptions are NOT new subscriptions - they're re-establishing existing ones
- Original subscription already incremented the metric when first created
- After each reconnection, metric increases even though number of active subscriptions stays the same
- After N reconnections: metric shows (N+1) × actual subscription count

**Example Failure Scenario**:

1. Client subscribes to `newHeads` → `ws_subscriptions = 1` ✅
2. Reconnection happens, subscription replayed → `ws_subscriptions = 2` ❌ (should still be 1)
3. Another reconnection → `ws_subscriptions = 3` ❌ (should still be 1)
4. **Metric shows 3 subscriptions, actual count is 1**

**Impact on Monitoring**:
- Grafana dashboards show inflated subscription counts
- Alerts based on subscription thresholds trigger incorrectly
- Capacity planning based on wrong metrics
- No way to distinguish replay-inflated counts from real growth

**Fix** (line 707-709):

```rust
// AFTER
if is_replay {
    // Map new upstream ID to original client ID
    if let Some(original_id) = original_client_sub_id {
        tracker.lock().await.map_upstream_id(sub_id, &original_id);
        debug!(
            new_upstream_id = sub_id,
            original_client_id = original_id,
            "Mapped replayed subscription ID (not forwarding response)"
        );
    } else {
        error!("Replayed subscription missing original client ID");
    }
    // Note: Don't increment ws_subscriptions metric for replays
    // The subscription was already counted when originally created
    return Ok(());
}
```

**Key Changes**:
1. **Removed** `VixyMetrics::inc_ws_subscriptions()` call for replays
2. **Added comment** explaining why we don't increment for replays
3. **Normal subscriptions** (line 715) still increment correctly

**Metric Behavior**:

| Event | Before Fix | After Fix |
|-------|-----------|-----------|
| Client subscribes | +1 | +1 |
| Reconnection (replay) | +1 ❌ | +0 ✅ |
| Client unsubscribes | -1 | -1 |

**Impact**:
- ✅ Metric now shows actual number of active subscriptions
- ✅ No double-counting after reconnections
- ✅ Accurate monitoring and alerting
- ✅ Correct capacity planning data

---

### Finding 2 (Medium): HTTP proxy drops all request headers except content-type

**Locations**:
- `src/proxy/http.rs:142-147` - `forward_request` function
- `src/proxy/http.rs:186-191` - `forward_request_to_url` function

**Problem**:

Both HTTP forwarding functions only extracted and forwarded the `content-type` header:

```rust
// BEFORE (lines 142-147)
let method = request.method().clone();
let content_type = request
    .headers()
    .get("content-type")
    .and_then(|v| v.to_str().ok())
    .map(String::from);

// BEFORE (lines 158-161)
let mut forward_request = client.request(method, target_url);
if let Some(ct) = content_type {
    forward_request = forward_request.header("content-type", ct);
}
```

**Issue**:
- All other headers are dropped: `Authorization`, `User-Agent`, `Accept`, custom headers, etc.
- Upstreams that require API keys in headers will fail
- Upstreams that check `User-Agent` will fail
- Custom headers for authentication, tracing, or routing are lost
- Breaks compatibility with many Ethereum node implementations

**Example Failure Scenarios**:

**Scenario 1: API Key Authentication**
```bash
# Client request with API key
curl -H "Authorization: Bearer secret-key" https://vixy/el

# Forwarded to upstream WITHOUT Authorization header
# Upstream rejects: 401 Unauthorized ❌
```

**Scenario 2: Custom Headers**
```bash
# Client request with custom tracing header
curl -H "X-Request-ID: abc123" https://vixy/el

# Forwarded to upstream WITHOUT X-Request-ID
# Distributed tracing broken ❌
```

**Scenario 3: Accept Header**
```bash
# Client request specifying JSON response
curl -H "Accept: application/json" https://vixy/el

# Forwarded WITHOUT Accept header
# Upstream may return XML or other format ❌
```

**Fix** (lines 141-177 in `forward_request`):

```rust
// AFTER
// Extract method and headers
let method = request.method().clone();
let headers = request.headers().clone();

let body_bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
    Ok(bytes) => bytes,
    Err(e) => {
        warn!(error = %e, "Failed to read request body");
        return (StatusCode::BAD_REQUEST, "Failed to read request body").into_response();
    }
};

// Build the forwarded request with all headers except hop-by-hop headers
let mut forward_request = client.request(method, target_url);

// Forward all headers except hop-by-hop headers (RFC 2616)
// Exclude: Connection, Keep-Alive, Proxy-Authenticate, Proxy-Authorization,
//          TE, Trailers, Transfer-Encoding, Upgrade, Host
for (name, value) in headers.iter() {
    let name_str = name.as_str().to_lowercase();
    if !matches!(
        name_str.as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
            | "host"
    ) {
        forward_request = forward_request.header(name, value);
    }
}

forward_request = forward_request.body(body_bytes);
```

**Same fix applied to `forward_request_to_url`** (lines 193-231).

**Key Changes**:
1. **Extract ALL headers** instead of just content-type
2. **Forward all headers** except hop-by-hop headers (RFC 2616)
3. **Exclude hop-by-hop headers**: Connection, Keep-Alive, Proxy-Authenticate, Proxy-Authorization, TE, Trailers, Transfer-Encoding, Upgrade
4. **Exclude Host header**: reqwest sets this correctly based on target URL

**Why Exclude Hop-by-Hop Headers?**

Per [RFC 2616 Section 13.5.1](https://www.rfc-editor.org/rfc/rfc2616#section-13.5.1):
> Hop-by-hop headers are meaningful only for a single transport-level connection and must not be retransmitted by proxies.

**Headers Now Forwarded**:
- ✅ `Authorization` - API keys, bearer tokens
- ✅ `Content-Type` - Request body format
- ✅ `Accept` - Desired response format
- ✅ `User-Agent` - Client identification
- ✅ `X-*` custom headers - Tracing, routing, etc.
- ✅ All other end-to-end headers

**Impact**:
- ✅ API key authentication works
- ✅ Custom headers preserved
- ✅ Distributed tracing works
- ✅ Proper User-Agent forwarding
- ✅ Compliant with HTTP proxy standards (RFC 2616)
- ✅ Compatible with more Ethereum node implementations

---

## Summary of Changes

### Files Modified

1. `src/proxy/ws.rs` - Removed metric increment for replays
   - Line 707: Removed `VixyMetrics::inc_ws_subscriptions()`
   - Lines 707-709: Added explanatory comment

2. `src/proxy/http.rs` - Forward all headers except hop-by-hop
   - Lines 141-177: Updated `forward_request` to forward all headers
   - Lines 193-231: Updated `forward_request_to_url` to forward all headers
   - Both functions now filter hop-by-hop headers per RFC 2616

### Key Improvements

**Metrics**:
- Before: Metric doubled/tripled after reconnections
- After: Metric shows actual active subscription count

**HTTP Proxying**:
- Before: Only content-type header forwarded
- After: All end-to-end headers forwarded (excluding hop-by-hop per RFC 2616)

**Production Impact**:
- ✅ Accurate subscription metrics for monitoring and alerting
- ✅ API key authentication works through proxy
- ✅ Custom headers preserved for tracing and routing
- ✅ Standards-compliant HTTP proxy behavior

---

**Date**: January 23, 2026
**Review Round**: Ninth (Metrics & Headers)
**Reviewer**: Independent code review
**Implementer**: Claude Sonnet 4.5
