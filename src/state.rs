//! Shared state management for Vixy
//!
//! Contains the application state including EL/CL node states and chain head tracking.

use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
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
    /// Whether the node is currently healthy
    pub is_healthy: bool,
    /// Current lag from chain head (in blocks)
    pub lag: u64,
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
}

/// Main application state shared across all handlers
#[derive(Debug)]
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
}

impl AppState {
    /// Create a new AppState from configuration
    pub fn new(_config: &crate::config::Config) -> Self {
        unimplemented!("AppState::new not yet implemented")
    }
}

#[cfg(test)]
mod tests {
    // Tests will be added in Phase 4
}
