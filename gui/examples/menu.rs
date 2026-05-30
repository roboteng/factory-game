use bevy::prelude::*;
use common::inventory::Inventory;
use gui::spawn_hotbar;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.build().set(AssetPlugin {
            file_path: "../assets/".to_string(),
            ..default()
        }))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);
    let mut cmd = commands.spawn_empty();
    spawn_hotbar(&mut cmd, &asset_server, &Inventory::new());
}
