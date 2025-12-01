use crate::{
    http::{auth, handlers as h},
    state::AppState,
};
use axum::{
    Router, middleware,
    routing::{get, post},
};

pub fn build(state: AppState) -> Router {
    let protected = Router::new()
        .route("/areas", get(h::list_areas))
        .route("/devices", get(h::list_devices))
        .route("/entities", get(h::list_entities))
        .route("/states/{entity_id}", get(h::get_state).post(h::set_state))
        .route("/command/{entity_id}", post(h::send_command))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_auth));

    Router::new().route("/healthz", get(h::healthz)).merge(protected).with_state(state)
}
