//! Shared state management for Vixy
//!
//! Contains the application state including EL/CL node states and chain head tracking.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use tokio::sync::RwLock;

/// State for an EL (Execution Layer) node
#[derive(Debug, Clone)]
pub struct ElNodeState {
    /// Human-readable name
    pub name: String,
    /// HTTP URL for JSON-RPC
    pub http_url: String,
    /// WebSocket URL
    pub ws_url: String,
    /// Whether this is a primary node (true) or backup (false)
    pub is_primary: bool,
    /// Current block number reported by the node
    pub block_number: u64,
    /// Whether the last health check succeeded (node was reachable)
    pub check_ok: bool,
    /// Whether the node is currently healthy (check_ok AND within lag threshold)
    pub is_healthy: bool,
    /// Current lag from chain head (in blocks)
    pub lag: u64,
    /// Number of consecutive health check failures
    pub consecutive_failures: u32,
}

impl ElNodeState {
    /// Create an ElNodeState from an ElNode config
    pub fn from_config(node: &crate::config::ElNode, is_primary: bool) -> Self {
        Self {
            name: node.name.clone(),
            http_url: node.http_url.clone(),
            ws_url: node.ws_url.clone(),
            is_primary,
            block_number: 0,
            check_ok: false,   // Start with check not ok
            is_healthy: false, // Start unhealthy until health check passes
            lag: 0,
            consecutive_failures: 0,
        }
    }
}

/// State for a CL (Consensus Layer) node
#[derive(Debug, Clone)]
pub struct ClNodeState {
    /// Human-readable name
    pub name: String,
    /// Base URL for beacon API
    pub url: String,
    /// Current slot reported by the node
    pub slot: u64,
    /// Whether the health endpoint returns 200
    pub health_ok: bool,
    /// Whether the node is currently healthy (health_ok AND within lag threshold)
    pub is_healthy: bool,
    /// Current lag from chain head (in slots)
    pub lag: u64,
    /// Number of consecutive health check failures
    pub consecutive_failures: u32,
}

impl ClNodeState {
    /// Create a ClNodeState from a Cl config
    pub fn from_config(node: &crate::config::Cl) -> Self {
        Self {
            name: node.name.clone(),
            url: node.url.clone(),
            slot: 0,
            health_ok: false,  // Start with health not ok
            is_healthy: false, // Start unhealthy until health check passes
            lag: 0,
            consecutive_failures: 0,
        }
    }
}

/// Main application state shared across all handlers
pub struct AppState {
    /// EL node states (both primary and backup)
    pub el_nodes: Arc<RwLock<Vec<ElNodeState>>>,
    /// CL node states
    pub cl_nodes: Arc<RwLock<Vec<ClNodeState>>>,
    /// Current EL chain head (highest block number seen)
    pub el_chain_head: AtomicU64,
    /// Current CL chain head (highest slot seen)
    pub cl_chain_head: AtomicU64,
    /// Whether we're in failover mode (using backup EL nodes)
    pub el_failover_active: AtomicBool,
    /// Maximum allowed EL lag in blocks
    pub max_el_lag: u64,
    /// Maximum allowed CL lag in slots
    pub max_cl_lag: u64,
    /// Proxy request timeout in milliseconds
    pub proxy_timeout_ms: u64,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Number of consecutive health check failures before marking node as unhealthy
    pub health_check_max_failures: u32,
    /// Maximum request body size in bytes
    pub max_body_size: usize,
    /// Shared HTTP client for proxy requests (reuses connections)
    pub http_client: reqwest::Client,
}

impl AppState {
    /// Create a new AppState from configuration
    pub fn new(config: &crate::config::Config) -> Self {
        // Create EL node states - primary nodes first, then backup
        let mut el_nodes = Vec::new();

        // Add primary EL nodes first
        for node in &config.el.primary {
            el_nodes.push(ElNodeState::from_config(node, true));
        }

        // Add backup EL nodes
        for node in &config.el.backup {
            el_nodes.push(ElNodeState::from_config(node, false));
        }

        // Create CL node states
        let cl_nodes: Vec<ClNodeState> = config.cl.iter().map(ClNodeState::from_config).collect();

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(
                config.global.proxy_timeout_ms,
            ))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            el_nodes: Arc::new(RwLock::new(el_nodes)),
            cl_nodes: Arc::new(RwLock::new(cl_nodes)),
            el_chain_head: AtomicU64::new(0),
            cl_chain_head: AtomicU64::new(0),
            el_failover_active: AtomicBool::new(false),
            max_el_lag: config.global.max_el_lag_blocks,
            max_cl_lag: config.global.max_cl_lag_slots,
            proxy_timeout_ms: config.global.proxy_timeout_ms,
            max_retries: config.global.max_retries,
            health_check_max_failures: config.global.health_check_max_failures,
            max_body_size: config.global.max_body_size,
            http_client,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn sample_config() -> crate::config::Config {
        let toml_str = r#"
[global]
max_el_lag_blocks = 5
max_cl_lag_slots = 3
health_check_interval_ms = 1000

[el]
[[el.primary]]
name = "geth-1"
http_url = "http://localhost:8545"
ws_url = "ws://localhost:8546"

[[el.primary]]
name = "geth-2"
http_url = "http://localhost:8547"
ws_url = "ws://localhost:8548"

[[el.backup]]
name = "alchemy-1"
http_url = "https://eth-mainnet.g.alchemy.com/v2/xxx"
ws_url = "wss://eth-mainnet.g.alchemy.com/v2/xxx"

[[cl]]
name = "lighthouse-1"
url = "http://localhost:5052"

[[cl]]
name = "prysm-1"
url = "http://localhost:5053"
"#;
        crate::config::Config::parse(toml_str).expect("Sample config should parse")
    }

    #[test]
    fn test_el_node_state_from_config() {
        let config = sample_config();
        let el_node = &config.el.primary[0];

        let state = ElNodeState::from_config(el_node, true);

        assert_eq!(state.name, "geth-1");
        assert_eq!(state.http_url, "http://localhost:8545");
        assert_eq!(state.ws_url, "ws://localhost:8546");
        assert!(state.is_primary);
        assert_eq!(state.block_number, 0);
        assert!(!state.is_healthy); // Initially unhealthy
        assert_eq!(state.lag, 0);
    }

    #[test]
    fn test_el_node_state_backup() {
        let config = sample_config();
        let backup_node = &config.el.backup[0];

        let state = ElNodeState::from_config(backup_node, false);

        assert_eq!(state.name, "alchemy-1");
        assert!(!state.is_primary); // Should be backup
    }

    #[test]
    fn test_cl_node_state_from_config() {
        let config = sample_config();
        let cl_node = &config.cl[0];

        let state = ClNodeState::from_config(cl_node);

        assert_eq!(state.name, "lighthouse-1");
        assert_eq!(state.url, "http://localhost:5052");
        assert_eq!(state.slot, 0);
        assert!(!state.health_ok); // Initially not ok
        assert!(!state.is_healthy); // Initially unhealthy
        assert_eq!(state.lag, 0);
    }

    #[tokio::test]
    async fn test_app_state_initialization() {
        let config = sample_config();
        let state = AppState::new(&config);

        // Check EL nodes
        let el_nodes = state.el_nodes.read().await;
        assert_eq!(el_nodes.len(), 3); // 2 primary + 1 backup

        // Check CL nodes
        let cl_nodes = state.cl_nodes.read().await;
        assert_eq!(cl_nodes.len(), 2);

        // Check chain heads start at 0
        assert_eq!(state.el_chain_head.load(Ordering::SeqCst), 0);
        assert_eq!(state.cl_chain_head.load(Ordering::SeqCst), 0);

        // Check failover starts as inactive
        assert!(!state.el_failover_active.load(Ordering::SeqCst));

        // Check max lag values from config
        assert_eq!(state.max_el_lag, 5);
        assert_eq!(state.max_cl_lag, 3);
    }

    #[tokio::test]
    async fn test_initial_health_is_false() {
        let config = sample_config();
        let state = AppState::new(&config);

        // All EL nodes should start unhealthy
        let el_nodes = state.el_nodes.read().await;
        for node in el_nodes.iter() {
            assert!(
                !node.is_healthy,
                "EL node {} should start unhealthy",
                node.name
            );
        }

        // All CL nodes should start unhealthy
        let cl_nodes = state.cl_nodes.read().await;
        for node in cl_nodes.iter() {
            assert!(
                !node.is_healthy,
                "CL node {} should start unhealthy",
                node.name
            );
            assert!(
                !node.health_ok,
                "CL node {} should start with health_ok=false",
                node.name
            );
        }
    }

    #[tokio::test]
    async fn test_primary_nodes_ordered_before_backup() {
        let config = sample_config();
        let state = AppState::new(&config);

        let el_nodes = state.el_nodes.read().await;

        // Primary nodes should come first
        assert!(el_nodes[0].is_primary, "First node should be primary");
        assert!(el_nodes[1].is_primary, "Second node should be primary");
        assert!(!el_nodes[2].is_primary, "Third node should be backup");
    }
}
