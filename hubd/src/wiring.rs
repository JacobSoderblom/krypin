use std::sync::Arc;

use adapter_mqtt::MqttBus;
use anyhow::{Ok, Result};
use hub_core::{
    bus::{Bus, InMemoryBus},
    storage::{InMemoryStorage, Storage},
};

use crate::{
    config::{BusKind, Config, StorageKind},
    state::AppState,
};

pub async fn build_state(cfg: &Config) -> Result<AppState> {
    let bus: Arc<dyn Bus> = match cfg.bus.clone() {
        BusKind::InMem => Arc::new(InMemoryBus::default()),
        BusKind::Mqtt => {
            Arc::new(MqttBus::connect(&cfg.mqtt.host, cfg.mqtt.port, &cfg.mqtt.client_id).await?)
        }
    };

    let store: Arc<dyn Storage> = match cfg.storage {
        StorageKind::InMem => Arc::new(InMemoryStorage::default()),
    };

    Ok(AppState { store, bus, auth: cfg.auth.clone() })
}
