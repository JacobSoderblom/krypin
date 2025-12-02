use crate::state::AppState;
use automations::TriggerEvent;
use hub_core::bus_contract::{Heartbeat, TOPIC_HEARTBEAT};
use tokio_stream::StreamExt;

pub fn spawn(app: AppState) {
    tokio::spawn(async move {
        if let Ok(mut stream) = app.bus.subscribe(TOPIC_HEARTBEAT).await {
            while let Some(msg) = stream.next().await {
                match serde_json::from_slice::<Heartbeat>(&msg.payload) {
                    Ok(hb) => {
                        if let Err(e) = app
                            .automations
                            .handle_event(TriggerEvent::Heartbeat { ts: hb.ts })
                            .await
                        {
                            tracing::warn!("automation heartbeat dispatch failed: {e}");
                        }
                    }
                    Err(e) => tracing::warn!("bad heartbeat payload: {e}"),
                }
            }
        }
    });
}
