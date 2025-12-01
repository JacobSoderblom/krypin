use crate::{http::auth::AuthenticatedUser, state::AppState};
use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use bytes::Bytes;
use chrono::Utc;
use hub_core::{
    bus_contract::{CommandSet, TOPIC_COMMAND_PREFIX},
    model::{EntityId, EntityState},
};
use uuid::Uuid;

pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

pub async fn list_areas(State(app): State<AppState>) -> impl IntoResponse {
    match app.store.list_areas().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_devices(State(app): State<AppState>) -> impl IntoResponse {
    match app.store.list_devices().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_entities(State(app): State<AppState>) -> impl IntoResponse {
    match app.store.list_entities().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn get_state(
    State(app): State<AppState>,
    Path(entity_id): Path<String>,
) -> impl IntoResponse {
    let Ok(eid) = parse_entity_id(&entity_id) else {
        return (StatusCode::BAD_REQUEST, "invalid entity_id").into_response();
    };
    match app.store.latest_entity_state(eid).await {
        Ok(Some(s)) => Json(s).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "no state").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
pub struct SetStateBody {
    value: serde_json::Value,
    #[serde(default)]
    attributes: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    source: Option<String>,
}

#[derive(serde::Serialize)]
pub struct SetStateResp {
    ok: bool,
}

pub async fn set_state(
    State(app): State<AppState>,
    Path(entity_id): Path<String>,
    maybe_user: Option<Extension<AuthenticatedUser>>,
    Json(body): Json<SetStateBody>,
) -> impl IntoResponse {
    let Ok(eid) = parse_entity_id(&entity_id) else {
        return (StatusCode::BAD_REQUEST, "invalid entity_id").into_response();
    };
    let now = Utc::now();
    let source =
        body.source.or_else(|| maybe_user.as_ref().map(|Extension(user)| user.label().to_string()));
    let state = EntityState {
        entity_id: eid,
        value: body.value,
        attributes: body.attributes.into_iter().collect(),
        last_changed: now,
        last_updated: now,
        source,
    };
    match app.store.set_entity_state(state).await {
        Ok(_) => Json(SetStateResp { ok: true }).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

pub async fn send_command(
    State(app): State<AppState>,
    Path(entity_id): Path<String>,
    maybe_user: Option<Extension<AuthenticatedUser>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let Ok(eid) = parse_entity_id(&entity_id) else {
        return (StatusCode::BAD_REQUEST, "invalid entity_id").into_response();
    };
    let cmd = CommandSet {
        action: body.get("action").and_then(|v| v.as_str()).unwrap_or("set").to_string(),
        value: body.get("value").cloned().unwrap_or(serde_json::Value::Null),
        correlation_id: body
            .get("correlation_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::try_parse(s).ok()),
    };
    let topic = format!("{TOPIC_COMMAND_PREFIX}{}", (eid.0));
    let payload = Bytes::from(serde_json::to_vec(&cmd).unwrap());
    let user_label = maybe_user.as_ref().map(|Extension(user)| user.label()).unwrap_or("anonymous");
    tracing::info!(entity_id = %eid.0, user = %user_label, "sending command" );
    if let Err(e) = app.bus.publish(&topic, payload).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    (StatusCode::ACCEPTED, "").into_response()
}

fn parse_entity_id(s: &str) -> Result<EntityId, ()> {
    Uuid::try_parse(s).map(EntityId).map_err(|_| ())
}
