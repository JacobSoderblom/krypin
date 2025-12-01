use chrono::{DateTime, Duration, Timelike, Utc};
use hub_core::{
    model::{Area, AreaId, Device, DeviceId, Entity, EntityDomain, EntityId, EntityState},
    storage::{PostgresStorage, Storage},
};
use serde_json::json;
use std::collections::BTreeMap;
use testcontainers::{
    GenericImage, ImageExt, TestcontainersError,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use uuid::Uuid;

fn postgres_image() -> testcontainers::ContainerRequest<GenericImage> {
    GenericImage::new("postgres", "16-alpine")
        .with_wait_for(WaitFor::message_on_stdout("database system is ready to accept connections"))
        .with_exposed_port(5432.tcp())
        .with_env_var("POSTGRES_PASSWORD", "password")
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_DB", "hub")
}

fn truncate_to_pg_precision(ts: DateTime<Utc>) -> DateTime<Utc> {
    // Postgres timestamptz stores microsecond precision; drop sub-micro fractional nanos.
    let micros = ts.timestamp_subsec_micros();
    ts.with_nanosecond(micros * 1000).expect("valid timestamp")
}

#[tokio::test]
async fn postgres_storage_persists_entities_and_state() -> Result<(), TestcontainersError> {
    let node = match postgres_image().start().await {
        Ok(container) => container,
        Err(err @ TestcontainersError::Client(_)) => {
            eprintln!("skipping postgres storage test: {err}");
            return Ok(());
        }
        Err(err) => return Err(err),
    };

    let port = node.get_host_port_ipv4(5432).await.expect("failed to get host port");
    let database_url = format!("postgres://postgres:password@127.0.0.1:{port}/hub");

    let storage =
        PostgresStorage::connect(&database_url).await.expect("failed to connect to postgres");

    let area = Area { id: AreaId(Uuid::new_v4()), name: "Living Room".into(), parent: None };
    storage.upsert_area(area.clone()).await.unwrap();
    assert_eq!(storage.get_area(area.id).await.unwrap(), Some(area.clone()));

    let device = Device {
        id: DeviceId(Uuid::new_v4()),
        name: "Light Controller".into(),
        adapter: "mqtt".into(),
        manufacturer: Some("Acme".into()),
        model: Some("LC1000".into()),
        sw_version: Some("1.0.0".into()),
        hw_version: None,
        area: Some(area.id),
        metadata: BTreeMap::new(),
    };
    storage.upsert_device(device.clone()).await.unwrap();
    assert_eq!(storage.get_device(device.id).await.unwrap(), Some(device.clone()));

    let entity = Entity {
        id: EntityId(Uuid::new_v4()),
        device_id: device.id,
        name: "Ceiling Light".into(),
        domain: EntityDomain::Light,
        icon: Some("mdi:lightbulb".into()),
        key: Some("ceiling_light".into()),
        attributes: BTreeMap::new(),
    };
    storage.upsert_entity(entity.clone()).await.unwrap();
    assert_eq!(storage.get_entity(entity.id).await.unwrap(), Some(entity.clone()));

    let base_time = truncate_to_pg_precision(Utc::now());
    let state1 = EntityState {
        entity_id: entity.id,
        value: json!({"state": "off"}),
        attributes: BTreeMap::new(),
        last_changed: truncate_to_pg_precision(base_time),
        last_updated: truncate_to_pg_precision(base_time),
        source: Some("test".into()),
    };
    storage.set_entity_state(state1.clone()).await.unwrap();

    let state2 = EntityState {
        entity_id: entity.id,
        value: json!({"state": "on"}),
        attributes: BTreeMap::new(),
        last_changed: truncate_to_pg_precision(base_time + Duration::seconds(1)),
        last_updated: truncate_to_pg_precision(base_time + Duration::seconds(1)),
        source: Some("test".into()),
    };
    storage.set_entity_state(state2.clone()).await.unwrap();

    let state3 = EntityState {
        entity_id: entity.id,
        value: json!({"state": "dim", "brightness": 50}),
        attributes: BTreeMap::new(),
        last_changed: truncate_to_pg_precision(base_time + Duration::seconds(2)),
        last_updated: truncate_to_pg_precision(base_time + Duration::seconds(2)),
        source: Some("test".into()),
    };
    storage.set_entity_state(state3.clone()).await.unwrap();

    assert_eq!(storage.latest_entity_state(entity.id).await.unwrap().as_ref(), Some(&state3));

    let history = storage.entity_state_history(entity.id, None, 2).await.unwrap();
    assert_eq!(history, vec![state3.clone(), state2.clone()]);

    let filtered_history = storage
        .entity_state_history(entity.id, Some(base_time + Duration::seconds(1)), 10)
        .await
        .unwrap();
    assert_eq!(filtered_history, vec![state3.clone(), state2.clone()]);

    drop(storage);
    drop(node);

    Ok(())
}
