use bevy::prelude::*;

#[cfg(feature = "ui")]
mod ui;

pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "ui")]
        app.add_plugins(ui::DevUiPlugin);

        // Future: Add non-UI dev tools here (always available when dev feature is on)
        // Example: app.add_plugins(PerfMonitorPlugin);
    }
}
