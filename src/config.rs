//! Configuration parsing for Vixy
//!
//! Handles TOML config file parsing for EL/CL node definitions and global settings.

use serde::Deserialize;

/// Global configuration settings
#[derive(Debug, Clone, Deserialize)]
pub struct Global {
    /// Maximum allowed block lag for EL nodes before marking as unhealthy
    pub max_el_lag_blocks: u64,
    /// Maximum allowed slot lag for CL nodes before marking as unhealthy
    pub max_cl_lag_slots: u64,
    /// Health check interval in milliseconds
    pub health_check_interval_ms: u64,
}

/// EL (Execution Layer) node configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ElNode {
    /// Human-readable name for the node
    pub name: String,
    /// HTTP URL for JSON-RPC requests
    pub http_url: String,
    /// WebSocket URL for subscriptions
    pub ws_url: String,
}

/// EL node configuration with primary and backup nodes
#[derive(Debug, Clone, Deserialize)]
pub struct El {
    /// Primary EL nodes - used first
    pub primary: Vec<ElNode>,
    /// Backup EL nodes - only used when ALL primary nodes are unavailable
    #[serde(default)]
    pub backup: Vec<ElNode>,
}

/// CL (Consensus Layer) node configuration
#[derive(Debug, Clone, Deserialize)]
pub struct Cl {
    /// Human-readable name for the node
    pub name: String,
    /// Base URL for beacon API requests
    pub url: String,
}

/// Main configuration struct
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Global settings
    pub global: Global,
    /// EL node configuration
    pub el: El,
    /// CL nodes configuration
    pub cl: Vec<Cl>,
}

impl Config {
    /// Load configuration from a file path
    pub fn load(_path: &str) -> eyre::Result<Self> {
        unimplemented!("Config::load not yet implemented")
    }

    /// Parse configuration from a TOML string
    pub fn parse(_s: &str) -> eyre::Result<Self> {
        unimplemented!("Config::parse not yet implemented")
    }
}

#[cfg(test)]
mod tests {
    // Tests will be added in Phase 3
}
