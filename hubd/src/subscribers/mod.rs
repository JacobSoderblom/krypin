mod automation;
mod device;
mod entity;
mod state;

use crate::state::AppState;

pub fn spawn_all(app: AppState) {
    device::spawn(app.clone());
    entity::spawn(app.clone());
    state::spawn(app.clone());
    automation::spawn(app);
}
