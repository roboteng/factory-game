use bevy::prelude::*;
use common::inventory::Inventory;
use views::spawn_inventory;

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
                .disable::<bevy::audio::AudioPlugin>()
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
    let inv = Inventory::new();
    spawn_inventory(&mut cmd, &inv, asset_server.as_ref());
}
