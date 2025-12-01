use std::{sync::Arc, time::Duration};

use chrono::Utc;
use hub_core::{
    bus::Bus,
    bus_contract::{Heartbeat, TOPIC_HEARTBEAT},
};
use tracing::warn;

pub fn spawn(bus: Arc<dyn Bus>) {
    spawn_with_interval(bus, Duration::from_secs(30));
}

pub fn spawn_with_interval(bus: Arc<dyn Bus>, interval: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            let heartbeat = Heartbeat { ts: Utc::now() };
            let payload = match serde_json::to_vec(&heartbeat) {
                Ok(p) => p,
                Err(e) => {
                    warn!("failed to serialize heartbeat: {e}");
                    continue;
                }
            };

            if let Err(e) = bus.publish(TOPIC_HEARTBEAT, payload.into()).await {
                warn!("failed to publish heartbeat: {e}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use hub_core::bus::InMemoryBus;
    use tokio_stream::StreamExt;

    #[tokio::test(start_paused = true)]
    async fn publishes_heartbeats_at_interval() {
        let bus = Arc::new(InMemoryBus::default());
        let mut sub = bus.subscribe(TOPIC_HEARTBEAT).await.expect("subscribe heartbeat");

        spawn_with_interval(bus, Duration::from_secs(1));

        tokio::time::advance(Duration::from_secs(1)).await;
        let first = sub.next().await.expect("first heartbeat");
        let hb: Heartbeat = serde_json::from_slice(&first.payload).expect("decode heartbeat");
        assert_eq!(first.topic, TOPIC_HEARTBEAT);
        assert_ne!(hb.ts.timestamp_millis(), 0);

        tokio::time::advance(Duration::from_secs(1)).await;
        let second = sub.next().await.expect("second heartbeat");
        assert_eq!(second.topic, TOPIC_HEARTBEAT);
        let second_hb: Heartbeat =
            serde_json::from_slice(&second.payload).expect("decode heartbeat");
        assert!(second_hb.ts >= hb.ts);
    }
}
