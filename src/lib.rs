//! Vixy - Ethereum EL and CL Proxy
//!
//! A Rust proxy that monitors Ethereum Execution Layer (EL) and Consensus Layer (CL) nodes,
//! tracks their health, and routes requests to healthy nodes.

pub mod config;
pub mod health;
pub mod metrics;
pub mod monitor;
pub mod proxy;
pub mod state;
