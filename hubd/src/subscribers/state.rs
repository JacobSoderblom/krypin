use crate::state::AppState;
use hub_core::bus_contract::{StateUpdate, TOPIC_STATE_UPDATE_PREFIX};
use tokio_stream::StreamExt;

pub fn spawn(app: AppState) {
    tokio::spawn(async move {
        let pattern = format!("{TOPIC_STATE_UPDATE_PREFIX}*");
        if let Ok(mut stream) = app.bus.subscribe(&pattern).await {
            while let Some(msg) = stream.next().await {
                match serde_json::from_slice::<StateUpdate>(&msg.payload) {
                    Ok(v) => {
                        let st = hub_core::model::EntityState {
                            entity_id: v.entity_id,
                            value: v.value,
                            attributes: v.attributes,
                            last_changed: v.ts,
                            last_updated: v.ts,
                            source: v.source,
                        };
                        if let Err(e) = app.store.set_entity_state(st).await {
                            tracing::warn!("state set failed: {e}");
                        }
                    }
                    Err(e) => tracing::warn!("bad state update payload: {e}"),
                }
            }
        }
    });
}
