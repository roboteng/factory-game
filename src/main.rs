use crate::game::{CorePlugin, CreateTile, SimPlugin, ui::UIPlugin};
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

fn startup(mut msgs: MessageWriter<CreateTile>, mut cmds: Commands) {
    for x in -5..=5 {
        for y in -5..=5 {
            let entity = cmds.spawn_empty().id();
            msgs.write(CreateTile(
                entity,
                Vec2 {
                    x: x as f32 * 32.0,
                    y: y as f32 * 32.0,
                },
            ));
        }
    }
}
