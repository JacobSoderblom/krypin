use anyhow::{self, Result};
use dotenv::dotenv;
use std::{
    fmt::{self, Display, Formatter},
    net::SocketAddr,
    str::FromStr,
};
use url::Url;

#[derive(Clone, Debug, PartialEq)]
pub enum BusKind {
    InMem,
    Mqtt,
}

impl FromStr for BusKind {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "inmem" => Ok(BusKind::InMem),
            "mqtt" => Ok(BusKind::Mqtt),
            _ => Err(()),
        }
    }
}

impl BusKind {
    fn as_str(&self) -> &'static str {
        match self {
            BusKind::InMem => "inmem",
            BusKind::Mqtt => "mqtt",
        }
    }
}

impl Display for BusKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StorageKind {
    InMem,
}

impl FromStr for StorageKind {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "inmem" => Ok(StorageKind::InMem),
            _ => Err(()),
        }
    }
}

impl StorageKind {
    fn as_str(&self) -> &'static str {
        match self {
            StorageKind::InMem => "inmem",
        }
    }
}

impl Display for StorageKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: SocketAddr,
    pub bus: BusKind,
    pub mqtt: MqttConfig,
    pub storage: StorageKind,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".parse().unwrap(),
            bus: BusKind::InMem,
            mqtt: MqttConfig::default(),
            storage: StorageKind::InMem,
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv().ok();
        let mut c = Self::default();
        if let Ok(s) = std::env::var("KRYPIN_BIND") {
            c.bind = s.parse()?;
        }
        if let Ok(s) = std::env::var("KRYPIN_BUS") {
            c.bus = BusKind::from_str(&s).unwrap();
        }
        if let Ok(conn) = std::env::var("KRYPIN_MQTT_URL") {
            c.mqtt = MqttConfig::from_connection_string(&conn)?;
        }
        if let Ok(s) = std::env::var("KRYPIN_MQTT_HOST") {
            c.mqtt.host = s;
        }
        if let Ok(s) = std::env::var("KRYPIN_MQTT_PORT") {
            c.mqtt.port = s.parse()?;
        }
        if let Ok(s) = std::env::var("KRYPIN_MQTT_CLIENT_ID") {
            c.mqtt.client_id = s;
        }
        if let Ok(s) = std::env::var("KRYPIN_STORAGE") {
            c.storage = StorageKind::from_str(&s).unwrap();
        }
        Ok(c)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub client_id: String,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self { host: "127.0.0.1".to_string(), port: 1883, client_id: "hubd".to_string() }
    }
}

impl MqttConfig {
    fn from_connection_string(conn: &str) -> Result<Self> {
        let url = Url::parse(conn)?;
        if url.scheme() != "mqtt" {
            anyhow::bail!("unsupported mqtt url scheme: {}", url.scheme());
        }

        let host =
            url.host_str().ok_or_else(|| anyhow::anyhow!("mqtt url missing host"))?.to_string();
        let port = url.port().unwrap_or(1883);
        let client_id = url
            .query_pairs()
            .find(|(k, _)| k == "client_id")
            .map(|(_, v)| v.into_owned())
            .unwrap_or_else(|| "hubd".to_string());

        Ok(Self { host, port, client_id })
    }
}
