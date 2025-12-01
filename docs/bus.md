# Bus contract and transport notes

## Topics and payloads

| Topic | Payload type | Publisher(s) | Consumer(s) | Notes |
| --- | --- | --- | --- | --- |
| `krypin.device.announce` | `DeviceAnnounce` | Device adapters | Hub subscriber stores/updates devices | Must include area/device metadata; stored via `hubd` subscriber. |
| `krypin.entity.announce` | `EntityAnnounce` | Device adapters | Hub subscriber stores/updates entities | Usually sent after device announce. |
| `krypin.state.update.<entity_id>` | `StateUpdate` | Device adapters, automations | Hub state subscriber persists latest state | `source` may be adapter name/user; correlation handled at higher layers. |
| `krypin.command.<entity_id>` | `CommandSet` | Hub HTTP API, automations | Device adapters | Carries desired value and optional `correlation_id`. |
| `krypin.hub.heartbeat` | `Heartbeat` | Hub daemon | Automations, monitors | Periodic liveness signal. |

Message envelopes on the in-process bus now include a `received_at` timestamp, allowing latency measurement between publish and handling.

## Transport choice

* **Primary:** MQTT via `adapter-mqtt` (QoS 1 / AtLeastOnce) for cross-process delivery. The client subscribes to `#` and automatically reconnects/resends when the broker drops, so transient losses are retried by the MQTT library.
* **Local/testing:** In-memory broadcast bus for fast single-process scenarios (no network retries, but ordered delivery within the process).

### MQTT guarantees

* **QoS:** All publishes use QoS 1 (`AtLeastOnce`), so duplicates are possible but drops are retried by the client.
* **Ordering:** MQTT preserves order per topic on a single connection; cross-topic ordering is not guaranteed.
* **Retries:** `rumqttc` handles reconnects and resending unacknowledged QoS1 publishes. The hub uses clean sessions per start-up.

### In-memory bus characteristics

* **QoS:** Best-effort within the process; publish never fails.
* **Ordering:** Tokio broadcast preserves send order for subscribers in the same process.
* **Retries:** None; use MQTT for durable delivery.

## Observability

* Publish counters (`bus.publish.success` / `bus.publish.failures`) are emitted from the HTTP command endpoint.
* Subscribers for device/entity/state updates track decode/handling errors (`bus.message.decode_error`, `bus.message.handle_error`) and emit `bus.message.latency_ms` histograms using the envelope timestamp.
* The new integration test (`hubd/tests/bus_roundtrip.rs`) exercises the announce → command → telemetry flow end-to-end using the in-memory bus and mock adapter.
