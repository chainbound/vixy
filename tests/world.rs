//! Test world state for BDD tests

use cucumber::World;

/// The test world state for Vixy BDD tests (unit tests)
#[derive(Debug, Default, World)]
pub struct VixyWorld {
    /// Raw TOML configuration string for parsing tests
    pub config_toml: Option<String>,
    /// Loaded configuration (if any)
    pub config: Option<vixy::config::Config>,
    /// EL node states for testing
    pub el_nodes: Vec<vixy::state::ElNodeState>,
    /// CL node states for testing
    pub cl_nodes: Vec<vixy::state::ClNodeState>,
    /// Current EL chain head for health check tests
    pub el_chain_head: u64,
    /// Current CL chain head for health check tests
    pub cl_chain_head: u64,
    /// Max EL lag threshold for health check tests
    pub max_el_lag: u64,
    /// Max CL lag threshold for health check tests
    pub max_cl_lag: u64,
    /// The last error message (if any)
    pub last_error: Option<String>,
}

/// The test world state for Vixy integration tests
/// Used for tests that run against real Docker/Kurtosis infrastructure
#[derive(Debug, Default, World)]
pub struct IntegrationWorld {
    /// Vixy server URL (e.g., "http://127.0.0.1:8080")
    pub vixy_url: Option<String>,
    /// Last HTTP status code received
    pub last_status_code: Option<u16>,
    /// Last response body received
    pub last_response_body: Option<String>,
    /// Number of healthy EL nodes
    pub healthy_el_count: usize,
    /// Number of healthy CL nodes
    pub healthy_cl_count: usize,
    /// List of stopped Docker containers (for cleanup)
    pub stopped_containers: Vec<String>,
    /// Whether WebSocket is connected
    pub ws_connected: bool,
}
