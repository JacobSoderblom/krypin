use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use hub_core::bus::{Bus, Message};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use tokio::sync::broadcast;
use tokio_stream::{Stream, StreamExt, wrappers::BroadcastStream};

#[derive(Clone)]
pub struct MqttBus {
    client: AsyncClient,
    tx: Arc<broadcast::Sender<Message>>,
}

impl MqttBus {
    pub async fn connect(host: &str, port: u16, client_id: &str) -> Result<Self> {
        let mut opts = MqttOptions::new(client_id, host, port);
        opts.set_keep_alive(Duration::from_secs(5));
        opts.set_clean_session(true);

        let (client, mut eventloop) = AsyncClient::new(opts, 10);
        client.subscribe("#", QoS::AtLeastOnce).await?;

        let (tx, _rx) = broadcast::channel(1024);
        let tx = Arc::new(tx);
        let forwarder_tx = Arc::clone(&tx);

        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Incoming::Publish(p))) => {
                        let _ = forwarder_tx.send(Message {
                            topic: p.topic,
                            payload: Bytes::from(p.payload.to_vec()),
                        });
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("mqtt event loop error: {e}");
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        });

        Ok(Self { client, tx })
    }
}

#[async_trait]
impl Bus for MqttBus {
    async fn publish(&self, topic: &str, payload: Bytes) -> Result<()> {
        self.client
            .publish(topic, QoS::AtLeastOnce, false, payload)
            .await
            .context("publish mqtt message")?;
        Ok(())
    }

    async fn subscribe(&self, pattern: &str) -> Result<Box<dyn Stream<Item = Message> + Unpin + Send>> {
        let rx = self.tx.subscribe();
        let pattern = pattern.to_string();
        let stream = BroadcastStream::new(rx).filter_map(move |item| {
            let pat = pattern.clone();
            match item {
                Ok(msg) if topic_matches(&pat, &msg.topic) => Some(msg),
                _ => None,
            }
        });
        Ok(Box::new(stream))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::ErrorKind, net::TcpListener, process::{Child, Command, Stdio}};
    use tokio::time::{sleep, Duration};

    struct MosquittoGuard(Child);

    impl Drop for MosquittoGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
        }
    }

    async fn start_broker() -> Result<(MosquittoGuard, u16)> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);

        let child = Command::new("mosquitto")
            .args(["-p", &port.to_string(), "-v"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("spawn mosquitto")?;

        let guard = MosquittoGuard(child); // ensures kill on drop
        let mut attempts = 0;
        loop {
            match tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
                Ok(_) => break,
                Err(_) if attempts < 20 => {
                    attempts += 1;
                    sleep(Duration::from_millis(50)).await;
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok((guard, port))
    }

    #[tokio::test]
    async fn publishes_and_receives() -> Result<()> {
        let (_guard, port) = match start_broker().await {
            Ok(ok) => ok,
            Err(e) if e.downcast_ref::<std::io::Error>().map(|io| io.kind()) == Some(ErrorKind::NotFound) => {
                eprintln!("skipping publishes_and_receives: mosquitto not installed");
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        let bus = MqttBus::connect("127.0.0.1", port, "test-client").await?;

        let mut stream = bus.subscribe("test/topic").await?;
        bus.publish("test/topic", Bytes::from_static(b"hello"))
            .await?;

        let msg = stream.next().await.expect("message expected");
        assert_eq!(msg.topic, "test/topic");
        assert_eq!(msg.payload, Bytes::from_static(b"hello"));
        Ok(())
    }

    #[tokio::test]
    async fn pattern_filtering() -> Result<()> {
        let (_guard, port) = match start_broker().await {
            Ok(ok) => ok,
            Err(e) if e.downcast_ref::<std::io::Error>().map(|io| io.kind()) == Some(ErrorKind::NotFound) => {
                eprintln!("skipping pattern_filtering: mosquitto not installed");
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        let bus = MqttBus::connect("127.0.0.1", port, "test-filter").await?;

        let mut stream = bus.subscribe("sensor.*").await?;
        bus.publish("sensor.temp", Bytes::from_static(b"20"))
            .await?;
        bus.publish("other", Bytes::from_static(b"ignore"))
            .await?;

        let msg = stream.next().await.expect("filtered message");
        assert_eq!(msg.topic, "sensor.temp");
        assert_eq!(msg.payload, Bytes::from_static(b"20"));
        Ok(())
    }
}
