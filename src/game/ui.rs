use bevy::{color::palettes::css::GRAY, ecs::error::info, prelude::*};

use crate::game::*;

pub struct UIPlugin;
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, startup_camera);
        app.add_systems(Update, (create_tile, create_conveyor, create_item));
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
    for CreateConveyor { entity, .. } in msgs.read() {
        cmd.entity(*entity)
            .insert(Sprite::from_image(assets.load("sprites/conveyor.png")));
    }
}

fn create_item(
    mut msgs: MessageReader<CreateConveyorItem>,
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for item in msgs.read() {
        let shape = meshes.add(Rectangle {
            half_size: Vec2 { x: 8.0, y: 8.0 },
        });
        let mat = materials.add(Color::from(GRAY));

        cmd.entity(item.entity)
            .insert((Mesh2d(shape), MeshMaterial2d(mat)));
    }
}
