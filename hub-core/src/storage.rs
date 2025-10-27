use crate::model::{Area, AreaId, Device, DeviceId, Entity, EntityId, EntityState};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[async_trait]
pub trait Storage: Send + Sync {
    async fn list_areas(&self) -> Result<Vec<Area>>;
    async fn upsert_area(&self, area: Area) -> Result<()>;
    async fn get_area(&self, id: AreaId) -> Result<Option<Area>>;

    async fn list_devices(&self) -> Result<Vec<Device>>;
    async fn upsert_device(&self, device: Device) -> Result<()>;
    async fn get_device(&self, id: DeviceId) -> Result<Option<Device>>;

    async fn list_entities(&self) -> Result<Vec<Entity>>;
    async fn upsert_entity(&self, entity: Entity) -> Result<()>;
    async fn get_entity(&self, id: EntityId) -> Result<Option<Entity>>;

    async fn set_entity_state(&self, state: EntityState) -> Result<()>;
    async fn latest_entity_state(&self, id: EntityId) -> Result<Option<EntityState>>;
    async fn entity_state_history(
        &self,
        id: EntityId,
        since: Option<DateTime<Utc>>,
        limit: usize,
    ) -> Result<Vec<EntityState>>;
}

#[derive(Default, Clone)]
pub struct InMemoryStorage {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Default)]
struct Inner {
    areas: HashMap<AreaId, Area>,
    devices: HashMap<DeviceId, Device>,
    entities: HashMap<EntityId, Entity>,
    states_by_entity: HashMap<EntityId, Vec<EntityState>>,
}

#[async_trait]
impl Storage for InMemoryStorage {
    async fn list_areas(&self) -> Result<Vec<Area>> {
        let g = self.inner.read().unwrap();
        Ok(g.areas.values().cloned().collect())
    }

    async fn upsert_area(&self, area: Area) -> Result<()> {
        let mut g = self.inner.write().unwrap();
        match area.parent {
            Some(parent) => {
                if !g.areas.contains_key(&parent) {
                    return Err(anyhow!("parent area not found: {}", parent.0));
                }
            }
            None => todo!(),
        }
        g.areas.insert(area.id, area);
        Ok(())
    }

    async fn get_area(&self, id: AreaId) -> Result<Option<Area>> {
        let g = self.inner.read().unwrap();
        Ok(g.areas.get(&id).cloned())
    }

    async fn list_devices(&self) -> Result<Vec<Device>> {
        let g = self.inner.read().unwrap();
        Ok(g.devices.values().cloned().collect())
    }

    async fn upsert_device(&self, device: Device) -> Result<()> {
        let mut g = self.inner.write().unwrap();
        match device.area {
            Some(area) => {
                if !g.areas.contains_key(&area) {
                    return Err(anyhow!("area not found for device: {}", area.0));
                }
            }
            None => todo!(),
        }
        g.devices.insert(device.id, device);
        Ok(())
    }

    async fn get_device(&self, id: DeviceId) -> Result<Option<Device>> {
        let g = self.inner.read().unwrap();
        Ok(g.devices.get(&id).cloned())
    }

    async fn list_entities(&self) -> Result<Vec<Entity>> {
        let g = self.inner.read().unwrap();
        Ok(g.entities.values().cloned().collect())
    }

    async fn upsert_entity(&self, entity: Entity) -> Result<()> {
        let mut g = self.inner.write().unwrap();
        if !g.devices.contains_key(&entity.device_id) {
            return Err(anyhow!("device not found for entity: {}", (entity.device_id).0));
        }
        g.entities.insert(entity.id, entity);
        Ok(())
    }

    async fn get_entity(&self, id: EntityId) -> Result<Option<Entity>> {
        let g = self.inner.read().unwrap();
        Ok(g.entities.get(&id).cloned())
    }

    async fn set_entity_state(&self, state: EntityState) -> Result<()> {
        let mut g = self.inner.write().unwrap();
        if !g.entities.contains_key(&state.entity_id) {
            return Err(anyhow!("entity not found: {}", (state.entity_id).0));
        }
        let entry = g.states_by_entity.entry(state.entity_id).or_default();
        entry.push(state);
        entry.sort_by_key(|s| s.last_changed);
        Ok(())
    }

    async fn latest_entity_state(&self, id: EntityId) -> Result<Option<EntityState>> {
        let g = self.inner.read().unwrap();
        Ok(g.states_by_entity.get(&id).and_then(|v| v.last().cloned()))
    }

    async fn entity_state_history(
        &self,
        id: EntityId,
        since: Option<DateTime<Utc>>,
        limit: usize,
    ) -> Result<Vec<EntityState>> {
        let g = self.inner.read().unwrap();
        let mut list: Vec<EntityState> = match g.states_by_entity.get(&id) {
            Some(v) => {
                if let Some(since_ts) = since {
                    v.iter().filter(|s| s.last_changed >= since_ts).cloned().collect()
                } else {
                    v.clone()
                }
            }
            None => Vec::new(),
        };
        if list.len() > limit {
            let drop = list.len() - limit;
            list.drain(0..drop);
        }
        Ok(list)
    }
}
