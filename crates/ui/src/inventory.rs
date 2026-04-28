use bevy::prelude::*;

use factory_core::{inventory::Inventory, HandCrafter, MachineStatus, Player, Recipe, Recipes};

use super::common::{
    pane_node, section_label, spawn_inventory_panel, spawn_slot, spawn_title_bar, stack_label,
    LABEL_FONT_SIZE, SLOT_BG, SLOT_BORDER,
};
use super::{InteractionMode, ScreenMode};

const MAX_OUTPUT_SLOTS: usize = 4;

#[derive(Component)]
pub(super) struct InventoryPane;

#[derive(Component)]
pub(super) struct CloseInventoryButton;

#[derive(Component)]
pub(super) struct InventoryRecipePanel;

#[derive(Component)]
pub(super) struct InventoryProcessingPanel;

#[derive(Component)]
pub(super) struct InventoryRecipeButton(usize);

#[derive(Component)]
pub(super) struct InventoryProgressFill;

#[derive(Component)]
pub(super) struct InventoryOutputSlot(usize);

fn recipe_label(recipe: &Recipe) -> String {
    let inputs = recipe
        .inputs()
        .iter()
        .map(|s| s.item.name())
        .collect::<Vec<_>>()
        .join(" + ");
    let outputs = recipe
        .outputs()
        .iter()
        .map(|s| s.item.name())
        .collect::<Vec<_>>()
        .join(" + ");
    format!("{} \u{2192} {}", inputs, outputs)
}

pub(super) fn setup_inventory_pane(mut cmd: Commands, recipes: Res<Recipes>) {
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
        spawn_title_bar(parent, "Crafting", CloseInventoryButton);

        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|parent| {
                // Left panel: recipe selection or processing view
                parent
                    .spawn(Node {
                        width: Val::Percent(40.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(16.0)),
                        ..default()
                    })
                    .with_children(|parent| {
                        // Recipe selection panel
                        parent
                            .spawn((
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    ..default()
                                },
                                InventoryRecipePanel,
                            ))
                            .with_children(|parent| {
                                section_label(parent, "Select a Recipe");
                                for (i, recipe) in recipes.hand_craftable() {
                                    let label = recipe_label(recipe);
                                    parent
                                        .spawn((
                                            Button,
                                            Node {
                                                padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                                                margin: UiRect::bottom(Val::Px(8.0)),
                                                border: UiRect::all(Val::Px(2.0)),
                                                ..default()
                                            },
                                            BackgroundColor(SLOT_BG),
                                            BorderColor::all(SLOT_BORDER),
                                            InventoryRecipeButton(i),
                                        ))
                                        .with_children(|parent| {
                                            parent.spawn((
                                                Text::new(label),
                                                TextFont {
                                                    font_size: LABEL_FONT_SIZE,
                                                    ..default()
                                                },
                                                TextColor(Color::WHITE),
                                            ));
                                        });
                                }
                            });

                        // Processing panel
                        parent
                            .spawn((
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    ..default()
                                },
                                InventoryProcessingPanel,
                            ))
                            .with_children(|parent| {
                                section_label(parent, "Output");
                                parent
                                    .spawn(Node {
                                        flex_direction: FlexDirection::Row,
                                        flex_wrap: FlexWrap::Wrap,
                                        ..default()
                                    })
                                    .with_children(|parent| {
                                        for i in 0..MAX_OUTPUT_SLOTS {
                                            spawn_slot(parent, InventoryOutputSlot(i));
                                        }
                                    });

                                section_label(parent, "Progress");
                                parent
                                    .spawn(Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(24.0),
                                        margin: UiRect::axes(Val::Px(4.0), Val::Px(4.0)),
                                        ..default()
                                    })
                                    .insert(BackgroundColor(SLOT_BG))
                                    .with_children(|parent| {
                                        parent.spawn((
                                            Node {
                                                width: Val::Percent(0.0),
                                                height: Val::Percent(100.0),
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgba(0.2, 0.8, 0.2, 1.0)),
                                            InventoryProgressFill,
                                        ));
                                    });
                            });
                    });

                // Right panel: player inventory
                parent
                    .spawn(Node {
                        width: Val::Percent(60.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|parent| {
                        spawn_inventory_panel(parent, ());
                    });
            });
    });
}

pub(super) fn update_inventory_pane(
    mode: Res<InteractionMode>,
    mut pane: Single<
        &mut Visibility,
        (
            With<InventoryPane>,
            Without<InventoryRecipePanel>,
            Without<InventoryProcessingPanel>,
        ),
    >,
    recipe_panel: Single<
        (&mut Visibility, &mut Node),
        (
            With<InventoryRecipePanel>,
            Without<InventoryPane>,
            Without<InventoryProcessingPanel>,
            Without<InventoryProgressFill>,
        ),
    >,
    processing_panel: Single<
        (&mut Visibility, &mut Node),
        (
            With<InventoryProcessingPanel>,
            Without<InventoryPane>,
            Without<InventoryRecipePanel>,
            Without<InventoryProgressFill>,
        ),
    >,
    player: Res<Player>,
    hand_crafter_q: Query<&HandCrafter>,
    output_slots: Query<(&InventoryOutputSlot, &Children)>,
    mut fill_node: Single<&mut Node, With<InventoryProgressFill>>,
    mut texts: Query<&mut Text>,
) {
    if !matches!(*mode, InteractionMode::InScreen(ScreenMode::Inventory)) {
        **pane = Visibility::Hidden;
        return;
    }
    **pane = Visibility::Visible;

    let Ok(hand_crafter) = hand_crafter_q.get(player.0) else {
        return;
    };

    match &hand_crafter.status {
        MachineStatus::Idle => {
            let (mut vis, mut node) = recipe_panel.into_inner();
            vis.set_if_neq(Visibility::Inherited);
            node.display = Display::Flex;
            let (mut vis, mut node) = processing_panel.into_inner();
            vis.set_if_neq(Visibility::Hidden);
            node.display = Display::None;
        }
        MachineStatus::Processing {
            recipe,
            elapsed_ticks,
        } => {
            let (mut vis, mut node) = recipe_panel.into_inner();
            vis.set_if_neq(Visibility::Hidden);
            node.display = Display::None;
            let (mut vis, mut node) = processing_panel.into_inner();
            vis.set_if_neq(Visibility::Inherited);
            node.display = Display::Flex;

            let ticks = recipe.ticks();
            let progress = if ticks > 0 {
                *elapsed_ticks as f32 / ticks as f32
            } else {
                1.0
            };
            fill_node.width = Val::Percent(progress * 100.0);

            let outputs = recipe.outputs();
            for (slot_marker, children) in &output_slots {
                let label = stack_label(outputs.get(slot_marker.0).copied());
                if let Some(&child) = children.first() {
                    if let Ok(mut text) = texts.get_mut(child) {
                        **text = label.into();
                    }
                }
            }
        }
    }
}

pub(super) fn handle_inventory_recipe_button(
    interactions: Query<(&Interaction, &InventoryRecipeButton), Changed<Interaction>>,
    mode: Res<InteractionMode>,
    recipes: Res<Recipes>,
    player: Res<Player>,
    mut q: Query<(&mut HandCrafter, &mut Inventory)>,
) {
    if !matches!(*mode, InteractionMode::InScreen(ScreenMode::Inventory)) {
        return;
    }
    let Ok((mut crafter, mut inventory)) = q.get_mut(player.0) else {
        return;
    };
    for (&interaction, btn) in &interactions {
        if interaction != Interaction::Pressed {
            continue;
        }
        let recipe = &recipes.0[btn.0];
        if !matches!(crafter.status, MachineStatus::Idle) {
            continue;
        }
        if !recipe
            .inputs()
            .iter()
            .all(|s| inventory.item_count(s.item) >= s.count)
        {
            continue;
        }
        for s in recipe.inputs() {
            inventory.take_items(s);
        }
        crafter.status = MachineStatus::Processing {
            recipe: recipe.clone(),
            elapsed_ticks: 1,
        };
    }
}
