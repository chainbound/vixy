//! Node selection logic with health checking and failover support

use crate::state::{ClNodeState, ElNodeState};

/// Select a healthy EL node, preferring primary nodes over backup
pub fn select_el_node(_nodes: &[ElNodeState], _failover_active: bool) -> Option<&ElNodeState> {
    unimplemented!("select_el_node not yet implemented")
}

/// Select a healthy CL node
pub fn select_cl_node(_nodes: &[ClNodeState]) -> Option<&ClNodeState> {
    unimplemented!("select_cl_node not yet implemented")
}

#[cfg(test)]
mod tests {
    // Tests will be added in Phase 8
}
