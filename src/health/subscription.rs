//! EL `newHeads` subscription liveness probe.
//!
//! Vixy maintains its own `newHeads` subscription to each EL upstream, independent of
//! the per-client WebSocket relay. It records the time of the latest head into shared
//! state so the monitor can detect when a node's subscription stalls (stops advancing)
//! even though HTTP polling still succeeds.
//!
//! This is the failure mode that otherwise goes undetected: an upstream's `newHeads`
//! subscription silently stops delivering while `eth_blockNumber` polling keeps
//! returning fresh heads, so the node looks healthy while every relayed client is
//! served a frozen head.
//!
//! Design notes:
//! - Detection is **time-since-last-head**, not a block-number comparison: a node is
//!   subscription-stale when no head has arrived within `subscription_stall_timeout_ms`.
//!   That timeout is the single, intuitive knob; it does not depend on `max_el_lag`.
//! - The resulting [`crate::state::ElNodeState::subscription_healthy`] flag is consumed
//!   ONLY by WebSocket relay selection ([`crate::proxy::selection::select_el_ws_node`]).
//!   It is never folded into `is_healthy`, so a stalled subscription does not divert
//!   HTTP traffic or flip the failover flag.
//! - The probe's own stall timer keys on the last *head*, not on any frame, so WS-level
//!   keepalive pings do not mask a head stall.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};

use crate::health::el::parse_hex_block_number;
use crate::state::AppState;

/// Base delay between probe (re)connection attempts; grows exponentially per
/// consecutive *connection* failure up to [`PROBE_RECONNECT_BACKOFF_MAX`].
const PROBE_RECONNECT_BACKOFF_BASE: Duration = Duration::from_secs(2);

/// Maximum delay between probe (re)connection attempts.
const PROBE_RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(30);

/// Timeout for establishing the probe WebSocket connection.
const PROBE_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Whether a node's `newHeads` subscription is currently fresh.
///
/// Returns `true` (do not gate) when the feature is disabled or the probe has never
/// delivered a head (`last_head_at` is `None`) — so a probe that has not yet connected
/// cannot, on its own, mark an otherwise-healthy node unhealthy. Once at least one head
/// has been seen, the subscription is fresh iff the most recent head arrived within
/// `stall_timeout`.
pub fn is_subscription_fresh(
    last_head_at: Option<Instant>,
    stall_timeout: Duration,
    enabled: bool,
) -> bool {
    if !enabled {
        return true;
    }
    match last_head_at {
        None => true,
        Some(t) => t.elapsed() <= stall_timeout,
    }
}

/// Spawn one `newHeads` probe task per configured EL node.
///
/// Each probe maintains a persistent subscription and reconnects on stall/error. Returns
/// after fanning out the tasks (they run for the lifetime of the process). No-op when
/// subscription health gating is disabled in config.
pub async fn run_subscription_probes(state: Arc<AppState>) {
    if !state.subscription_health_enabled {
        info!("EL subscription health gating disabled; not starting newHeads probes");
        return;
    }

    // Snapshot the node list under a read lock, then release it. The node set is fixed
    // at startup (AppState::new only ever pushes), so a one-time snapshot is complete.
    let nodes: Vec<(String, String)> = {
        let el_nodes = state.el_nodes.read().await;
        el_nodes
            .iter()
            .map(|n| (n.name.clone(), n.ws_url.clone()))
            .collect()
    };

    info!(count = nodes.len(), "Starting EL newHeads liveness probes");

    for (name, ws_url) in nodes {
        let state = state.clone();
        tokio::spawn(async move {
            probe_node(state, name, ws_url).await;
        });
    }
}

/// Maintain a `newHeads` subscription to a single node, reconnecting forever on
/// stall/error/close. Connection failures back off exponentially (capped); a successful
/// connection resets the backoff.
async fn probe_node(state: Arc<AppState>, name: String, ws_url: String) {
    let stall_timeout = Duration::from_millis(state.subscription_stall_timeout_ms);
    let mut consecutive_connect_failures: u32 = 0;

    loop {
        match tokio::time::timeout(PROBE_CONNECT_TIMEOUT, connect_async(&ws_url)).await {
            Ok(Ok((ws, _))) => {
                consecutive_connect_failures = 0;
                debug!(node = %name, "newHeads probe connected");
                if let Err(e) = run_probe_connection(&state, &name, ws, stall_timeout).await {
                    debug!(node = %name, error = %e, "newHeads probe connection ended");
                }
                // Clean stall/close: reconnect promptly (base backoff).
                tokio::time::sleep(PROBE_RECONNECT_BACKOFF_BASE).await;
            }
            Ok(Err(e)) => {
                consecutive_connect_failures = consecutive_connect_failures.saturating_add(1);
                let backoff = reconnect_backoff(consecutive_connect_failures);
                warn!(node = %name, error = %e, backoff_secs = backoff.as_secs(),
                      "newHeads probe failed to connect, backing off");
                tokio::time::sleep(backoff).await;
            }
            Err(_) => {
                consecutive_connect_failures = consecutive_connect_failures.saturating_add(1);
                let backoff = reconnect_backoff(consecutive_connect_failures);
                warn!(node = %name, timeout_secs = PROBE_CONNECT_TIMEOUT.as_secs(),
                      backoff_secs = backoff.as_secs(),
                      "newHeads probe connection timed out, backing off");
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

/// Exponential backoff with a cap: base * 2^(failures-1), clamped to the max.
fn reconnect_backoff(consecutive_failures: u32) -> Duration {
    let shift = consecutive_failures.saturating_sub(1).min(5);
    PROBE_RECONNECT_BACKOFF_BASE
        .saturating_mul(1u32 << shift)
        .min(PROBE_RECONNECT_BACKOFF_MAX)
}

/// Subscribe to `newHeads` on an established connection and record incoming heads until
/// the subscription stalls (no head within `stall_timeout`), errors, or closes.
///
/// The stall timer keys on the last *head* only — non-head frames (WS keepalive pings,
/// the subscribe ack) do not reset it — so a head stall is detected even on a socket
/// that is otherwise kept alive. Returning drops the connection; the caller reconnects,
/// which re-establishes a fresh subscription (recovering the common case where an
/// upstream's per-subscription delivery stalls but the node is otherwise fine).
async fn run_probe_connection<S>(
    state: &Arc<AppState>,
    name: &str,
    mut ws: S,
    stall_timeout: Duration,
) -> Result<(), String>
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin,
{
    let sub_req =
        r#"{"jsonrpc":"2.0","id":1,"method":"eth_subscribe","params":["newHeads"]}"#.to_string();
    ws.send(Message::Text(sub_req.into()))
        .await
        .map_err(|e| format!("subscribe send failed: {e}"))?;

    debug!(node = %name, "newHeads probe subscribed");

    // Deadline for the next head. Reset ONLY when a head arrives, never on other frames.
    let mut head_deadline = tokio::time::Instant::now() + stall_timeout;

    loop {
        tokio::select! {
            msg = ws.next() => match msg {
                Some(Ok(Message::Text(text))) => {
                    let text = text.as_str();
                    if let Some(block) = parse_new_head_block(text) {
                        head_deadline = tokio::time::Instant::now() + stall_timeout;
                        record_head(state, name, block).await;
                    } else if let Some(err) = jsonrpc_error_message(text) {
                        // Surface a rejected subscription clearly instead of looping silently.
                        warn!(node = %name, error = %err, "newHeads subscription rejected by upstream");
                    }
                }
                // Ping/pong/binary/close etc.: ignore and keep the head deadline running.
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(format!("ws error: {e}")),
                None => return Ok(()),
            },
            _ = tokio::time::sleep_until(head_deadline) => {
                warn!(
                    node = %name,
                    timeout_secs = stall_timeout.as_secs(),
                    "newHeads probe received no head within stall timeout; reconnecting"
                );
                return Ok(());
            }
        }
    }
}

/// Extract the block number from a `newHeads` subscription notification, returning
/// `None` for any other message (subscribe ack, error, other subscription kinds).
fn parse_new_head_block(text: &str) -> Option<u64> {
    let json: Value = serde_json::from_str(text).ok()?;
    if json.get("method")?.as_str()? != "eth_subscription" {
        return None;
    }
    let number = json.get("params")?.get("result")?.get("number")?.as_str()?;
    parse_hex_block_number(number).ok()
}

/// If `text` is a JSON-RPC error response, return its message (for logging).
fn jsonrpc_error_message(text: &str) -> Option<String> {
    let json: Value = serde_json::from_str(text).ok()?;
    let error = json.get("error")?;
    let message = error
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown error");
    Some(message.to_string())
}

/// Record the latest subscription head for a node: store the block number (latest seen,
/// not a running max, so it stays accurate across reorgs/resyncs) and stamp the head
/// time used for freshness.
async fn record_head(state: &Arc<AppState>, name: &str, block: u64) {
    let mut el_nodes = state.el_nodes.write().await;
    if let Some(node) = el_nodes.iter_mut().find(|n| n.name == name) {
        node.sub_block_number = block;
        node.sub_last_head_at = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_subscription_fresh_disabled_is_always_true() {
        // Even a long-stale head is "fresh" when the feature is disabled.
        let stale = Instant::now() - Duration::from_secs(3600);
        assert!(is_subscription_fresh(
            Some(stale),
            Duration::from_secs(30),
            false
        ));
    }

    #[test]
    fn test_is_subscription_fresh_none_is_true() {
        // Probe has never delivered a head → do not gate the node.
        assert!(is_subscription_fresh(None, Duration::from_secs(30), true));
    }

    #[test]
    fn test_is_subscription_fresh_recent_head_is_true() {
        let recent = Instant::now() - Duration::from_secs(5);
        assert!(is_subscription_fresh(
            Some(recent),
            Duration::from_secs(30),
            true
        ));
    }

    #[test]
    fn test_is_subscription_fresh_stale_head_is_false() {
        // No head within the stall window → stale.
        let stale = Instant::now() - Duration::from_secs(120);
        assert!(!is_subscription_fresh(
            Some(stale),
            Duration::from_secs(30),
            true
        ));
    }

    #[test]
    fn test_reconnect_backoff_is_capped_and_grows() {
        assert_eq!(reconnect_backoff(0), PROBE_RECONNECT_BACKOFF_BASE); // shift 0
        assert_eq!(reconnect_backoff(1), PROBE_RECONNECT_BACKOFF_BASE); // shift 0
        assert_eq!(
            reconnect_backoff(2),
            PROBE_RECONNECT_BACKOFF_BASE.saturating_mul(2)
        );
        assert_eq!(
            reconnect_backoff(3),
            PROBE_RECONNECT_BACKOFF_BASE.saturating_mul(4)
        );
        // Large failure counts are clamped to the max.
        assert_eq!(reconnect_backoff(100), PROBE_RECONNECT_BACKOFF_MAX);
    }

    #[test]
    fn test_parse_new_head_block_valid() {
        let frame = r#"{"jsonrpc":"2.0","method":"eth_subscription","params":{"subscription":"0xabc","result":{"number":"0x2cb6c9","hash":"0xdead"}}}"#;
        assert_eq!(parse_new_head_block(frame), Some(0x2cb6c9));
    }

    #[test]
    fn test_parse_new_head_block_ignores_subscribe_ack() {
        let ack = r#"{"jsonrpc":"2.0","id":1,"result":"0xsubid"}"#;
        assert_eq!(parse_new_head_block(ack), None);
    }

    #[test]
    fn test_parse_new_head_block_ignores_other_subscriptions() {
        let logs = r#"{"jsonrpc":"2.0","method":"eth_subscription","params":{"subscription":"0xabc","result":{"address":"0x0","data":"0x"}}}"#;
        assert_eq!(parse_new_head_block(logs), None);
    }

    #[test]
    fn test_parse_new_head_block_invalid_json() {
        assert_eq!(parse_new_head_block("not json"), None);
    }

    #[test]
    fn test_jsonrpc_error_message_extracts_message() {
        let err = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"the method eth_subscribe does not exist"}}"#;
        assert_eq!(
            jsonrpc_error_message(err).as_deref(),
            Some("the method eth_subscribe does not exist")
        );
    }

    #[test]
    fn test_jsonrpc_error_message_none_for_non_error() {
        let ack = r#"{"jsonrpc":"2.0","id":1,"result":"0xsubid"}"#;
        assert_eq!(jsonrpc_error_message(ack), None);
    }
}
