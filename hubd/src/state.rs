use std::sync::Arc;

use crate::config::AuthConfig;
use automations::AutomationEngine;
use hub_core::{bus::Bus, storage::Storage};

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<dyn Storage>,
    pub bus: Arc<dyn Bus>,
    pub auth: AuthConfig,
    pub automations: Arc<AutomationEngine>,
}
