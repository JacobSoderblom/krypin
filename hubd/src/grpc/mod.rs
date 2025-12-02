use crate::{config::Config, state::AppState};
use automations::{AutomationId, NewAutomation, TriggerEvent};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use bytes::Bytes;
use futures::Stream;
use hub_core::{
    bus_contract::{CommandSet, TOPIC_COMMAND_PREFIX},
    model::{Device, Entity, EntityId, EntityState},
};
use serde_json::Value;
use std::pin::Pin;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status, transport::Server};
use uuid::Uuid;

pub mod pb {
    tonic::include_proto!("krypin.hub");
}

use pb::hub_service_server::{HubService, HubServiceServer};
use pb::{
    Automation, AutomationIdRequest, AutomationList, BusEvent, CommandResponse, Device as DevicePb,
    Empty, Entity as EntityPb, EntityState as EntityStatePb, GetStateRequest, ListDevicesResponse,
    ListEntitiesResponse, NewAutomation as NewAutomationPb, SendCommandRequest,
    TestAutomationRequest, TestAutomationResponse,
};

type TonicResult<T> = Result<Response<T>, Status>;

pub async fn serve_grpc(app: AppState, cfg: Config) -> anyhow::Result<()> {
    let addr = cfg.grpc_bind;
    let svc = HubServiceServer::new(HubSvc { app });
    tracing::info!("krypin hub listening on gRPC {addr}");
    Server::builder()
        .add_service(svc)
        .serve_with_shutdown(addr, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

#[derive(Clone)]
struct HubSvc {
    app: AppState,
}

#[tonic::async_trait]
impl HubService for HubSvc {
    async fn list_devices(&self, _: Request<Empty>) -> TonicResult<ListDevicesResponse> {
        let devices = self
            .app
            .store
            .list_devices()
            .await
            .map_err(internal)?
            .into_iter()
            .map(pb_device)
            .collect();
        Ok(Response::new(ListDevicesResponse { devices }))
    }

    async fn list_entities(&self, _: Request<Empty>) -> TonicResult<ListEntitiesResponse> {
        let entities = self
            .app
            .store
            .list_entities()
            .await
            .map_err(internal)?
            .into_iter()
            .map(pb_entity)
            .collect();
        Ok(Response::new(ListEntitiesResponse { entities }))
    }

    async fn get_state(&self, req: Request<GetStateRequest>) -> TonicResult<EntityStatePb> {
        let entity_id = parse_entity_id(&req.get_ref().entity_id)?;
        let Some(state) = self.app.store.latest_entity_state(entity_id).await.map_err(internal)?
        else {
            return Err(Status::not_found("state not found"));
        };
        Ok(Response::new(pb_state(state)))
    }

    async fn send_command(&self, req: Request<SendCommandRequest>) -> TonicResult<CommandResponse> {
        let body = req.into_inner();
        let entity_id = parse_entity_id(&body.entity_id)?;
        let cmd = CommandSet {
            action: if body.action.is_empty() { "set".to_string() } else { body.action },
            value: serde_json::from_str(&body.value_json).unwrap_or(Value::Null),
            correlation_id: Uuid::try_parse(&body.correlation_id).ok(),
        };
        let topic = format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0);
        let payload = Bytes::from(serde_json::to_vec(&cmd).map_err(|e| internal(e.into()))?);
        self.app.bus.publish(&topic, payload).await.map_err(internal)?;
        Ok(Response::new(CommandResponse { accepted: true }))
    }

    async fn list_automations(&self, _: Request<Empty>) -> TonicResult<AutomationList> {
        let list = self
            .app
            .automations
            .list_automations()
            .await
            .map_err(internal)?
            .into_iter()
            .map(pb_automation)
            .collect();
        Ok(Response::new(AutomationList { automations: list }))
    }

    async fn create_automation(&self, req: Request<NewAutomationPb>) -> TonicResult<Automation> {
        let new = parse_new_automation(req.into_inner())?;
        let created = self.app.automations.create_automation(new).await.map_err(bad_request)?;
        Ok(Response::new(pb_automation(created)))
    }

    async fn enable_automation(
        &self,
        req: Request<AutomationIdRequest>,
    ) -> TonicResult<Automation> {
        let id = parse_automation_id(&req.get_ref().id)?;
        let updated = self.app.automations.set_enabled(id, true).await.map_err(not_found)?;
        Ok(Response::new(pb_automation(updated)))
    }

    async fn disable_automation(
        &self,
        req: Request<AutomationIdRequest>,
    ) -> TonicResult<Automation> {
        let id = parse_automation_id(&req.get_ref().id)?;
        let updated = self.app.automations.set_enabled(id, false).await.map_err(not_found)?;
        Ok(Response::new(pb_automation(updated)))
    }

    async fn test_automation(
        &self,
        req: Request<TestAutomationRequest>,
    ) -> TonicResult<TestAutomationResponse> {
        let body = req.into_inner();
        let id = parse_automation_id(&body.id)?;
        let event: TriggerEvent = serde_json::from_str(&body.event_json)
            .map_err(|e| Status::invalid_argument(format!("invalid trigger event: {e}")))?;
        let result = self.app.automations.test_automation(id, event).await.map_err(bad_request)?;
        Ok(Response::new(TestAutomationResponse {
            executed: result.executed,
            reason: result.reason.unwrap_or_default(),
        }))
    }

    type StreamEventsStream =
        Pin<Box<dyn Stream<Item = Result<BusEvent, Status>> + Send + 'static>>;

    async fn stream_events(
        &self,
        _req: Request<Empty>,
    ) -> Result<Response<Self::StreamEventsStream>, Status> {
        let mut sub = self.app.bus.subscribe("*").await.map_err(internal)?;
        let stream: Self::StreamEventsStream = Box::pin(async_stream::stream! {
            while let Some(msg) = sub.next().await {
                let payload = serde_json::from_slice::<Value>(&msg.payload)
                    .unwrap_or_else(|_| Value::String(STANDARD.encode(&msg.payload)));
                yield Ok(BusEvent { topic: msg.topic, payload_json: payload.to_string() });
            }
        });
        Ok(Response::new(stream))
    }
}

fn pb_device(device: Device) -> DevicePb {
    DevicePb {
        id: device.id.0.to_string(),
        name: device.name,
        adapter: device.adapter,
        manufacturer: device.manufacturer.unwrap_or_default(),
        model: device.model.unwrap_or_default(),
        sw_version: device.sw_version.unwrap_or_default(),
        hw_version: device.hw_version.unwrap_or_default(),
        area_id: device.area.map(|a| a.0.to_string()).unwrap_or_default(),
    }
}

fn pb_entity(entity: Entity) -> EntityPb {
    EntityPb {
        id: entity.id.0.to_string(),
        device_id: entity.device_id.0.to_string(),
        name: entity.name,
        domain: serde_json::to_string(&entity.domain).unwrap_or_default(),
        icon: entity.icon.unwrap_or_default(),
        key: entity.key.unwrap_or_default(),
        attributes: entity.attributes.into_iter().map(|(k, v)| (k, v.to_string())).collect(),
    }
}

fn pb_state(state: EntityState) -> EntityStatePb {
    EntityStatePb {
        entity_id: state.entity_id.0.to_string(),
        value_json: state.value.to_string(),
        attributes: state.attributes.into_iter().map(|(k, v)| (k, v.to_string())).collect(),
        last_changed: state.last_changed.to_rfc3339(),
        last_updated: state.last_updated.to_rfc3339(),
        source: state.source.unwrap_or_default(),
    }
}

fn pb_automation(auto: automations::AutomationDefinition) -> Automation {
    Automation {
        id: auto.id.0.to_string(),
        name: auto.name,
        description: auto.description.unwrap_or_default(),
        trigger_json: serde_json::to_string(&auto.trigger).unwrap_or_default(),
        conditions_json: auto
            .conditions
            .into_iter()
            .map(|c| serde_json::to_string(&c).unwrap_or_default())
            .collect(),
        actions_json: auto
            .actions
            .into_iter()
            .map(|a| serde_json::to_string(&a).unwrap_or_default())
            .collect(),
        enabled: auto.enabled,
        created_at: auto.created_at.to_rfc3339(),
        updated_at: auto.updated_at.to_rfc3339(),
    }
}

fn parse_new_automation(pb: NewAutomationPb) -> Result<NewAutomation, Status> {
    let trigger: automations::Trigger = serde_json::from_str(&pb.trigger_json)
        .map_err(|e| Status::invalid_argument(format!("invalid trigger: {e}")))?;
    let conditions = pb
        .conditions_json
        .into_iter()
        .map(|c| {
            serde_json::from_str(&c)
                .map_err(|e| Status::invalid_argument(format!("invalid condition: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let actions = pb
        .actions_json
        .into_iter()
        .map(|a| {
            serde_json::from_str(&a)
                .map_err(|e| Status::invalid_argument(format!("invalid action: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NewAutomation {
        name: pb.name,
        description: if pb.description.is_empty() { None } else { Some(pb.description) },
        trigger,
        conditions,
        actions,
        enabled: pb.enabled,
    })
}

fn parse_entity_id(id: &str) -> Result<EntityId, Status> {
    Uuid::try_parse(id).map(EntityId).map_err(|_| Status::invalid_argument("invalid entity id"))
}

fn parse_automation_id(id: &str) -> Result<AutomationId, Status> {
    Uuid::try_parse(id)
        .map(AutomationId)
        .map_err(|_| Status::invalid_argument("invalid automation id"))
}

fn not_found(err: anyhow::Error) -> Status {
    Status::not_found(err.to_string())
}

fn bad_request(err: anyhow::Error) -> Status {
    Status::invalid_argument(err.to_string())
}

fn internal(err: anyhow::Error) -> Status {
    Status::internal(err.to_string())
}
