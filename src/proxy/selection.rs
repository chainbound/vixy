//! Node selection logic with health checking and failover support

use crate::state::{ClNodeState, ElNodeState};

/// Select a healthy EL node, preferring primary nodes over backup
///
/// Failover derives from live node state: backups are eligible whenever no
/// healthy primary exists. The `el_failover_active` flag is a per-cycle
/// observability snapshot and deliberately does not gate routing.
pub fn select_el_node(nodes: &[ElNodeState]) -> Option<&ElNodeState> {
    nodes
        .iter()
        .find(|n| n.is_primary && n.is_healthy)
        .or_else(|| nodes.iter().find(|n| !n.is_primary && n.is_healthy))
}

/// Select an EL node for the WebSocket relay, where a stalled `newHeads` subscription
/// means clients would be served a frozen head.
///
/// Prefers, in order:
/// 1. a healthy primary whose subscription is also fresh,
/// 2. any healthy node (primary or backup) whose subscription is fresh — a primary that
///    is HTTP-healthy but subscription-stale cannot serve WS, so a fresh backup is
///    preferred,
/// 3. as a last resort, the plain health-based selection ([`select_el_node`]), so WS is
///    never *worse* off than HTTP during a total subscription outage.
pub fn select_el_ws_node(nodes: &[ElNodeState]) -> Option<&ElNodeState> {
    // 1. Healthy + fresh primary.
    if let Some(n) = nodes
        .iter()
        .find(|n| n.is_primary && n.is_healthy && n.subscription_healthy)
    {
        return Some(n);
    }

    // 2. Any healthy + fresh node (backups included).
    if let Some(n) = nodes
        .iter()
        .find(|n| n.is_healthy && n.subscription_healthy)
    {
        return Some(n);
    }

    // 3. Fall back to plain health-based selection (may be subscription-stale).
    select_el_node(nodes)
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
            sub_block_number: 1000,
            sub_last_head_at: None,
            subscription_healthy: true,
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

        let selected = select_el_node(&nodes);

        assert!(selected.is_some(), "Should select a healthy node");
        assert_eq!(selected.unwrap().name, "geth-1");
    }

    #[test]
    fn test_select_skips_unhealthy_nodes() {
        let nodes = vec![
            make_el_node("geth-1", true, false), // unhealthy
            make_el_node("geth-2", true, true),  // healthy
        ];

        let selected = select_el_node(&nodes);

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

        let selected = select_el_node(&nodes);

        assert!(selected.is_some());
        assert!(
            selected.unwrap().is_primary,
            "Should prefer primary over backup when both are healthy"
        );
    }

    // =========================================================================
    // WebSocket EL node selection tests (subscription-aware)
    // =========================================================================

    #[test]
    fn test_select_el_ws_prefers_fresh_primary() {
        let mut nodes = vec![
            make_el_node("primary-1", true, true),
            make_el_node("primary-2", true, true),
        ];
        nodes[0].subscription_healthy = false; // stale sub
        nodes[1].subscription_healthy = true; // fresh

        let selected = select_el_ws_node(&nodes);

        assert_eq!(
            selected.unwrap().name,
            "primary-2",
            "WS selection should prefer a healthy primary whose subscription is fresh"
        );
    }

    #[test]
    fn test_select_el_ws_uses_fresh_backup_when_primary_sub_stale() {
        // Primary is HTTP-healthy but subscription-stale; a fresh backup exists. Even
        // with failover INACTIVE (HTTP primary is healthy), WS must avoid the stale
        // primary and use the fresh backup — it cannot serve fresh heads.
        let mut nodes = vec![
            make_el_node("primary-1", true, true),
            make_el_node("backup-1", false, true),
        ];
        nodes[0].subscription_healthy = false;
        nodes[1].subscription_healthy = true;

        let selected = select_el_ws_node(&nodes);

        assert_eq!(
            selected.unwrap().name,
            "backup-1",
            "WS selection should use a fresh backup when the only primary is subscription-stale"
        );
    }

    #[test]
    fn test_select_el_ws_falls_back_to_health_when_none_fresh() {
        // No node has a fresh subscription → fall back to plain health-based selection
        // (so WS is never worse off than HTTP during a total subscription outage).
        let mut nodes = vec![
            make_el_node("primary-1", true, true),
            make_el_node("backup-1", false, true),
        ];
        nodes[0].subscription_healthy = false;
        nodes[1].subscription_healthy = false;

        let selected = select_el_ws_node(&nodes);

        assert_eq!(
            selected.unwrap().name,
            "primary-1",
            "WS selection should fall back to the healthy primary when no node is fresh"
        );
    }

    #[test]
    fn test_select_el_ws_none_when_no_healthy_node() {
        let nodes = vec![make_el_node("primary-1", true, false)];
        assert!(
            select_el_ws_node(&nodes).is_none(),
            "WS selection should return None when no node is healthy"
        );
    }

    #[test]
    fn test_select_backup_when_no_primary_available() {
        let nodes = vec![
            make_el_node("primary-1", true, false), // primary, unhealthy
            make_el_node("backup-1", false, true),  // backup, healthy
        ];

        // Failover is derived from live state: no healthy primary → backup is
        // selected immediately, without waiting for any external flag.
        let selected = select_el_node(&nodes);
        assert!(
            selected.is_some(),
            "Backup should be selected as soon as no primary is healthy"
        );
        assert_eq!(selected.unwrap().name, "backup-1");
    }

    #[test]
    fn test_select_returns_none_when_all_unavailable() {
        let nodes = vec![
            make_el_node("primary-1", true, false), // unhealthy
            make_el_node("backup-1", false, false), // unhealthy
        ];

        let selected = select_el_node(&nodes);

        assert!(
            selected.is_none(),
            "Should return None when all nodes unhealthy"
        );
    }

    #[test]
    fn test_select_empty_list_returns_none() {
        let nodes: Vec<ElNodeState> = vec![];

        let selected = select_el_node(&nodes);

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
