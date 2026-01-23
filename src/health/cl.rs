//! CL (Consensus Layer) health checking
//!
//! Checks CL node health via /eth/v1/node/health and /eth/v1/beacon/headers/head.

use crate::state::ClNodeState;
use eyre::{Result, WrapErr};
use serde::Deserialize;

/// Response structure for /eth/v1/beacon/headers/head
#[derive(Debug, Deserialize)]
struct BeaconHeaderResponse {
    data: BeaconHeaderData,
}

#[derive(Debug, Deserialize)]
struct BeaconHeaderData {
    header: BeaconHeader,
}

#[derive(Debug, Deserialize)]
struct BeaconHeader {
    message: BeaconHeaderMessage,
}

#[derive(Debug, Deserialize)]
struct BeaconHeaderMessage {
    slot: String,
}

/// Check if the CL node's health endpoint returns 200
pub async fn check_cl_health(url: &str) -> Result<bool> {
    // Use a timeout to prevent health checks from blocking indefinitely if the node is unresponsive
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .wrap_err("failed to build HTTP client")?;

    let health_url = format!("{}/eth/v1/node/health", url.trim_end_matches('/'));

    match client.get(&health_url).send().await {
        Ok(response) => Ok(response.status().is_success()),
        Err(_) => Ok(false), // Connection failure means unhealthy
    }
}

/// Get the current slot from the CL node's beacon headers endpoint
pub async fn check_cl_slot(url: &str) -> Result<u64> {
    // Use a timeout to prevent health checks from blocking indefinitely if the node is unresponsive
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .wrap_err("failed to build HTTP client")?;

    let headers_url = format!("{}/eth/v1/beacon/headers/head", url.trim_end_matches('/'));

    let response = client
        .get(&headers_url)
        .send()
        .await
        .wrap_err("failed to send request to CL node")?;

    let body: BeaconHeaderResponse = response
        .json()
        .await
        .wrap_err("failed to parse beacon header response")?;

    body.data
        .header
        .message
        .slot
        .parse::<u64>()
        .wrap_err("failed to parse slot number")
}

/// Check both health and slot for a CL node
pub async fn check_cl_node(url: &str) -> Result<(bool, u64)> {
    // Check health endpoint
    let health_ok = check_cl_health(url).await?;

    // Get current slot
    let slot = check_cl_slot(url).await?;

    Ok((health_ok, slot))
}

/// Find the highest slot across all CL nodes (the chain head)
pub fn update_cl_chain_head(nodes: &[ClNodeState]) -> u64 {
    nodes.iter().map(|n| n.slot).max().unwrap_or(0)
}

/// Calculate health status for a CL node based on chain head and max lag
pub fn calculate_cl_health(node: &mut ClNodeState, chain_head: u64, max_lag: u64) {
    // Calculate lag (how far behind the node is from chain head)
    node.lag = chain_head.saturating_sub(node.slot);

    // Node is healthy if health endpoint is OK AND lag is within threshold
    node.is_healthy = node.health_ok && node.lag <= max_lag;
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // =========================================================================
    // check_cl_health tests
    // =========================================================================

    #[tokio::test]
    async fn test_check_cl_health_returns_true_on_200() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/eth/v1/node/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let result = check_cl_health(&mock_server.uri())
            .await
            .expect("Should check health");

        assert!(result, "Should return true on 200 response");
    }

    #[tokio::test]
    async fn test_check_cl_health_returns_false_on_503() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/eth/v1/node/health"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let result = check_cl_health(&mock_server.uri())
            .await
            .expect("Should check health");

        assert!(!result, "Should return false on 503 response");
    }

    #[tokio::test]
    async fn test_check_cl_health_returns_false_on_connection_failure() {
        // Use an invalid URL that will fail to connect
        let result = check_cl_health("http://localhost:99999")
            .await
            .expect("Should handle connection failure");

        assert!(!result, "Should return false on connection failure");
    }

    // =========================================================================
    // check_cl_slot tests
    // =========================================================================

    #[tokio::test]
    async fn test_check_cl_slot_parses_json() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/eth/v1/beacon/headers/head"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "root": "0x...",
                    "canonical": true,
                    "header": {
                        "message": {
                            "slot": "12345",
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

        let slot = check_cl_slot(&mock_server.uri())
            .await
            .expect("Should parse slot");

        assert_eq!(slot, 12345);
    }

    #[tokio::test]
    async fn test_check_cl_slot_invalid_json() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/eth/v1/beacon/headers/head"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&mock_server)
            .await;

        let result = check_cl_slot(&mock_server.uri()).await;
        assert!(result.is_err(), "Should fail on invalid JSON");
    }

    // =========================================================================
    // calculate_cl_health tests
    // =========================================================================

    fn make_cl_node(name: &str, slot: u64, health_ok: bool) -> ClNodeState {
        ClNodeState {
            name: name.to_string(),
            url: "http://localhost:5052".to_string(),
            slot,
            health_ok,
            is_healthy: false,
            lag: 0,
        }
    }

    #[test]
    fn test_calculate_cl_lag() {
        let mut node = make_cl_node("test", 1000, true);
        let chain_head = 1005;

        calculate_cl_health(&mut node, chain_head, 10);

        assert_eq!(node.lag, 5, "Lag should be chain_head - slot");
    }

    #[test]
    fn test_cl_node_unhealthy_when_health_fails() {
        let mut node = make_cl_node("test", 1000, false); // health_ok = false
        let chain_head = 1000;
        let max_lag = 3;

        calculate_cl_health(&mut node, chain_head, max_lag);

        assert!(
            !node.is_healthy,
            "Node should be unhealthy when health_ok is false"
        );
        assert_eq!(node.lag, 0);
    }

    #[test]
    fn test_cl_node_unhealthy_when_lagging() {
        let mut node = make_cl_node("test", 990, true); // health_ok = true
        let chain_head = 1000;
        let max_lag = 3;

        calculate_cl_health(&mut node, chain_head, max_lag);

        assert!(
            !node.is_healthy,
            "Node should be unhealthy when lag > max_lag"
        );
        assert_eq!(node.lag, 10);
    }

    #[test]
    fn test_cl_node_healthy_when_both_pass() {
        let mut node = make_cl_node("test", 998, true); // health_ok = true
        let chain_head = 1000;
        let max_lag = 3;

        calculate_cl_health(&mut node, chain_head, max_lag);

        assert!(
            node.is_healthy,
            "Node should be healthy when health_ok AND lag <= max_lag"
        );
        assert_eq!(node.lag, 2);
    }

    #[test]
    fn test_cl_node_healthy_at_exact_max_lag() {
        let mut node = make_cl_node("test", 997, true);
        let chain_head = 1000;
        let max_lag = 3;

        calculate_cl_health(&mut node, chain_head, max_lag);

        assert!(
            node.is_healthy,
            "Node should be healthy when lag == max_lag"
        );
        assert_eq!(node.lag, 3);
    }

    // =========================================================================
    // update_cl_chain_head tests
    // =========================================================================

    #[test]
    fn test_update_cl_chain_head_finds_max() {
        let nodes = vec![
            make_cl_node("node1", 1000, true),
            make_cl_node("node2", 1005, true),
            make_cl_node("node3", 998, true),
        ];

        let chain_head = update_cl_chain_head(&nodes);

        assert_eq!(chain_head, 1005, "Chain head should be the maximum slot");
    }

    #[test]
    fn test_update_cl_chain_head_single_node() {
        let nodes = vec![make_cl_node("node1", 1000, true)];

        let chain_head = update_cl_chain_head(&nodes);

        assert_eq!(chain_head, 1000);
    }

    #[test]
    fn test_update_cl_chain_head_empty_returns_zero() {
        let nodes: Vec<ClNodeState> = vec![];

        let chain_head = update_cl_chain_head(&nodes);

        assert_eq!(chain_head, 0, "Empty nodes should return 0");
    }
}
