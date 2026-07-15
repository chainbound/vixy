//! Health checking for EL and CL nodes

use std::time::Duration;

pub mod cl;
pub mod el;
pub mod subscription;

/// Per-probe deadline, applied per request so a hung node can't stall a
/// monitor cycle. Probes run on the shared proxy client (`AppState::http_client`)
/// so they measure the same pooled connections that proxied traffic uses,
/// rather than paying a fresh DNS + TCP + TLS handshake per probe.
pub(crate) const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
