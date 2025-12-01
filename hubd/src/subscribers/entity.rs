use crate::state::AppState;
use chrono::Utc;
use hub_core::bus_contract::{EntityAnnounce, TOPIC_ENTITY_ANNOUNCE};
use metrics::{counter, histogram};
use tokio_stream::StreamExt;

pub fn spawn(app: AppState) {
    tokio::spawn(async move {
        if let Ok(mut stream) = app.bus.subscribe(TOPIC_ENTITY_ANNOUNCE).await {
            while let Some(msg) = stream.next().await {
                let latency_ms = (Utc::now() - msg.received_at).num_milliseconds();
                histogram!("bus.message.latency_ms").record(latency_ms as f64);
                match serde_json::from_slice::<EntityAnnounce>(&msg.payload) {
                    Ok(v) => {
                        let entity = hub_core::model::Entity {
                            id: v.id,
                            device_id: v.device_id,
                            name: v.name,
                            domain: v.domain,
                            icon: v.icon,
                            key: v.key,
                            attributes: v.attributes,
                        };
                        if let Err(e) = app.store.upsert_entity(entity).await {
                            counter!("bus.message.handle_error").increment(1);
                            tracing::warn!("entity upsert failed: {e}");
                        }
                    }
                    Err(e) => {
                        counter!("bus.message.decode_error").increment(1);
                        tracing::warn!("bad entity announce payload: {e}");
                    }
                }
            }
        }
    });
}
