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

fn startup(mut cmds: Commands) {
    for x in -5..=5 {
        for y in -5..=5 {
            let entity = cmds.spawn_empty().id();
            cmds.trigger(CreateTile {
                entity,
                coords: WorldCoords { x, y },
            });
        }
    }
    let mut belt: Option<Entity> = None;
    for x in 0..=4 {
        let entity = cmds.spawn_empty().id();
        cmds.trigger(CreateBelt {
            entity,
            coords: WorldCoords { x, y: 0 },
            dir: game::Direction::East,
        });
        if belt.is_none() {
            belt = Some(entity);
        }
    }
    let entity = cmds.spawn_empty().id();
    cmds.trigger(CreateBelt {
        entity,
        coords: WorldCoords { x: -1, y: 0 },
        dir: game::Direction::West,
    });
    let entity = cmds.spawn_empty().id();
    cmds.trigger(CreateBelt {
        entity,
        coords: WorldCoords { x: 0, y: 1 },
        dir: game::Direction::North,
    });
    let entity = cmds.spawn_empty().id();
    cmds.trigger(CreateBelt {
        entity,
        coords: WorldCoords { x: 0, y: -1 },
        dir: game::Direction::South,
    });
    let entity = cmds.spawn_empty().id();
    cmds.trigger(CreateBelt {
        entity,
        coords: WorldCoords { x: 0, y: 2 },
        dir: game::Direction::East,
    });
    let entity = cmds.spawn_empty().id();
    cmds.trigger(CreateBelt {
        entity,
        coords: WorldCoords { x: -2, y: 0 },
        dir: game::Direction::South,
    });
    let entity = cmds.spawn_empty().id();
    cmds.trigger(CreateBeltItem {
        entity,
        belt: belt.unwrap(),
        position: POSITIONS_PER_TILE - 1,
    });
    let entity = cmds.spawn_empty().id();
    cmds.trigger(CreateBeltItem {
        entity,
        belt: belt.unwrap(),
        position: POSITIONS_PER_TILE / 2 - 1,
    });
}
