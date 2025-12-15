use crate::core::*;
use bevy::{
    input::mouse::{AccumulatedMouseMotion, MouseButtonInput, MouseMotion},
    prelude::*,
    window::{CursorOptions, PrimaryWindow},
};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, mouse_input);
    }
}

fn mouse_input(
    mut cmd: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        let pos = window.cursor_position().unwrap();
        info!("Mouse at: {:?}", pos);
        let coords = WorldCoords {
            x: ((-window.width() / 2.0 + pos.x) / 32.0).round() as i32,
            y: ((window.height() / 2.0 - pos.y) / 32.0).round() as i32,
        };
        let entity = cmd.spawn_empty().id();
        cmd.trigger(PlaceBelt {
            entity,
            dir: Dir::East,
            coords,
        });
    }
}
