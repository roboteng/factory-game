use crate::core::{inventory::Inventory, *};

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
