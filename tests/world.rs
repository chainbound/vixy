//! Test world state for BDD tests

use cucumber::World;
use futures_util::stream::{SplitSink, SplitStream};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};

/// Type alias for WebSocket stream
pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
/// Type alias for WebSocket sender
pub type WsSender = SplitSink<WsStream, Message>;
/// Type alias for WebSocket receiver
pub type WsReceiver = SplitStream<WsStream>;

/// The test world state for Vixy BDD tests (unit tests)
#[derive(Debug, Default, World)]
pub struct VixyWorld {
    /// Raw TOML configuration string for parsing tests
    pub config_toml: Option<String>,
    /// Loaded configuration (if any)
    pub config: Option<vixy::config::Config>,
    /// EL node states for testing
    pub el_nodes: Vec<vixy::state::ElNodeState>,
    /// CL node states for testing
    pub cl_nodes: Vec<vixy::state::ClNodeState>,
    /// Current EL chain head for health check tests
    pub el_chain_head: u64,
    /// Current CL chain head for health check tests
    pub cl_chain_head: u64,
    /// Max EL lag threshold for health check tests
    pub max_el_lag: u64,
    /// Max CL lag threshold for health check tests
    pub max_cl_lag: u64,
    /// The last error message (if any)
    pub last_error: Option<String>,
}

/// Container for WebSocket connection state (not Debug because streams don't implement it)
pub struct WsConnection {
    /// WebSocket sender half
    pub sender: WsSender,
    /// WebSocket receiver half
    pub receiver: WsReceiver,
}

/// The test world state for Vixy integration tests
/// Used for tests that run against real Docker/Kurtosis infrastructure
#[derive(Default, World)]
pub struct IntegrationWorld {
    /// Vixy server URL (e.g., "http://127.0.0.1:8080")
    pub vixy_url: Option<String>,
    /// Last HTTP status code received
    pub last_status_code: Option<u16>,
    /// Last response body received
    pub last_response_body: Option<String>,
    /// Number of healthy EL nodes
    pub healthy_el_count: usize,
    /// Number of healthy CL nodes
    pub healthy_cl_count: usize,
    /// List of stopped Docker containers (for cleanup)
    pub stopped_containers: Vec<String>,
    /// Whether WebSocket is connected
    pub ws_connected: bool,
    /// WebSocket connection (sender + receiver)
    pub ws_connection: Option<WsConnection>,
    /// Subscription ID from eth_subscribe response
    pub subscription_id: Option<String>,
    /// Last received subscription event (for verification)
    pub last_subscription_event: Option<serde_json::Value>,
}

impl std::fmt::Debug for IntegrationWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntegrationWorld")
            .field("vixy_url", &self.vixy_url)
            .field("last_status_code", &self.last_status_code)
            .field("last_response_body", &self.last_response_body)
            .field("healthy_el_count", &self.healthy_el_count)
            .field("healthy_cl_count", &self.healthy_cl_count)
            .field("stopped_containers", &self.stopped_containers)
            .field("ws_connected", &self.ws_connected)
            .field("ws_connection", &self.ws_connection.is_some())
            .field("subscription_id", &self.subscription_id)
            .field("last_subscription_event", &self.last_subscription_event)
            .finish()
    }
}
