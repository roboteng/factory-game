use bevy::{prelude::*, window::CompositeAlphaMode};
use common::{
    Item,
    inventory::{Inventory, Stack},
};
use gui::{
    InteractionMode, WorldMode,
    hotbar::{Hotbar, PlacementItem},
};
use views::hotbar::hotbar_scene;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .build()
                .set(AssetPlugin {
                    file_path: "../assets/".to_string(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        composite_alpha_mode: CompositeAlphaMode::Opaque,
                        ..default()
                    }),
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
    let mut hotbar = Hotbar(Default::default());
    hotbar.0[0] = Some(Item::PickAxe);
    hotbar.0[1] = Some(Item::IronOre);
    hotbar.0[2] = Some(Item::Rock);
    hotbar.0[3] = Some(Item::Dirt);
    hotbar.0[4] = Some(Item::Rock);

    let mut inventory = Inventory::new();
    inventory.insert(Item::PickAxe.into()).unwrap();
    inventory
        .insert(Stack {
            item: Item::Dirt,
            count: 3,
        })
        .unwrap();

    commands.spawn_scene(hotbar_scene(
        &hotbar,
        &inventory,
        &InteractionMode::InWorld(WorldMode::Placing(PlacementItem::Custom(Item::Rock))),
    ));
}
