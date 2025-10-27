pub mod handlers;
mod routes;

use crate::{config::Config, state::AppState};
use anyhow::Result;
use axum::Router;

pub async fn serve(app_state: AppState, cfg: Config) -> Result<()> {
    let app: Router = routes::build(app_state);
    tracing::info!("krypin hub listening on http://{}", cfg.bind);
    axum::serve(tokio::net::TcpListener::bind(cfg.bind).await?, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
