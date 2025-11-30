use crate::game::{ui::UIPlugin, *};
use bevy::prelude::*;
mod game;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(CorePlugin)
        .add_plugins(SimPlugin)
        .add_plugins(UIPlugin)
        .add_systems(Startup, startup)
        .run();
}

fn startup(
    mut tiles: MessageWriter<CreateTile>,
    mut conveyors: MessageWriter<CreateConveyor>,
    mut items: MessageWriter<CreateWorldItem>,
    mut cmds: Commands,
) {
    for x in -5..=5 {
        for y in -5..=5 {
            let entity = cmds.spawn_empty().id();
            tiles.write(CreateTile(entity, WorldCoords { x, y }));
        }
    }
    for x in 0..=5 {
        let entity = cmds.spawn_empty().id();
        conveyors.write(CreateConveyor(
            entity,
            WorldCoords { x, y: 0 },
            game::Direction::East,
        ));
    }
    let entity = cmds.spawn_empty().id();
    items.write(CreateWorldItem(
        entity,
        HalfWorldCoords {
            coords: WorldCoords { x: 0, y: 0 },
            conrner: Corner::NE,
        },
    ));
    let entity = cmds.spawn_empty().id();
    items.write(CreateWorldItem(
        entity,
        HalfWorldCoords {
            coords: WorldCoords { x: -1, y: 0 },
            conrner: Corner::NE,
        },
    ));
}
