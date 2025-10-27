use crate::state::AppState;
use hub_core::bus_contract::{EntityAnnounce, TOPIC_ENTITY_ANNOUNCE};
use tokio_stream::StreamExt;

pub fn spawn(app: AppState) {
    tokio::spawn(async move {
        if let Ok(mut stream) = app.bus.subscribe(TOPIC_ENTITY_ANNOUNCE).await {
            while let Some(msg) = stream.next().await {
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
                            tracing::warn!("entity upsert failed: {e}");
                        }
                    }
                    Err(e) => tracing::warn!("bad entity announce payload: {e}"),
                }
            }
        }
    });
}
