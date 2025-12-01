# MQTT Adapter

## Overview
This crate provides an MQTT-backed implementation of the shared bus interface, using Mosquitto-compatible brokers through `rumqttc`.

## Test prerequisites
The integration tests spawn a local Mosquitto broker and require access to the public crates.io index when building dependencies. In restricted environments, tests can fail for two reasons:

1. **No network access to crates.io** – Cargo needs to download dependencies from the crates.io index. If the index cannot be fetched (for example, due to a 403 from a corporate proxy), `cargo test -p adapter-mqtt` will stop before compiling. Ensure the environment permits outbound access to `https://index.crates.io/` or provide a vendored registry cache.
2. **`mosquitto` binary not installed** – The tests shell out to `mosquitto -p <port> -v`. Install the Mosquitto broker (e.g., `sudo apt-get install mosquitto`) so the broker can be started during the tests.

If either prerequisite is missing, tests will fail before exercising the adapter. Once both are available, run `cargo test -p adapter-mqtt` to validate publish/subscribe behavior and topic filtering.
