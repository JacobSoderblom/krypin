use crate::state::AppState;
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    label: String,
}

impl AuthenticatedUser {
    fn from_token(token: String) -> Self {
        let suffix: String =
            token.chars().rev().take(4).collect::<String>().chars().rev().collect();
        Self { label: format!("token:{suffix}") }
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

pub async fn require_auth(
    State(app): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    if !app.auth.is_enabled() {
        return next.run(req).await;
    }

    let bearer = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| v.trim().to_string());

    let api_key =
        req.headers().get("x-api-key").and_then(|h| h.to_str().ok()).map(|v| v.to_string());

    let Some(token) = bearer.or(api_key) else {
        return unauthorized();
    };

    if !app.auth.matches(&token) {
        return unauthorized();
    }

    req.extensions_mut().insert(AuthenticatedUser::from_token(token));
    next.run(req).await
}

fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, "unauthorized").into_response()
}
