use bevy::prelude::*;

use super::common::{pane_node, spawn_screen_layout};
use super::{InteractionMode, ScreenMode};

#[derive(Component)]
pub(super) struct InventoryPane;

#[derive(Component)]
pub(super) struct CloseInventoryButton;

pub(super) fn setup_inventory_pane(mut cmd: Commands) {
    cmd.spawn((
        pane_node(
            Val::Percent(5.0),
            Val::Percent(5.0),
            Val::Percent(5.0),
            Val::Percent(5.0),
        ),
        InventoryPane,
    ))
    .with_children(|parent| {
        spawn_screen_layout(parent, "Inventory", CloseInventoryButton, |_| {}, ());
    });
}

pub(super) fn update_inventory_pane(
    mode: Res<InteractionMode>,
    mut pane: Single<&mut Visibility, With<InventoryPane>>,
) {
    **pane = if matches!(*mode, InteractionMode::InScreen(ScreenMode::Inventory)) {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}
