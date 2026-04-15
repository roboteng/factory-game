use bevy::prelude::*;

use super::common::PANE_BG;
use super::{InteractionMode, ScreenMode};

#[derive(Component)]
pub(super) struct MenuPane;

#[derive(Component)]
pub(super) struct ResumeButton;

pub(super) fn setup_menu_pane(mut cmd: Commands) {
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
        BackgroundColor(PANE_BG),
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

pub(super) fn update_menu_pane(
    mode: Res<InteractionMode>,
    mut pane: Single<&mut Visibility, With<MenuPane>>,
) {
    **pane = if matches!(*mode, InteractionMode::InScreen(ScreenMode::Menu)) {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}
