use std::sync::Arc;

use hub_core::{bus::Bus, storage::Storage};

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<dyn Storage>,
    pub bus: Arc<dyn Bus>,
}
