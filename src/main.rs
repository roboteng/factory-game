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
            tiles.write(CreateTile(
                entity,
                WorldCoords {
                    x: x as f32,
                    y: y as f32,
                },
            ));
        }
    }
    for x in 0..=5 {
        let entity = cmds.spawn_empty().id();
        conveyors.write(CreateConveyor(
            entity,
            Vec2 {
                x: x as f32,
                y: 0.0,
            },
            game::Direction::East,
        ));
    }
    let entity = cmds.spawn_empty().id();
    items.write(CreateWorldItem(entity, Vec2 { x: 0.0, y: 0.0 }));
    let entity = cmds.spawn_empty().id();
    items.write(CreateWorldItem(entity, Vec2 { x: -1.0, y: 0.0 }));
}
