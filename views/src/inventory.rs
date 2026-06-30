use bevy::prelude::*;
use common::{
    inventory::{Inventory, Stack},
    Player,
};
use gui::{InteractionMode, ItemExt, ScreenMode};

#[derive(Component, Clone, Default)]
pub struct InventoryRoot;

pub fn view(
    roots: Query<Entity, With<InventoryRoot>>,
    mode: Res<InteractionMode>,
    player: Res<Player>,
    invs: Query<Ref<Inventory>>,
    mut cmd: Commands,
) {
    let mode_changed = mode.is_changed();
    match mode.into_inner() {
        InteractionMode::InWorld(_) => {
            for root in roots {
                cmd.entity(root).despawn();
            }
        }
        InteractionMode::InScreen(screen_mode) => {
            use ScreenMode::*;
            match screen_mode {
                Inventory => {
                    let Ok(inv) = invs.get(player.0) else { return };
                    if inv.is_changed() || mode_changed {
                        for root in roots {
                            cmd.entity(root).despawn();
                        }
                        cmd.spawn_scene(inventory_scene(&inv));
                    }
                }
                Menu => todo!(),
                Furnace(_entity) => todo!(),
                Assembler(_entity) => todo!(),
                Source(_entity) => todo!(),
                Miner(_entity) => todo!(),
            }
        }
    }
}

const SLOT_SIZE: f64 = 64.0;

pub fn inventory_scene(inv: &Inventory) -> impl Scene {
    let slots: Vec<_> = (0..20).map(|i| slot(inv.get(i))).collect();
    bsn!(
        InventoryRoot
        Node {
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        Children [
            Node {
                height: percent(75),
                width: percent(75),
            }
            BackgroundColor(Color::srgba(0.25, 0.25, 0.25, 0.875))
            Children [
                Node {
                    width: percent(100),
                    height: percent(100),
                    column_gap: px(5.0),
                    row_gap: px(5.0),
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Start,
                    align_content: AlignContent::Start,
                }
                Children [{ slots }]
            ]
        ]
    )
}

fn slot(stack: Option<Stack>) -> impl Scene {
    bsn!(
        Node {
            height: px(SLOT_SIZE),
            width: px(SLOT_SIZE),
        }
        BackgroundColor(Color::srgba(0.35, 0.35, 0.35, 0.875))
        Children [
            { stack.map(stack_content) }
        ]
    )
}

fn stack_content(stack: Stack) -> impl SceneList {
    bsn_list! {
        stack_icon(stack),
        item_count(stack),
    }
}

fn stack_icon(stack: Stack) -> impl Scene {
    bsn! {
        ImageNode {
            image: { stack.item.icon() },
        }
        Node {
            width: percent(100),
            height: percent(100),
        }
    }
}

fn item_count(stack: Stack) -> impl Scene {
    if stack.count == 1 && stack.item.stack_size() == 1 {
        Box::new(bsn!()) as Box<dyn Scene>
    } else {
        Box::new(bsn!(
            Text::new(format!("{}", stack.count))
            Node {
                position_type: PositionType::Absolute,
                bottom: px(2.0),
                right: px(4.0),
            }
        ))
    }
}
