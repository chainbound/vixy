//! Health monitoring loop
//!
//! Background task that periodically checks all EL and CL nodes and updates their health state.

use std::sync::Arc;

use crate::state::AppState;

/// Run the health monitoring loop
///
/// This function runs forever, periodically checking all nodes and updating their health state.
pub async fn run_health_monitor(_state: Arc<AppState>, _interval_ms: u64) {
    unimplemented!("run_health_monitor not yet implemented")
}

#[cfg(test)]
mod tests {
    // Tests will be added in Phase 7
}
