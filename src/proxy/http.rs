//! HTTP proxy handlers for EL and CL requests

use axum::extract::State;
use axum::response::Response;
use std::sync::Arc;

use crate::state::AppState;

/// Handle EL HTTP proxy requests (POST /el)
pub async fn el_proxy_handler(
    State(_state): State<Arc<AppState>>,
    _request: axum::extract::Request,
) -> Response {
    unimplemented!("el_proxy_handler not yet implemented")
}

/// Handle CL HTTP proxy requests (GET/POST /cl/*)
pub async fn cl_proxy_handler(
    State(_state): State<Arc<AppState>>,
    _request: axum::extract::Request,
) -> Response {
    unimplemented!("cl_proxy_handler not yet implemented")
}

#[cfg(test)]
mod tests {
    // Tests will be added in Phase 8
}
