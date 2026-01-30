//! Step definitions for el_health.feature

use cucumber::{given, then};
use vixy::state::ElNodeState;

use crate::world::VixyWorld;

// ============================================================================
// Helper functions
// ============================================================================

fn make_el_node(name: &str, block_number: u64, check_ok: bool) -> ElNodeState {
    ElNodeState {
        name: name.to_string(),
        http_url: "http://localhost:8545".to_string(),
        ws_url: "ws://localhost:8546".to_string(),
        is_primary: true,
        block_number,
        check_ok,
        is_healthy: false,
        lag: 0,
        consecutive_failures: 0,
    }
}

// ============================================================================
// Background steps
// ============================================================================

#[given("a configured Vixy instance with EL nodes")]
fn given_configured_vixy_with_el_nodes(world: &mut VixyWorld) {
    // Initialize with empty state - nodes will be added by subsequent steps
    world.el_nodes.clear();
    world.el_chain_head = 0;
    world.max_el_lag = 5; // Default
}

// ============================================================================
// Given steps
// ============================================================================

#[given(expr = "an EL node at block {int}")]
fn given_el_node_at_block(world: &mut VixyWorld, block: u64) {
    let node = make_el_node("test-node", block, true);
    world.el_nodes.push(node);
}

#[given("an EL node that is unreachable")]
fn given_el_node_unreachable(world: &mut VixyWorld) {
    // Unreachable node has check_ok = false
    let node = make_el_node("unreachable-node", 0, false);
    world.el_nodes.push(node);
}

#[given("an EL node that returns an invalid response")]
fn given_el_node_invalid_response(world: &mut VixyWorld) {
    // Invalid response also means check_ok = false
    let node = make_el_node("invalid-node", 0, false);
    world.el_nodes.push(node);
}

#[given(expr = "EL nodes at blocks {int}, {int}, and {int}")]
fn given_el_nodes_at_blocks(world: &mut VixyWorld, block1: u64, block2: u64, block3: u64) {
    world.el_nodes.push(make_el_node("node-1", block1, true));
    world.el_nodes.push(make_el_node("node-2", block2, true));
    world.el_nodes.push(make_el_node("node-3", block3, true));
}

#[given(expr = "the EL chain head is at block {int}")]
fn given_el_chain_head(world: &mut VixyWorld, block: u64) {
    world.el_chain_head = block;
}

#[given(expr = "the max EL lag is {int} blocks")]
fn given_max_el_lag(world: &mut VixyWorld, lag: u64) {
    world.max_el_lag = lag;
}

// ============================================================================
// When steps (shared - defined in cl_health_steps.rs)
// ============================================================================

// Note: "When the health check runs" is defined in cl_health_steps.rs
// to avoid duplicate step definitions. It handles both EL and CL nodes.

// ============================================================================
// Then steps
// ============================================================================

#[then("the EL node should be marked as healthy")]
fn then_el_node_healthy(world: &mut VixyWorld) {
    assert!(!world.el_nodes.is_empty(), "No EL nodes configured in test");
    let node = &world.el_nodes[0];
    assert!(
        node.is_healthy,
        "Expected EL node '{}' to be healthy, but it was unhealthy (check_ok={}, lag={}, max_lag={})",
        node.name, node.check_ok, node.lag, world.max_el_lag
    );
}

#[then("the EL node should be marked as unhealthy")]
fn then_el_node_unhealthy(world: &mut VixyWorld) {
    assert!(!world.el_nodes.is_empty(), "No EL nodes configured in test");
    let node = &world.el_nodes[0];
    assert!(
        !node.is_healthy,
        "Expected EL node '{}' to be unhealthy, but it was healthy",
        node.name
    );
}

#[then(expr = "the EL node lag should be {int} blocks")]
fn then_el_node_lag(world: &mut VixyWorld, expected_lag: u64) {
    assert!(!world.el_nodes.is_empty(), "No EL nodes configured in test");
    let node = &world.el_nodes[0];
    assert_eq!(
        node.lag, expected_lag,
        "Expected EL node lag to be {}, but got {}",
        expected_lag, node.lag
    );
}

#[then(expr = "the EL chain head should be {int}")]
fn then_el_chain_head(world: &mut VixyWorld, expected_head: u64) {
    assert_eq!(
        world.el_chain_head, expected_head,
        "Expected EL chain head to be {}, but got {}",
        expected_head, world.el_chain_head
    );
}
