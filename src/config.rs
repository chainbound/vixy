//! Configuration parsing for Vixy
//!
//! Handles TOML config file parsing for EL/CL node definitions and global settings.

use eyre::{Result, WrapErr, eyre};
use serde::Deserialize;

/// Configuration error type
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid URL '{url}': {reason}")]
    InvalidUrl { url: String, reason: String },

    #[error("validation failed: {0}")]
    ValidationFailed(String),
}

/// Global configuration settings
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Global {
    /// Maximum allowed block lag for EL nodes before marking as unhealthy
    pub max_el_lag_blocks: u64,
    /// Maximum allowed slot lag for CL nodes before marking as unhealthy
    pub max_cl_lag_slots: u64,
    /// Health check interval in milliseconds
    pub health_check_interval_ms: u64,
    /// Proxy request timeout in milliseconds
    pub proxy_timeout_ms: u64,
    /// Maximum number of retry attempts for failed proxy requests
    pub max_retries: u32,
    /// Number of consecutive health check failures before marking node as unhealthy
    pub health_check_max_failures: u32,
}

/// Metrics configuration settings
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Metrics {
    /// Whether metrics are enabled
    pub enabled: bool,
    /// Port to serve metrics on (if separate from main server)
    /// If None, metrics are served on the main server at /metrics
    pub port: Option<u16>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            enabled: true,
            port: None,
        }
    }
}

impl Default for Global {
    fn default() -> Self {
        Self {
            max_el_lag_blocks: 5,
            max_cl_lag_slots: 3,
            health_check_interval_ms: 1000,
            proxy_timeout_ms: 30000,
            max_retries: 2,
            health_check_max_failures: 3,
        }
    }
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

impl ElNode {
    /// Validate the EL node configuration
    fn validate(&self) -> Result<()> {
        validate_url(&self.http_url, "http_url")?;
        validate_url(&self.ws_url, "ws_url")?;
        Ok(())
    }
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

impl El {
    /// Validate the EL configuration
    fn validate(&self) -> Result<()> {
        if self.primary.is_empty() {
            return Err(ConfigError::ValidationFailed(
                "at least one primary EL node is required".to_string(),
            )
            .into());
        }

        for node in &self.primary {
            node.validate()
                .wrap_err_with(|| format!("invalid primary EL node '{}'", node.name))?;
        }

        for node in &self.backup {
            node.validate()
                .wrap_err_with(|| format!("invalid backup EL node '{}'", node.name))?;
        }

        Ok(())
    }
}

/// CL (Consensus Layer) node configuration
#[derive(Debug, Clone, Deserialize)]
pub struct Cl {
    /// Human-readable name for the node
    pub name: String,
    /// Base URL for beacon API requests
    pub url: String,
}

impl Cl {
    /// Validate the CL node configuration
    fn validate(&self) -> Result<()> {
        validate_url(&self.url, "url")?;
        Ok(())
    }
}

/// Main configuration struct
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Global settings
    #[serde(default)]
    pub global: Global,
    /// Metrics settings
    #[serde(default)]
    pub metrics: Metrics,
    /// EL node configuration
    pub el: El,
    /// CL nodes configuration
    pub cl: Vec<Cl>,
}

impl Config {
    /// Load configuration from a file path
    pub fn load(path: &str) -> Result<Self> {
        let content =
            std::fs::read_to_string(path).wrap_err_with(|| format!("failed to read {path}"))?;
        Self::parse(&content)
    }

    /// Parse configuration from a TOML string
    pub fn parse(s: &str) -> Result<Self> {
        let config: Config = toml::from_str(s).wrap_err("failed to parse TOML configuration")?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the entire configuration
    fn validate(&self) -> Result<()> {
        self.el.validate().wrap_err("invalid EL configuration")?;

        if self.cl.is_empty() {
            return Err(ConfigError::ValidationFailed(
                "at least one CL node is required".to_string(),
            )
            .into());
        }

        for node in &self.cl {
            node.validate()
                .wrap_err_with(|| format!("invalid CL node '{}'", node.name))?;
        }

        Ok(())
    }
}

/// Validate that a string is a valid URL
fn validate_url(url: &str, field_name: &str) -> Result<()> {
    // Check for basic URL structure
    if !url.starts_with("http://")
        && !url.starts_with("https://")
        && !url.starts_with("ws://")
        && !url.starts_with("wss://")
    {
        return Err(ConfigError::InvalidUrl {
            url: url.to_string(),
            reason: format!("{field_name} must start with http://, https://, ws://, or wss://"),
        }
        .into());
    }

    // Try to parse as URL to validate further
    url::Url::parse(url).map_err(|e| {
        eyre!(ConfigError::InvalidUrl {
            url: url.to_string(),
            reason: e.to_string(),
        })
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CONFIG: &str = r#"
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

    #[test]
    fn test_parse_valid_config() {
        let config = Config::parse(VALID_CONFIG).expect("Should parse valid config");

        assert_eq!(config.global.max_el_lag_blocks, 5);
        assert_eq!(config.global.max_cl_lag_slots, 3);
        assert_eq!(config.global.health_check_interval_ms, 1000);

        assert_eq!(config.el.primary.len(), 2);
        assert_eq!(config.el.backup.len(), 1);
        assert_eq!(config.cl.len(), 2);

        assert_eq!(config.el.primary[0].name, "geth-1");
        assert_eq!(config.el.primary[0].http_url, "http://localhost:8545");
        assert_eq!(config.el.primary[0].ws_url, "ws://localhost:8546");

        assert_eq!(config.cl[0].name, "lighthouse-1");
        assert_eq!(config.cl[0].url, "http://localhost:5052");
    }

    #[test]
    fn test_parse_config_missing_el_fails() {
        let config_str = r#"
[global]
max_el_lag_blocks = 5
max_cl_lag_slots = 3
health_check_interval_ms = 1000

[[cl]]
name = "lighthouse-1"
url = "http://localhost:5052"
"#;

        let result = Config::parse(config_str);
        assert!(result.is_err(), "Should fail when EL config is missing");
    }

    #[test]
    fn test_parse_config_missing_cl_fails() {
        let config_str = r#"
[global]
max_el_lag_blocks = 5
max_cl_lag_slots = 3
health_check_interval_ms = 1000

[el]
[[el.primary]]
name = "geth-1"
http_url = "http://localhost:8545"
ws_url = "ws://localhost:8546"
"#;

        let result = Config::parse(config_str);
        assert!(result.is_err(), "Should fail when CL config is missing");
    }

    #[test]
    fn test_parse_config_invalid_url_fails() {
        let config_str = r#"
[global]
max_el_lag_blocks = 5
max_cl_lag_slots = 3
health_check_interval_ms = 1000

[el]
[[el.primary]]
name = "geth-1"
http_url = "not-a-valid-url"
ws_url = "ws://localhost:8546"

[[cl]]
name = "lighthouse-1"
url = "http://localhost:5052"
"#;

        let result = Config::parse(config_str);
        assert!(result.is_err(), "Should fail when URL is invalid");
    }

    #[test]
    fn test_default_values_applied() {
        // Config without explicit global values - should use defaults
        let config_str = r#"
[el]
[[el.primary]]
name = "geth-1"
http_url = "http://localhost:8545"
ws_url = "ws://localhost:8546"

[[cl]]
name = "lighthouse-1"
url = "http://localhost:5052"
"#;

        let config = Config::parse(config_str).expect("Should parse with defaults");

        // These are the default values specified in AGENT.md
        assert_eq!(config.global.max_el_lag_blocks, 5);
        assert_eq!(config.global.max_cl_lag_slots, 3);
        assert_eq!(config.global.health_check_interval_ms, 1000);
    }

    #[test]
    fn test_empty_backup_is_valid() {
        let config_str = r#"
[global]
max_el_lag_blocks = 5
max_cl_lag_slots = 3
health_check_interval_ms = 1000

[el]
[[el.primary]]
name = "geth-1"
http_url = "http://localhost:8545"
ws_url = "ws://localhost:8546"

[[cl]]
name = "lighthouse-1"
url = "http://localhost:5052"
"#;

        let config = Config::parse(config_str).expect("Should parse config without backup nodes");
        assert!(
            config.el.backup.is_empty(),
            "Backup should default to empty"
        );
    }
}
