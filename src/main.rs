use bevy::prelude::*;

use crate::core::*;
use crate::player::PlayerPlugin;
use crate::sim::SimPlugin;
use crate::ui::UiPlugin;

mod core;
mod player;
mod sim;
mod ui;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(UiPlugin);
    app.add_plugins(PlayerPlugin);
    app.add_plugins(SimPlugin);
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
