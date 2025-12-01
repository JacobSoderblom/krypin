# Adapter SDK

The adapter SDK provides the smallest possible surface area for integrating device adapters with the hub. It focuses on three things:

1. Connecting to the hub message bus.
2. Announcing devices/entities discovered by the adapter.
3. Reporting telemetry and responding to incoming commands.

## Core traits and lifecycle hooks

Use `runtime::AdapterLifecycle` when wiring an adapter. Implementors receive the following hooks:

- `init(&self, ctx)`: perform one-time setup, such as connecting to the target platform or hydrating caches.
- `discover(&self, ctx)`: announce devices/entities via `AdapterContext::announce_device`/`announce_entity`, then spawn command listeners (for example with `runtime::spawn_command_loop`). This is the place to register all known capabilities and queue up background jobs.
- `handle_command(&self, ctx, entity_id, cmd)`: invoked for every `CommandSet` received on the entity's command topic. Do any validation before touching real devices.
- `telemetry_tick(&self, ctx)`: optional periodic publishing of state via `AdapterContext::publish_state`. If your adapter exposes a heartbeat, you can call this at a fixed interval via `tokio::spawn` or an external scheduler.

`AdapterContext` is a thin wrapper around `hub_core::bus::Bus` that standardizes the bus contract. It exposes:

- `announce_device(DeviceAnnounce)` and `announce_entity(EntityAnnounce)` to publish discovery events.
- `publish_state(StateUpdate)` to report telemetry.
- `subscribe_commands(entity_id)` to receive parsed `CommandSet` streams for a specific entity.

The helper `spawn_command_loop` takes an `Arc<AdapterLifecycle>` implementer and runs a background task that subscribes to the command topic and forwards every decoded `CommandSet` into `handle_command`.

## Error handling

- Methods on `AdapterLifecycle` should bubble errors via `anyhow::Result`. The runtime helpers will log failures and continue processing other messages.
- Command handlers should validate capabilities (for example, `SwitchDescription::validate`) before mutating device state and publishing telemetry.
- Use structured errors for recoverable issues (missing fields, unsupported commands) and let fatal initialization errors propagate from `init`/`discover`.
- Avoid panics inside background loops; prefer returning an error so the caller can restart the adapter if needed.
- Command subscription drops malformed payloads but logs decoding failures with the entity id to aid debugging.

## Capability registration

Entity metadata is where capabilities are declared. For example, a switch can set attributes that match `hub_core::cap::switch::SwitchFeatures`:

```rust
let mut attributes = BTreeMap::new();
attributes.insert("features".into(), (SwitchFeatures::ONOFF | SwitchFeatures::TOGGLE).bits().into());
let announce = EntityAnnounce {
    id: entity_id,
    device_id,
    name: "Demo Switch".into(),
    domain: EntityDomain::Switch,
    icon: None,
    key: None,
    attributes,
};
ctx.announce_entity(announce).await?;
```

## Example

The `examples/template-adapter` crate shows a minimal, compilable adapter that:

- Implements `AdapterLifecycle`.
- Announces a single switch entity.
- Subscribes to `CommandSet` messages and publishes `StateUpdate` payloads using the hub bus contract.

The example also includes a self-test that publishes a loopback command through the bus, demonstrating end-to-end behavior of `spawn_command_loop` and `AdapterContext::publish_state`.

Run it with:

```bash
cargo run -p template-adapter
```
