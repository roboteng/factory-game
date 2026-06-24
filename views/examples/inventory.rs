use bevy::prelude::*;
use common::{
    Item,
    inventory::{Inventory, Stack},
};
use views::inventory::spawn_inventory;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .build()
                .set(AssetPlugin {
                    file_path: "../assets/".to_string(),
                    ..default()
                })
                .disable::<bevy::pbr::PbrPlugin>()
                .disable::<bevy::gltf::GltfPlugin>()
                .disable::<bevy::gilrs::GilrsPlugin>()
                .disable::<bevy::animation::AnimationPlugin>()
                .disable::<bevy::gizmos::GizmoPlugin>()
                .disable::<bevy::gizmos_render::GizmoRenderPlugin>()
                .disable::<bevy::light::LightPlugin>()
                .disable::<bevy::anti_alias::AntiAliasPlugin>()
                .disable::<bevy::post_process::PostProcessPlugin>(),
        )
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);
    let mut cmd = commands.spawn_empty();
    let mut inv = Inventory::new();
    inv.insert(Stack {
        item: Item::Furnace,
        count: 3,
    })
    .unwrap();
    inv.insert(Stack {
        item: Item::Belt,
        count: 4,
    })
    .unwrap();
    spawn_inventory(&mut cmd, &inv, asset_server.as_ref());
}
