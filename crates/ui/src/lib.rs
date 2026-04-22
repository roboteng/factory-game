use factory_core::{inventory::Inventory, Player};

use bevy::prelude::*;

mod assembler;
mod common;
mod furnace;
mod hotbar;
mod inventory;
mod menu;
mod miner;
mod player_controller;
mod source;
mod visuals;

use assembler::{
    handle_assembler_inventory_slot_clicks, handle_assembler_output_slot_clicks,
    handle_assembler_recipe_button, handle_clear_assembler_recipe, setup_assembler_pane,
    update_assembler_pane, CloseAssemblerButton,
};
use common::{stack_label, InventorySlot};
use furnace::{
    handle_furnace_inventory_slot_clicks, handle_furnace_output_slot_clicks, setup_furnace_pane,
    update_furnace_pane, CloseFurnaceButton,
};
use hotbar::PlacementItem;
use inventory::{setup_inventory_pane, update_inventory_pane, CloseInventoryButton};
use menu::{setup_menu_pane, update_menu_pane, ResumeButton};
use miner::{
    handle_miner_output_slot_clicks, setup_miner_pane, update_miner_pane, CloseMinerButton,
};
use source::{
    handle_source_item_button, handle_source_scroll, setup_source_pane, update_source_pane,
    CloseSourceButton,
};

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
        app.add_systems(Startup, setup_assembler_pane);

        app.add_systems(
            Update,
            update_inventory_pane.after(player_controller::cursor_grab),
        );
        app.add_systems(Update, handle_close_button::<CloseInventoryButton>);
        app.add_systems(
            Update,
            update_menu_pane.after(player_controller::cursor_grab),
        );
        app.add_systems(Update, handle_close_button::<ResumeButton>);
        app.add_systems(
            Update,
            update_furnace_pane.after(player_controller::cursor_grab),
        );
        app.add_systems(Update, handle_close_button::<CloseFurnaceButton>);
        app.add_systems(Update, handle_furnace_inventory_slot_clicks);
        app.add_systems(Update, handle_furnace_output_slot_clicks);
        app.add_systems(
            Update,
            update_assembler_pane.after(player_controller::cursor_grab),
        );
        app.add_systems(Update, handle_close_button::<CloseAssemblerButton>);
        app.add_systems(Update, handle_assembler_recipe_button);
        app.add_systems(Update, handle_clear_assembler_recipe);
        app.add_systems(Update, handle_assembler_inventory_slot_clicks);
        app.add_systems(Update, handle_assembler_output_slot_clicks);
        app.add_systems(Startup, setup_source_pane);
        app.add_systems(
            Update,
            update_source_pane.after(player_controller::cursor_grab),
        );
        app.add_systems(Update, handle_close_button::<CloseSourceButton>);
        app.add_systems(Update, handle_source_item_button);
        app.add_systems(Update, handle_source_scroll);
        app.add_systems(Startup, setup_miner_pane);
        app.add_systems(
            Update,
            update_miner_pane.after(player_controller::cursor_grab),
        );
        app.add_systems(Update, handle_close_button::<CloseMinerButton>);
        app.add_systems(Update, handle_miner_output_slot_clicks);
        app.add_systems(Update, update_inventory_slots);
    }
}

/// When present and `true`, the player uses a free-flying noclip camera
/// instead of the physics-based controller. Set via the `--fly` CLI flag.
#[derive(Resource)]
pub struct FlyMode(pub bool);

#[derive(Default, PartialEq, Eq)]
pub(crate) enum WorldMode {
    #[default]
    None,
    Placing(PlacementItem),
    Deleting,
    ChangingIncline,
}

#[derive(PartialEq, Eq)]
pub(crate) enum ScreenMode {
    Inventory,
    Menu,
    Furnace(Entity),
    Assembler(Entity),
    Source(Entity),
    Miner(Entity),
}

#[derive(Resource, PartialEq, Eq)]
pub(crate) enum InteractionMode {
    InWorld(WorldMode),
    InScreen(ScreenMode),
}

impl Default for InteractionMode {
    fn default() -> Self {
        InteractionMode::InWorld(WorldMode::None)
    }
}

/// Generic close button handler for all screen close buttons.
/// Closes the current screen and returns to world mode.
fn handle_close_button<T: Component>(
    interaction: Query<&Interaction, (Changed<Interaction>, With<T>)>,
    mut mode: ResMut<InteractionMode>,
) {
    for &interaction in interaction.iter() {
        if interaction == Interaction::Pressed {
            *mode = InteractionMode::InWorld(WorldMode::None);
        }
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
        let label = stack_label(inventory.get(slot_marker.0));
        if let Some(&child) = children.first() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = label.into();
            }
        }
    }
}
