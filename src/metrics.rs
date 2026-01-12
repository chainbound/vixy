//! Prometheus metrics for Vixy
//!
//! Provides metrics collection and exposition for monitoring Vixy health and performance.

use std::sync::atomic::{AtomicU64, Ordering};

/// Vixy metrics collector
#[derive(Debug, Default)]
pub struct VixyMetrics {
    /// Total EL requests proxied
    pub el_requests_total: AtomicU64,
    /// Total CL requests proxied
    pub cl_requests_total: AtomicU64,
    /// Total EL failovers triggered
    pub el_failovers_total: AtomicU64,
    /// Current EL chain head
    pub el_chain_head: AtomicU64,
    /// Current CL chain head
    pub cl_chain_head: AtomicU64,
    /// Number of healthy EL nodes
    pub el_healthy_nodes: AtomicU64,
    /// Number of healthy CL nodes
    pub cl_healthy_nodes: AtomicU64,
}

impl VixyMetrics {
    /// Create a new VixyMetrics instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment EL request counter
    pub fn inc_el_requests(&self) {
        self.el_requests_total.fetch_add(1, Ordering::SeqCst);
    }

    /// Increment CL request counter
    pub fn inc_cl_requests(&self) {
        self.cl_requests_total.fetch_add(1, Ordering::SeqCst);
    }

    /// Increment failover counter
    pub fn inc_failovers(&self) {
        self.el_failovers_total.fetch_add(1, Ordering::SeqCst);
    }

    /// Update EL chain head gauge
    pub fn set_el_chain_head(&self, value: u64) {
        self.el_chain_head.store(value, Ordering::SeqCst);
    }

    /// Update CL chain head gauge
    pub fn set_cl_chain_head(&self, value: u64) {
        self.cl_chain_head.store(value, Ordering::SeqCst);
    }

    /// Update healthy EL nodes count
    pub fn set_el_healthy_nodes(&self, count: u64) {
        self.el_healthy_nodes.store(count, Ordering::SeqCst);
    }

    /// Update healthy CL nodes count
    pub fn set_cl_healthy_nodes(&self, count: u64) {
        self.cl_healthy_nodes.store(count, Ordering::SeqCst);
    }

    /// Get current metrics as a Prometheus-formatted string
    pub fn render(&self) -> String {
        let mut output = String::new();

        // EL metrics
        output.push_str("# HELP vixy_el_requests_total Total EL requests proxied\n");
        output.push_str("# TYPE vixy_el_requests_total counter\n");
        output.push_str(&format!(
            "vixy_el_requests_total {}\n",
            self.el_requests_total.load(Ordering::SeqCst)
        ));

        output.push_str("# HELP vixy_cl_requests_total Total CL requests proxied\n");
        output.push_str("# TYPE vixy_cl_requests_total counter\n");
        output.push_str(&format!(
            "vixy_cl_requests_total {}\n",
            self.cl_requests_total.load(Ordering::SeqCst)
        ));

        output.push_str("# HELP vixy_el_failovers_total Total EL failovers triggered\n");
        output.push_str("# TYPE vixy_el_failovers_total counter\n");
        output.push_str(&format!(
            "vixy_el_failovers_total {}\n",
            self.el_failovers_total.load(Ordering::SeqCst)
        ));

        output.push_str("# HELP vixy_el_chain_head Current EL chain head block number\n");
        output.push_str("# TYPE vixy_el_chain_head gauge\n");
        output.push_str(&format!(
            "vixy_el_chain_head {}\n",
            self.el_chain_head.load(Ordering::SeqCst)
        ));

        output.push_str("# HELP vixy_cl_chain_head Current CL chain head slot\n");
        output.push_str("# TYPE vixy_cl_chain_head gauge\n");
        output.push_str(&format!(
            "vixy_cl_chain_head {}\n",
            self.cl_chain_head.load(Ordering::SeqCst)
        ));

        output.push_str("# HELP vixy_el_healthy_nodes Number of healthy EL nodes\n");
        output.push_str("# TYPE vixy_el_healthy_nodes gauge\n");
        output.push_str(&format!(
            "vixy_el_healthy_nodes {}\n",
            self.el_healthy_nodes.load(Ordering::SeqCst)
        ));

        output.push_str("# HELP vixy_cl_healthy_nodes Number of healthy CL nodes\n");
        output.push_str("# TYPE vixy_cl_healthy_nodes gauge\n");
        output.push_str(&format!(
            "vixy_cl_healthy_nodes {}\n",
            self.cl_healthy_nodes.load(Ordering::SeqCst)
        ));

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        let metrics = VixyMetrics::new();

        assert_eq!(metrics.el_requests_total.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.cl_requests_total.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.el_failovers_total.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.el_chain_head.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.cl_chain_head.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_el_request_counter_increments() {
        let metrics = VixyMetrics::new();

        assert_eq!(metrics.el_requests_total.load(Ordering::SeqCst), 0);

        metrics.inc_el_requests();
        assert_eq!(metrics.el_requests_total.load(Ordering::SeqCst), 1);

        metrics.inc_el_requests();
        metrics.inc_el_requests();
        assert_eq!(metrics.el_requests_total.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_cl_request_counter_increments() {
        let metrics = VixyMetrics::new();

        metrics.inc_cl_requests();
        metrics.inc_cl_requests();

        assert_eq!(metrics.cl_requests_total.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_gauge_updates() {
        let metrics = VixyMetrics::new();

        metrics.set_el_chain_head(1000);
        assert_eq!(metrics.el_chain_head.load(Ordering::SeqCst), 1000);

        metrics.set_cl_chain_head(5000);
        assert_eq!(metrics.cl_chain_head.load(Ordering::SeqCst), 5000);

        metrics.set_el_healthy_nodes(3);
        assert_eq!(metrics.el_healthy_nodes.load(Ordering::SeqCst), 3);

        metrics.set_cl_healthy_nodes(2);
        assert_eq!(metrics.cl_healthy_nodes.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_metrics_render() {
        let metrics = VixyMetrics::new();
        metrics.inc_el_requests();
        metrics.set_el_chain_head(12345);

        let output = metrics.render();

        assert!(output.contains("vixy_el_requests_total 1"));
        assert!(output.contains("vixy_el_chain_head 12345"));
        assert!(output.contains("# TYPE vixy_el_requests_total counter"));
        assert!(output.contains("# TYPE vixy_el_chain_head gauge"));
    }

    #[test]
    fn test_failover_counter() {
        let metrics = VixyMetrics::new();

        metrics.inc_failovers();
        metrics.inc_failovers();

        assert_eq!(metrics.el_failovers_total.load(Ordering::SeqCst), 2);
    }
}
