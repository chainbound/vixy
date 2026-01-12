//! Health monitoring loop
//!
//! Background task that periodically checks all EL and CL nodes and updates their health state.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::health::{cl, el};
use crate::state::AppState;

/// Run a single health check cycle for all nodes
///
/// This function checks all EL and CL nodes once and updates their state.
/// Returns true if at least one primary EL node is healthy.
pub async fn run_health_check_cycle(state: &Arc<AppState>) -> bool {
    // Check all EL nodes
    let any_primary_healthy = check_all_el_nodes(state).await;

    // Check all CL nodes
    check_all_cl_nodes(state).await;

    // Update failover flag
    update_failover_flag(state, any_primary_healthy);

    any_primary_healthy
}

/// Check all EL nodes and update their state
///
/// Returns true if at least one primary EL node is healthy.
pub async fn check_all_el_nodes(state: &Arc<AppState>) -> bool {
    // First pass: check each node and update block numbers
    {
        let mut el_nodes = state.el_nodes.write().await;

        for node in el_nodes.iter_mut() {
            match el::check_el_node(&node.http_url).await {
                Ok(block_number) => {
                    node.block_number = block_number;
                    node.check_ok = true;
                    debug!(
                        node = %node.name,
                        block_number,
                        "EL node check successful"
                    );
                }
                Err(e) => {
                    // On error, mark check as failed
                    warn!(
                        node = %node.name,
                        error = %e,
                        "EL node check failed"
                    );
                    node.check_ok = false;
                    // Keep old block_number but it will be unhealthy due to check_ok = false
                }
            }
        }
    }

    // Calculate chain head (max block number across all nodes)
    let chain_head = {
        let el_nodes = state.el_nodes.read().await;
        el::update_el_chain_head(&el_nodes)
    };

    // Store chain head
    state.el_chain_head.store(chain_head, Ordering::SeqCst);

    // Second pass: calculate health for each node
    let any_primary_healthy = {
        let mut el_nodes = state.el_nodes.write().await;
        let mut any_primary = false;

        for node in el_nodes.iter_mut() {
            el::calculate_el_health(node, chain_head, state.max_el_lag);

            if node.is_primary && node.is_healthy {
                any_primary = true;
            }

            debug!(
                node = %node.name,
                is_primary = node.is_primary,
                block_number = node.block_number,
                check_ok = node.check_ok,
                lag = node.lag,
                is_healthy = node.is_healthy,
                "EL node health calculated"
            );
        }

        any_primary
    };

    any_primary_healthy
}

/// Check all CL nodes and update their state
pub async fn check_all_cl_nodes(state: &Arc<AppState>) {
    // First pass: check each node and update slots
    {
        let mut cl_nodes = state.cl_nodes.write().await;

        for node in cl_nodes.iter_mut() {
            match cl::check_cl_node(&node.url).await {
                Ok((health_ok, slot)) => {
                    node.health_ok = health_ok;
                    node.slot = slot;
                    debug!(
                        node = %node.name,
                        health_ok,
                        slot,
                        "CL node check successful"
                    );
                }
                Err(e) => {
                    // On error, mark as unhealthy
                    warn!(
                        node = %node.name,
                        error = %e,
                        "CL node check failed"
                    );
                    node.health_ok = false;
                    node.slot = 0;
                }
            }
        }
    }

    // Calculate chain head (max slot across all nodes)
    let chain_head = {
        let cl_nodes = state.cl_nodes.read().await;
        cl::update_cl_chain_head(&cl_nodes)
    };

    // Store chain head
    state.cl_chain_head.store(chain_head, Ordering::SeqCst);

    // Second pass: calculate health for each node
    {
        let mut cl_nodes = state.cl_nodes.write().await;

        for node in cl_nodes.iter_mut() {
            cl::calculate_cl_health(node, chain_head, state.max_cl_lag);

            debug!(
                node = %node.name,
                slot = node.slot,
                health_ok = node.health_ok,
                lag = node.lag,
                is_healthy = node.is_healthy,
                "CL node health calculated"
            );
        }
    }
}

/// Update the failover flag based on primary EL node availability
pub fn update_failover_flag(state: &Arc<AppState>, any_primary_healthy: bool) {
    let was_failover = state.el_failover_active.load(Ordering::SeqCst);
    let is_failover = !any_primary_healthy;

    if was_failover != is_failover {
        state
            .el_failover_active
            .store(is_failover, Ordering::SeqCst);

        if is_failover {
            warn!("EL failover ACTIVATED - all primary nodes unhealthy, using backups");
        } else {
            info!("EL failover DEACTIVATED - primary node recovered");
        }
    }
}

/// Run the health monitoring loop
///
/// This function runs forever, periodically checking all nodes and updating their health state.
pub async fn run_health_monitor(state: Arc<AppState>, interval_ms: u64) {
    let interval = Duration::from_millis(interval_ms);

    info!(interval_ms, "Starting health monitor");

    loop {
        run_health_check_cycle(&state).await;
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::sync::atomic::Ordering;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Helper to create a config with mock server URLs
    fn create_test_config(el_urls: &[&str], cl_urls: &[&str]) -> Config {
        let el_primary: Vec<String> = el_urls
            .iter()
            .enumerate()
            .map(|(i, url)| {
                format!(
                    r#"[[el.primary]]
name = "geth-{i}"
http_url = "{url}"
ws_url = "{url}""#
                )
            })
            .collect();

        let cl_nodes: Vec<String> = if cl_urls.is_empty() {
            // Add a dummy CL node if none provided (config requires at least one)
            vec![r#"[[cl]]
name = "dummy-cl"
url = "http://localhost:5052""#
                .to_string()]
        } else {
            cl_urls
                .iter()
                .enumerate()
                .map(|(i, url)| {
                    format!(
                        r#"[[cl]]
name = "lighthouse-{i}"
url = "{url}""#
                    )
                })
                .collect()
        };

        let toml_str = format!(
            r#"[global]
max_el_lag_blocks = 5
max_cl_lag_slots = 3
health_check_interval_ms = 100

[el]
{}

{}
"#,
            el_primary.join("\n\n"),
            cl_nodes.join("\n\n")
        );

        Config::parse(&toml_str).expect("Test config should parse")
    }

    // Helper to create config with primary and backup EL nodes
    fn create_config_with_backup(
        primary_urls: &[&str],
        backup_urls: &[&str],
        cl_urls: &[&str],
    ) -> Config {
        let el_primary: Vec<String> = primary_urls
            .iter()
            .enumerate()
            .map(|(i, url)| {
                format!(
                    r#"[[el.primary]]
name = "primary-{i}"
http_url = "{url}"
ws_url = "{url}""#
                )
            })
            .collect();

        let el_backup: Vec<String> = backup_urls
            .iter()
            .enumerate()
            .map(|(i, url)| {
                format!(
                    r#"[[el.backup]]
name = "backup-{i}"
http_url = "{url}"
ws_url = "{url}""#
                )
            })
            .collect();

        let cl_nodes: Vec<String> = if cl_urls.is_empty() {
            // Add a dummy CL node if none provided (config requires at least one)
            vec![r#"[[cl]]
name = "dummy-cl"
url = "http://localhost:5052""#
                .to_string()]
        } else {
            cl_urls
                .iter()
                .enumerate()
                .map(|(i, url)| {
                    format!(
                        r#"[[cl]]
name = "lighthouse-{i}"
url = "{url}""#
                    )
                })
                .collect()
        };

        let toml_str = format!(
            r#"[global]
max_el_lag_blocks = 5
max_cl_lag_slots = 3
health_check_interval_ms = 100

[el]
{}

{}

{}
"#,
            el_primary.join("\n\n"),
            el_backup.join("\n\n"),
            cl_nodes.join("\n\n")
        );

        Config::parse(&toml_str).expect("Test config should parse")
    }

    // =========================================================================
    // test_monitor_updates_el_node_state
    // =========================================================================

    #[tokio::test]
    async fn test_monitor_updates_el_node_state() {
        let mock_server = MockServer::start().await;

        // Mock eth_blockNumber response
        Mock::given(method("POST"))
            .and(body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_blockNumber",
                "params": [],
                "id": 1
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "0x3e8", // 1000 in hex
                "id": 1
            })))
            .mount(&mock_server)
            .await;

        let config = create_test_config(&[&mock_server.uri()], &[]);
        let state = Arc::new(AppState::new(&config));

        // Run health check
        check_all_el_nodes(&state).await;

        // Verify node state was updated
        let el_nodes = state.el_nodes.read().await;
        assert_eq!(el_nodes[0].block_number, 1000);
        assert!(el_nodes[0].is_healthy, "Node should be healthy");
        assert_eq!(el_nodes[0].lag, 0, "Lag should be 0 when at chain head");
    }

    // =========================================================================
    // test_monitor_updates_cl_node_state
    // =========================================================================

    #[tokio::test]
    async fn test_monitor_updates_cl_node_state() {
        let mock_server = MockServer::start().await;

        // Mock CL health endpoint
        Mock::given(method("GET"))
            .and(path("/eth/v1/node/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        // Mock CL headers endpoint
        Mock::given(method("GET"))
            .and(path("/eth/v1/beacon/headers/head"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "root": "0x...",
                    "canonical": true,
                    "header": {
                        "message": {
                            "slot": "5000",
                            "proposer_index": "1234",
                            "parent_root": "0x...",
                            "state_root": "0x...",
                            "body_root": "0x..."
                        },
                        "signature": "0x..."
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        // Create config with EL node (required) and CL node
        let el_mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "0x1",
                "id": 1
            })))
            .mount(&el_mock)
            .await;

        let config = create_test_config(&[&el_mock.uri()], &[&mock_server.uri()]);
        let state = Arc::new(AppState::new(&config));

        // Run health check
        check_all_cl_nodes(&state).await;

        // Verify node state was updated
        let cl_nodes = state.cl_nodes.read().await;
        assert_eq!(cl_nodes[0].slot, 5000);
        assert!(cl_nodes[0].health_ok, "Health endpoint returned 200");
        assert!(cl_nodes[0].is_healthy, "Node should be healthy");
        assert_eq!(cl_nodes[0].lag, 0, "Lag should be 0 when at chain head");
    }

    // =========================================================================
    // test_monitor_calculates_chain_head
    // =========================================================================

    #[tokio::test]
    async fn test_monitor_calculates_chain_head() {
        // Create two mock EL servers with different block numbers
        let mock1 = MockServer::start().await;
        let mock2 = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "0x3e8", // 1000
                "id": 1
            })))
            .mount(&mock1)
            .await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "0x3eb", // 1003
                "id": 1
            })))
            .mount(&mock2)
            .await;

        let config = create_test_config(&[&mock1.uri(), &mock2.uri()], &[]);
        let state = Arc::new(AppState::new(&config));

        // Run health check
        check_all_el_nodes(&state).await;

        // Chain head should be the maximum (1003)
        assert_eq!(state.el_chain_head.load(Ordering::SeqCst), 1003);

        // Check individual node lags
        let el_nodes = state.el_nodes.read().await;
        assert_eq!(el_nodes[0].lag, 3, "First node should have lag of 3");
        assert_eq!(el_nodes[1].lag, 0, "Second node should have lag of 0");
    }

    // =========================================================================
    // test_monitor_sets_failover_flag
    // =========================================================================

    #[tokio::test]
    async fn test_monitor_sets_failover_flag() {
        // Create mock servers - primary will fail, backup will succeed
        let primary_mock = MockServer::start().await;
        let backup_mock = MockServer::start().await;

        // Primary returns nothing (connection failure simulated by not mounting mock)
        // We'll just not mount any response

        // Backup returns valid response
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "0x3e8",
                "id": 1
            })))
            .mount(&backup_mock)
            .await;

        let config = create_config_with_backup(&[&primary_mock.uri()], &[&backup_mock.uri()], &[]);
        let state = Arc::new(AppState::new(&config));

        // Initially failover should be inactive
        assert!(!state.el_failover_active.load(Ordering::SeqCst));

        // Run health check - primary will fail
        let any_primary_healthy = check_all_el_nodes(&state).await;

        // Update failover flag
        update_failover_flag(&state, any_primary_healthy);

        // Failover should now be active since no primary is healthy
        assert!(
            state.el_failover_active.load(Ordering::SeqCst),
            "Failover should be active when no primary nodes are healthy"
        );
    }

    // =========================================================================
    // test_monitor_clears_failover_when_primary_recovers
    // =========================================================================

    #[tokio::test]
    async fn test_monitor_clears_failover_when_primary_recovers() {
        let primary_mock = MockServer::start().await;
        let backup_mock = MockServer::start().await;

        // Primary returns valid response
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "0x3e8",
                "id": 1
            })))
            .mount(&primary_mock)
            .await;

        // Backup also returns valid response
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "0x3e8",
                "id": 1
            })))
            .mount(&backup_mock)
            .await;

        let config = create_config_with_backup(&[&primary_mock.uri()], &[&backup_mock.uri()], &[]);
        let state = Arc::new(AppState::new(&config));

        // Set failover as active (simulating previous failure)
        state.el_failover_active.store(true, Ordering::SeqCst);

        // Run health check - primary is now healthy
        let any_primary_healthy = check_all_el_nodes(&state).await;

        // Update failover flag
        update_failover_flag(&state, any_primary_healthy);

        // Failover should be cleared since primary is healthy
        assert!(
            !state.el_failover_active.load(Ordering::SeqCst),
            "Failover should be inactive when a primary node is healthy"
        );
    }

    // =========================================================================
    // test_monitor_runs_at_configured_interval
    // =========================================================================

    #[tokio::test]
    async fn test_monitor_runs_at_configured_interval() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "0x1",
                "id": 1
            })))
            .mount(&mock_server)
            .await;

        let config = create_test_config(&[&mock_server.uri()], &[]);
        let state = Arc::new(AppState::new(&config));

        // Run monitor for a short duration (300ms with 100ms interval)
        let monitor_handle = {
            let state = state.clone();
            tokio::spawn(async move {
                run_health_monitor(state, 100).await;
            })
        };

        // Let it run for a bit
        tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;

        // Abort the monitor
        monitor_handle.abort();

        // We should have had at least 2-3 checks in 350ms with 100ms interval
        // We verify by checking that the node state was updated
        let el_nodes = state.el_nodes.read().await;
        assert_eq!(
            el_nodes[0].block_number, 1,
            "Node should have been checked and updated"
        );
    }

    // =========================================================================
    // test_el_node_marked_unhealthy_on_connection_failure
    // =========================================================================

    #[tokio::test]
    async fn test_el_node_marked_unhealthy_on_connection_failure() {
        // Mock server with no response mounted (will cause connection failure)
        let mock_server = MockServer::start().await;
        // Don't mount any mock - requests will get 404

        let config = create_test_config(&[&mock_server.uri()], &[]);
        let state = Arc::new(AppState::new(&config));

        // Run health check
        check_all_el_nodes(&state).await;

        // Node should be marked unhealthy
        let el_nodes = state.el_nodes.read().await;
        assert!(
            !el_nodes[0].is_healthy,
            "Node should be unhealthy on connection failure"
        );
    }

    // =========================================================================
    // test_cl_node_marked_unhealthy_on_health_endpoint_failure
    // =========================================================================

    #[tokio::test]
    async fn test_cl_node_marked_unhealthy_on_health_endpoint_failure() {
        let mock_server = MockServer::start().await;

        // CL health endpoint returns 503
        Mock::given(method("GET"))
            .and(path("/eth/v1/node/health"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        // CL headers endpoint still works
        Mock::given(method("GET"))
            .and(path("/eth/v1/beacon/headers/head"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "root": "0x...",
                    "canonical": true,
                    "header": {
                        "message": {
                            "slot": "1000",
                            "proposer_index": "1234",
                            "parent_root": "0x...",
                            "state_root": "0x...",
                            "body_root": "0x..."
                        },
                        "signature": "0x..."
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        // Need an EL mock too
        let el_mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "0x1",
                "id": 1
            })))
            .mount(&el_mock)
            .await;

        let config = create_test_config(&[&el_mock.uri()], &[&mock_server.uri()]);
        let state = Arc::new(AppState::new(&config));

        // Run health check
        check_all_cl_nodes(&state).await;

        // Node should be marked unhealthy because health endpoint returned 503
        let cl_nodes = state.cl_nodes.read().await;
        assert!(!cl_nodes[0].health_ok, "Health should not be ok on 503");
        assert!(
            !cl_nodes[0].is_healthy,
            "Node should be unhealthy when health endpoint fails"
        );
    }
}
