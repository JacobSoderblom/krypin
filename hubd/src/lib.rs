pub mod config;
pub mod heartbeat;
pub mod http;
pub mod state;
pub mod subscribers;
pub mod telemetry;
pub mod wiring;

use crate::{config::Config, http::serve, telemetry::init_tracing, wiring::build_state};

pub async fn run(cfg: Config) -> anyhow::Result<()> {
    init_tracing(&cfg)?;
    let app_state = build_state(&cfg).await?;
    heartbeat::spawn(app_state.bus.clone());
    subscribers::spawn_all(app_state.clone());
    serve(app_state, cfg).await
}
