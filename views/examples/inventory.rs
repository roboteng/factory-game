use bevy::prelude::*;
use common::{
    inventory::{Inventory, Stack},
    Item,
};
use views::inventory::inventory_scene;

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

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
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
    commands.spawn_scene(inventory_scene(&inv));
}
