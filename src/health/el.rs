//! EL (Execution Layer) health checking
//!
//! Checks EL node health by calling eth_getBlockNumber and tracking chain head.

use crate::state::ElNodeState;

/// Parse a hex block number string (with or without 0x prefix) to u64
pub fn parse_hex_block_number(_hex: &str) -> eyre::Result<u64> {
    unimplemented!("parse_hex_block_number not yet implemented")
}

/// Check an EL node's current block number via JSON-RPC
pub async fn check_el_node(_url: &str) -> eyre::Result<u64> {
    unimplemented!("check_el_node not yet implemented")
}

/// Find the highest block number across all EL nodes (the chain head)
pub fn update_el_chain_head(_nodes: &[ElNodeState]) -> u64 {
    unimplemented!("update_el_chain_head not yet implemented")
}

/// Calculate health status for an EL node based on chain head and max lag
pub fn calculate_el_health(_node: &mut ElNodeState, _chain_head: u64, _max_lag: u64) {
    unimplemented!("calculate_el_health not yet implemented")
}

#[cfg(test)]
mod tests {
    // Tests will be added in Phase 5
}
