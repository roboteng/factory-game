use bevy::prelude::*;

use crate::core::{HorizontalDir, WorldCoords};

mod core;
#[cfg(feature = "invariant-ckeck")]
mod invariants;
mod sim;
#[cfg(feature = "ui")]
mod ui;

fn main() {
    let mut app = App::new();

    app.add_plugins((DefaultPlugins, core::CorePlugin, sim::SimPlugin));

    #[cfg(feature = "ui")]
    app.add_plugins(ui::UiPlugin);
    #[cfg(feature = "invariant-ckeck")]
    app.add_plugins(invariants::InvariantsPlugin);

    app.add_systems(Startup, setup);

    app.run();
}

fn setup(mut cmd: Commands) {
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBelt {
        entity,
        coords: (1, 0, 0).into(),
        dir: HorizontalDir::East,
    });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBelt {
        entity,
        coords: (0, 0, 1).into(),
        dir: HorizontalDir::North,
    })
}
