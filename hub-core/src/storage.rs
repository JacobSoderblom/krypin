use crate::model::{Area, AreaId, Device, DeviceId, Entity, EntityDomain, EntityId, EntityState};
use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{
    PgPool, QueryBuilder, Row,
    postgres::{PgPoolOptions, PgRow},
};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

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

#[derive(Clone)]
pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new().max_connections(10).connect(database_url).await?;

        sqlx::migrate!().run(&pool).await?;

        Ok(Self { pool })
    }

    fn parse_entity_domain(domain: String) -> Result<EntityDomain> {
        serde_json::from_str(&format!("\"{}\"", domain)).context("invalid entity domain")
    }

    fn parse_metadata(value: serde_json::Value) -> BTreeMap<String, serde_json::Value> {
        serde_json::from_value(value).unwrap_or_default()
    }

    fn row_to_area(row: PgRow) -> Result<Area> {
        Ok(Area {
            id: AreaId(row.try_get("id")?),
            name: row.try_get("name")?,
            parent: row.try_get::<Option<Uuid>, _>("parent")?.map(AreaId),
        })
    }

    fn row_to_device(row: PgRow) -> Result<Device> {
        Ok(Device {
            id: DeviceId(row.try_get("id")?),
            name: row.try_get("name")?,
            adapter: row.try_get("adapter")?,
            manufacturer: row.try_get("manufacturer")?,
            model: row.try_get("model")?,
            sw_version: row.try_get("sw_version")?,
            hw_version: row.try_get("hw_version")?,
            area: row.try_get::<Option<Uuid>, _>("area")?.map(AreaId),
            metadata: Self::parse_metadata(row.try_get("metadata")?),
        })
    }

    fn row_to_entity(row: PgRow) -> Result<Entity> {
        let domain: String = row.try_get("domain")?;
        Ok(Entity {
            id: EntityId(row.try_get("id")?),
            device_id: DeviceId(row.try_get("device_id")?),
            name: row.try_get("name")?,
            domain: Self::parse_entity_domain(domain)?,
            icon: row.try_get("icon")?,
            key: row.try_get("key")?,
            attributes: Self::parse_metadata(row.try_get("attributes")?),
        })
    }

    fn row_to_state(row: PgRow) -> Result<EntityState> {
        Ok(EntityState {
            entity_id: EntityId(row.try_get("entity_id")?),
            value: row.try_get("value")?,
            attributes: Self::parse_metadata(row.try_get("attributes")?),
            last_changed: row.try_get("last_changed")?,
            last_updated: row.try_get("last_updated")?,
            source: row.try_get("source")?,
        })
    }
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

#[async_trait]
impl Storage for PostgresStorage {
    async fn list_areas(&self) -> Result<Vec<Area>> {
        let rows = sqlx::query("SELECT id, name, parent FROM areas").fetch_all(&self.pool).await?;

        Ok(rows.into_iter().map(Self::row_to_area).collect::<Result<Vec<_>>>()?)
    }

    async fn upsert_area(&self, area: Area) -> Result<()> {
        if let Some(parent) = area.parent {
            let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM areas WHERE id = $1")
                .bind(parent.0)
                .fetch_one(&self.pool)
                .await?;
            if exists == 0 {
                return Err(anyhow!("parent area not found: {}", parent.0));
            }
        }

        sqlx::query(
            "INSERT INTO areas (id, name, parent) VALUES ($1, $2, $3)
            ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, parent = EXCLUDED.parent",
        )
        .bind(area.id.0)
        .bind(area.name)
        .bind(area.parent.map(|p| p.0))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_area(&self, id: AreaId) -> Result<Option<Area>> {
        let row = sqlx::query("SELECT id, name, parent FROM areas WHERE id = $1")
            .bind(id.0)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(Self::row_to_area).transpose()?)
    }

    async fn list_devices(&self) -> Result<Vec<Device>> {
        let rows = sqlx::query(
            "SELECT id, name, adapter, manufacturer, model, sw_version, hw_version, area, metadata
            FROM devices",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Self::row_to_device).collect::<Result<Vec<_>>>()?)
    }

    async fn upsert_device(&self, device: Device) -> Result<()> {
        if let Some(area) = device.area {
            let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM areas WHERE id = $1")
                .bind(area.0)
                .fetch_one(&self.pool)
                .await?;
            if exists == 0 {
                return Err(anyhow!("area not found for device: {}", area.0));
            }
        }

        sqlx::query(
            "INSERT INTO devices (id, name, adapter, manufacturer, model, sw_version, hw_version, area, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                adapter = EXCLUDED.adapter,
                manufacturer = EXCLUDED.manufacturer,
                model = EXCLUDED.model,
                sw_version = EXCLUDED.sw_version,
                hw_version = EXCLUDED.hw_version,
                area = EXCLUDED.area,
                metadata = EXCLUDED.metadata",
        )
        .bind(device.id.0)
        .bind(device.name)
        .bind(device.adapter)
        .bind(device.manufacturer)
        .bind(device.model)
        .bind(device.sw_version)
        .bind(device.hw_version)
        .bind(device.area.map(|a| a.0))
        .bind(serde_json::to_value(device.metadata)?)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_device(&self, id: DeviceId) -> Result<Option<Device>> {
        let row = sqlx::query(
            "SELECT id, name, adapter, manufacturer, model, sw_version, hw_version, area, metadata
            FROM devices WHERE id = $1",
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Self::row_to_device).transpose()?)
    }

    async fn list_entities(&self) -> Result<Vec<Entity>> {
        let rows =
            sqlx::query("SELECT id, device_id, name, domain, icon, key, attributes FROM entities")
                .fetch_all(&self.pool)
                .await?;

        Ok(rows.into_iter().map(Self::row_to_entity).collect::<Result<Vec<_>>>()?)
    }

    async fn upsert_entity(&self, entity: Entity) -> Result<()> {
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM devices WHERE id = $1")
            .bind(entity.device_id.0)
            .fetch_one(&self.pool)
            .await?;
        if exists == 0 {
            return Err(anyhow!("device not found for entity: {}", (entity.device_id).0));
        }

        let domain = serde_json::to_value(&entity.domain)
            .context("failed to serialize entity domain")?
            .as_str()
            .context("invalid entity domain value")?
            .to_string();

        sqlx::query(
            "INSERT INTO entities (id, device_id, name, domain, icon, key, attributes)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE SET
                device_id = EXCLUDED.device_id,
                name = EXCLUDED.name,
                domain = EXCLUDED.domain,
                icon = EXCLUDED.icon,
                key = EXCLUDED.key,
                attributes = EXCLUDED.attributes",
        )
        .bind(entity.id.0)
        .bind(entity.device_id.0)
        .bind(entity.name)
        .bind(domain)
        .bind(entity.icon)
        .bind(entity.key)
        .bind(serde_json::to_value(entity.attributes)?)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_entity(&self, id: EntityId) -> Result<Option<Entity>> {
        let row = sqlx::query(
            "SELECT id, device_id, name, domain, icon, key, attributes FROM entities WHERE id = $1",
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Self::row_to_entity).transpose()?)
    }

    async fn set_entity_state(&self, state: EntityState) -> Result<()> {
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM entities WHERE id = $1")
            .bind(state.entity_id.0)
            .fetch_one(&self.pool)
            .await?;
        if exists == 0 {
            return Err(anyhow!("entity not found: {}", (state.entity_id).0));
        }

        sqlx::query(
            "INSERT INTO entity_states (entity_id, value, attributes, last_changed, last_updated, source)
            VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(state.entity_id.0)
        .bind(state.value)
        .bind(serde_json::to_value(state.attributes)?)
        .bind(state.last_changed)
        .bind(state.last_updated)
        .bind(state.source)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn latest_entity_state(&self, id: EntityId) -> Result<Option<EntityState>> {
        let row = sqlx::query(
            "SELECT entity_id, value, attributes, last_changed, last_updated, source
            FROM entity_states WHERE entity_id = $1 ORDER BY last_updated DESC LIMIT 1",
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Self::row_to_state).transpose()?)
    }

    async fn entity_state_history(
        &self,
        id: EntityId,
        since: Option<DateTime<Utc>>,
        limit: usize,
    ) -> Result<Vec<EntityState>> {
        let mut qb = QueryBuilder::new(
            "SELECT entity_id, value, attributes, last_changed, last_updated, source FROM entity_states WHERE entity_id = ",
        );
        qb.push_bind(id.0);
        if let Some(since_ts) = since {
            qb.push(" AND last_changed >= ");
            qb.push_bind(since_ts);
        }
        qb.push(" ORDER BY last_updated DESC LIMIT ");
        qb.push_bind(limit as i64);

        let rows = qb.build().fetch_all(&self.pool).await?;

        Ok(rows.into_iter().map(Self::row_to_state).collect::<Result<Vec<_>>>()?)
    }
}
