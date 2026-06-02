use bevy::prelude::*;
use common::{
    Item,
    inventory::{Inventory, Stack},
};
use gui::{InteractionMode, WorldMode, hotbar::PlacementItem, views::spawn_inventory};

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
    let inv = Inventory::new();
    spawn_inventory(&mut cmd, &inv, asset_server.as_ref());
}
