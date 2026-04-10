use bevy::prelude::*;

use crate::core::{
    Assembler, InputBuffer, LoadMachineInput, MachineStatus, OutputBuffer, Recipe, Recipes,
    SetAssemblerRecipe, UnloadMachineOutput,
};

use super::common::{
    pane_node, section_label, spawn_inventory_panel, spawn_slot, spawn_title_bar, stack_label,
    InventorySlot, CLOSE_BTN_BG, CLOSE_BTN_FONT_SIZE, CLOSE_BTN_SIZE, LABEL_FONT_SIZE, SLOT_BG,
    SLOT_BORDER,
};
use super::{InteractionMode, ScreenMode};

const MAX_INPUT_SLOTS: usize = 4;
const MAX_OUTPUT_SLOTS: usize = 4;

#[derive(Component)]
pub(super) struct AssemblerPane;

#[derive(Component)]
pub(super) struct CloseAssemblerButton;

#[derive(Component)]
pub(super) struct AssemblerRecipePanel;

/// Index into RECIPES for a specific assembler recipe button.
#[derive(Component)]
pub(super) struct AssemblerRecipeButton(usize);

#[derive(Component)]
pub(super) struct AssemblerProcessingPanel;

#[derive(Component)]
pub(super) struct AssemblerInputSlot(pub(super) usize);

#[derive(Component)]
pub(super) struct AssemblerOutputSlot(pub(super) usize);

#[derive(Component)]
pub(super) struct AssemblerProgressFill;

#[derive(Component, Clone)]
pub(super) struct AssemblerInventoryPanel;

#[derive(Component)]
pub(super) struct ClearAssemblerRecipeButton;

fn recipe_label(recipe: &Recipe) -> String {
    let Recipe::AssemblerRecipe(ar) = recipe else {
        return String::new();
    };
    let inputs = ar
        .input
        .iter()
        .map(|s| s.item.name())
        .collect::<Vec<_>>()
        .join(" + ");
    let outputs = ar
        .output
        .iter()
        .map(|s| s.item.name())
        .collect::<Vec<_>>()
        .join(" + ");
    format!("{} \u{2192} {}", inputs, outputs)
}

pub(super) fn setup_assembler_pane(mut cmd: Commands, recipes: Res<Recipes>) {
    cmd.spawn((
        pane_node(
            Val::Percent(5.0),
            Val::Percent(5.0),
            Val::Percent(5.0),
            Val::Percent(5.0),
        ),
        AssemblerPane,
    ))
    .with_children(|parent| {
        spawn_title_bar(parent, "Assembler", CloseAssemblerButton);

        // Recipe selection panel (shown when no recipe is set)
        parent
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    ..default()
                },
                AssemblerRecipePanel,
                Visibility::Inherited,
            ))
            .with_children(|parent| {
                section_label(parent, "Select a Recipe");
                for (i, recipe) in recipes
                    .0
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| matches!(r, Recipe::AssemblerRecipe(_)))
                {
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
                            AssemblerRecipeButton(i),
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

        // Processing panel (shown when a recipe is set)
        parent
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_grow: 1.0,
                    ..default()
                },
                AssemblerProcessingPanel,
                Visibility::Inherited,
            ))
            .with_children(|parent| {
                // Left panel: slots + progress + clear button
                parent
                    .spawn(Node {
                        width: Val::Percent(40.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(16.0)),
                        ..default()
                    })
                    .with_children(|parent| {
                        section_label(parent, "Input");
                        parent
                            .spawn(Node {
                                flex_direction: FlexDirection::Row,
                                flex_wrap: FlexWrap::Wrap,
                                ..default()
                            })
                            .with_children(|parent| {
                                for i in 0..MAX_INPUT_SLOTS {
                                    spawn_slot(parent, AssemblerInputSlot(i));
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
                            .insert(BackgroundColor(super::common::SLOT_BG))
                            .with_children(|parent| {
                                parent.spawn((
                                    Node {
                                        width: Val::Percent(0.0),
                                        height: Val::Percent(100.0),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.2, 0.8, 0.2, 1.0)),
                                    AssemblerProgressFill,
                                ));
                            });

                        section_label(parent, "Output");
                        parent
                            .spawn(Node {
                                flex_direction: FlexDirection::Row,
                                flex_wrap: FlexWrap::Wrap,
                                ..default()
                            })
                            .with_children(|parent| {
                                for i in 0..MAX_OUTPUT_SLOTS {
                                    spawn_slot(parent, AssemblerOutputSlot(i));
                                }
                            });

                        // Clear Recipe button
                        parent
                            .spawn((
                                Button,
                                Node {
                                    width: Val::Px(CLOSE_BTN_SIZE * 4.0),
                                    height: Val::Px(CLOSE_BTN_SIZE),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    margin: UiRect::top(Val::Px(12.0)),
                                    ..default()
                                },
                                BackgroundColor(CLOSE_BTN_BG),
                                ClearAssemblerRecipeButton,
                            ))
                            .with_children(|parent| {
                                parent.spawn((
                                    Text::new("Clear Recipe"),
                                    TextFont {
                                        font_size: CLOSE_BTN_FONT_SIZE,
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                ));
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
                        spawn_inventory_panel(parent, AssemblerInventoryPanel);
                    });
            });
    });
}

pub(super) fn update_assembler_pane(
    mode: Res<InteractionMode>,
    mut pane: Single<
        &mut Visibility,
        (
            With<AssemblerPane>,
            (
                Without<AssemblerRecipePanel>,
                Without<AssemblerProcessingPanel>,
            ),
        ),
    >,
    recipe_panel: Single<
        (&mut Visibility, &mut Node),
        (
            With<AssemblerRecipePanel>,
            (
                Without<AssemblerPane>,
                Without<AssemblerProcessingPanel>,
                Without<AssemblerProgressFill>,
            ),
        ),
    >,
    processing_panel: Single<
        (&mut Visibility, &mut Node),
        (
            With<AssemblerProcessingPanel>,
            (
                Without<AssemblerPane>,
                Without<AssemblerRecipePanel>,
                Without<AssemblerProgressFill>,
            ),
        ),
    >,
    assembler_q: Query<(&Assembler, &InputBuffer, &OutputBuffer)>,
    input_slots: Query<(&AssemblerInputSlot, &Children)>,
    output_slots: Query<(&AssemblerOutputSlot, &Children)>,
    mut fill_node: Single<&mut Node, With<AssemblerProgressFill>>,
    mut texts: Query<&mut Text>,
) {
    let InteractionMode::InScreen(ScreenMode::Assembler(assembler_entity)) = *mode else {
        **pane = Visibility::Hidden;
        return;
    };
    **pane = Visibility::Visible;

    let Ok((assembler, input_buf, output_buf)) = assembler_q.get(assembler_entity) else {
        return;
    };

    if assembler.configured_recipe.is_none() {
        let (mut vis, mut node) = recipe_panel.into_inner();
        vis.set_if_neq(Visibility::Inherited);
        node.display = Display::Flex;
        let (mut vis, mut node) = processing_panel.into_inner();
        vis.set_if_neq(Visibility::Hidden);
        node.display = Display::None;
        return;
    }

    let (mut vis, mut node) = recipe_panel.into_inner();
    vis.set_if_neq(Visibility::Hidden);
    node.display = Display::None;
    let (mut vis, mut node) = processing_panel.into_inner();
    vis.set_if_neq(Visibility::Inherited);
    node.display = Display::Flex;

    let progress_fraction = match &assembler.status {
        MachineStatus::Processing {
            recipe,
            elapsed_ticks,
        } if recipe.ticks > 0 => *elapsed_ticks as f32 / recipe.ticks as f32,
        _ => 0.0,
    };
    fill_node.width = Val::Percent(progress_fraction * 100.0);

    for (slot_marker, children) in &input_slots {
        let label = stack_label(input_buf.slots.get(slot_marker.0).copied());
        if let Some(&child) = children.first() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = label.into();
            }
        }
    }

    let view = output_buf.view();
    for (slot_marker, children) in &output_slots {
        let label = stack_label(view.get(slot_marker.0).copied());
        if let Some(&child) = children.first() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = label.into();
            }
        }
    }
}

pub(super) fn handle_assembler_recipe_button(
    interactions: Query<(&Interaction, &AssemblerRecipeButton), Changed<Interaction>>,
    mode: Res<InteractionMode>,
    mut cmd: Commands,
    recipes: Res<Recipes>,
) {
    let InteractionMode::InScreen(ScreenMode::Assembler(assembler_entity)) = *mode else {
        return;
    };
    for (&interaction, recipe_btn) in &interactions {
        if interaction == Interaction::Pressed {
            if let Recipe::AssemblerRecipe(ar) = &recipes.0[recipe_btn.0] {
                cmd.trigger(SetAssemblerRecipe {
                    assembler: assembler_entity,
                    recipe: Some(ar.clone()),
                });
            }
        }
    }
}

pub(super) fn handle_clear_assembler_recipe(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ClearAssemblerRecipeButton>)>,
    mode: Res<InteractionMode>,
    mut cmd: Commands,
) {
    let InteractionMode::InScreen(ScreenMode::Assembler(assembler_entity)) = *mode else {
        return;
    };
    for &interaction in &interactions {
        if interaction == Interaction::Pressed {
            cmd.trigger(SetAssemblerRecipe {
                assembler: assembler_entity,
                recipe: None,
            });
        }
    }
}

pub(super) fn handle_assembler_inventory_slot_clicks(
    interactions: Query<
        (&Interaction, &InventorySlot),
        (Changed<Interaction>, With<AssemblerInventoryPanel>),
    >,
    mode: Res<InteractionMode>,
    mut cmd: Commands,
) {
    let InteractionMode::InScreen(ScreenMode::Assembler(assembler_entity)) = *mode else {
        return;
    };
    for (&interaction, slot_marker) in &interactions {
        if interaction == Interaction::Pressed {
            cmd.trigger(LoadMachineInput {
                player_inventory_slot: slot_marker.0,
                machine: assembler_entity,
            });
        }
    }
}

pub(super) fn handle_assembler_output_slot_clicks(
    interactions: Query<(&Interaction, &AssemblerOutputSlot), Changed<Interaction>>,
    mode: Res<InteractionMode>,
    mut cmd: Commands,
) {
    let InteractionMode::InScreen(ScreenMode::Assembler(assembler_entity)) = *mode else {
        return;
    };
    for (&interaction, slot_marker) in &interactions {
        if interaction == Interaction::Pressed {
            cmd.trigger(UnloadMachineOutput {
                machine: assembler_entity,
                output_slot: slot_marker.0,
            });
        }
    }
}
