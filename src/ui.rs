use std::num::NonZeroU16;

use crate::core::{inventory::Inventory, inventory::Stack, *};

use bevy::prelude::*;

mod hotbar;
mod player_controller;
mod visuals;

use hotbar::PlacementItem;

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

#[derive(Component)]
struct InventoryPane;

#[derive(Component)]
struct CloseInventoryButton;

fn setup_inventory_pane(mut cmd: Commands) {
    cmd.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(5.0),
            right: Val::Percent(5.0),
            top: Val::Percent(5.0),
            bottom: Val::Percent(5.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(Color::srgba(0.08, 0.08, 0.12, 0.95)),
        Visibility::Hidden,
        InventoryPane,
    ))
    .with_children(|parent| {
        // Title bar row
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                ..default()
            })
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Inventory"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));

                // Close button
                parent
                    .spawn((
                        Button,
                        Node {
                            width: Val::Px(32.0),
                            height: Val::Px(32.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.3, 0.1, 0.1, 0.9)),
                        CloseInventoryButton,
                    ))
                    .with_children(|parent| {
                        parent.spawn((
                            Text::new("X"),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            });
    });
}

fn update_inventory_pane(
    mode: Res<InteractionMode>,
    mut pane: Single<&mut Visibility, With<InventoryPane>>,
) {
    **pane = if matches!(*mode, InteractionMode::InScreen(ScreenMode::Inventory)) {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

fn handle_inventory_close_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<CloseInventoryButton>)>,
    mut mode: ResMut<InteractionMode>,
) {
    for &interaction in interaction.iter() {
        if interaction == Interaction::Pressed {
            *mode = InteractionMode::InWorld(WorldMode::None);
        }
    }
}

#[derive(Component)]
struct MenuPane;

#[derive(Component)]
struct ResumeButton;

fn setup_menu_pane(mut cmd: Commands) {
    cmd.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(30.0),
            right: Val::Percent(30.0),
            top: Val::Percent(20.0),
            bottom: Val::Percent(20.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(16.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.08, 0.08, 0.12, 0.95)),
        Visibility::Hidden,
        MenuPane,
    ))
    .with_children(|parent| {
        parent.spawn((
            Text::new("Menu"),
            TextFont {
                font_size: 24.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ));

        parent
            .spawn((
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.2, 0.4, 0.2, 0.9)),
                ResumeButton,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Resume"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
    });
}

fn update_menu_pane(mode: Res<InteractionMode>, mut pane: Single<&mut Visibility, With<MenuPane>>) {
    **pane = if matches!(*mode, InteractionMode::InScreen(ScreenMode::Menu)) {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

fn handle_menu_resume_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<ResumeButton>)>,
    mut mode: ResMut<InteractionMode>,
) {
    for &interaction in interaction.iter() {
        if interaction == Interaction::Pressed {
            *mode = InteractionMode::InWorld(WorldMode::None);
        }
    }
}

// ── Furnace pane ─────────────────────────────────────────────────────────────

#[derive(Component)]
struct FurnacePane;

#[derive(Component)]
struct CloseFurnaceButton;

#[derive(Component)]
struct FurnaceInputSlot(usize);

#[derive(Component)]
struct FurnaceOutputSlot(usize);

#[derive(Component)]
struct FurnaceProgressFill;

#[derive(Component)]
struct FurnaceInventorySlot(u16);

const FURNACE_INVENTORY_SLOTS: u16 = 64;

fn spawn_slot(parent: &mut ChildSpawnerCommands, marker: impl Bundle) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(64.0),
                height: Val::Px(64.0),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                margin: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.8)),
            BorderColor::all(Color::srgba(0.3, 0.3, 0.3, 0.8)),
            marker,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn section_label(parent: &mut ChildSpawnerCommands, label: &str) {
    parent.spawn((
        Text::new(label),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgba(0.7, 0.7, 0.7, 1.0)),
        Node {
            margin: UiRect::axes(Val::Px(4.0), Val::Px(8.0)),
            ..default()
        },
    ));
}

fn setup_furnace_pane(mut cmd: Commands) {
    cmd.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(5.0),
            right: Val::Percent(5.0),
            top: Val::Percent(5.0),
            bottom: Val::Percent(5.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(Color::srgba(0.08, 0.08, 0.12, 0.95)),
        Visibility::Hidden,
        FurnacePane,
    ))
    .with_children(|parent| {
        // Title bar
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                ..default()
            })
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Furnace"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));

                parent
                    .spawn((
                        Button,
                        Node {
                            width: Val::Px(32.0),
                            height: Val::Px(32.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.3, 0.1, 0.1, 0.9)),
                        CloseFurnaceButton,
                    ))
                    .with_children(|parent| {
                        parent.spawn((
                            Text::new("X"),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            });

        // Content row
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|parent| {
                // Left panel — furnace internals
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
                                ..default()
                            })
                            .with_children(|parent| {
                                spawn_slot(parent, FurnaceInputSlot(0));
                            });

                        section_label(parent, "Progress");
                        // Progress bar track
                        parent
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(24.0),
                                margin: UiRect::axes(Val::Px(4.0), Val::Px(4.0)),
                                ..default()
                            })
                            .insert(BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.8)))
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
                    });

                // Right panel — player inventory
                parent
                    .spawn(Node {
                        width: Val::Percent(60.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(16.0)),
                        ..default()
                    })
                    .with_children(|parent| {
                        section_label(parent, "Inventory");
                        parent
                            .spawn(Node {
                                flex_direction: FlexDirection::Row,
                                flex_wrap: FlexWrap::Wrap,
                                ..default()
                            })
                            .with_children(|parent| {
                                for i in 0..FURNACE_INVENTORY_SLOTS {
                                    spawn_slot(parent, FurnaceInventorySlot(i));
                                }
                            });
                    });
            });
    });
}

fn update_furnace_pane(
    mode: Res<InteractionMode>,
    mut pane: Single<&mut Visibility, With<FurnacePane>>,
    furnace_q: Query<(&Furnace, &InputBuffer, &OutputBuffer)>,
    player: Res<Player>,
    inventory_q: Query<&Inventory>,
    input_slots: Query<(&FurnaceInputSlot, &Children)>,
    output_slots: Query<(&FurnaceOutputSlot, &Children)>,
    inv_slots: Query<(&FurnaceInventorySlot, &Children)>,
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

    // Compute progress fraction by matching input slots against known recipes
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

    // Update input slot labels
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

    // Update output slot labels
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

    // Update inventory slot labels
    let Ok(inventory) = inventory_q.get(player.0) else {
        return;
    };
    for (slot_marker, children) in &inv_slots {
        let label = match inventory.get(slot_marker.0) {
            Some(stack) => format!("{}\n\u{00d7}{}", stack.item.name(), stack.count),
            _ => String::new(),
        };
        if let Some(&child) = children.first() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = label.into();
            }
        }
    }
}

fn handle_furnace_close_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<CloseFurnaceButton>)>,
    mut mode: ResMut<InteractionMode>,
) {
    for &interaction in interaction.iter() {
        if interaction == Interaction::Pressed {
            *mode = InteractionMode::InWorld(WorldMode::None);
        }
    }
}

fn handle_furnace_inventory_slot_clicks(
    interactions: Query<(&Interaction, &FurnaceInventorySlot), Changed<Interaction>>,
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
            // Return remainder if stack had more than 1
            let remaining = stack.count - 1;
            inv.insert(Stack::new(stack.item, remaining)).unwrap();
        } else {
            // Furnace doesn't accept it — put the stack back
            let _ = inv.insert(stack);
        }
    }
}

fn handle_furnace_output_slot_clicks(
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
