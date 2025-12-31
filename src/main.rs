use bevy::prelude::*;

use crate::core::*;
#[cfg(feature = "dev")]
use crate::dev_tools::DevToolsPlugin;
#[cfg(feature = "ui")]
use crate::player::PlayerPlugin;
use crate::sim::SimPlugin;
#[cfg(feature = "ui")]
use crate::ui::UiPlugin;

mod core;
#[cfg(feature = "dev")]
mod dev_tools;
#[cfg(feature = "ui")]
mod player;
mod sim;
#[cfg(feature = "ui")]
mod ui;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(CorePlugin);
    #[cfg(feature = "ui")]
    app.add_plugins(UiPlugin);
    #[cfg(feature = "ui")]
    app.add_plugins(PlayerPlugin);
    app.add_plugins(SimPlugin);

    #[cfg(feature = "dev")]
    app.add_plugins(DevToolsPlugin);

    app.add_systems(Startup, startup);
    app.run();
}

fn startup(mut cmd: Commands) {
    let entity = cmd.spawn_empty().id();
    cmd.trigger(PlaceBelt {
        entity,
        dir: Dir::East,
        coords: (0, 0).into(),
    });
}
