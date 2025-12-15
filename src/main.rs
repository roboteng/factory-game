use bevy::prelude::*;

use crate::core::*;
use crate::ui::*;

mod core;
mod ui;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(UiPlugin);
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
