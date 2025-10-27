use std::sync::Arc;

use anyhow::{Ok, Result, bail};
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
        other => bail!("unsupported bus: {other}"),
    };

    let store: Arc<dyn Storage> = match cfg.storage {
        StorageKind::InMem => Arc::new(InMemoryStorage::default()),
    };

    Ok(AppState { store, bus })
}
