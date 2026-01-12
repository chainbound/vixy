//! WebSocket proxy for EL subscriptions

use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::response::Response;
use std::sync::Arc;

use crate::state::AppState;

/// Handle EL WebSocket upgrade requests (GET /el/ws)
pub async fn el_ws_handler(State(_state): State<Arc<AppState>>, _ws: WebSocketUpgrade) -> Response {
    unimplemented!("el_ws_handler not yet implemented")
}

#[cfg(test)]
mod tests {
    // Tests will be added in Phase 8
}
