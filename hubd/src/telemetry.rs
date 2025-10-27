use anyhow::Result;
use tracing_subscriber::{filter::EnvFilter, fmt};

use crate::config::Config;

pub fn init_tracing(_cfg: &Config) -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).compact().init();
    Ok(())
}
