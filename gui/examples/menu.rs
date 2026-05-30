use bevy::{ecs::relationship::RelatedSpawnerCommands, prelude::*};
use common::{inventory::Stack, Item};
use gui::ItemExt;
const SLOT_SIZE: f64 = 64.0;

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
    commands
        .spawn((Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexEnd,
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            ..default()
        },))
        .with_children(|cmd| spawn_hotbar(cmd, &asset_server));
}

fn spawn_hotbar(cmd: &mut RelatedSpawnerCommands<ChildOf>, asset_server: &AssetServer) {
    cmd.spawn(Node {
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        flex_direction: FlexDirection::Row,
        column_gap: px(10.0),
        padding: UiRect::all(px(5.0)),
        ..default()
    })
    .with_children(|c| {
        for _ in 0..10 {
            c.spawn(slot(
                Stack {
                    item: Item::PickAxe,
                    count: 2,
                },
                asset_server,
            ));
        }
    });
}

fn slot(stack: Stack, asset_server: &AssetServer) -> impl Bundle {
    (
        Node {
            height: px(SLOT_SIZE),
            width: px(SLOT_SIZE),
            border: UiRect::all(px(2)),
            position_type: PositionType::Relative,
            ..default()
        },
        BorderColor::all(Color::BLACK),
        children![
            (
                ImageNode::new(asset_server.load(stack.item.icon())),
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
            ),
            (
                Text::new(format!("{}", stack.count)),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(2.0),
                    right: px(4.0),
                    ..default()
                },
            ),
        ],
    )
}
