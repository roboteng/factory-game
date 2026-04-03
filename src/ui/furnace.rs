use bevy::prelude::*;

use crate::core::{
    inventory::{Inventory, Stack},
    Furnace, InputBuffer, OutputBuffer, Player, ProcessingMethod, RECIPES,
};

use super::common::{pane_node, section_label, spawn_screen_layout, spawn_slot, InventorySlot};
use super::{InteractionMode, ScreenMode, WorldMode};

#[derive(Component)]
pub(super) struct FurnacePane;

#[derive(Component)]
pub(super) struct CloseFurnaceButton;

#[derive(Component)]
pub(super) struct FurnaceInputSlot(pub(super) usize);

#[derive(Component)]
pub(super) struct FurnaceOutputSlot(pub(super) usize);

#[derive(Component)]
pub(super) struct FurnaceProgressFill;

/// Marks the InventorySlot entities that belong to the furnace screen's inventory panel.
/// Used to distinguish them from the standalone inventory screen's slots for click handling.
#[derive(Component, Clone)]
pub(super) struct FurnaceInventoryPanel;

pub(super) fn setup_furnace_pane(mut cmd: Commands) {
    cmd.spawn((
        pane_node(
            Val::Percent(5.0),
            Val::Percent(5.0),
            Val::Percent(5.0),
            Val::Percent(5.0),
        ),
        FurnacePane,
    ))
    .with_children(|parent| {
        spawn_screen_layout(
            parent,
            "Furnace",
            CloseFurnaceButton,
            |parent| {
                section_label(parent, "Input");
                parent
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        ..default()
                    })
                    .with_children(|parent| {
                        spawn_slot(parent, FurnaceInputSlot(0));
                    });

                section_label(parent, "Progress");
                parent
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(24.0),
                        margin: UiRect::axes(Val::Px(4.0), Val::Px(4.0)),
                        ..default()
                    })
                    .insert(BackgroundColor(super::common::SLOT_BG))
                    .with_children(|parent| {
                        parent.spawn((
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.9, 0.5, 0.1, 1.0)),
                            FurnaceProgressFill,
                        ));
                    });

                section_label(parent, "Output");
                parent
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        ..default()
                    })
                    .with_children(|parent| {
                        spawn_slot(parent, FurnaceOutputSlot(0));
                    });
            },
            FurnaceInventoryPanel,
        );
    });
}

pub(super) fn update_furnace_pane(
    mode: Res<InteractionMode>,
    mut pane: Single<&mut Visibility, With<FurnacePane>>,
    furnace_q: Query<(&Furnace, &InputBuffer, &OutputBuffer)>,
    input_slots: Query<(&FurnaceInputSlot, &Children)>,
    output_slots: Query<(&FurnaceOutputSlot, &Children)>,
    mut fill_node: Single<&mut Node, With<FurnaceProgressFill>>,
    mut texts: Query<&mut Text>,
) {
    let InteractionMode::InScreen(ScreenMode::Furnace(furnace_entity)) = *mode else {
        **pane = Visibility::Hidden;
        return;
    };
    **pane = Visibility::Visible;

    let Ok((furnace, input_buf, output_buf)) = furnace_q.get(furnace_entity) else {
        return;
    };

    let progress_fraction = {
        let active_recipe = RECIPES
            .iter()
            .filter(|r| r.method == ProcessingMethod::Furnace)
            .find(|r| {
                r.inputs.len() == input_buf.slots.len()
                    && r.inputs
                        .iter()
                        .zip(&input_buf.slots)
                        .all(|(exp, slot)| slot.as_ref() == Some(exp))
            });
        match active_recipe {
            Some(r) if r.ticks > 0 => furnace.ticks as f32 / r.ticks as f32,
            _ => 0.0,
        }
    };
    fill_node.width = Val::Percent(progress_fraction * 100.0);

    for (slot_marker, children) in &input_slots {
        let label = input_buf
            .slots
            .get(slot_marker.0)
            .and_then(|s| *s)
            .map(|item| item.name())
            .unwrap_or("");
        if let Some(&child) = children.first() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = label.into();
            }
        }
    }

    for (slot_marker, children) in &output_slots {
        let label = output_buf
            .items
            .get(slot_marker.0)
            .map(|item| item.name())
            .unwrap_or("");
        if let Some(&child) = children.first() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = label.into();
            }
        }
    }
}

pub(super) fn handle_furnace_close_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<CloseFurnaceButton>)>,
    mut mode: ResMut<InteractionMode>,
) {
    for &interaction in interaction.iter() {
        if interaction == Interaction::Pressed {
            *mode = InteractionMode::InWorld(WorldMode::None);
        }
    }
}

pub(super) fn handle_furnace_inventory_slot_clicks(
    interactions: Query<
        (&Interaction, &InventorySlot),
        (Changed<Interaction>, With<FurnaceInventoryPanel>),
    >,
    mode: Res<InteractionMode>,
    player: Res<Player>,
    mut inventories: Query<&mut Inventory>,
    mut furnace_inputs: Query<&mut InputBuffer, With<Furnace>>,
) {
    let InteractionMode::InScreen(ScreenMode::Furnace(furnace_entity)) = *mode else {
        return;
    };
    for (&interaction, slot_marker) in &interactions {
        if interaction != Interaction::Pressed {
            continue;
        }
        let Ok(mut inv) = inventories.get_mut(player.0) else {
            continue;
        };
        let Ok(mut input_buf) = furnace_inputs.get_mut(furnace_entity) else {
            continue;
        };
        let Some(stack) = inv.take_slot(slot_marker.0) else {
            continue;
        };
        if input_buf.accepts(stack.item) {
            input_buf.fill_slot(stack.item);
            let remaining = stack.count - 1;
            inv.insert(Stack::new(stack.item, remaining)).unwrap();
        } else {
            let _ = inv.insert(stack);
        }
    }
}

pub(super) fn handle_furnace_output_slot_clicks(
    interactions: Query<(&Interaction, &FurnaceOutputSlot), Changed<Interaction>>,
    mode: Res<InteractionMode>,
    player: Res<Player>,
    mut inventories: Query<&mut Inventory>,
    mut furnace_outputs: Query<&mut OutputBuffer, With<Furnace>>,
) {
    let InteractionMode::InScreen(ScreenMode::Furnace(furnace_entity)) = *mode else {
        return;
    };
    for (&interaction, slot_marker) in &interactions {
        if interaction != Interaction::Pressed {
            continue;
        }
        let Ok(mut inv) = inventories.get_mut(player.0) else {
            continue;
        };
        let Ok(mut output_buf) = furnace_outputs.get_mut(furnace_entity) else {
            continue;
        };
        if slot_marker.0 < output_buf.items.len() {
            let item = output_buf.items.remove(slot_marker.0);
            let _ = inv.insert(Stack::new(item, 1.try_into().unwrap()));
        }
    }
}
