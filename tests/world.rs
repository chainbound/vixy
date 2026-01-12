//! Test world state for BDD tests

use cucumber::World;

/// The test world state for Vixy BDD tests
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
