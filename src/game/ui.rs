use bevy::prelude::*;

use crate::game::*;

pub struct UIPlugin;
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, startup_camera);
        app.add_systems(Update, (create_tile, create_conveyor));
    }
}

fn startup_camera(mut cmd: Commands) {
    cmd.spawn(Camera2d);
}

fn create_tile(mut msgs: MessageReader<CreateTile>, mut cmd: Commands, assets: Res<AssetServer>) {
    for CreateTile(entity, _) in msgs.read() {
        cmd.entity(*entity)
            .insert(Sprite::from_image(assets.load("sprites/tile.png")));
    }
}

fn create_conveyor(
    mut msgs: MessageReader<CreateConveyor>,
    mut cmd: Commands,
    assets: Res<AssetServer>,
) {
    for CreateConveyor(entity, _, _) in msgs.read() {
        cmd.entity(*entity)
            .insert(Sprite::from_image(assets.load("sprites/conveyor.png")));
    }
}
