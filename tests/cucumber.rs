//! BDD test harness using cucumber
//!
//! This runs unit-level BDD tests. For integration tests against real
//! infrastructure, use `cargo test --test integration_cucumber`.

mod steps;
mod world;

use cucumber::World;
use world::VixyWorld;

/// Check if any tag matches "integration" (case insensitive)
fn has_integration_tag(tags: &[String]) -> bool {
    tags.iter().any(|t| t.to_lowercase() == "integration")
}

fn main() {
    // Run cucumber unit tests synchronously
    // Excludes @integration tagged scenarios by using filter_run
    futures::executor::block_on(
        VixyWorld::cucumber()
            .max_concurrent_scenarios(1)
            .filter_run("tests/features", |feature, _rule, scenario| {
                // Skip if feature is tagged with @integration
                if has_integration_tag(&feature.tags) {
                    return false;
                }

                // Skip if scenario is tagged with @integration
                // scenario might be Option<&Scenario> or &Scenario depending on version
                if has_integration_tag(&scenario.tags) {
                    return false;
                }

                true
            }),
    );
}
