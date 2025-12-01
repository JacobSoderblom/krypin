pub mod config;
pub mod grpc;
pub mod heartbeat;
pub mod http;
pub mod state;
pub mod subscribers;
pub mod telemetry;
pub mod wiring;

use crate::{
    config::Config, grpc::serve_grpc, http::serve, telemetry::init_tracing, wiring::build_state,
};

pub async fn run(cfg: Config) -> anyhow::Result<()> {
    init_tracing(&cfg)?;
    let app_state = build_state(&cfg).await?;
    heartbeat::spawn(app_state.bus.clone());
    subscribers::spawn_all(app_state.clone());
    let http = serve(app_state.clone(), cfg.clone());
    let grpc = serve_grpc(app_state, cfg);
    tokio::try_join!(http, grpc).map(|_| ())
}
