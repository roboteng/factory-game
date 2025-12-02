use crate::game::{sim::SimPlugin, ui::UIPlugin, *};
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
    mut items: MessageWriter<CreateConveyorItem>,
    mut cmds: Commands,
) {
    for x in -5..=5 {
        for y in -5..=5 {
            let entity = cmds.spawn_empty().id();
            tiles.write(CreateTile(entity, WorldCoords { x, y }));
        }
    }
    let mut belt: Option<Entity> = None;
    for x in 0..=4 {
        let entity = cmds.spawn_empty().id();
        conveyors.write(CreateConveyor {
            entity,
            pos: WorldCoords { x, y: 0 },
            dir: game::Direction::East,
        });
        if belt.is_none() {
            belt = Some(entity);
        }
    }
    let entity = cmds.spawn_empty().id();
    items.write(CreateConveyorItem {
        entity,
        conveyor: belt.unwrap(),
        position: POSITIONS_PER_TILE - 1,
    });
}
