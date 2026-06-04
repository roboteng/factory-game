use bevy::{ecs::relationship::RelatedSpawnerCommands, prelude::*};
use common::{
    Player,
    inventory::{Inventory, Stack},
};
use gui::{InteractionMode, ItemExt, ScreenMode};

#[derive(Component)]
pub struct UiRoot;

pub fn view(
    roots: Query<Entity, With<UiRoot>>,
    mode: Res<InteractionMode>,
    player: Res<Player>,
    invs: Query<Ref<Inventory>>,
    asset_server: Res<AssetServer>,
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
                        let mut cmds = cmd.spawn(UiRoot);
                        spawn_inventory(&mut cmds, &inv, &asset_server);
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

const SLOT_SIZE: f32 = 64.0;

pub fn spawn_inventory(cmd: &mut EntityCommands, inv: &Inventory, asset_server: &AssetServer) {
    cmd.insert(Node {
        width: percent(100.0),
        height: percent(100.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|cmd| {
        cmd.spawn((
            Node {
                height: percent(75.0),
                width: percent(75.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.25, 0.25, 0.25, 0.875)),
        ))
        .with_children(|cmd| {
            cmd.spawn(
                (Node {
                    width: percent(100.0),
                    height: percent(100.0),
                    column_gap: px(5.0),
                    row_gap: px(5.0),
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Start,
                    align_content: AlignContent::Start,
                    ..default()
                }),
            )
            .with_children(|cmd| {
                for i in 0..20 {
                    slot(cmd, inv.get(i), asset_server);
                }
            });
        });
    });
}

fn slot(
    cmd: &mut RelatedSpawnerCommands<ChildOf>,
    stack: Option<Stack>,
    asset_server: &AssetServer,
) {
    let mut c = cmd.spawn((
        Node {
            height: px(SLOT_SIZE),
            width: px(SLOT_SIZE),
            ..default()
        },
        BackgroundColor(Color::srgba(0.35, 0.35, 0.35, 0.875)),
    ));
    let Some(stack) = stack else { return };
    c.with_children(|cmd| {
        cmd.spawn(ImageNode::new(asset_server.load(stack.item.icon())));
        cmd.spawn((
            Node {
                right: px(0),
                bottom: px(0),
                position_type: PositionType::Absolute,
                ..default()
            },
            Text::new(stack.count.to_string()),
        ));
    });
}
