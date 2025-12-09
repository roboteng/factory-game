use bevy::{color::palettes::css::GRAY, prelude::*};

use crate::game::*;

pub struct UIPlugin;
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, startup_camera);
        app.add_observer(on_create_tile);
        app.add_observer(on_create_belt);
        app.add_observer(on_create_item);
    }
}

fn startup_camera(mut cmd: Commands) {
    cmd.spawn(Camera2d);
}

fn on_create_tile(trigger: On<CreateTile>, mut cmd: Commands, assets: Res<AssetServer>) {
    cmd.entity(trigger.entity)
        .insert(Sprite::from_image(assets.load("sprites/tile.png")));
}

fn on_create_belt(trigger: On<BeltCreated>, mut cmd: Commands, assets: Res<AssetServer>) {
    let sprite = match trigger.new_belt.curvature() {
        Curvature::Straight => Sprite::from_image(assets.load("sprites/belt.png")),
        Curvature::Counterclockwise => Sprite::from_image(assets.load("sprites/belt_curved.png")),
        Curvature::Clockwise => {
            let mut s = Sprite::from_image(assets.load("sprites/belt_curved.png"));
            s.flip_y = true;
            s
        }
    };
    cmd.entity(trigger.entity).insert(sprite);
}

fn on_create_item(
    trigger: On<CreateBeltItem>,
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let shape = meshes.add(Rectangle {
        half_size: Vec2 { x: 3.5, y: 3.5 },
    });
    let mat = materials.add(Color::from(GRAY));

    cmd.entity(trigger.entity)
        .insert((Mesh2d(shape), MeshMaterial2d(mat)));
}
