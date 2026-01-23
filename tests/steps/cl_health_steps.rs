//! Step definitions for cl_health.feature
//!
//! Also contains the shared "When the health check runs" step used by both
//! EL and CL health tests.

use cucumber::{given, then, when};
use vixy::health::cl::{calculate_cl_health, update_cl_chain_head};
use vixy::health::el::{calculate_el_health, update_el_chain_head};
use vixy::state::ClNodeState;

use crate::world::VixyWorld;

// ============================================================================
// Helper functions
// ============================================================================

fn make_cl_node(name: &str, slot: u64, health_ok: bool) -> ClNodeState {
    ClNodeState {
        name: name.to_string(),
        url: "http://localhost:5052".to_string(),
        slot,
        health_ok,
        is_healthy: false,
        lag: 0,
        consecutive_failures: 0,
    }
}

// ============================================================================
// Background steps
// ============================================================================

#[given("a configured Vixy instance with CL nodes")]
fn given_configured_vixy_with_cl_nodes(world: &mut VixyWorld) {
    // Initialize with empty state - nodes will be added by subsequent steps
    world.cl_nodes.clear();
    world.cl_chain_head = 0;
    world.max_cl_lag = 3; // Default
}

// ============================================================================
// Given steps
// ============================================================================

#[given("a CL node that returns 200 on health endpoint")]
fn given_cl_node_healthy_endpoint(world: &mut VixyWorld) {
    // Will be configured with slot in next step
    let node = make_cl_node("healthy-node", 0, true);
    world.cl_nodes.push(node);
}

#[given("a CL node that returns 503 on health endpoint")]
fn given_cl_node_unhealthy_endpoint(world: &mut VixyWorld) {
    let node = make_cl_node("unhealthy-node", 0, false);
    world.cl_nodes.push(node);
}

#[given("a CL node that is unreachable")]
fn given_cl_node_unreachable(world: &mut VixyWorld) {
    // Unreachable node has health_ok = false
    let node = make_cl_node("unreachable-node", 0, false);
    world.cl_nodes.push(node);
}

#[given(expr = "CL nodes at slots {int}, {int}, and {int}")]
fn given_cl_nodes_at_slots(world: &mut VixyWorld, slot1: u64, slot2: u64, slot3: u64) {
    world.cl_nodes.push(make_cl_node("node-1", slot1, true));
    world.cl_nodes.push(make_cl_node("node-2", slot2, true));
    world.cl_nodes.push(make_cl_node("node-3", slot3, true));
}

#[given(expr = "the CL node is at slot {int}")]
fn given_cl_node_at_slot(world: &mut VixyWorld, slot: u64) {
    // Update the last added node's slot
    if let Some(node) = world.cl_nodes.last_mut() {
        node.slot = slot;
    }
}

#[given(expr = "the CL chain head is at slot {int}")]
fn given_cl_chain_head(world: &mut VixyWorld, slot: u64) {
    world.cl_chain_head = slot;
}

#[given(expr = "the max CL lag is {int} slots")]
fn given_max_cl_lag(world: &mut VixyWorld, lag: u64) {
    world.max_cl_lag = lag;
}

// ============================================================================
// When steps (shared between EL and CL health tests)
// ============================================================================

#[when("the health check runs")]
fn when_health_check_runs(world: &mut VixyWorld) {
    // Handle EL nodes if present
    if !world.el_nodes.is_empty() {
        // If chain head wasn't explicitly set, calculate it from nodes
        if world.el_chain_head == 0 {
            world.el_chain_head = update_el_chain_head(&world.el_nodes);
        }

        // Calculate health for each EL node
        for node in world.el_nodes.iter_mut() {
            calculate_el_health(node, world.el_chain_head, world.max_el_lag, 3);
        }
    }

    // Handle CL nodes if present
    if !world.cl_nodes.is_empty() {
        // If chain head wasn't explicitly set, calculate it from nodes
        if world.cl_chain_head == 0 {
            world.cl_chain_head = update_cl_chain_head(&world.cl_nodes);
        }

        // Calculate health for each CL node
        for node in world.cl_nodes.iter_mut() {
            calculate_cl_health(node, world.cl_chain_head, world.max_cl_lag, 3);
        }
    }
}

// ============================================================================
// Then steps
// ============================================================================

#[then("the CL node should be marked as healthy")]
fn then_cl_node_healthy(world: &mut VixyWorld) {
    assert!(!world.cl_nodes.is_empty(), "No CL nodes configured in test");
    let node = &world.cl_nodes[0];
    assert!(
        node.is_healthy,
        "Expected CL node '{}' to be healthy, but it was unhealthy (health_ok={}, lag={}, max_lag={})",
        node.name, node.health_ok, node.lag, world.max_cl_lag
    );
}

#[then("the CL node should be marked as unhealthy")]
fn then_cl_node_unhealthy(world: &mut VixyWorld) {
    assert!(!world.cl_nodes.is_empty(), "No CL nodes configured in test");
    let node = &world.cl_nodes[0];
    assert!(
        !node.is_healthy,
        "Expected CL node '{}' to be unhealthy, but it was healthy",
        node.name
    );
}

#[then(expr = "the CL node lag should be {int} slots")]
fn then_cl_node_lag(world: &mut VixyWorld, expected_lag: u64) {
    assert!(!world.cl_nodes.is_empty(), "No CL nodes configured in test");
    let node = &world.cl_nodes[0];
    assert_eq!(
        node.lag, expected_lag,
        "Expected CL node lag to be {}, but got {}",
        expected_lag, node.lag
    );
}

#[then(expr = "the CL chain head should be {int}")]
fn then_cl_chain_head(world: &mut VixyWorld, expected_head: u64) {
    assert_eq!(
        world.cl_chain_head, expected_head,
        "Expected CL chain head to be {}, but got {}",
        expected_head, world.cl_chain_head
    );
}
