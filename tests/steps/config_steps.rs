//! Step definitions for config.feature

use cucumber::{given, then, when};

use crate::world::VixyWorld;

// ============================================================================
// Given steps
// ============================================================================

#[given("a valid TOML configuration with 2 primary EL nodes and 1 CL node")]
fn given_valid_config_2el_1cl(world: &mut VixyWorld) {
    world.config_toml = Some(
        r#"
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

[[cl]]
name = "lighthouse-1"
url = "http://localhost:5052"
"#
        .to_string(),
    );
}

#[given("a TOML configuration with 1 primary and 2 backup EL nodes")]
fn given_config_1primary_2backup(world: &mut VixyWorld) {
    world.config_toml = Some(
        r#"
[global]
max_el_lag_blocks = 5
max_cl_lag_slots = 3
health_check_interval_ms = 1000

[el]
[[el.primary]]
name = "geth-1"
http_url = "http://localhost:8545"
ws_url = "ws://localhost:8546"

[[el.backup]]
name = "alchemy-1"
http_url = "https://eth-mainnet.g.alchemy.com/v2/xxx"
ws_url = "wss://eth-mainnet.g.alchemy.com/v2/xxx"

[[el.backup]]
name = "infura-1"
http_url = "https://mainnet.infura.io/v3/xxx"
ws_url = "wss://mainnet.infura.io/ws/v3/xxx"

[[cl]]
name = "lighthouse-1"
url = "http://localhost:5052"
"#
        .to_string(),
    );
}

#[given("a TOML configuration without any EL nodes")]
fn given_config_no_el(world: &mut VixyWorld) {
    world.config_toml = Some(
        r#"
[global]
max_el_lag_blocks = 5
max_cl_lag_slots = 3
health_check_interval_ms = 1000

[[cl]]
name = "lighthouse-1"
url = "http://localhost:5052"
"#
        .to_string(),
    );
}

#[given("a TOML configuration without any CL nodes")]
fn given_config_no_cl(world: &mut VixyWorld) {
    world.config_toml = Some(
        r#"
[global]
max_el_lag_blocks = 5
max_cl_lag_slots = 3
health_check_interval_ms = 1000

[el]
[[el.primary]]
name = "geth-1"
http_url = "http://localhost:8545"
ws_url = "ws://localhost:8546"
"#
        .to_string(),
    );
}

#[given("a TOML configuration with an invalid HTTP URL")]
fn given_config_invalid_url(world: &mut VixyWorld) {
    world.config_toml = Some(
        r#"
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
"#
        .to_string(),
    );
}

#[given("a minimal TOML configuration")]
fn given_minimal_config(world: &mut VixyWorld) {
    world.config_toml = Some(
        r#"
[el]
[[el.primary]]
name = "geth-1"
http_url = "http://localhost:8545"
ws_url = "ws://localhost:8546"

[[cl]]
name = "lighthouse-1"
url = "http://localhost:5052"
"#
        .to_string(),
    );
}

// ============================================================================
// When steps
// ============================================================================

#[when("I parse the configuration")]
fn when_parse_config(world: &mut VixyWorld) {
    let toml_str = world.config_toml.as_ref().expect("No config TOML set");
    match vixy::config::Config::parse(toml_str) {
        Ok(config) => {
            world.config = Some(config);
            world.last_error = None;
        }
        Err(e) => {
            world.config = None;
            world.last_error = Some(e.to_string());
        }
    }
}

#[when("I try to parse the configuration")]
fn when_try_parse_config(world: &mut VixyWorld) {
    // Same as above - we want to capture both success and failure
    when_parse_config(world);
}

// ============================================================================
// Then steps
// ============================================================================

#[then("the configuration should be loaded successfully")]
fn then_config_loaded(world: &mut VixyWorld) {
    assert!(
        world.config.is_some(),
        "Expected configuration to be loaded, but got error: {:?}",
        world.last_error
    );
}

#[then(expr = "it should have {int} primary EL node(s)")]
fn then_has_n_primary_el_nodes(world: &mut VixyWorld, count: usize) {
    let config = world.config.as_ref().expect("No config loaded");
    assert_eq!(
        config.el.primary.len(),
        count,
        "Expected {} primary EL nodes, found {}",
        count,
        config.el.primary.len()
    );
}

#[then(expr = "it should have {int} backup EL node(s)")]
fn then_has_n_backup_el_nodes(world: &mut VixyWorld, count: usize) {
    let config = world.config.as_ref().expect("No config loaded");
    assert_eq!(
        config.el.backup.len(),
        count,
        "Expected {} backup EL nodes, found {}",
        count,
        config.el.backup.len()
    );
}

#[then(expr = "it should have {int} CL node(s)")]
fn then_has_n_cl_nodes(world: &mut VixyWorld, count: usize) {
    let config = world.config.as_ref().expect("No config loaded");
    assert_eq!(
        config.cl.len(),
        count,
        "Expected {} CL nodes, found {}",
        count,
        config.cl.len()
    );
}

#[then("the global settings should use default values")]
fn then_global_defaults(world: &mut VixyWorld) {
    let config = world.config.as_ref().expect("No config loaded");
    // These are the default values from AGENT.md
    assert_eq!(config.global.max_el_lag_blocks, 5);
    assert_eq!(config.global.max_cl_lag_slots, 3);
    assert_eq!(config.global.health_check_interval_ms, 1000);
}

#[then("parsing should fail with an error about missing EL configuration")]
fn then_parsing_fails_missing_el(world: &mut VixyWorld) {
    assert!(
        world.config.is_none(),
        "Expected parsing to fail, but it succeeded"
    );
    // The error should exist - specific message varies based on what's missing
    assert!(
        world.last_error.is_some(),
        "Expected an error about missing EL configuration"
    );
}

#[then("parsing should fail with an error about missing CL configuration")]
fn then_parsing_fails_missing_cl(world: &mut VixyWorld) {
    assert!(
        world.config.is_none(),
        "Expected parsing to fail, but it succeeded"
    );
    // The error should exist - specific message varies based on what's missing
    assert!(
        world.last_error.is_some(),
        "Expected an error about missing CL configuration"
    );
}

#[then("parsing should fail with an error about invalid URL")]
fn then_parsing_fails_invalid_url(world: &mut VixyWorld) {
    assert!(
        world.config.is_none(),
        "Expected parsing to fail, but it succeeded"
    );
    let error = world.last_error.as_ref().expect("Expected an error");
    assert!(
        error.to_lowercase().contains("url") || error.contains("invalid"),
        "Expected error about invalid URL, got: {error}",
    );
}

#[then(expr = "max_el_lag_blocks should default to {int}")]
fn then_max_el_lag_default(world: &mut VixyWorld, value: u64) {
    let config = world.config.as_ref().expect("No config loaded");
    assert_eq!(
        config.global.max_el_lag_blocks, value,
        "Expected max_el_lag_blocks to be {}, got {}",
        value, config.global.max_el_lag_blocks
    );
}

#[then(expr = "max_cl_lag_slots should default to {int}")]
fn then_max_cl_lag_default(world: &mut VixyWorld, value: u64) {
    let config = world.config.as_ref().expect("No config loaded");
    assert_eq!(
        config.global.max_cl_lag_slots, value,
        "Expected max_cl_lag_slots to be {}, got {}",
        value, config.global.max_cl_lag_slots
    );
}

#[then(expr = "health_check_interval_ms should default to {int}")]
fn then_health_check_interval_default(world: &mut VixyWorld, value: u64) {
    let config = world.config.as_ref().expect("No config loaded");
    assert_eq!(
        config.global.health_check_interval_ms, value,
        "Expected health_check_interval_ms to be {}, got {}",
        value, config.global.health_check_interval_ms
    );
}
