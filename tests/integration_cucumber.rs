//! Integration test harness using cucumber
//!
//! This runs integration tests against real Kurtosis Ethereum infrastructure.
//!
//! Prerequisites:
//!   1. Setup Kurtosis: `just kurtosis-up`
//!   2. Start Vixy: `just kurtosis-vixy`
//!   3. Run tests: `just kurtosis-test`
//!
//! Or all at once: `just integration-test`

mod steps;
mod world;

use cucumber::World;
use world::IntegrationWorld;

#[tokio::main]
async fn main() {
    // Check if we should run integration tests
    let skip_check = std::env::var("VIXY_SKIP_INTEGRATION_CHECK").is_ok();

    if !skip_check {
        // Verify Vixy is running before starting tests
        let client = reqwest::Client::new();
        match client
            .get("http://127.0.0.1:8080/status")
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                eprintln!("✓ Vixy is running at http://127.0.0.1:8080");
            }
            _ => {
                eprintln!("╔════════════════════════════════════════════════════════════════╗");
                eprintln!("║  Integration tests require Kurtosis infrastructure!            ║");
                eprintln!("╠════════════════════════════════════════════════════════════════╣");
                eprintln!("║  Quick start:                                                  ║");
                eprintln!("║     just integration-test                                      ║");
                eprintln!("║                                                                ║");
                eprintln!("║  Or step by step:                                              ║");
                eprintln!("║     1. just kurtosis-up      # Start Kurtosis enclave          ║");
                eprintln!("║     2. just kurtosis-vixy    # Start Vixy with Kurtosis config ║");
                eprintln!("║     3. just kurtosis-test    # Run integration tests           ║");
                eprintln!("║                                                                ║");
                eprintln!("║  Cleanup:                                                      ║");
                eprintln!("║     just kurtosis-down       # Stop Kurtosis enclave           ║");
                eprintln!("╚════════════════════════════════════════════════════════════════╝");
                eprintln!();
                eprintln!("Skipping integration tests (Vixy not running)");
                return;
            }
        }
    }

    // Run integration cucumber tests with Tokio runtime
    IntegrationWorld::cucumber()
        .with_default_cli()
        // Run only integration features
        .run("tests/features/integration")
        .await;
}
