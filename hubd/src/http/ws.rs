use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::{
    Router,
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use tokio::select;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    #[serde(default = "default_pattern")]
    pub pattern: String,
}

fn default_pattern() -> String {
    "*".to_string()
}

pub fn router() -> Router<AppState> {
    Router::new().route("/ws/events", get(events))
}

pub async fn events(
    ws: WebSocketUpgrade,
    State(app): State<AppState>,
    Query(params): Query<WsQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, app, params))
}

async fn handle_ws(stream: WebSocket, app: AppState, params: WsQuery) {
    let mut ws = stream;
    let Ok(mut sub) = app.bus.subscribe(&params.pattern).await else {
        let _ = ws.send(Message::Close(None)).await;
        return;
    };

    loop {
        select! {
            Some(msg) = ws.next() => {
                if let Some(Message::Close(_)) = msg.ok() {
                    break;
                }
            }
            maybe_bus = sub.next() => {
                match maybe_bus {
                    Some(bus_msg) => {
                        let payload = serde_json::from_slice::<Value>(&bus_msg.payload)
                            .unwrap_or_else(|_| Value::String(STANDARD.encode(&bus_msg.payload)));
                        let json = serde_json::json!({
                            "id": Uuid::new_v4(),
                            "topic": bus_msg.topic,
                            "payload": payload,
                        });
                        if ws.send(Message::Text(json.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
    }
}
