use crate::core::*;
use bevy::prelude::*;

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui);
        app.add_systems(Update, apply_belt_sprites);
    }
}

fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn apply_belt_sprites(
    belts: Query<(Entity, &Belt), Changed<Belt>>,
    assets: Res<AssetServer>,
    mut cmd: Commands,
) {
    let belt_mesh = assets.load("sprites/belt.png");
    for (ent, belt) in belts.iter() {
        cmd.entity(ent)
            .insert(Sprite::from_image(belt_mesh.clone()));
    }
}
