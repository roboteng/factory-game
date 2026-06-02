pub mod hotbar;

use bevy::prelude::*;
use common::{Player, inventory::Inventory};

use crate::InteractionMode;

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
    for root in roots {
        cmd.entity(root).despawn();
    }
    match mode.into_inner() {
        InteractionMode::InWorld(_) => {
            for root in roots {
                cmd.entity(root).despawn();
            }
        }
        InteractionMode::InScreen(screen_mode) => {
            let mut cmds = cmd.spawn(UiRoot);
            use crate::ScreenMode::*;
            match screen_mode {
                Inventory => {
                    let Ok(inv) = invs.get(player.0) else { return };
                    spawn_inventory(&mut cmds, &inv, &asset_server);
                }
                Menu => todo!(),
                Furnace(entity) => todo!(),
                Assembler(entity) => todo!(),
                Source(entity) => todo!(),
                Miner(entity) => todo!(),
            }
        }
    }
}

pub fn spawn_inventory(cmd: &mut EntityCommands, inv: &Inventory, asset_server: &AssetServer) {
    cmd.insert(Node {
        width: percent(100.0),
        height: percent(100.0),
        justify_content: JustifyContent::Center,
        align_content: AlignContent::Center,
        ..default()
    })
    .with_children(|cmd| {
        cmd.spawn((
            Node {
                height: px(100.0),
                width: px(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.5, 1.0, 0.5)),
        ));
    });
}
