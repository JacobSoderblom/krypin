use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::{Stream, StreamExt, wrappers::BroadcastStream};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub id: Uuid,
    pub topic: String,
    pub payload: serde_json::Value,
    pub ts: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub topic: String,
    pub payload: Bytes,
}

#[async_trait]
pub trait Bus: Send + Sync {
    async fn publish(&self, topic: &str, payload: Bytes) -> Result<()>;
    async fn subscribe(
        &self,
        pattern: &str,
    ) -> Result<Box<dyn Stream<Item = Message> + Unpin + Send>>;
}

#[derive(Clone)]
pub struct InMemoryBus {
    tx: Arc<broadcast::Sender<Message>>,
}

impl Default for InMemoryBus {
    fn default() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx: Arc::new(tx) }
    }
}

#[async_trait::async_trait]
impl Bus for InMemoryBus {
    async fn publish(&self, topic: &str, payload: Bytes) -> anyhow::Result<()> {
        let _ = self.tx.send(Message { topic: topic.to_string(), payload });
        Ok(())
    }

    async fn subscribe(
        &self,
        pattern: &str,
    ) -> anyhow::Result<Box<dyn Stream<Item = Message> + Unpin + Send>> {
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
