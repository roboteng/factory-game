use bevy::prelude::*;
use common::{
    Player,
    inventory::{Inventory, Stack},
};
use gui::{
    InteractionMode, ItemExt, WorldMode,
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
        padding: UiRect::bottom(px(26.0)),
        ..default()
    })
    .with_children(|cmd| {
        cmd.spawn(Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Row,
            column_gap: px(3.0),
            padding: UiRect::all(px(4.0)),
            border: UiRect::all(px(1.0)),
            border_radius: BorderRadius::all(px(5.0)),
            ..default()
        })
        .insert(BackgroundColor(Color::srgba_u8(12, 15, 18, 184)))
        .insert(BorderColor::all(Color::srgba_u8(180, 210, 230, 18)))
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
                slot(cmd, i, stack, asset_server, selected);
            }
        });
    });
}

const SLOT_SIZE: f64 = 64.0;

const SLOT_SELECTED: Srgba = Srgba {
    red: 245.0 / 255.0,
    green: 170.0 / 255.0,
    blue: 48.0 / 255.0,
    alpha: 1.0,
};

const SLOT_DEFAULT: Srgba = Srgba {
    red: 180.0 / 255.0,
    green: 210.0 / 255.0,
    blue: 230.0 / 255.0,
    alpha: 1.0,
};

const SLOT_KEY_COLOR: Color = Color::Srgba(Srgba {
    red: 86.0 / 255.0,
    green: 96.0 / 255.0,
    blue: 112.0 / 255.0,
    alpha: 1.0,
});

fn slot(
    cmd: &mut ChildSpawnerCommands,
    slot_idx: usize,
    stack: Option<Stack>,
    asset_server: &AssetServer,
    selected: bool,
) {
    let (border_color, bg_color) = if selected {
        (
            Color::Srgba(SLOT_SELECTED),
            Color::Srgba(Srgba {
                alpha: 23.0 / 255.0,
                ..SLOT_SELECTED
            }),
        )
    } else {
        (
            Color::Srgba(Srgba {
                alpha: 20.0 / 255.0,
                ..SLOT_DEFAULT
            }),
            Color::Srgba(Srgba {
                alpha: 8.0 / 255.0,
                ..SLOT_DEFAULT
            }),
        )
    };
    let mut child_cmd = cmd.spawn(Node {
        height: px(SLOT_SIZE),
        width: px(SLOT_SIZE),
        border: UiRect::all(px(1.0)),
        border_radius: BorderRadius::all(px(3.0)),
        position_type: PositionType::Relative,
        ..default()
    });
    child_cmd
        .insert(BorderColor::all(border_color))
        .insert(BackgroundColor(bg_color));
    child_cmd.with_children(|cmd| {
        cmd.spawn((
            Text::new(((slot_idx + 1) % 10).to_string()),
            TextFont {
                font_size: bevy::text::FontSize::Px(9.0),
                ..default()
            },
            TextColor(SLOT_KEY_COLOR),
            Node {
                position_type: PositionType::Absolute,
                top: px(3.0),
                left: px(5.0),
                ..default()
            },
        ));
        if let Some(stack) = stack {
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
        }
    });
}
