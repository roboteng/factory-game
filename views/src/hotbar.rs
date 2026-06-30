use bevy::{prelude::*, scene::RelatedScenes};
use common::{
    inventory::{Inventory, Stack},
    Player,
};
use gui::{
    hotbar::{self, Hotbar, PlacementItem},
    InteractionMode, WorldMode,
};

use crate::slot;

#[derive(Component, Clone, Default)]
pub struct HotbarRoot;

pub fn hotbar_view(
    player: Res<Player>,
    invs: Query<Ref<Inventory>>,
    hotbar: Res<hotbar::Hotbar>,
    mode: Res<InteractionMode>,
    prev_hotbar: Query<Entity, With<HotbarRoot>>,
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

    cmd.spawn_scene(hotbar_scene(&hotbar, &inv, &mode));
}

pub fn hotbar_scene(hotbar: &Hotbar, inv: &Inventory, mode: &InteractionMode) -> impl Scene {
    let slots: Vec<_> = (0..10)
        .map(|i| {
            let stack = hotbar.0[i].map(|item| Stack {
                count: inv.item_count(item),
                item,
            });
            let selected = match mode {
                InteractionMode::InWorld(WorldMode::Placing(PlacementItem::HotbarSlot(slot))) => {
                    *slot as usize == i
                }
                InteractionMode::InWorld(WorldMode::Placing(PlacementItem::Custom(item))) => {
                    Some(*item) == hotbar.0[i]
                }
                InteractionMode::InWorld(_) => false,
                InteractionMode::InScreen(_) => false,
            };
            slot(i, stack, selected)
        })
        .collect();
    bsn!(
        HotbarRoot
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexEnd,
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            padding: UiRect::bottom(px(26.0)),
        }
        Children [
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Row,
                column_gap: px(3.0),
                padding: UiRect::all(px(4.0)),
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::all(px(5.0)),
            }
            BackgroundColor(Color::srgba_u8(12, 15, 18, 184))
            BorderColor::all(Color::srgba_u8(180, 210, 230, 18))
            Children [{ slots }]
        ]
    )
}

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

fn slot(slot_idx: usize, stack: Option<Stack>, selected: bool) -> impl Scene {
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
    bsn!(
        Node {
            height: px(slot::SIZE),
            width: px(slot::SIZE),
            border: UiRect::all(px(1.0)),
            border_radius: BorderRadius::all(px(3.0)),
            position_type: PositionType::Relative,
        }
        BorderColor::all(border_color)
        BackgroundColor(bg_color)
        Children [
            (
                Text::new(((slot_idx + 1) % 10).to_string())
                TextFont {
                    font_size: bevy::text::FontSize::Px(9.0),
                }
                TextColor(SLOT_KEY_COLOR)
                Node {
                    position_type: PositionType::Absolute,
                    top: px(3.0),
                    left: px(5.0),
                }
            ),
            { stack.map(slot::stack_content) },
        ]
    )
}
