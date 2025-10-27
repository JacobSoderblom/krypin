use crate::{http::handlers as h, state::AppState};
use axum::{
    Router,
    routing::{get, post},
};

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(h::healthz))
        .route("/areas", get(h::list_areas))
        .route("/devices", get(h::list_devices))
        .route("/entities", get(h::list_entities))
        .route("/states/{entity_id}", get(h::get_state).post(h::set_state_dev_only))
        .route("/command/{entity_id}", post(h::send_command))
        .with_state(state)
}
