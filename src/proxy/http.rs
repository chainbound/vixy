//! HTTP proxy handlers for EL and CL requests

use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use crate::metrics::VixyMetrics;
use crate::proxy::selection;
use crate::state::AppState;

/// Default timeout for proxy requests
const DEFAULT_TIMEOUT_MS: u64 = 30000;

/// Handle EL HTTP proxy requests (POST /el)
pub async fn el_proxy_handler(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> Response {
    let start = Instant::now();

    // Read the failover flag
    let failover_active = state.el_failover_active.load(Ordering::SeqCst);

    // Get a read lock on EL nodes and extract what we need
    let (target_url, node_name, tier) = {
        let el_nodes = state.el_nodes.read().await;

        // Select a healthy node
        match selection::select_el_node(&el_nodes, failover_active) {
            Some(n) => {
                let tier = if n.is_primary { "primary" } else { "backup" };
                (n.http_url.clone(), n.name.clone(), tier)
            }
            None => {
                warn!("No healthy EL node available");
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "No healthy EL node available",
                )
                    .into_response();
            }
        }
    };

    debug!(target_url, node_name, tier, "Proxying EL request");

    // Forward the request
    let response = forward_request(request, &target_url).await;

    // Record metrics
    let duration = start.elapsed().as_secs_f64();
    VixyMetrics::inc_el_requests(&node_name, tier);
    VixyMetrics::observe_el_duration(&node_name, tier, duration);

    response
}

/// Handle CL HTTP proxy requests (GET/POST /cl/*)
pub async fn cl_proxy_handler(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> Response {
    let start = Instant::now();

    // Get a read lock on CL nodes and extract what we need
    let (target_url, node_name) = {
        let cl_nodes = state.cl_nodes.read().await;

        // Select a healthy node
        match selection::select_cl_node(&cl_nodes) {
            Some(n) => (n.url.clone(), n.name.clone()),
            None => {
                warn!("No healthy CL node available");
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "No healthy CL node available",
                )
                    .into_response();
            }
        }
    };

    // Extract the path from the request (strip /cl prefix)
    let path = request.uri().path();
    let cl_path = path
        .strip_prefix("/cl/")
        .or_else(|| path.strip_prefix("/cl"))
        .unwrap_or(path);
    // Ensure path starts with / for proper URL construction
    let cl_path = if cl_path.is_empty() || cl_path == "/" {
        ""
    } else if cl_path.starts_with('/') {
        cl_path
    } else {
        // This shouldn't happen but handle it gracefully
        cl_path
    };
    let query = request
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();

    // Build full URL, ensuring proper slash handling
    let base_url = target_url.trim_end_matches('/');
    let full_url = if cl_path.is_empty() {
        format!("{base_url}{query}")
    } else if cl_path.starts_with('/') {
        format!("{base_url}{cl_path}{query}")
    } else {
        format!("{base_url}/{cl_path}{query}")
    };

    debug!(full_url, node_name, "Proxying CL request");

    // Forward the request to the constructed URL
    let response = forward_request_to_url(request, &full_url).await;

    // Record metrics
    let duration = start.elapsed().as_secs_f64();
    VixyMetrics::inc_cl_requests(&node_name);
    VixyMetrics::observe_cl_duration(&node_name, duration);

    response
}

/// Forward a request to a target URL
async fn forward_request(request: Request<Body>, target_url: &str) -> Response {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS))
        .build()
        .expect("Failed to build HTTP client");

    // Extract method, headers, and body
    let method = request.method().clone();
    let content_type = request
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let body_bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            warn!(error = %e, "Failed to read request body");
            return (StatusCode::BAD_REQUEST, "Failed to read request body").into_response();
        }
    };

    // Build the forwarded request with Content-Type header
    let mut forward_request = client.request(method, target_url);
    if let Some(ct) = content_type {
        forward_request = forward_request.header("content-type", ct);
    }
    forward_request = forward_request.body(body_bytes);

    // Send the request
    match forward_request.send().await {
        Ok(response) => convert_response(response).await,
        Err(e) => {
            if e.is_timeout() {
                warn!(error = %e, "Proxy request timed out");
                return (StatusCode::GATEWAY_TIMEOUT, "Request timed out").into_response();
            }
            warn!(error = %e, "Proxy request failed");
            (StatusCode::BAD_GATEWAY, "Upstream request failed").into_response()
        }
    }
}

/// Forward a request to a specific URL (used for CL with path construction)
async fn forward_request_to_url(request: Request<Body>, target_url: &str) -> Response {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS))
        .build()
        .expect("Failed to build HTTP client");

    // Extract method, headers, and body
    let method = request.method().clone();
    let content_type = request
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let body_bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            warn!(error = %e, "Failed to read request body");
            return (StatusCode::BAD_REQUEST, "Failed to read request body").into_response();
        }
    };

    // Build the forwarded request with Content-Type header
    let mut forward_request = client.request(method, target_url);
    if let Some(ct) = content_type {
        forward_request = forward_request.header("content-type", ct);
    }
    if !body_bytes.is_empty() {
        forward_request = forward_request.body(body_bytes);
    }

    // Send the request
    match forward_request.send().await {
        Ok(response) => convert_response(response).await,
        Err(e) => {
            if e.is_timeout() {
                warn!(error = %e, "Proxy request timed out");
                return (StatusCode::GATEWAY_TIMEOUT, "Request timed out").into_response();
            }
            warn!(error = %e, "Proxy request failed");
            (StatusCode::BAD_GATEWAY, "Upstream request failed").into_response()
        }
    }
}

/// Convert a reqwest response to an axum response
async fn convert_response(response: reqwest::Response) -> Response {
    let status = StatusCode::from_u16(response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    match response.bytes().await {
        Ok(bytes) => (status, bytes.to_vec()).into_response(),
        Err(e) => {
            warn!(error = %e, "Failed to read response body");
            (StatusCode::BAD_GATEWAY, "Failed to read upstream response").into_response()
        }
    }
}

// ============================================================================
// Status endpoint
// ============================================================================

/// EL node status for JSON response
#[derive(Debug, Serialize)]
pub struct ElNodeStatus {
    pub name: String,
    pub http_url: String,
    pub is_primary: bool,
    pub block_number: u64,
    pub lag: u64,
    pub check_ok: bool,
    pub is_healthy: bool,
}

/// CL node status for JSON response
#[derive(Debug, Serialize)]
pub struct ClNodeStatus {
    pub name: String,
    pub url: String,
    pub slot: u64,
    pub lag: u64,
    pub health_ok: bool,
    pub is_healthy: bool,
}

/// Full status response
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub el_chain_head: u64,
    pub cl_chain_head: u64,
    pub el_failover_active: bool,
    pub el_nodes: Vec<ElNodeStatus>,
    pub cl_nodes: Vec<ClNodeStatus>,
}

/// Handle status requests (GET /status)
///
/// Returns JSON with all node health states
pub async fn status_handler(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    let el_chain_head = state.el_chain_head.load(Ordering::SeqCst);
    let cl_chain_head = state.cl_chain_head.load(Ordering::SeqCst);
    let el_failover_active = state.el_failover_active.load(Ordering::SeqCst);

    // Collect EL node statuses
    let el_nodes = {
        let nodes = state.el_nodes.read().await;
        nodes
            .iter()
            .map(|n| ElNodeStatus {
                name: n.name.clone(),
                http_url: n.http_url.clone(),
                is_primary: n.is_primary,
                block_number: n.block_number,
                lag: n.lag,
                check_ok: n.check_ok,
                is_healthy: n.is_healthy,
            })
            .collect()
    };

    // Collect CL node statuses
    let cl_nodes = {
        let nodes = state.cl_nodes.read().await;
        nodes
            .iter()
            .map(|n| ClNodeStatus {
                name: n.name.clone(),
                url: n.url.clone(),
                slot: n.slot,
                lag: n.lag,
                health_ok: n.health_ok,
                is_healthy: n.is_healthy,
            })
            .collect()
    };

    Json(StatusResponse {
        el_chain_head,
        cl_chain_head,
        el_failover_active,
        el_nodes,
        cl_nodes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ClNodeState, ElNodeState};
    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tower::util::ServiceExt;
    use wiremock::matchers::{body_string, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Helper to create minimal AppState for testing
    fn create_test_state(el_nodes: Vec<ElNodeState>, cl_nodes: Vec<ClNodeState>) -> Arc<AppState> {
        Arc::new(AppState {
            el_nodes: Arc::new(RwLock::new(el_nodes)),
            cl_nodes: Arc::new(RwLock::new(cl_nodes)),
            el_chain_head: std::sync::atomic::AtomicU64::new(0),
            cl_chain_head: std::sync::atomic::AtomicU64::new(0),
            el_failover_active: std::sync::atomic::AtomicBool::new(false),
            max_el_lag: 5,
            max_cl_lag: 3,
            proxy_timeout_ms: 30000,
            max_retries: 2,
        })
    }

    fn make_el_node(name: &str, url: &str, is_healthy: bool) -> ElNodeState {
        ElNodeState {
            name: name.to_string(),
            http_url: url.to_string(),
            ws_url: url.to_string(),
            is_primary: true,
            block_number: 1000,
            check_ok: is_healthy,
            is_healthy,
            lag: 0,
        }
    }

    fn make_cl_node(name: &str, url: &str, is_healthy: bool) -> ClNodeState {
        ClNodeState {
            name: name.to_string(),
            url: url.to_string(),
            slot: 5000,
            health_ok: is_healthy,
            is_healthy,
            lag: 0,
        }
    }

    // =========================================================================
    // EL proxy tests
    // =========================================================================

    #[tokio::test]
    async fn test_el_proxy_forwards_request() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(body_string(
                r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#,
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "0x10d4f",
                "id": 1
            })))
            .mount(&mock_server)
            .await;

        let el_nodes = vec![make_el_node("geth-1", &mock_server.uri(), true)];
        let state = create_test_state(el_nodes, vec![]);

        let app = Router::new()
            .route("/el", axum::routing::post(el_proxy_handler))
            .with_state(state);

        let request = Request::builder()
            .method("POST")
            .uri("/el")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#,
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["result"], "0x10d4f");
    }

    #[tokio::test]
    async fn test_el_proxy_returns_503_no_healthy_nodes() {
        let el_nodes = vec![make_el_node("geth-1", "http://localhost:8545", false)]; // unhealthy
        let state = create_test_state(el_nodes, vec![]);

        let app = Router::new()
            .route("/el", axum::routing::post(el_proxy_handler))
            .with_state(state);

        let request = Request::builder()
            .method("POST")
            .uri("/el")
            .body(Body::from("{}"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    // =========================================================================
    // CL proxy tests
    // =========================================================================

    #[tokio::test]
    async fn test_cl_proxy_forwards_get_request() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/eth/v1/node/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let cl_nodes = vec![make_cl_node("lighthouse-1", &mock_server.uri(), true)];
        let state = create_test_state(vec![], cl_nodes);

        let app = Router::new()
            .route("/cl/{*path}", axum::routing::get(cl_proxy_handler))
            .with_state(state);

        let request = Request::builder()
            .method("GET")
            .uri("/cl/eth/v1/node/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_cl_proxy_preserves_path() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/eth/v1/beacon/headers/head"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "root": "0x...",
                    "header": { "message": { "slot": "12345" } }
                }
            })))
            .mount(&mock_server)
            .await;

        let cl_nodes = vec![make_cl_node("lighthouse-1", &mock_server.uri(), true)];
        let state = create_test_state(vec![], cl_nodes);

        let app = Router::new()
            .route("/cl/{*path}", axum::routing::get(cl_proxy_handler))
            .with_state(state);

        let request = Request::builder()
            .method("GET")
            .uri("/cl/eth/v1/beacon/headers/head")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["data"]["header"]["message"]["slot"], "12345");
    }

    #[tokio::test]
    async fn test_cl_proxy_returns_503_no_healthy_nodes() {
        let cl_nodes = vec![make_cl_node("lighthouse-1", "http://localhost:5052", false)]; // unhealthy
        let state = create_test_state(vec![], cl_nodes);

        let app = Router::new()
            .route("/cl/{*path}", axum::routing::get(cl_proxy_handler))
            .with_state(state);

        let request = Request::builder()
            .method("GET")
            .uri("/cl/eth/v1/node/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    // Note: timeout test is hard to implement without mocking the client
    // We've verified the timeout handling code is in place

    // =========================================================================
    // Trailing slash tests
    // =========================================================================

    #[tokio::test]
    async fn test_el_proxy_with_trailing_slash() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": "0x123",
                "id": 1
            })))
            .mount(&mock_server)
            .await;

        let el_nodes = vec![make_el_node("geth-1", &mock_server.uri(), true)];
        let state = create_test_state(el_nodes, vec![]);

        let app = Router::new()
            .route("/el", axum::routing::post(el_proxy_handler))
            .route("/el/", axum::routing::post(el_proxy_handler))
            .with_state(state);

        // Test with trailing slash
        let request = Request::builder()
            .method("POST")
            .uri("/el/")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#,
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_cl_proxy_with_trailing_slash_base() {
        let mock_server = MockServer::start().await;

        // The path should be /eth/v1/beacon/genesis when called via /cl/eth/v1/beacon/genesis
        Mock::given(method("GET"))
            .and(path("/eth/v1/beacon/genesis"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "genesis_time": "1234567890"
                }
            })))
            .mount(&mock_server)
            .await;

        let cl_nodes = vec![make_cl_node("lighthouse-1", &mock_server.uri(), true)];
        let state = create_test_state(vec![], cl_nodes);

        let app = Router::new()
            .route("/cl", axum::routing::any(cl_proxy_handler))
            .route("/cl/", axum::routing::any(cl_proxy_handler))
            .route("/cl/{*path}", axum::routing::any(cl_proxy_handler))
            .with_state(state);

        // Test /cl/ + eth/v1/beacon/genesis (simulating mk1's URL joining)
        let request = Request::builder()
            .method("GET")
            .uri("/cl/eth/v1/beacon/genesis")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["data"]["genesis_time"], "1234567890");
    }

    #[tokio::test]
    async fn test_cl_proxy_bare_cl_path() {
        let mock_server = MockServer::start().await;

        // Mock the root path
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string("OK"))
            .mount(&mock_server)
            .await;

        let cl_nodes = vec![make_cl_node("lighthouse-1", &mock_server.uri(), true)];
        let state = create_test_state(vec![], cl_nodes);

        let app = Router::new()
            .route("/cl", axum::routing::any(cl_proxy_handler))
            .route("/cl/", axum::routing::any(cl_proxy_handler))
            .with_state(state);

        // Test bare /cl
        let request = Request::builder()
            .method("GET")
            .uri("/cl")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Should either succeed or return 404 from upstream (not 503)
        assert_ne!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
