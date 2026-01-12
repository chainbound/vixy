//! Integration test harness using cucumber
//!
//! This runs integration tests against real Docker/Kurtosis infrastructure.
//! Prerequisites:
//!   1. Start infrastructure: `cd docker && docker-compose up -d`
//!   2. Start Vixy: `cargo run -- --config docker/vixy-integration.toml`
//!   3. Run tests: `cargo test --test integration_cucumber`
//!
//! Or use the helper script: `./scripts/run-integration-tests.sh`

mod steps;
mod world;

use cucumber::World;
use world::IntegrationWorld;

fn main() {
    // Check if we should run integration tests
    let skip_check = std::env::var("VIXY_SKIP_INTEGRATION_CHECK").is_ok();

    if !skip_check {
        // Verify Vixy is running before starting tests
        let client = reqwest::blocking::Client::new();
        match client
            .get("http://127.0.0.1:8080/status")
            .timeout(std::time::Duration::from_secs(2))
            .send()
        {
            Ok(resp) if resp.status().is_success() => {
                eprintln!("✓ Vixy is running at http://127.0.0.1:8080");
            }
            _ => {
                eprintln!("╔════════════════════════════════════════════════════════════════╗");
                eprintln!("║  Integration tests require running infrastructure!             ║");
                eprintln!("╠════════════════════════════════════════════════════════════════╣");
                eprintln!("║  1. Start Docker containers:                                   ║");
                eprintln!("║     cd docker && docker-compose up -d                          ║");
                eprintln!("║                                                                ║");
                eprintln!("║  2. Start Vixy:                                                ║");
                eprintln!("║     cargo run -- --config docker/vixy-integration.toml         ║");
                eprintln!("║                                                                ║");
                eprintln!("║  3. Run tests:                                                 ║");
                eprintln!("║     cargo test --test integration_cucumber                     ║");
                eprintln!("║                                                                ║");
                eprintln!("║  Or use: ./scripts/run-integration-tests.sh                    ║");
                eprintln!("╚════════════════════════════════════════════════════════════════╝");
                eprintln!();
                eprintln!("Skipping integration tests (Vixy not running)");
                return;
            }
        }
    }

    // Run integration cucumber tests
    futures::executor::block_on(
        IntegrationWorld::cucumber()
            .with_default_cli()
            // Run only integration features
            .run("tests/features/integration"),
    );
}
