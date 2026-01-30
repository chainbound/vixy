//! Node selection logic with health checking and failover support

use crate::state::{ClNodeState, ElNodeState};

/// Select a healthy EL node, preferring primary nodes over backup
///
/// When failover_active is false, only primary nodes are considered.
/// When failover_active is true, both primary and backup nodes are considered.
pub fn select_el_node(nodes: &[ElNodeState], failover_active: bool) -> Option<&ElNodeState> {
    // First try to find a healthy primary node
    let primary = nodes.iter().find(|n| n.is_primary && n.is_healthy);

    if primary.is_some() {
        return primary;
    }

    // If no healthy primary and failover is active, try backup nodes
    if failover_active {
        return nodes.iter().find(|n| !n.is_primary && n.is_healthy);
    }

    None
}

/// Select a healthy CL node
///
/// Returns the first healthy CL node found.
pub fn select_cl_node(nodes: &[ClNodeState]) -> Option<&ClNodeState> {
    nodes.iter().find(|n| n.is_healthy)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create EL nodes for testing
    fn make_el_node(name: &str, is_primary: bool, is_healthy: bool) -> ElNodeState {
        ElNodeState {
            name: name.to_string(),
            http_url: format!("http://{name}.local:8545"),
            ws_url: format!("ws://{name}.local:8546"),
            is_primary,
            block_number: 1000,
            check_ok: is_healthy,
            is_healthy,
            lag: 0,
            consecutive_failures: 0,
        }
    }

    // Helper to create CL nodes for testing
    fn make_cl_node(name: &str, is_healthy: bool) -> ClNodeState {
        ClNodeState {
            name: name.to_string(),
            url: format!("http://{name}.local:5052"),
            slot: 5000,
            health_ok: is_healthy,
            is_healthy,
            lag: 0,
            consecutive_failures: 0,
        }
    }

    // =========================================================================
    // EL node selection tests
    // =========================================================================

    #[test]
    fn test_select_healthy_node_from_list() {
        let nodes = vec![
            make_el_node("geth-1", true, true),
            make_el_node("geth-2", true, true),
        ];

        let selected = select_el_node(&nodes, false);

        assert!(selected.is_some(), "Should select a healthy node");
        assert_eq!(selected.unwrap().name, "geth-1");
    }

    #[test]
    fn test_select_skips_unhealthy_nodes() {
        let nodes = vec![
            make_el_node("geth-1", true, false), // unhealthy
            make_el_node("geth-2", true, true),  // healthy
        ];

        let selected = select_el_node(&nodes, false);

        assert!(selected.is_some(), "Should find a healthy node");
        assert_eq!(
            selected.unwrap().name,
            "geth-2",
            "Should skip unhealthy node"
        );
    }

    #[test]
    fn test_select_primary_before_backup() {
        let nodes = vec![
            make_el_node("backup-1", false, true), // backup, healthy
            make_el_node("primary-1", true, true), // primary, healthy
        ];

        let selected = select_el_node(&nodes, true); // failover active

        assert!(selected.is_some());
        assert!(
            selected.unwrap().is_primary,
            "Should prefer primary over backup even when failover active"
        );
    }

    #[test]
    fn test_select_backup_when_no_primary_available() {
        let nodes = vec![
            make_el_node("primary-1", true, false), // primary, unhealthy
            make_el_node("backup-1", false, true),  // backup, healthy
        ];

        // Without failover, should return None (no healthy primary)
        let without_failover = select_el_node(&nodes, false);
        assert!(
            without_failover.is_none(),
            "Without failover, should not select backup"
        );

        // With failover, should select backup
        let with_failover = select_el_node(&nodes, true);
        assert!(
            with_failover.is_some(),
            "With failover, should select backup"
        );
        assert_eq!(with_failover.unwrap().name, "backup-1");
    }

    #[test]
    fn test_select_returns_none_when_all_unavailable() {
        let nodes = vec![
            make_el_node("primary-1", true, false), // unhealthy
            make_el_node("backup-1", false, false), // unhealthy
        ];

        let selected = select_el_node(&nodes, true); // even with failover

        assert!(
            selected.is_none(),
            "Should return None when all nodes unhealthy"
        );
    }

    #[test]
    fn test_select_empty_list_returns_none() {
        let nodes: Vec<ElNodeState> = vec![];

        let selected = select_el_node(&nodes, true);

        assert!(selected.is_none(), "Empty list should return None");
    }

    // =========================================================================
    // CL node selection tests
    // =========================================================================

    #[test]
    fn test_select_cl_healthy_node() {
        let nodes = vec![
            make_cl_node("lighthouse-1", true),
            make_cl_node("prysm-1", true),
        ];

        let selected = select_cl_node(&nodes);

        assert!(selected.is_some(), "Should select a healthy CL node");
        assert_eq!(selected.unwrap().name, "lighthouse-1");
    }

    #[test]
    fn test_select_cl_skips_unhealthy() {
        let nodes = vec![
            make_cl_node("lighthouse-1", false), // unhealthy
            make_cl_node("prysm-1", true),       // healthy
        ];

        let selected = select_cl_node(&nodes);

        assert!(selected.is_some());
        assert_eq!(
            selected.unwrap().name,
            "prysm-1",
            "Should skip unhealthy CL node"
        );
    }

    #[test]
    fn test_select_cl_returns_none_when_all_unhealthy() {
        let nodes = vec![
            make_cl_node("lighthouse-1", false),
            make_cl_node("prysm-1", false),
        ];

        let selected = select_cl_node(&nodes);

        assert!(
            selected.is_none(),
            "Should return None when all CL nodes unhealthy"
        );
    }
}
