use crate::state::AppState;
use automations::{AutomationId, NewAutomation, TriggerEvent};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use uuid::Uuid;

pub async fn create(
    State(app): State<AppState>,
    Json(body): Json<NewAutomation>,
) -> impl IntoResponse {
    match app.automations.create_automation(body).await {
        Ok(def) => (StatusCode::CREATED, Json(def)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

pub async fn list(State(app): State<AppState>) -> impl IntoResponse {
    match app.automations.list_automations().await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn enable(State(app): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    update_enabled(app, id, true).await
}

pub async fn disable(State(app): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    update_enabled(app, id, false).await
}

pub async fn test(
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(event): Json<TriggerEvent>,
) -> impl IntoResponse {
    let Ok(parsed) = Uuid::try_parse(&id).map(AutomationId) else {
        return (StatusCode::BAD_REQUEST, "invalid id").into_response();
    };
    match app.automations.test_automation(parsed, event).await {
        Ok(result) => Json(result).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn update_enabled(app: AppState, id: String, enabled: bool) -> impl IntoResponse {
    let Ok(parsed) = Uuid::try_parse(&id).map(AutomationId) else {
        return (StatusCode::BAD_REQUEST, "invalid id").into_response();
    };
    match app.automations.set_enabled(parsed, enabled).await {
        Ok(a) => Json(a).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}
