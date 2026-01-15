//! Prometheus metrics for Vixy using prometric
//!
//! Provides metrics collection and exposition for monitoring Vixy health and performance.

use prometheus::TextEncoder;
use prometric::{Counter, Gauge, Histogram};
use prometric_derive::metrics;
use std::sync::LazyLock;

/// Vixy metrics collector using prometric derive macro
#[metrics(scope = "vixy")]
pub struct VixyMetrics {
    // EL metrics
    /// Total EL requests proxied
    #[metric(rename = "el_requests_total", labels = ["node", "tier"])]
    el_requests: Counter,

    /// EL request duration in seconds
    #[metric(rename = "el_request_duration_seconds", labels = ["node", "tier"])]
    el_request_duration: Histogram,

    /// Current block number for EL node
    #[metric(rename = "el_node_block_number", labels = ["node", "tier"])]
    el_block_number: Gauge,

    /// Block lag for EL node
    #[metric(rename = "el_node_lag_blocks", labels = ["node", "tier"])]
    el_lag: Gauge,

    /// EL node health status (1=healthy, 0=unhealthy)
    #[metric(rename = "el_node_healthy", labels = ["node", "tier"])]
    el_healthy: Gauge,

    /// EL failover active status (1=active, 0=inactive)
    #[metric(rename = "el_failover_active")]
    el_failover_active: Gauge,

    /// Total EL failovers triggered
    #[metric(rename = "el_failovers_total")]
    el_failovers: Counter,

    /// Current EL chain head block number
    #[metric(rename = "el_chain_head")]
    el_chain_head: Gauge,

    /// Number of healthy EL nodes
    #[metric(rename = "el_healthy_nodes")]
    el_healthy_nodes: Gauge,

    // CL metrics
    /// Total CL requests proxied
    #[metric(rename = "cl_requests_total", labels = ["node"])]
    cl_requests: Counter,

    /// CL request duration in seconds
    #[metric(rename = "cl_request_duration_seconds", labels = ["node"])]
    cl_request_duration: Histogram,

    /// Current slot for CL node
    #[metric(rename = "cl_node_slot", labels = ["node"])]
    cl_slot: Gauge,

    /// Slot lag for CL node
    #[metric(rename = "cl_node_lag_slots", labels = ["node"])]
    cl_lag: Gauge,

    /// CL node health status (1=healthy, 0=unhealthy)
    #[metric(rename = "cl_node_healthy", labels = ["node"])]
    cl_healthy: Gauge,

    /// Current CL chain head slot
    #[metric(rename = "cl_chain_head")]
    cl_chain_head: Gauge,

    /// Number of healthy CL nodes
    #[metric(rename = "cl_healthy_nodes")]
    cl_healthy_nodes: Gauge,

    // WebSocket metrics
    /// Active WebSocket connections
    #[metric(rename = "ws_connections_active")]
    ws_connections: Gauge,

    /// Total WebSocket connections established (lifetime)
    #[metric(rename = "ws_connections_total")]
    ws_connections_total: Counter,

    /// Total WebSocket messages
    #[metric(rename = "ws_messages_total", labels = ["direction"])]
    ws_messages: Counter,

    /// Total WebSocket reconnections due to unhealthy upstream
    #[metric(rename = "ws_reconnections_total")]
    ws_reconnections: Counter,

    /// WebSocket reconnection attempts (including failures)
    #[metric(rename = "ws_reconnection_attempts_total", labels = ["status"])]
    ws_reconnection_attempts: Counter,

    /// Active WebSocket subscriptions
    #[metric(rename = "ws_subscriptions_active")]
    ws_subscriptions: Gauge,

    /// Total WebSocket subscriptions created (lifetime)
    #[metric(rename = "ws_subscriptions_total")]
    ws_subscriptions_total: Counter,

    /// Current upstream node for WebSocket connections
    #[metric(rename = "ws_upstream_node", labels = ["node"])]
    ws_upstream_node: Gauge,
}

/// Global metrics instance
pub static METRICS: LazyLock<VixyMetrics> = LazyLock::new(|| VixyMetrics::builder().build());

impl VixyMetrics {
    /// Render metrics in Prometheus text format
    pub fn render() -> String {
        let encoder = TextEncoder::new();
        let metrics = prometheus::default_registry().gather();
        encoder.encode_to_string(&metrics).unwrap_or_default()
    }

    // =========================================================================
    // EL Metrics helpers
    // =========================================================================

    /// Increment EL request counter
    pub fn inc_el_requests(node: &str, tier: &str) {
        METRICS.el_requests(node, tier).inc();
    }

    /// Record EL request duration
    pub fn observe_el_duration(node: &str, tier: &str, duration_secs: f64) {
        METRICS
            .el_request_duration(node, tier)
            .observe(duration_secs);
    }

    /// Set EL node block number
    pub fn set_el_block_number(node: &str, tier: &str, block: u64) {
        METRICS.el_block_number(node, tier).set(block);
    }

    /// Set EL node lag
    pub fn set_el_lag(node: &str, tier: &str, lag: u64) {
        METRICS.el_lag(node, tier).set(lag);
    }

    /// Set EL node health status (1 = healthy, 0 = unhealthy)
    pub fn set_el_healthy(node: &str, tier: &str, healthy: bool) {
        METRICS
            .el_healthy(node, tier)
            .set(if healthy { 1u64 } else { 0u64 });
    }

    /// Set EL failover active status
    pub fn set_el_failover_active(active: bool) {
        METRICS
            .el_failover_active()
            .set(if active { 1u64 } else { 0u64 });
    }

    /// Increment EL failover counter
    pub fn inc_el_failovers() {
        METRICS.el_failovers().inc();
    }

    /// Set EL chain head
    pub fn set_el_chain_head(block: u64) {
        METRICS.el_chain_head().set(block);
    }

    /// Set number of healthy EL nodes
    pub fn set_el_healthy_nodes(count: u64) {
        METRICS.el_healthy_nodes().set(count);
    }

    // =========================================================================
    // CL Metrics helpers
    // =========================================================================

    /// Increment CL request counter
    pub fn inc_cl_requests(node: &str) {
        METRICS.cl_requests(node).inc();
    }

    /// Record CL request duration
    pub fn observe_cl_duration(node: &str, duration_secs: f64) {
        METRICS.cl_request_duration(node).observe(duration_secs);
    }

    /// Set CL node slot
    pub fn set_cl_slot(node: &str, slot: u64) {
        METRICS.cl_slot(node).set(slot);
    }

    /// Set CL node lag
    pub fn set_cl_lag(node: &str, lag: u64) {
        METRICS.cl_lag(node).set(lag);
    }

    /// Set CL node health status (1 = healthy, 0 = unhealthy)
    pub fn set_cl_healthy(node: &str, healthy: bool) {
        METRICS
            .cl_healthy(node)
            .set(if healthy { 1u64 } else { 0u64 });
    }

    /// Set CL chain head
    pub fn set_cl_chain_head(slot: u64) {
        METRICS.cl_chain_head().set(slot);
    }

    /// Set number of healthy CL nodes
    pub fn set_cl_healthy_nodes(count: u64) {
        METRICS.cl_healthy_nodes().set(count);
    }

    // =========================================================================
    // WebSocket Metrics helpers
    // =========================================================================

    /// Increment active WebSocket connections
    pub fn inc_ws_connections() {
        METRICS.ws_connections().inc();
        METRICS.ws_connections_total().inc();
    }

    /// Decrement active WebSocket connections
    pub fn dec_ws_connections() {
        METRICS.ws_connections().dec();
    }

    /// Increment WebSocket message counter
    pub fn inc_ws_messages(direction: &str) {
        METRICS.ws_messages(direction).inc();
    }

    /// Increment WebSocket reconnection counter (successful)
    pub fn inc_ws_reconnections() {
        METRICS.ws_reconnections().inc();
    }

    /// Record WebSocket reconnection attempt
    pub fn inc_ws_reconnection_attempt(status: &str) {
        METRICS.ws_reconnection_attempts(status).inc();
    }

    /// Increment active WebSocket subscriptions
    pub fn inc_ws_subscriptions() {
        METRICS.ws_subscriptions().inc();
        METRICS.ws_subscriptions_total().inc();
    }

    /// Decrement active WebSocket subscriptions
    pub fn dec_ws_subscriptions() {
        METRICS.ws_subscriptions().dec();
    }

    /// Set active subscriptions count directly
    pub fn set_ws_subscriptions(count: u64) {
        METRICS.ws_subscriptions().set(count);
    }

    /// Set current upstream node for WebSocket (1 = connected, 0 = not)
    pub fn set_ws_upstream_node(node: &str, connected: bool) {
        METRICS
            .ws_upstream_node(node)
            .set(if connected { 1u64 } else { 0u64 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        // Just access the metrics to ensure they initialize without panic
        let _ = &*METRICS;
    }

    #[test]
    fn test_metrics_render() {
        // Trigger some metrics
        VixyMetrics::set_el_chain_head(12345);
        VixyMetrics::set_cl_chain_head(67890);

        let output = VixyMetrics::render();

        // Should contain our metric names
        assert!(output.contains("vixy_el_chain_head"));
        assert!(output.contains("vixy_cl_chain_head"));
    }

    #[test]
    fn test_el_request_counter_increments() {
        VixyMetrics::inc_el_requests("test-node", "primary");
        // If we get here without panic, the counter is working
    }

    #[test]
    fn test_gauge_updates() {
        VixyMetrics::set_el_chain_head(1000);
        VixyMetrics::set_cl_chain_head(5000);
        VixyMetrics::set_el_healthy_nodes(3);
        VixyMetrics::set_cl_healthy_nodes(2);
        // If we get here without panic, gauges are working
    }

    #[test]
    fn test_labeled_metrics() {
        VixyMetrics::set_el_block_number("geth-1", "primary", 100);
        VixyMetrics::set_el_lag("geth-1", "primary", 5);
        VixyMetrics::set_el_healthy("geth-1", "primary", true);
        VixyMetrics::set_cl_slot("lighthouse-1", 200);
        VixyMetrics::set_cl_lag("lighthouse-1", 2);
        VixyMetrics::set_cl_healthy("lighthouse-1", true);

        let output = VixyMetrics::render();
        assert!(output.contains("vixy_el_node_block_number"));
        assert!(output.contains("vixy_cl_node_slot"));
    }

    #[test]
    fn test_ws_metrics() {
        VixyMetrics::inc_ws_connections();
        VixyMetrics::inc_ws_messages("upstream");
        VixyMetrics::inc_ws_messages("downstream");
        VixyMetrics::dec_ws_connections();
        // If we get here without panic, WS metrics are working
    }

    #[test]
    fn test_ws_reconnection_metrics() {
        VixyMetrics::inc_ws_reconnections();
        VixyMetrics::inc_ws_reconnection_attempt("success");
        VixyMetrics::inc_ws_reconnection_attempt("failed");
        // If we get here without panic, reconnection metrics are working
    }

    #[test]
    fn test_ws_subscription_metrics() {
        VixyMetrics::inc_ws_subscriptions();
        VixyMetrics::inc_ws_subscriptions();
        VixyMetrics::dec_ws_subscriptions();
        VixyMetrics::set_ws_subscriptions(5);
        VixyMetrics::set_ws_upstream_node("geth-1", true);
        VixyMetrics::set_ws_upstream_node("geth-2", false);
        // If we get here without panic, subscription metrics are working
    }

    #[test]
    fn test_failover_counter() {
        VixyMetrics::inc_el_failovers();
        VixyMetrics::set_el_failover_active(true);
        // If we get here without panic, failover metrics are working
    }
}
