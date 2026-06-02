use crate::visuals::ItemExt;
use bevy::prelude::*;
use common::{
    Player,
    inventory::{Inventory, Stack},
};

use crate::{
    InteractionMode, WorldMode,
    hotbar::{self, Hotbar, PlacementItem},
};

#[derive(Component)]
pub struct HotbarTag;

pub fn hotbar_view(
    player: Res<Player>,
    invs: Query<Ref<Inventory>>,
    hotbar: Res<hotbar::Hotbar>,
    mode: Res<InteractionMode>,
    asset_server: Res<AssetServer>,
    prev_hotbar: Query<Entity, With<HotbarTag>>,
    mut cmd: Commands,
) {
    let Ok(inv) = invs.get(player.0) else {
        return;
    };
    if !(hotbar.is_changed() || mode.is_changed() || inv.is_changed()) {
        return;
    }

    for hb in prev_hotbar {
        cmd.entity(hb).despawn();
    }

    let mut cmd = cmd.spawn(HotbarTag);

    spawn_hotbar(&mut cmd, &asset_server, &hotbar, &inv, &mode);
}

pub fn spawn_hotbar(
    cmd: &mut EntityCommands,
    asset_server: &AssetServer,
    hotbar: &Hotbar,
    inv: &Inventory,
    mode: &InteractionMode,
) {
    cmd.insert(Node {
        width: percent(100),
        height: percent(100),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::FlexEnd,
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        ..default()
    })
    .with_children(|cmd| {
        cmd.spawn(Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Row,
            column_gap: px(10.0),
            padding: UiRect::all(px(5.0)),
            ..default()
        })
        .with_children(|cmd| {
            for i in 0..10 {
                let stack = hotbar.0[i].map(|item| Stack {
                    count: inv.item_count(item),
                    item,
                });
                let selected = match mode {
                    InteractionMode::InWorld(WorldMode::Placing(PlacementItem::HotbarSlot(
                        slot,
                    ))) => *slot as usize == i,
                    InteractionMode::InWorld(WorldMode::Placing(PlacementItem::Custom(item))) => {
                        Some(*item) == hotbar.0[i]
                    }
                    InteractionMode::InWorld(_) => false,
                    InteractionMode::InScreen(_) => false,
                };
                slot(cmd, stack, asset_server, selected);
            }
        });
    });
}

const SLOT_SIZE: f64 = 64.0;

fn slot(
    cmd: &mut ChildSpawnerCommands,
    stack: Option<Stack>,
    asset_server: &AssetServer,
    selected: bool,
) {
    let mut child_cmd = cmd.spawn((
        Node {
            height: px(SLOT_SIZE),
            width: px(SLOT_SIZE),
            border: UiRect::all(px(2)),
            position_type: PositionType::Relative,
            ..default()
        },
        BorderColor::all(if selected { Color::WHITE } else { Color::BLACK }),
    ));
    if let Some(stack) = stack {
        child_cmd.with_children(|cmd| {
            cmd.spawn((
                ImageNode::new(asset_server.load(stack.item.icon())).with_color(
                    if stack.count == 0 {
                        Color::linear_rgb(0.25, 0.25, 0.25)
                    } else {
                        Color::WHITE
                    },
                ),
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
            ));
            if !(stack.count == 1 && stack.item.stack_size() == 1) {
                cmd.spawn((
                    Text::new(format!("{}", stack.count)),
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: px(2.0),
                        right: px(4.0),
                        ..default()
                    },
                ));
            }
        });
    }
}
