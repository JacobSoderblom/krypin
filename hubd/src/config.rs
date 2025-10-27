use anyhow::Result;
use dotenv::dotenv;
use std::{
    fmt::{self, Display, Formatter},
    net::SocketAddr,
    str::FromStr,
};

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
    pub storage: StorageKind,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".parse().unwrap(),
            bus: BusKind::InMem,
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
        if let Ok(s) = std::env::var("KRYPIN_STORAGE") {
            c.storage = StorageKind::from_str(&s).unwrap();
        }
        Ok(c)
    }
}
