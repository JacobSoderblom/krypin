use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
};
use chrono::{DateTime, Utc};
use hub_core::{
    bus::Bus,
    model::{EntityId, EntityState},
    storage::Storage,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod samples;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AutomationId(pub Uuid);

impl AutomationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for AutomationId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationDefinition {
    pub id: AutomationId,
    pub name: String,
    pub description: Option<String>,
    pub trigger: Trigger,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    Time { cron: String },
    StateChange { entity_id: EntityId, from: Option<Value>, to: Option<Value> },
    MqttTopic { pattern: String },
    Heartbeat,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    Always,
    EntityStateEquals { entity_id: EntityId, value: Value },
    PayloadEquals { path: String, value: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    SetEntityState {
        entity_id: EntityId,
        value: Value,
        #[serde(default)]
        attributes: BTreeMap<String, Value>,
    },
    PublishBusMessage {
        topic: String,
        payload: Value,
    },
    Log {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerEvent {
    TimeFired { cron: String },
    StateChanged { entity_id: EntityId, from: Option<Value>, to: Value },
    MqttMessage { topic: String, payload: Value },
    Heartbeat { ts: DateTime<Utc> },
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAutomation {
    pub name: String,
    pub description: Option<String>,
    pub trigger: Trigger,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRun {
    pub automation_id: AutomationId,
    pub executed: bool,
    pub reason: Option<String>,
}

#[async_trait]
pub trait AutomationStore: Send + Sync {
    async fn insert(&self, automation: AutomationDefinition) -> Result<AutomationDefinition>;
    async fn list(&self) -> Result<Vec<AutomationDefinition>>;
    async fn update(&self, automation: AutomationDefinition) -> Result<AutomationDefinition>;
    async fn get(&self, id: AutomationId) -> Result<Option<AutomationDefinition>>;
}

#[derive(Default, Clone)]
pub struct InMemoryAutomationStore {
    inner: Arc<RwLock<HashMap<AutomationId, AutomationDefinition>>>,
}

#[async_trait]
impl AutomationStore for InMemoryAutomationStore {
    async fn insert(&self, automation: AutomationDefinition) -> Result<AutomationDefinition> {
        let mut g = self.inner.write().await;
        g.insert(automation.id, automation.clone());
        Ok(automation)
    }

    async fn list(&self) -> Result<Vec<AutomationDefinition>> {
        let g = self.inner.read().await;
        Ok(g.values().cloned().collect())
    }

    async fn update(&self, automation: AutomationDefinition) -> Result<AutomationDefinition> {
        let mut g = self.inner.write().await;
        if !g.contains_key(&automation.id) {
            return Err(anyhow!("automation not found"));
        }
        g.insert(automation.id, automation.clone());
        Ok(automation)
    }

    async fn get(&self, id: AutomationId) -> Result<Option<AutomationDefinition>> {
        let g = self.inner.read().await;
        Ok(g.get(&id).cloned())
    }
}

#[derive(Clone)]
pub struct AutomationEngine {
    store: Arc<dyn AutomationStore>,
    storage: Arc<dyn Storage>,
    bus: Arc<dyn Bus>,
}

impl AutomationEngine {
    pub fn new(
        store: Arc<dyn AutomationStore>,
        storage: Arc<dyn Storage>,
        bus: Arc<dyn Bus>,
    ) -> Self {
        Self { store, storage, bus }
    }

    pub async fn create_automation(&self, new: NewAutomation) -> Result<AutomationDefinition> {
        let now = Utc::now();
        let automation = AutomationDefinition {
            id: AutomationId::new(),
            name: new.name,
            description: new.description,
            trigger: new.trigger,
            conditions: new.conditions,
            actions: new.actions,
            enabled: new.enabled,
            created_at: now,
            updated_at: now,
        };
        self.store.insert(automation).await
    }

    pub async fn list_automations(&self) -> Result<Vec<AutomationDefinition>> {
        self.store.list().await
    }

    pub async fn set_enabled(
        &self,
        id: AutomationId,
        enabled: bool,
    ) -> Result<AutomationDefinition> {
        let mut automation =
            self.store.get(id).await?.ok_or_else(|| anyhow!("automation not found"))?;
        automation.enabled = enabled;
        automation.updated_at = Utc::now();
        self.store.update(automation).await
    }

    pub async fn handle_event(&self, event: TriggerEvent) -> Result<()> {
        let automations = self.store.list().await?;
        for automation in automations.into_iter().filter(|a| a.enabled) {
            if !trigger_matches(&automation.trigger, &event) {
                continue;
            }
            if !self.conditions_hold(&automation.conditions, &event).await? {
                continue;
            }
            self.execute_actions(&automation.actions, &event).await?;
        }
        Ok(())
    }

    pub async fn test_automation(&self, id: AutomationId, event: TriggerEvent) -> Result<TestRun> {
        let Some(automation) = self.store.get(id).await? else {
            return Ok(TestRun {
                automation_id: id,
                executed: false,
                reason: Some("automation not found".into()),
            });
        };
        if !trigger_matches(&automation.trigger, &event) {
            return Ok(TestRun {
                automation_id: id,
                executed: false,
                reason: Some("trigger did not match".into()),
            });
        }
        if !self.conditions_hold(&automation.conditions, &event).await? {
            return Ok(TestRun {
                automation_id: id,
                executed: false,
                reason: Some("conditions failed".into()),
            });
        }
        self.execute_actions(&automation.actions, &event).await?;
        Ok(TestRun { automation_id: id, executed: true, reason: None })
    }

    async fn conditions_hold(
        &self,
        conditions: &[Condition],
        event: &TriggerEvent,
    ) -> Result<bool> {
        for condition in conditions {
            if !self.condition_holds(condition, event).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    async fn condition_holds(&self, condition: &Condition, event: &TriggerEvent) -> Result<bool> {
        match condition {
            Condition::Always => Ok(true),
            Condition::EntityStateEquals { entity_id, value } => match event {
                TriggerEvent::StateChanged { entity_id: ev_id, to, .. } if ev_id == entity_id => {
                    Ok(to == value)
                }
                _ => {
                    let state = self.storage.latest_entity_state(*entity_id).await?;
                    Ok(state.map(|s| &s.value == value).unwrap_or(false))
                }
            },
            Condition::PayloadEquals { path, value } => match event {
                TriggerEvent::MqttMessage { payload, .. } => {
                    Ok(payload.pointer(path).map(|p| p == value).unwrap_or(false))
                }
                _ => Ok(false),
            },
        }
    }

    async fn execute_actions(&self, actions: &[Action], event: &TriggerEvent) -> Result<()> {
        for action in actions {
            self.execute_action(action, event).await?;
        }
        Ok(())
    }

    async fn execute_action(&self, action: &Action, event: &TriggerEvent) -> Result<()> {
        match action {
            Action::SetEntityState { entity_id, value, attributes } => {
                let now = Utc::now();
                let state = EntityState {
                    entity_id: *entity_id,
                    value: value.clone(),
                    attributes: attributes.clone(),
                    last_changed: now,
                    last_updated: now,
                    source: Some("automation".into()),
                };
                self.storage.set_entity_state(state).await?;
                Ok(())
            }
            Action::PublishBusMessage { topic, payload } => {
                let bytes = serde_json::to_vec(payload).context("serializing bus payload")?;
                self.bus.publish(topic, bytes.into()).await
            }
            Action::Log { message } => {
                tracing::info!(target: "automation", "{} - event: {:?}", message, event);
                Ok(())
            }
        }
    }
}

fn trigger_matches(trigger: &Trigger, event: &TriggerEvent) -> bool {
    match (trigger, event) {
        (Trigger::Manual, TriggerEvent::Manual) => true,
        (Trigger::Time { cron }, TriggerEvent::TimeFired { cron: ev }) => cron == ev,
        (
            Trigger::StateChange { entity_id, from, to },
            TriggerEvent::StateChanged { entity_id: ev, from: old, to: new },
        ) => {
            if entity_id != ev {
                return false;
            }
            if let Some(expected) = from
                && old.as_ref() != Some(expected)
            {
                return false;
            }
            if let Some(expected) = to
                && new != expected
            {
                return false;
            }
            true
        }
        (Trigger::MqttTopic { pattern }, TriggerEvent::MqttMessage { topic, .. }) => {
            topic_matches(pattern, topic)
        }
        (Trigger::Heartbeat, TriggerEvent::Heartbeat { .. }) => true,
        _ => false,
    }
}

fn topic_matches(pattern: &str, topic: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if pattern == topic {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix(".*") {
        return topic == prefix || topic.starts_with(&(prefix.to_string() + "."));
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return topic.starts_with(prefix);
    }
    false
}

#[derive(Clone)]
pub struct ApiState {
    engine: Arc<AutomationEngine>,
}

impl ApiState {
    pub fn new(engine: Arc<AutomationEngine>) -> Self {
        Self { engine }
    }
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/automations", post(create_automation).get(list_automations))
        .route("/automations/:id/enable", post(enable_automation))
        .route("/automations/:id/disable", post(disable_automation))
        .route("/automations/:id/test", post(test_automation))
        .with_state(state)
}

async fn create_automation(
    State(state): State<ApiState>,
    Json(body): Json<NewAutomation>,
) -> impl IntoResponse {
    match state.engine.create_automation(body).await {
        Ok(a) => (StatusCode::CREATED, Json(a)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn list_automations(State(state): State<ApiState>) -> impl IntoResponse {
    match state.engine.list_automations().await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn enable_automation(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    update_enabled(state, id, true).await
}

async fn disable_automation(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    update_enabled(state, id, false).await
}

async fn test_automation(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(event): Json<TriggerEvent>,
) -> impl IntoResponse {
    let Ok(parsed) = Uuid::try_parse(&id).map(AutomationId) else {
        return (StatusCode::BAD_REQUEST, "invalid id").into_response();
    };
    match state.engine.test_automation(parsed, event).await {
        Ok(result) => Json(result).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn update_enabled(state: ApiState, id: String, enabled: bool) -> impl IntoResponse {
    let Ok(parsed) = Uuid::try_parse(&id).map(AutomationId) else {
        return (StatusCode::BAD_REQUEST, "invalid id").into_response();
    };
    match state.engine.set_enabled(parsed, enabled).await {
        Ok(a) => Json(a).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use hub_core::bus::InMemoryBus;
    use uuid::Uuid;

    #[derive(Default, Clone)]
    struct DummyStorage {
        inner: Arc<RwLock<HashMap<EntityId, EntityState>>>,
    }

    #[async_trait::async_trait]
    impl Storage for DummyStorage {
        async fn list_areas(&self) -> Result<Vec<hub_core::model::Area>> {
            Err(anyhow!("not implemented"))
        }

        async fn upsert_area(&self, _area: hub_core::model::Area) -> Result<()> {
            Err(anyhow!("not implemented"))
        }

        async fn get_area(
            &self,
            _id: hub_core::model::AreaId,
        ) -> Result<Option<hub_core::model::Area>> {
            Err(anyhow!("not implemented"))
        }

        async fn list_devices(&self) -> Result<Vec<hub_core::model::Device>> {
            Err(anyhow!("not implemented"))
        }

        async fn upsert_device(&self, _device: hub_core::model::Device) -> Result<()> {
            Err(anyhow!("not implemented"))
        }

        async fn get_device(
            &self,
            _id: hub_core::model::DeviceId,
        ) -> Result<Option<hub_core::model::Device>> {
            Err(anyhow!("not implemented"))
        }

        async fn list_entities(&self) -> Result<Vec<hub_core::model::Entity>> {
            Err(anyhow!("not implemented"))
        }

        async fn upsert_entity(&self, _entity: hub_core::model::Entity) -> Result<()> {
            Err(anyhow!("not implemented"))
        }

        async fn get_entity(&self, _id: EntityId) -> Result<Option<hub_core::model::Entity>> {
            Err(anyhow!("not implemented"))
        }

        async fn set_entity_state(&self, state: EntityState) -> Result<()> {
            let mut guard = self.inner.write().await;
            guard.insert(state.entity_id, state);
            Ok(())
        }

        async fn latest_entity_state(&self, id: EntityId) -> Result<Option<EntityState>> {
            let guard = self.inner.read().await;
            Ok(guard.get(&id).cloned())
        }

        async fn entity_state_history(
            &self,
            _id: EntityId,
            _since: Option<DateTime<Utc>>,
            _limit: usize,
        ) -> Result<Vec<EntityState>> {
            Err(anyhow!("not implemented"))
        }
    }

    #[test]
    fn heartbeat_trigger_matches_event() {
        let trigger = Trigger::Heartbeat;
        let event = TriggerEvent::Heartbeat { ts: Utc::now() };

        assert!(trigger_matches(&trigger, &event));
    }

    #[tokio::test]
    async fn runs_automation_on_state_change() {
        let store = Arc::new(InMemoryAutomationStore::default());
        let storage = Arc::new(DummyStorage::default());
        let bus = Arc::new(InMemoryBus::default());
        let engine = AutomationEngine::new(store, storage.clone(), bus);

        let entity_id = EntityId(Uuid::new_v4());
        let automation = engine
            .create_automation(NewAutomation {
                name: "set scene".into(),
                description: None,
                trigger: Trigger::StateChange { entity_id, from: None, to: None },
                conditions: vec![Condition::Always],
                actions: vec![Action::SetEntityState {
                    entity_id,
                    value: Value::String("on".into()),
                    attributes: BTreeMap::new(),
                }],
                enabled: true,
            })
            .await
            .unwrap();

        let event =
            TriggerEvent::StateChanged { entity_id, from: None, to: Value::String("off".into()) };
        engine.handle_event(event).await.unwrap();

        let latest = storage.latest_entity_state(entity_id).await.unwrap().unwrap();
        assert_eq!(latest.value, Value::String("on".into()));
        assert!(automation.enabled);
    }

    #[tokio::test]
    async fn runs_automation_on_heartbeat() {
        let store = Arc::new(InMemoryAutomationStore::default());
        let storage = Arc::new(DummyStorage::default());
        let bus = Arc::new(InMemoryBus::default());
        let engine = AutomationEngine::new(store, storage.clone(), bus);

        let entity_id = EntityId(Uuid::new_v4());
        engine
            .create_automation(NewAutomation {
                name: "heartbeat scene".into(),
                description: None,
                trigger: Trigger::Heartbeat,
                conditions: vec![Condition::Always],
                actions: vec![Action::SetEntityState {
                    entity_id,
                    value: Value::Bool(true),
                    attributes: BTreeMap::new(),
                }],
                enabled: true,
            })
            .await
            .unwrap();

        engine.handle_event(TriggerEvent::Heartbeat { ts: Utc::now() }).await.unwrap();

        let latest = storage.latest_entity_state(entity_id).await.unwrap().unwrap();
        assert_eq!(latest.value, Value::Bool(true));
    }

    #[tokio::test]
    async fn sample_motion_light_turns_on_light() {
        let store = Arc::new(InMemoryAutomationStore::default());
        let storage = Arc::new(DummyStorage::default());
        let bus = Arc::new(InMemoryBus::default());
        let engine = AutomationEngine::new(store, storage.clone(), bus);

        let motion = EntityId(Uuid::new_v4());
        let light = EntityId(Uuid::new_v4());
        engine.create_automation(samples::motion_light(motion, light)).await.unwrap();

        engine
            .handle_event(TriggerEvent::StateChanged {
                entity_id: motion,
                from: None,
                to: Value::Bool(true),
            })
            .await
            .unwrap();

        let latest = storage.latest_entity_state(light).await.unwrap().unwrap();
        assert_eq!(latest.value, Value::String("on".into()));
    }

    #[tokio::test]
    async fn sample_schedule_sets_temperature() {
        let store = Arc::new(InMemoryAutomationStore::default());
        let storage = Arc::new(DummyStorage::default());
        let bus = Arc::new(InMemoryBus::default());
        let engine = AutomationEngine::new(store, storage.clone(), bus);

        let thermostat = EntityId(Uuid::new_v4());
        let cron = "0 7 * * *";
        engine
            .create_automation(samples::thermostat_schedule(thermostat, 21.0, cron))
            .await
            .unwrap();

        engine.handle_event(TriggerEvent::TimeFired { cron: cron.to_string() }).await.unwrap();

        let latest = storage.latest_entity_state(thermostat).await.unwrap().unwrap();
        assert_eq!(latest.value, Value::Number(serde_json::Number::from_f64(21.0).unwrap()));
        assert_eq!(latest.attributes.get("unit"), Some(&Value::String("C".into())));
    }
}
