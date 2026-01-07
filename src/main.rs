use bevy::prelude::*;

use crate::core::*;

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
    app.add_systems(Update, update);

    app.init_resource::<ShouldRun>();

    app.run();
}

fn setup(mut cmd: Commands) {
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBelt {
        entity,
        coords: (1, 0, 0).into(),
        dir: HorizontalDir::North,
    });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBelt {
        entity,
        coords: (0, 0, 1).into(),
        dir: HorizontalDir::East,
    });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBelt {
        entity,
        coords: (0, 0, 0).into(),
        dir: HorizontalDir::North,
    });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBelt {
        entity,
        coords: (-1, 0, 0).into(),
        dir: HorizontalDir::North,
    });
}

#[derive(Resource, Default)]
struct ShouldRun(bool);

fn update(mut should_run: ResMut<ShouldRun>, belts: Query<(Entity, &InLane)>, mut cmd: Commands) {
    if !should_run.0 {
        info!("Ran once");
        for (entity, _) in belts.iter() {
            let item = cmd.spawn_empty().id();
            cmd.trigger(PlaceItem {
                entity: item,
                item: Item(0),
                belt: entity,
                lane: Lane::Left,
                position: 0,
            });
        }
    }
    should_run.0 = true;
}
