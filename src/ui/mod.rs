use crate::core::{inventory::Inventory, Player};

use bevy::prelude::*;

mod common;
mod furnace;
mod hotbar;
mod inventory;
mod menu;
mod player_controller;
mod visuals;

use common::InventorySlot;
use furnace::{
    handle_furnace_close_button, handle_furnace_inventory_slot_clicks,
    handle_furnace_output_slot_clicks, setup_furnace_pane, update_furnace_pane,
};
use hotbar::PlacementItem;
use inventory::{handle_inventory_close_button, setup_inventory_pane, update_inventory_pane};
use menu::{handle_menu_resume_button, setup_menu_pane, update_menu_pane};

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(hotbar::HotbarPlugin);
        app.add_plugins(player_controller::PlayerControllerPlugin);
        app.add_plugins(visuals::VisualsPlugin);
        app.init_resource::<InteractionMode>();
        app.add_systems(Startup, setup_inventory_pane);
        app.add_systems(Startup, setup_menu_pane);
        app.add_systems(Startup, setup_furnace_pane);

        app.add_systems(
            Update,
            update_inventory_pane.after(player_controller::cursor_grab),
        );
        app.add_systems(Update, handle_inventory_close_button);
        app.add_systems(
            Update,
            update_menu_pane.after(player_controller::cursor_grab),
        );
        app.add_systems(Update, handle_menu_resume_button);
        app.add_systems(
            Update,
            update_furnace_pane.after(player_controller::cursor_grab),
        );
        app.add_systems(Update, handle_furnace_close_button);
        app.add_systems(Update, handle_furnace_inventory_slot_clicks);
        app.add_systems(Update, handle_furnace_output_slot_clicks);
        app.add_systems(Update, update_inventory_slots);
    }
}

#[derive(Default, PartialEq, Eq)]
pub(super) enum WorldMode {
    #[default]
    None,
    Placing(PlacementItem),
    Deleting,
    ChangingIncline,
}

#[derive(PartialEq, Eq)]
pub(super) enum ScreenMode {
    Inventory,
    Menu,
    Furnace(Entity),
}

#[derive(Resource, PartialEq, Eq)]
pub(super) enum InteractionMode {
    InWorld(WorldMode),
    InScreen(ScreenMode),
}

impl Default for InteractionMode {
    fn default() -> Self {
        InteractionMode::InWorld(WorldMode::None)
    }
}

/// Updates all InventorySlot labels whenever the player's inventory changes.
/// Shared by the inventory screen and the furnace screen's inventory panel.
fn update_inventory_slots(
    player: Res<Player>,
    inventory_q: Query<&Inventory, Changed<Inventory>>,
    inv_slots: Query<(&InventorySlot, &Children)>,
    mut texts: Query<&mut Text>,
) {
    let Ok(inventory) = inventory_q.get(player.0) else {
        return;
    };
    for (slot_marker, children) in &inv_slots {
        let label = match inventory.get(slot_marker.0) {
            Some(stack) => format!("{}\n\u{00d7}{}", stack.item.name(), stack.count),
            None => String::new(),
        };
        if let Some(&child) = children.first() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = label.into();
            }
        }
    }
}
