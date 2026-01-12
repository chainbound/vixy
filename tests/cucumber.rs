//! BDD test harness using cucumber

mod steps;
mod world;

use cucumber::World;
use world::VixyWorld;

fn main() {
    // Run cucumber tests synchronously
    futures::executor::block_on(VixyWorld::run("tests/features"));
}
