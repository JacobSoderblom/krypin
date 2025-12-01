use anyhow::{Result, anyhow};
use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use hub_core::{
    bus::Message,
    model::{EntityId, EntityState},
    storage::Storage,
};
use hubd::{config::AuthConfig, http::build_router, state::AppState};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone, Default)]
struct RecordingStorage {
    last_state: Arc<Mutex<Option<EntityState>>>,
}

#[async_trait]
impl Storage for RecordingStorage {
    async fn list_areas(&self) -> Result<Vec<hub_core::model::Area>> {
        Ok(Vec::new())
    }

    async fn upsert_area(&self, _area: hub_core::model::Area) -> Result<()> {
        Err(anyhow!("unimplemented"))
    }

    async fn get_area(
        &self,
        _id: hub_core::model::AreaId,
    ) -> Result<Option<hub_core::model::Area>> {
        Ok(None)
    }

    async fn list_devices(&self) -> Result<Vec<hub_core::model::Device>> {
        Ok(Vec::new())
    }

    async fn upsert_device(&self, _device: hub_core::model::Device) -> Result<()> {
        Err(anyhow!("unimplemented"))
    }

    async fn get_device(
        &self,
        _id: hub_core::model::DeviceId,
    ) -> Result<Option<hub_core::model::Device>> {
        Ok(None)
    }

    async fn list_entities(&self) -> Result<Vec<hub_core::model::Entity>> {
        Ok(Vec::new())
    }

    async fn upsert_entity(&self, _entity: hub_core::model::Entity) -> Result<()> {
        Err(anyhow!("unimplemented"))
    }

    async fn get_entity(
        &self,
        _id: hub_core::model::EntityId,
    ) -> Result<Option<hub_core::model::Entity>> {
        Ok(None)
    }

    async fn set_entity_state(&self, state: EntityState) -> Result<()> {
        *self.last_state.lock().unwrap() = Some(state);
        Ok(())
    }

    async fn latest_entity_state(&self, id: EntityId) -> Result<Option<EntityState>> {
        let state = self.last_state.lock().unwrap().clone();
        let matches = state.as_ref().map(|s| s.entity_id == id).unwrap_or(false);
        Ok(state.filter(|_| matches))
    }

    async fn entity_state_history(
        &self,
        _id: hub_core::model::EntityId,
        _since: Option<chrono::DateTime<chrono::Utc>>,
        _limit: usize,
    ) -> Result<Vec<EntityState>> {
        Ok(Vec::new())
    }
}

#[derive(Clone, Default)]
struct RecordingBus {
    messages: Arc<Mutex<Vec<Message>>>,
}

#[async_trait]
impl hub_core::bus::Bus for RecordingBus {
    async fn publish(&self, topic: &str, payload: bytes::Bytes) -> Result<()> {
        self.messages.lock().unwrap().push(Message { topic: topic.to_string(), payload });
        Ok(())
    }

    async fn subscribe(
        &self,
        _pattern: &str,
    ) -> Result<Box<dyn tokio_stream::Stream<Item = Message> + Unpin + Send>> {
        Err(anyhow!("subscribe not implemented"))
    }
}

fn app_with_auth(tokens: Vec<&str>, store: RecordingStorage, bus: RecordingBus) -> Router {
    let state = AppState {
        store: Arc::new(store),
        bus: Arc::new(bus),
        auth: AuthConfig { tokens: tokens.into_iter().map(|s| s.to_string()).collect() },
    };

    build_router(state)
}

#[tokio::test]
async fn allows_state_changes_when_auth_is_disabled() {
    let store = RecordingStorage::default();
    let app = app_with_auth(vec![], store.clone(), RecordingBus::default());
    let entity_id = Uuid::new_v4();

    let res = app
        .oneshot(
            Request::post(format!("/states/{entity_id}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"value":true}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let stored = store.latest_entity_state(hub_core::model::EntityId(entity_id)).await.unwrap();
    assert!(stored.is_some());
    assert!(stored.unwrap().source.is_none());
}

#[tokio::test]
async fn rejects_when_missing_credentials() {
    let app = app_with_auth(vec!["secret"], RecordingStorage::default(), RecordingBus::default());

    let res = app
        .oneshot(Request::post(format!("/states/{}", Uuid::new_v4())).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rejects_when_token_is_invalid() {
    let app = app_with_auth(vec!["secret"], RecordingStorage::default(), RecordingBus::default());

    let res = app
        .oneshot(
            Request::post(format!("/states/{}", Uuid::new_v4()))
                .header(header::AUTHORIZATION, "Bearer wrong")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn allows_with_valid_bearer_token() {
    let store = RecordingStorage::default();
    let app = app_with_auth(vec!["secret"], store.clone(), RecordingBus::default());
    let entity_id = Uuid::new_v4();

    let res = app
        .oneshot(
            Request::post(format!("/states/{entity_id}"))
                .header(header::AUTHORIZATION, "Bearer secret")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"value":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let stored =
        store.latest_entity_state(hub_core::model::EntityId(entity_id)).await.unwrap().unwrap();
    assert_eq!(stored.source.as_deref(), Some("token:cret"));
}

#[tokio::test]
async fn allows_with_valid_api_key_header() {
    let store = RecordingStorage::default();
    let app = app_with_auth(vec!["secret"], store.clone(), RecordingBus::default());
    let entity_id = Uuid::new_v4();

    let res = app
        .oneshot(
            Request::post(format!("/states/{entity_id}"))
                .header("x-api-key", "secret")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"value":"on"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let stored =
        store.latest_entity_state(hub_core::model::EntityId(entity_id)).await.unwrap().unwrap();
    assert_eq!(stored.source.as_deref(), Some("token:cret"));
}
