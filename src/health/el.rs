//! EL (Execution Layer) health checking
//!
//! Checks EL node health by calling eth_getBlockNumber and tracking chain head.

use crate::state::ElNodeState;
use eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};

/// JSON-RPC request structure
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: &'static str,
    params: Vec<()>,
    id: u32,
}

/// JSON-RPC response structure
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<String>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC error structure
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

/// Parse a hex block number string (with or without 0x prefix) to u64
pub fn parse_hex_block_number(hex: &str) -> Result<u64> {
    if hex.is_empty() {
        return Err(eyre!("empty hex string"));
    }

    // Strip 0x prefix if present
    let hex_str = hex.strip_prefix("0x").unwrap_or(hex);

    if hex_str.is_empty() {
        return Err(eyre!("empty hex string after prefix"));
    }

    u64::from_str_radix(hex_str, 16).wrap_err_with(|| format!("invalid hex number: {hex}"))
}

/// Check an EL node's current block number via JSON-RPC
pub async fn check_el_node(url: &str) -> Result<u64> {
    let client = reqwest::Client::new();

    let request = JsonRpcRequest {
        jsonrpc: "2.0",
        method: "eth_blockNumber",
        params: vec![],
        id: 1,
    };

    let response = client
        .post(url)
        .json(&request)
        .send()
        .await
        .wrap_err("failed to send request to EL node")?;

    let rpc_response: JsonRpcResponse = response
        .json()
        .await
        .wrap_err("failed to parse JSON-RPC response")?;

    if let Some(error) = rpc_response.error {
        return Err(eyre!("JSON-RPC error {}: {}", error.code, error.message));
    }

    let result = rpc_response
        .result
        .ok_or_else(|| eyre!("missing result in JSON-RPC response"))?;

    parse_hex_block_number(&result)
}

/// Find the highest block number across all EL nodes (the chain head)
pub fn update_el_chain_head(nodes: &[ElNodeState]) -> u64 {
    nodes.iter().map(|n| n.block_number).max().unwrap_or(0)
}

/// Calculate health status for an EL node based on chain head and max lag
pub fn calculate_el_health(node: &mut ElNodeState, chain_head: u64, max_lag: u64) {
    // Calculate lag (how far behind the node is from chain head)
    node.lag = chain_head.saturating_sub(node.block_number);

    // Node is healthy if lag is within the allowed threshold
    node.is_healthy = node.lag <= max_lag;
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // =========================================================================
    // parse_hex_block_number tests
    // =========================================================================

    #[test]
    fn test_parse_hex_block_number_with_prefix() {
        let result = parse_hex_block_number("0x10d4f").expect("Should parse valid hex");
        assert_eq!(result, 68943);
    }

    #[test]
    fn test_parse_hex_block_number_without_prefix() {
        let result = parse_hex_block_number("10d4f").expect("Should parse hex without prefix");
        assert_eq!(result, 68943);
    }

    #[test]
    fn test_parse_hex_block_number_zero() {
        let result = parse_hex_block_number("0x0").expect("Should parse zero");
        assert_eq!(result, 0);
    }

    #[test]
    fn test_parse_hex_block_number_large() {
        // A large block number like on mainnet
        let result = parse_hex_block_number("0x12A05F200").expect("Should parse large number");
        assert_eq!(result, 5_000_000_000);
    }

    #[test]
    fn test_parse_hex_block_number_invalid() {
        let result = parse_hex_block_number("not-hex");
        assert!(result.is_err(), "Should fail on invalid hex");
    }

    #[test]
    fn test_parse_hex_block_number_empty() {
        let result = parse_hex_block_number("");
        assert!(result.is_err(), "Should fail on empty string");
    }

    // =========================================================================
    // check_el_node tests (with wiremock)
    // =========================================================================

    #[tokio::test]
    async fn test_check_el_node_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_blockNumber",
                "params": [],
                "id": 1
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "0x10d4f",
                "id": 1
            })))
            .mount(&mock_server)
            .await;

        let block_number = check_el_node(&mock_server.uri())
            .await
            .expect("Should get block number");

        assert_eq!(block_number, 68943);
    }

    #[tokio::test]
    async fn test_check_el_node_timeout() {
        let mock_server = MockServer::start().await;

        // Don't mount any mock - request will fail

        let result = check_el_node(&mock_server.uri()).await;
        assert!(result.is_err(), "Should fail on timeout/no response");
    }

    #[tokio::test]
    async fn test_check_el_node_invalid_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "not-a-hex-number",
                "id": 1
            })))
            .mount(&mock_server)
            .await;

        let result = check_el_node(&mock_server.uri()).await;
        assert!(result.is_err(), "Should fail on invalid hex in response");
    }

    // =========================================================================
    // calculate_el_lag / calculate_el_health tests
    // =========================================================================

    fn make_el_node(name: &str, block_number: u64) -> ElNodeState {
        ElNodeState {
            name: name.to_string(),
            http_url: "http://localhost:8545".to_string(),
            ws_url: "ws://localhost:8546".to_string(),
            is_primary: true,
            block_number,
            is_healthy: false,
            lag: 0,
        }
    }

    #[test]
    fn test_calculate_el_lag() {
        let mut node = make_el_node("test", 1000);
        let chain_head = 1005;

        calculate_el_health(&mut node, chain_head, 10);

        assert_eq!(node.lag, 5, "Lag should be chain_head - block_number");
    }

    #[test]
    fn test_el_node_healthy_within_lag() {
        let mut node = make_el_node("test", 1000);
        let chain_head = 1002;
        let max_lag = 5;

        calculate_el_health(&mut node, chain_head, max_lag);

        assert!(
            node.is_healthy,
            "Node should be healthy when lag <= max_lag"
        );
        assert_eq!(node.lag, 2);
    }

    #[test]
    fn test_el_node_unhealthy_exceeds_lag() {
        let mut node = make_el_node("test", 990);
        let chain_head = 1000;
        let max_lag = 5;

        calculate_el_health(&mut node, chain_head, max_lag);

        assert!(
            !node.is_healthy,
            "Node should be unhealthy when lag > max_lag"
        );
        assert_eq!(node.lag, 10);
    }

    #[test]
    fn test_el_node_healthy_at_exact_max_lag() {
        let mut node = make_el_node("test", 995);
        let chain_head = 1000;
        let max_lag = 5;

        calculate_el_health(&mut node, chain_head, max_lag);

        assert!(
            node.is_healthy,
            "Node should be healthy when lag == max_lag"
        );
        assert_eq!(node.lag, 5);
    }

    // =========================================================================
    // update_el_chain_head tests
    // =========================================================================

    #[test]
    fn test_update_chain_head_finds_max() {
        let nodes = vec![
            make_el_node("node1", 1000),
            make_el_node("node2", 1005),
            make_el_node("node3", 998),
        ];

        let chain_head = update_el_chain_head(&nodes);

        assert_eq!(
            chain_head, 1005,
            "Chain head should be the maximum block number"
        );
    }

    #[test]
    fn test_update_chain_head_single_node() {
        let nodes = vec![make_el_node("node1", 1000)];

        let chain_head = update_el_chain_head(&nodes);

        assert_eq!(chain_head, 1000);
    }

    #[test]
    fn test_update_chain_head_empty_returns_zero() {
        let nodes: Vec<ElNodeState> = vec![];

        let chain_head = update_el_chain_head(&nodes);

        assert_eq!(chain_head, 0, "Empty nodes should return 0");
    }
}
