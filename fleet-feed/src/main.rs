#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

mod actions;
mod bead_store;
mod branch_landing;
mod calculations;
mod data;
mod dolt_health;
mod generation;
mod launcher;
mod polecat_restart;
mod polecat_status;
mod review_beads;
mod scheduling;
mod session_cleanup;
mod worktree_health;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    actions::run_fleet_feed().await;
}
