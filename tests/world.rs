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
    /// Mock servers used in tests
    pub mock_servers: Vec<wiremock::MockServer>,
    /// The last selected node name (if any)
    pub selected_node: Option<String>,
    /// The last HTTP response received (if any)
    pub last_response: Option<String>,
    /// The last error message (if any)
    pub last_error: Option<String>,
}
