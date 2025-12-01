use crate::state::AppState;
use chrono::Utc;
use hub_core::bus_contract::{DeviceAnnounce, TOPIC_DEVICE_ANNOUNCE};
use metrics::{counter, histogram};
use tokio_stream::StreamExt;

pub fn spawn(app: AppState) {
    tokio::spawn(async move {
        if let Ok(mut stream) = app.bus.subscribe(TOPIC_DEVICE_ANNOUNCE).await {
            while let Some(msg) = stream.next().await {
                let latency_ms = (Utc::now() - msg.received_at).num_milliseconds();
                histogram!("bus.message.latency_ms").record(latency_ms as f64);
                match serde_json::from_slice::<DeviceAnnounce>(&msg.payload) {
                    Ok(v) => {
                        let device = hub_core::model::Device {
                            id: v.id,
                            name: v.name,
                            adapter: v.adapter,
                            manufacturer: v.manufacturer,
                            model: v.model,
                            sw_version: v.sw_version,
                            hw_version: v.hw_version,
                            area: v.area.map(hub_core::model::AreaId),
                            metadata: v.metadata,
                        };
                        if let Err(e) = app.store.upsert_device(device).await {
                            counter!("bus.message.handle_error").increment(1);
                            tracing::warn!("device upsert failed: {e}");
                        }
                    }
                    Err(e) => {
                        counter!("bus.message.decode_error").increment(1);
                        tracing::warn!("bad device announce payload: {e}");
                    }
                }
            }
        }
    });
}
