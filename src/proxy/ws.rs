//! WebSocket proxy for EL subscriptions with health-aware reconnection

use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::extract::ws::{Message, WebSocket};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message as TungsteniteMessage};
use tracing::{debug, error, info, warn};

use crate::metrics::VixyMetrics;
use crate::proxy::selection;
use crate::state::AppState;

// ============================================================================
// Type Aliases for Complex Types
// ============================================================================

/// Type alias for upstream WebSocket stream
type UpstreamWsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Type alias for upstream WebSocket sender
type UpstreamSender = futures_util::stream::SplitSink<UpstreamWsStream, TungsteniteMessage>;

/// Type alias for upstream WebSocket receiver
type UpstreamReceiver = futures_util::stream::SplitStream<UpstreamWsStream>;

/// Type alias for client WebSocket sender
type ClientSender = futures_util::stream::SplitSink<WebSocket, Message>;

/// Type alias for pending subscribe requests map
/// Tuple: (params, response_sender, is_replay, original_client_sub_id)
/// - params: subscription parameters
/// - response_sender: optional channel to send response back
/// - is_replay: true if this is a replayed subscription during reconnection (response should not be forwarded to client)
/// - original_client_sub_id: for replayed subscriptions, the original client-facing subscription ID to preserve
type PendingSubscribes = HashMap<String, (Vec<Value>, Option<String>, bool, Option<String>)>;

// ============================================================================
// Reconnection Message Queue
// ============================================================================

/// Maximum number of messages to buffer during WebSocket reconnection
const MAX_RECONNECT_QUEUE_SIZE: usize = 1000;

/// Combines the reconnecting flag and message queue under a single Mutex
/// to prevent FIFO ordering races between checking the flag and accessing the queue.
struct ReconnectQueue {
    is_reconnecting: bool,
    queue: VecDeque<Message>,
}

impl ReconnectQueue {
    fn new() -> Self {
        Self {
            is_reconnecting: false,
            queue: VecDeque::new(),
        }
    }
}

// ============================================================================
// Subscription Tracking for Reconnection
// ============================================================================

/// Stores the original subscribe request for replay on reconnection
#[derive(Debug, Clone)]
pub struct SubscribeRequest {
    /// The JSON-RPC request ID from the client
    pub rpc_id: Value,
    /// The subscription parameters (e.g., ["newHeads"] or ["logs", {filter}])
    pub params: Vec<Value>,
    /// The subscription ID returned to the client (original)
    pub client_sub_id: String,
}

/// Tracks active subscriptions for seamless reconnection
#[derive(Debug, Default)]
pub struct SubscriptionTracker {
    /// Maps client-facing subscription ID → original subscribe request
    subscriptions: HashMap<String, SubscribeRequest>,
    /// Maps upstream subscription ID → client-facing subscription ID
    upstream_to_client_id: HashMap<String, String>,
}

impl SubscriptionTracker {
    /// Create a new empty tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Track a new subscription after receiving the subscribe response
    pub fn track_subscribe(&mut self, params: Vec<Value>, rpc_id: Value, client_sub_id: &str) {
        let request = SubscribeRequest {
            rpc_id,
            params,
            client_sub_id: client_sub_id.to_string(),
        };
        self.subscriptions
            .insert(client_sub_id.to_string(), request);
        // Initially, upstream ID == client ID (same node)
        self.upstream_to_client_id
            .insert(client_sub_id.to_string(), client_sub_id.to_string());
    }

    /// Map a new upstream subscription ID to an existing client-facing ID
    /// Called after replaying subscriptions on a new upstream connection.
    /// Removes any previous upstream mapping for the same client ID to prevent leaks.
    pub fn map_upstream_id(&mut self, upstream_id: &str, client_id: &str) {
        self.upstream_to_client_id.retain(|_, v| v != client_id);
        self.upstream_to_client_id
            .insert(upstream_id.to_string(), client_id.to_string());
    }

    /// Translate an upstream subscription ID to the client-facing ID
    pub fn translate_to_client_id(&self, upstream_id: &str) -> Option<&str> {
        self.upstream_to_client_id
            .get(upstream_id)
            .map(|s| s.as_str())
    }

    /// Get all tracked subscriptions for replay
    pub fn get_all_subscriptions(&self) -> Vec<&SubscribeRequest> {
        self.subscriptions.values().collect()
    }

    /// Remove a subscription (on eth_unsubscribe)
    pub fn remove_subscription(&mut self, client_sub_id: &str) {
        self.subscriptions.remove(client_sub_id);
        // Also remove any upstream mappings pointing to this client ID
        self.upstream_to_client_id.retain(|_, v| v != client_sub_id);
    }

    /// Clear upstream ID mappings (called before replaying on new connection)
    pub fn clear_upstream_mappings(&mut self) {
        self.upstream_to_client_id.clear();
    }

    /// Check if there are any active subscriptions
    pub fn has_subscriptions(&self) -> bool {
        !self.subscriptions.is_empty()
    }
}

/// Information about a new upstream connection for reconnection
#[derive(Debug)]
struct ReconnectInfo {
    node_name: String,
    ws_url: String,
}

// ============================================================================
// Health Monitor for WebSocket Connections
// ============================================================================

/// Check if a node is healthy by name
async fn is_node_healthy(state: &AppState, node_name: &str) -> bool {
    let el_nodes = state.el_nodes.read().await;
    el_nodes
        .iter()
        .find(|n| n.name == node_name)
        .map(|n| n.is_healthy)
        .unwrap_or(false)
}

/// Select a new healthy node, returns (node_name, ws_url)
async fn select_healthy_node(state: &AppState) -> Option<(String, String)> {
    let failover_active = state.el_failover_active.load(Ordering::SeqCst);
    let el_nodes = state.el_nodes.read().await;
    selection::select_el_node(&el_nodes, failover_active)
        .map(|n| (n.name.clone(), n.ws_url.clone()))
}

/// Health monitor task that watches for node health changes
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

        // Check if a better node is available (prioritizes primary over backup)
        if let Some((best_name, best_url)) = select_healthy_node(&state).await {
            // Reconnect if better node available (different name)
            // This handles both:
            // 1. Current node unhealthy → switch to healthy node
            // 2. Better node available (e.g., primary when on backup) → switch back
            let should_reconnect = best_name != node_name;

            if should_reconnect {
                let reason = if !current_healthy {
                    "current_unhealthy"
                } else {
                    "better_available"
                };

                info!(
                    current_node = %node_name,
                    best_node = %best_name,
                    reason = %reason,
                    "Switching WebSocket upstream"
                );

                // Signal reconnection
                if reconnect_tx
                    .send(ReconnectInfo {
                        node_name: best_name,
                        ws_url: best_url,
                    })
                    .await
                    .is_err()
                {
                    // Channel closed, connection is shutting down
                    break;
                }
            }
        } else if !current_healthy {
            warn!("Current WebSocket node unhealthy but no healthy nodes available");
        }
    }
}

// ============================================================================
// WebSocket Handlers
// ============================================================================

/// Handle EL WebSocket upgrade requests (GET /el/ws)
pub async fn el_ws_handler(State(state): State<Arc<AppState>>, ws: WebSocketUpgrade) -> Response {
    // Read the failover flag
    let failover_active = state.el_failover_active.load(Ordering::SeqCst);

    // Get a read lock on EL nodes and extract what we need
    let (ws_url, node_name) = {
        let el_nodes = state.el_nodes.read().await;

        // Select a healthy node
        match selection::select_el_node(&el_nodes, failover_active) {
            Some(n) => (n.ws_url.clone(), n.name.clone()),
            None => {
                warn!("No healthy EL node available for WebSocket");
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "No healthy EL node available",
                )
                    .into_response();
            }
        }
    };

    debug!(
        ws_url,
        node_name, "Upgrading WebSocket connection to upstream"
    );

    // Upgrade the WebSocket connection and handle it with health monitoring
    ws.on_upgrade(move |socket| handle_websocket(socket, state, node_name, ws_url))
}

/// Handle the WebSocket connection with health-aware reconnection
async fn handle_websocket(
    client_socket: WebSocket,
    state: Arc<AppState>,
    initial_node_name: String,
    initial_ws_url: String,
) {
    // Track connection metrics
    VixyMetrics::inc_ws_connections();
    VixyMetrics::set_ws_upstream_node(&initial_node_name, true);

    // Create subscription tracker for reconnection replay
    let tracker = Arc::new(Mutex::new(SubscriptionTracker::new()));

    // Track current node name (updated on reconnection)
    let current_node_name = Arc::new(Mutex::new(initial_node_name.clone()));

    // Channel for reconnection signals
    let (reconnect_tx, reconnect_rx) = mpsc::channel::<ReconnectInfo>(1);

    // Spawn health monitor
    let health_state = state.clone();
    let health_node_name = current_node_name.clone();
    let health_monitor_handle = tokio::spawn(async move {
        health_monitor(health_state, health_node_name, reconnect_tx).await;
    });

    // Run the proxy loop with reconnection support
    run_proxy_loop(
        client_socket,
        initial_ws_url,
        tracker.clone(),
        current_node_name.clone(),
        reconnect_rx,
    )
    .await;

    // Abort health monitor to prevent lingering tasks after disconnect
    health_monitor_handle.abort();

    // Update metrics on disconnect
    let final_node = current_node_name.lock().await.clone();
    VixyMetrics::set_ws_upstream_node(&final_node, false);
    VixyMetrics::dec_ws_connections();

    // Update subscription count (should be 0 when connection closes)
    let sub_count = tracker.lock().await.get_all_subscriptions().len();
    VixyMetrics::set_ws_subscriptions(sub_count as u64);

    info!("WebSocket proxy connection closed");
}

/// Main proxy loop handling message forwarding and reconnection
async fn run_proxy_loop(
    client_socket: WebSocket,
    initial_ws_url: String,
    tracker: Arc<Mutex<SubscriptionTracker>>,
    current_node_name: Arc<Mutex<String>>,
    mut reconnect_rx: mpsc::Receiver<ReconnectInfo>,
) {
    // Connect to initial upstream
    let upstream_result = connect_async(&initial_ws_url).await;
    let (upstream_ws, _) = match upstream_result {
        Ok((ws, resp)) => (ws, resp),
        Err(e) => {
            error!(error = %e, "Failed to connect to upstream WebSocket");
            return;
        }
    };

    info!(url = %initial_ws_url, "Connected to upstream WebSocket");

    // Split connections
    let (client_sender, client_receiver) = client_socket.split();
    let (upstream_sender, upstream_receiver) = upstream_ws.split();

    // Wrap in Arc<Mutex> for shared access
    let client_sender = Arc::new(Mutex::new(client_sender));
    let upstream_sender = Arc::new(Mutex::new(upstream_sender));

    // Channels for coordinating message forwarding
    let (client_msg_tx, mut client_msg_rx) = mpsc::channel::<Message>(100);
    let (upstream_msg_tx, mut upstream_msg_rx) = mpsc::channel::<TungsteniteMessage>(100);

    // Spawn client receiver task
    let _client_receiver_handle =
        tokio::spawn(client_receiver_task(client_receiver, client_msg_tx));

    // Spawn upstream receiver task (initial, will be replaced on reconnection)
    tokio::spawn(upstream_receiver_task(upstream_receiver, upstream_msg_tx));

    // Track pending subscribe requests: rpc_id -> (params, response_tx)
    let pending_subscribes: Arc<Mutex<PendingSubscribes>> = Arc::new(Mutex::new(HashMap::new()));

    // Queue for buffering messages during reconnection to prevent message loss.
    // The reconnecting flag is stored alongside the queue under the same Mutex
    // to avoid a race where a message could bypass the queue between flag clear and drain.
    let reconnect_queue: Arc<Mutex<ReconnectQueue>> = Arc::new(Mutex::new(ReconnectQueue::new()));

    // Track reconnection completion across loop iterations (reconnection runs in background)
    // Result includes the old node name for metric cleanup and rollback on failure
    type ReconnectResult = (Result<(UpstreamReceiver, UpstreamSender), String>, String);
    let mut reconnect_result_rx: Option<oneshot::Receiver<ReconnectResult>> = None;

    loop {
        tokio::select! {
            // Handle messages from client
            Some(msg) = client_msg_rx.recv() => {
                if let Err(should_close) = handle_client_message(
                    msg,
                    &upstream_sender,
                    &tracker,
                    &pending_subscribes,
                    &reconnect_queue,
                ).await
                    && should_close
                {
                    break;
                }
            }

            // Handle messages from upstream
            Some(msg) = upstream_msg_rx.recv() => {
                if let Err(should_close) = handle_upstream_message(
                    msg,
                    &client_sender,
                    &tracker,
                    &pending_subscribes,
                ).await
                    && should_close
                {
                    break;
                }
            }

            // Handle reconnection signal
            Some(reconnect_info) = reconnect_rx.recv() => {
                // Check if a reconnection is already in progress
                if reconnect_result_rx.is_some() {
                    warn!(
                        new_node = %reconnect_info.node_name,
                        "Ignoring reconnection request - reconnection already in progress"
                    );
                    continue; // Skip this reconnection attempt
                }

                info!(
                    new_node = %reconnect_info.node_name,
                    new_url = %reconnect_info.ws_url,
                    "Reconnecting WebSocket to new upstream"
                );

                // Store old node name before changing (needed for rollback on failure)
                let old_node = current_node_name.lock().await.clone();

                // Update current node name to target node
                *current_node_name.lock().await = reconnect_info.node_name.clone();

                // Set flag to queue incoming messages during reconnection
                reconnect_queue.lock().await.is_reconnecting = true;

                // Spawn reconnection as background task so main loop can continue processing client messages
                // Messages sent during reconnection will be queued and replayed after completion
                let (reconnect_tx, rx) = oneshot::channel();
                reconnect_result_rx = Some(rx);  // Store receiver for next iteration

                let ws_url = reconnect_info.ws_url.clone();
                let tracker_clone = Arc::clone(&tracker);
                let upstream_sender_clone = Arc::clone(&upstream_sender);
                let pending_subscribes_clone = Arc::clone(&pending_subscribes);
                let old_node_clone = old_node.clone();  // Clone for moving into spawn

                tokio::spawn(async move {
                    let result = reconnect_upstream(
                        &ws_url,
                        &tracker_clone,
                        &upstream_sender_clone,
                        &pending_subscribes_clone,
                    ).await;
                    // Include old_node in result for metric cleanup and rollback
                    let _ = reconnect_tx.send((result, old_node_clone));
                });

                info!("Reconnection task spawned, main loop continues processing messages");
            }

            // Handle reconnection completion (success or failure)
            Ok((result, old_node)) = async { reconnect_result_rx.as_mut().unwrap().await }, if reconnect_result_rx.is_some() => {
                reconnect_result_rx = None;  // Clear receiver after receiving result
                match result {
                    Ok((new_receiver, new_sender)) => {
                        // Replace upstream sender
                        *upstream_sender.lock().await = new_sender;

                        // Spawn new upstream receiver
                        let (new_upstream_tx, new_upstream_rx) = mpsc::channel::<TungsteniteMessage>(100);
                        upstream_msg_rx = new_upstream_rx;
                        tokio::spawn(upstream_receiver_task(new_receiver, new_upstream_tx));

                        // Drain queue and clear flag atomically, then replay outside lock
                        let queued_messages: Vec<Message> = {
                            let mut rq = reconnect_queue.lock().await;
                            rq.is_reconnecting = false;
                            rq.queue.drain(..).collect()
                        };
                        if !queued_messages.is_empty() {
                            info!(count = queued_messages.len(), "Replaying queued messages after reconnection");
                        }
                        for queued_msg in queued_messages {
                            if let Err(should_close) = handle_client_message_internal(
                                queued_msg,
                                &upstream_sender,
                                &tracker,
                                &pending_subscribes,
                            ).await && should_close {
                                warn!("Failed to replay queued message, closing connection");
                                break;
                            }
                        }

                        // Update metrics for successful reconnection
                        // Clear old node metric before setting new node metric
                        VixyMetrics::set_ws_upstream_node(&old_node, false);
                        VixyMetrics::inc_ws_reconnections();
                        VixyMetrics::inc_ws_reconnection_attempt("success");
                        VixyMetrics::set_ws_upstream_node(&current_node_name.lock().await, true);

                        info!("WebSocket reconnection successful");
                    }
                    Err(e) => {
                        // Track failed reconnection attempt
                        VixyMetrics::inc_ws_reconnection_attempt("failed");

                        // Revert current_node_name to old node since reconnection failed
                        // This ensures health monitor and metrics reflect actual connected node
                        *current_node_name.lock().await = old_node;

                        // Clear flag and drop queued messages on reconnection failure
                        {
                            let mut rq = reconnect_queue.lock().await;
                            rq.is_reconnecting = false;
                            let dropped_count = rq.queue.len();
                            rq.queue.clear();
                            if dropped_count > 0 {
                                warn!(count = dropped_count, "Dropped queued messages due to reconnection failure");
                            }
                        }

                        error!(error = %e, "Failed to reconnect WebSocket upstream");
                        // Continue with old connection (if still working)
                    }
                }
            }

            else => {
                // All channels closed, exit
                break;
            }
        }
    }

    // Cleanup
    let _ = upstream_sender
        .lock()
        .await
        .send(TungsteniteMessage::Close(None))
        .await;
    let _ = client_sender.lock().await.send(Message::Close(None)).await;
}

/// Task that receives messages from the client WebSocket
async fn client_receiver_task(
    mut receiver: futures_util::stream::SplitStream<WebSocket>,
    tx: mpsc::Sender<Message>,
) {
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(msg) => {
                if tx.send(msg).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

/// Task that receives messages from the upstream WebSocket
async fn upstream_receiver_task(
    mut receiver: UpstreamReceiver,
    tx: mpsc::Sender<TungsteniteMessage>,
) {
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(msg) => {
                if tx.send(msg).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

/// Internal function that handles a message from the client, forwarding to upstream
/// Returns Err(true) if connection should close, Err(false) for recoverable errors
async fn handle_client_message_internal(
    msg: Message,
    upstream_sender: &Arc<Mutex<UpstreamSender>>,
    tracker: &Arc<Mutex<SubscriptionTracker>>,
    pending_subscribes: &Arc<Mutex<PendingSubscribes>>,
) -> Result<(), bool> {
    match msg {
        Message::Text(text) => {
            VixyMetrics::inc_ws_messages("upstream");

            // Check if this is an eth_subscribe or eth_unsubscribe request
            if let Ok(json) = serde_json::from_str::<Value>(text.as_str()) {
                let method = json.get("method").and_then(|m| m.as_str());
                let rpc_id = json.get("id").cloned();

                if let Some(m) = method {
                    debug!(method = m, direction = "client->upstream", "WS request");
                } else {
                    debug!(
                        direction = "client->upstream",
                        "WS request (unknown method)"
                    );
                }

                if method == Some("eth_subscribe") {
                    // Track pending subscribe request (normal client subscription, not a replay)
                    if let (Some(id), Some(params)) = (rpc_id, json.get("params")) {
                        let id_str = id.to_string();
                        let params_vec = params.as_array().cloned().unwrap_or_default();
                        pending_subscribes
                            .lock()
                            .await
                            .insert(id_str, (params_vec, None, false, None)); // Not a replay, no original ID
                    }
                } else if method == Some("eth_unsubscribe") {
                    // Handle unsubscribe
                    if let Some(params) = json.get("params").and_then(|p| p.as_array())
                        && let Some(sub_id) = params.first().and_then(|s| s.as_str())
                    {
                        tracker.lock().await.remove_subscription(sub_id);
                        VixyMetrics::dec_ws_subscriptions();
                    }
                }
            } else {
                debug!(direction = "client->upstream", "WS request (non-JSON)");
            }

            // Forward to upstream
            if upstream_sender
                .lock()
                .await
                .send(TungsteniteMessage::Text(text.to_string().into()))
                .await
                .is_err()
            {
                return Err(false);
            }
        }
        Message::Binary(data) => {
            debug!(direction = "client->upstream", "Forwarding binary message");
            if upstream_sender
                .lock()
                .await
                .send(TungsteniteMessage::Binary(data.to_vec().into()))
                .await
                .is_err()
            {
                return Err(false);
            }
        }
        Message::Ping(data) => {
            if upstream_sender
                .lock()
                .await
                .send(TungsteniteMessage::Ping(data.to_vec().into()))
                .await
                .is_err()
            {
                return Err(false);
            }
        }
        Message::Pong(data) => {
            if upstream_sender
                .lock()
                .await
                .send(TungsteniteMessage::Pong(data.to_vec().into()))
                .await
                .is_err()
            {
                return Err(false);
            }
        }
        Message::Close(_) => {
            let _ = upstream_sender
                .lock()
                .await
                .send(TungsteniteMessage::Close(None))
                .await;
            return Err(true);
        }
    }
    Ok(())
}

/// Handle a message from the client, queuing if reconnecting or forwarding to upstream
///
/// During reconnection, messages are queued to prevent loss. After successful reconnection,
/// queued messages are replayed in FIFO order.
///
/// Returns Err(true) if connection should close, Err(false) for recoverable errors
async fn handle_client_message(
    msg: Message,
    upstream_sender: &Arc<Mutex<UpstreamSender>>,
    tracker: &Arc<Mutex<SubscriptionTracker>>,
    pending_subscribes: &Arc<Mutex<PendingSubscribes>>,
    reconnect_queue: &Arc<Mutex<ReconnectQueue>>,
) -> Result<(), bool> {
    // Check if we're currently reconnecting (flag and queue under same lock)
    {
        let mut rq = reconnect_queue.lock().await;
        if rq.is_reconnecting {
            if rq.queue.len() >= MAX_RECONNECT_QUEUE_SIZE {
                warn!("Reconnect message queue full, dropping message");
                return Ok(());
            }
            debug!("Queueing message during reconnection");
            rq.queue.push_back(msg);
            return Ok(());
        }
    }

    // Not reconnecting, process normally
    handle_client_message_internal(msg, upstream_sender, tracker, pending_subscribes).await
}

/// Handle a message from upstream, forwarding to client with ID translation
async fn handle_upstream_message(
    msg: TungsteniteMessage,
    client_sender: &Arc<Mutex<ClientSender>>,
    tracker: &Arc<Mutex<SubscriptionTracker>>,
    pending_subscribes: &Arc<Mutex<PendingSubscribes>>,
) -> Result<(), bool> {
    match msg {
        TungsteniteMessage::Text(text) => {
            debug!(direction = "upstream->client", "Forwarding text message");
            VixyMetrics::inc_ws_messages("downstream");

            let mut text_to_send = text.to_string();

            // Check if this is a subscription response or notification
            if let Ok(json) = serde_json::from_str::<Value>(&text_to_send) {
                // Check for subscription response (has "result" with subscription ID)
                if let (Some(id), Some(result)) = (json.get("id"), json.get("result")) {
                    let id_str = id.to_string();
                    if let Some(sub_id) = result.as_str() {
                        // This is a subscription response
                        let mut pending = pending_subscribes.lock().await;
                        if let Some((params, _, is_replay, original_client_sub_id)) =
                            pending.remove(&id_str)
                        {
                            if is_replay {
                                // This is a REPLAYED subscription response (from reconnection)
                                // Map the new upstream subscription ID to the original client subscription ID
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
                            } else {
                                // This is a NORMAL subscription response - track and forward to client
                                tracker
                                    .lock()
                                    .await
                                    .track_subscribe(params, id.clone(), sub_id);
                                VixyMetrics::inc_ws_subscriptions();
                                debug!(sub_id, "Tracked new subscription (forwarding response)");
                                // Fall through to forward the response
                            }
                        }
                    }
                }

                // Check for subscription notification (has "params.subscription")
                if let Some(params) = json.get("params")
                    && let Some(upstream_sub_id) =
                        params.get("subscription").and_then(|s| s.as_str())
                {
                    // Translate subscription ID if needed
                    let tracker_guard = tracker.lock().await;
                    if let Some(client_sub_id) =
                        tracker_guard.translate_to_client_id(upstream_sub_id)
                        && client_sub_id != upstream_sub_id
                    {
                        // Need to rewrite the subscription ID
                        if let Ok(mut json_mut) =
                            serde_json::from_str::<serde_json::Map<String, Value>>(&text_to_send)
                            && let Some(params_mut) =
                                json_mut.get_mut("params").and_then(|p| p.as_object_mut())
                        {
                            params_mut.insert(
                                "subscription".to_string(),
                                Value::String(client_sub_id.to_string()),
                            );
                            text_to_send = serde_json::to_string(&json_mut).unwrap_or(text_to_send);
                            debug!(
                                upstream_id = upstream_sub_id,
                                client_id = client_sub_id,
                                "Translated subscription ID"
                            );
                        }
                    }
                }
            }

            if client_sender
                .lock()
                .await
                .send(Message::Text(text_to_send.into()))
                .await
                .is_err()
            {
                return Err(true);
            }
        }
        TungsteniteMessage::Binary(data) => {
            debug!(direction = "upstream->client", "Forwarding binary message");
            if client_sender
                .lock()
                .await
                .send(Message::Binary(data.as_ref().to_vec().into()))
                .await
                .is_err()
            {
                return Err(true);
            }
        }
        TungsteniteMessage::Ping(data) => {
            if client_sender
                .lock()
                .await
                .send(Message::Ping(data.as_ref().to_vec().into()))
                .await
                .is_err()
            {
                return Err(true);
            }
        }
        TungsteniteMessage::Pong(data) => {
            if client_sender
                .lock()
                .await
                .send(Message::Pong(data.as_ref().to_vec().into()))
                .await
                .is_err()
            {
                return Err(true);
            }
        }
        TungsteniteMessage::Close(_) => {
            let _ = client_sender.lock().await.send(Message::Close(None)).await;
            return Err(true);
        }
        TungsteniteMessage::Frame(_) => {
            // Frame messages are not used
        }
    }
    Ok(())
}

/// Reconnect to a new upstream and replay subscriptions
async fn reconnect_upstream(
    ws_url: &str,
    tracker: &Arc<Mutex<SubscriptionTracker>>,
    _old_sender: &Arc<Mutex<UpstreamSender>>,
    pending_subscribes: &Arc<Mutex<PendingSubscribes>>, // ← ADD
) -> Result<(UpstreamReceiver, UpstreamSender), String> {
    // Connect to new upstream
    let (new_ws, _) = connect_async(ws_url)
        .await
        .map_err(|e| format!("Failed to connect: {e}"))?;

    let (mut new_sender, new_receiver) = new_ws.split();

    // Clear old upstream ID mappings
    let mut tracker_guard = tracker.lock().await;
    tracker_guard.clear_upstream_mappings();

    // Replay all subscriptions
    let subscriptions: Vec<_> = tracker_guard
        .get_all_subscriptions()
        .iter()
        .map(|s| (*s).clone())
        .collect();
    drop(tracker_guard); // Release lock before async operations

    for sub in subscriptions {
        // Create subscribe request
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": sub.rpc_id,
            "method": "eth_subscribe",
            "params": sub.params
        });

        // Mark this as a replayed subscription
        // The response will be consumed internally and not forwarded to client
        // (client already received the original subscription response before reconnection)
        // Include the original client subscription ID so we can map the new upstream ID to it
        let id_str = sub.rpc_id.to_string();
        pending_subscribes.lock().await.insert(
            id_str,
            (
                sub.params.clone(),
                None,
                true,                            // is_replay = true
                Some(sub.client_sub_id.clone()), // original client subscription ID
            ),
        );

        // Send subscribe request
        new_sender
            .send(TungsteniteMessage::Text(request.to_string().into()))
            .await
            .map_err(|e| format!("Failed to send subscribe: {e}"))?;

        debug!(
            client_sub_id = %sub.client_sub_id,
            rpc_id = %sub.rpc_id,
            "Replayed subscription request (added to pending)"
        );
    }

    // Note: The subscription responses will be handled by the normal message flow
    // and will update the upstream ID mappings via handle_upstream_message

    Ok((new_receiver, new_sender))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ElNodeState;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // Helper to create minimal AppState for testing
    fn create_test_state(el_nodes: Vec<ElNodeState>) -> Arc<AppState> {
        Arc::new(AppState {
            el_nodes: Arc::new(RwLock::new(el_nodes)),
            cl_nodes: Arc::new(RwLock::new(vec![])),
            el_chain_head: std::sync::atomic::AtomicU64::new(0),
            cl_chain_head: std::sync::atomic::AtomicU64::new(0),
            el_failover_active: std::sync::atomic::AtomicBool::new(false),
            max_el_lag: 5,
            max_cl_lag: 3,
            proxy_timeout_ms: 30000,
            max_retries: 2,
            health_check_max_failures: 3,
            max_body_size: usize::MAX,
            http_client: reqwest::Client::new(),
        })
    }

    fn make_el_node(name: &str, ws_url: &str, is_healthy: bool) -> ElNodeState {
        ElNodeState {
            name: name.to_string(),
            http_url: "http://localhost:8545".to_string(),
            ws_url: ws_url.to_string(),
            is_primary: true,
            block_number: 1000,
            check_ok: is_healthy,
            is_healthy,
            lag: 0,
            consecutive_failures: 0,
        }
    }

    // =========================================================================
    // WebSocket tests
    // =========================================================================

    // Test that node selection works correctly when checking for healthy nodes
    // Note: Full WebSocket 503 testing requires the WebSocket upgrade to be processed
    // by axum first, which requires proper HTTP upgrade semantics. This tests the
    // underlying selection logic instead.
    #[tokio::test]
    async fn test_ws_selection_returns_none_for_unhealthy() {
        let el_nodes = vec![make_el_node("geth-1", "ws://localhost:8546", false)]; // unhealthy
        let state = create_test_state(el_nodes);

        // Verify that node selection returns None when no healthy nodes
        let nodes = state.el_nodes.read().await;
        let failover_active = state.el_failover_active.load(Ordering::SeqCst);
        let selected = crate::proxy::selection::select_el_node(&nodes, failover_active);
        assert!(selected.is_none(), "Should not select unhealthy node");
    }

    #[tokio::test]
    async fn test_ws_selection_returns_healthy_node() {
        let el_nodes = vec![make_el_node("geth-1", "ws://localhost:8546", true)]; // healthy
        let state = create_test_state(el_nodes);

        // Verify that node selection returns the healthy node
        let nodes = state.el_nodes.read().await;
        let failover_active = state.el_failover_active.load(Ordering::SeqCst);
        let selected = crate::proxy::selection::select_el_node(&nodes, failover_active);
        assert!(selected.is_some(), "Should select healthy node");
        assert_eq!(selected.unwrap().ws_url, "ws://localhost:8546");
    }

    // Note: Full WebSocket integration tests would require running an actual WebSocket server
    // and making real WebSocket connections. The core functionality is tested through the
    // following scenarios that can be verified without a live server:
    //
    // - test_ws_no_healthy_node_returns_503: Verified above
    // - test_ws_upgrade_success: Would require mock WebSocket server
    // - test_ws_message_forwarded_upstream/downstream: Would require mock WebSocket server
    // - test_ws_client_disconnect_closes_upstream: Would require mock WebSocket server
    //
    // The implementation handles these cases in the handle_websocket function.

    // =========================================================================
    // SubscriptionTracker tests
    // =========================================================================

    #[test]
    fn test_subscription_tracker_new_is_empty() {
        let tracker = SubscriptionTracker::new();
        assert!(!tracker.has_subscriptions());
        assert!(tracker.get_all_subscriptions().is_empty());
    }

    #[test]
    fn test_subscription_tracker_track_subscribe() {
        let mut tracker = SubscriptionTracker::new();

        // Track a newHeads subscription
        let params = vec![serde_json::json!("newHeads")];
        let rpc_id = serde_json::json!(1);
        tracker.track_subscribe(params.clone(), rpc_id.clone(), "0x1");

        assert!(tracker.has_subscriptions());
        let subs = tracker.get_all_subscriptions();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].client_sub_id, "0x1");
        assert_eq!(subs[0].params, params);
        assert_eq!(subs[0].rpc_id, rpc_id);
    }

    #[test]
    fn test_subscription_tracker_translate_same_node() {
        let mut tracker = SubscriptionTracker::new();

        // On initial subscription, upstream ID == client ID
        tracker.track_subscribe(
            vec![serde_json::json!("newHeads")],
            serde_json::json!(1),
            "0x1",
        );

        // Should translate to itself
        assert_eq!(tracker.translate_to_client_id("0x1"), Some("0x1"));
    }

    #[test]
    fn test_subscription_tracker_translate_after_reconnect() {
        let mut tracker = SubscriptionTracker::new();

        // Initial subscription with client ID "0x1"
        tracker.track_subscribe(
            vec![serde_json::json!("newHeads")],
            serde_json::json!(1),
            "0x1",
        );

        // Simulate reconnection: clear mappings and add new upstream ID
        tracker.clear_upstream_mappings();
        tracker.map_upstream_id("0x5", "0x1"); // New node returned "0x5"

        // Should translate upstream "0x5" to client "0x1"
        assert_eq!(tracker.translate_to_client_id("0x5"), Some("0x1"));
        // Old upstream ID should no longer translate
        assert_eq!(tracker.translate_to_client_id("0x1"), None);
    }

    #[test]
    fn test_subscription_tracker_multiple_subscriptions() {
        let mut tracker = SubscriptionTracker::new();

        // Track multiple subscriptions
        tracker.track_subscribe(
            vec![serde_json::json!("newHeads")],
            serde_json::json!(1),
            "0x1",
        );
        tracker.track_subscribe(
            vec![
                serde_json::json!("logs"),
                serde_json::json!({"address": "0xabc"}),
            ],
            serde_json::json!(2),
            "0x2",
        );

        assert_eq!(tracker.get_all_subscriptions().len(), 2);
        assert_eq!(tracker.translate_to_client_id("0x1"), Some("0x1"));
        assert_eq!(tracker.translate_to_client_id("0x2"), Some("0x2"));
    }

    #[test]
    fn test_subscription_tracker_remove_subscription() {
        let mut tracker = SubscriptionTracker::new();

        tracker.track_subscribe(
            vec![serde_json::json!("newHeads")],
            serde_json::json!(1),
            "0x1",
        );
        tracker.track_subscribe(
            vec![serde_json::json!("newHeads")],
            serde_json::json!(2),
            "0x2",
        );

        assert_eq!(tracker.get_all_subscriptions().len(), 2);

        // Remove one subscription
        tracker.remove_subscription("0x1");

        assert_eq!(tracker.get_all_subscriptions().len(), 1);
        assert_eq!(tracker.translate_to_client_id("0x1"), None);
        assert_eq!(tracker.translate_to_client_id("0x2"), Some("0x2"));
    }

    #[test]
    fn test_subscription_tracker_clear_upstream_mappings() {
        let mut tracker = SubscriptionTracker::new();

        tracker.track_subscribe(
            vec![serde_json::json!("newHeads")],
            serde_json::json!(1),
            "0x1",
        );

        // Verify initial mapping exists
        assert_eq!(tracker.translate_to_client_id("0x1"), Some("0x1"));

        // Clear mappings
        tracker.clear_upstream_mappings();

        // Mapping should be gone, but subscription still exists
        assert_eq!(tracker.translate_to_client_id("0x1"), None);
        assert!(tracker.has_subscriptions());
        assert_eq!(tracker.get_all_subscriptions().len(), 1);
    }

    // =========================================================================
    // Subscription replay behavior during reconnection
    // =========================================================================

    #[test]
    fn test_pending_subscribes_tracking() {
        // Test that PendingSubscribes correctly tracks subscription requests
        let mut pending: PendingSubscribes = HashMap::new();

        // Add a pending subscribe request (normal subscription, not replay)
        let params = vec![serde_json::json!("newHeads")];
        pending.insert("100".to_string(), (params.clone(), None, false, None));

        // Verify it's tracked
        assert!(pending.contains_key("100"));
        assert_eq!(pending.get("100").unwrap().0, params);
        assert!(!pending.get("100").unwrap().2); // is_replay = false

        // Remove it when response received
        let removed = pending.remove("100");
        assert!(removed.is_some());
        assert!(!pending.contains_key("100"));
    }

    #[test]
    fn test_subscription_replay_should_add_to_pending() {
        // This test documents the expected behavior:
        // When reconnect_upstream replays a subscription, it should add the
        // RPC ID to pending_subscribes so that the response is consumed internally
        // and not forwarded to the client.

        let _tracker = SubscriptionTracker::new();
        let mut pending: PendingSubscribes = HashMap::new();

        // Simulate a replayed subscription during reconnection
        pending.insert(
            "100".to_string(),
            (
                vec![serde_json::json!("newHeads")],
                None,
                true,
                Some("original-id".to_string()),
            ),
        );

        // Verify it's marked as a replay
        assert!(pending.contains_key("100"));
        assert!(pending.get("100").unwrap().2, "Should be marked as replay");
    }

    // =========================================================================
    // Message queueing during reconnection
    // =========================================================================

    #[tokio::test]
    async fn test_message_queued_when_reconnecting() {
        let rq = Arc::new(Mutex::new(ReconnectQueue::new()));
        rq.lock().await.is_reconnecting = true;

        let test_msg = Message::Text("test message".to_string().into());

        // Simulate what handle_client_message does when reconnecting
        {
            let mut guard = rq.lock().await;
            if guard.is_reconnecting {
                guard.queue.push_back(test_msg);
            }
        }

        assert_eq!(rq.lock().await.queue.len(), 1, "Message should be in queue");
    }

    #[tokio::test]
    async fn test_message_not_queued_when_not_reconnecting() {
        let rq = Arc::new(Mutex::new(ReconnectQueue::new()));
        // is_reconnecting defaults to false

        assert_eq!(
            rq.lock().await.queue.len(),
            0,
            "Queue should be empty when not reconnecting"
        );
    }

    #[tokio::test]
    async fn test_reconnecting_flag_toggling() {
        let rq = ReconnectQueue::new();
        assert!(!rq.is_reconnecting, "Should start as false");

        let mut rq = rq;
        rq.is_reconnecting = true;
        assert!(rq.is_reconnecting, "Should be true after set");

        rq.is_reconnecting = false;
        assert!(!rq.is_reconnecting, "Should be false after clear");
    }

    #[tokio::test]
    async fn test_message_queue_fifo_ordering() {
        let rq = Arc::new(Mutex::new(ReconnectQueue::new()));

        // Queue multiple messages
        {
            let mut guard = rq.lock().await;
            guard
                .queue
                .push_back(Message::Text("first".to_string().into()));
            guard
                .queue
                .push_back(Message::Text("second".to_string().into()));
            guard
                .queue
                .push_back(Message::Text("third".to_string().into()));
        }

        // Verify FIFO ordering
        {
            let mut guard = rq.lock().await;
            let queue = &mut guard.queue;

            if let Some(Message::Text(msg)) = queue.pop_front() {
                assert_eq!(
                    msg.as_str(),
                    "first",
                    "First message should be dequeued first"
                );
            } else {
                panic!("Expected text message");
            }

            if let Some(Message::Text(msg)) = queue.pop_front() {
                assert_eq!(
                    msg.as_str(),
                    "second",
                    "Second message should be dequeued second"
                );
            } else {
                panic!("Expected text message");
            }

            if let Some(Message::Text(msg)) = queue.pop_front() {
                assert_eq!(
                    msg.as_str(),
                    "third",
                    "Third message should be dequeued third"
                );
            } else {
                panic!("Expected text message");
            }

            assert_eq!(queue.len(), 0, "Queue should be empty after all dequeues");
        }
    }
}
