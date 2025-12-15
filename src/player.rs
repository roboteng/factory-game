use crate::core::*;
use bevy::{prelude::*, window::PrimaryWindow};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlaceDirection>();
        app.add_systems(PreUpdate, mouse_input);
        app.add_systems(Update, change_place_direction);
    }
}

#[derive(Resource, Clone, Copy, Default)]
struct PlaceDirection(Option<Dir>);

impl PlaceDirection {
    fn next(self) -> Self {
        match self.0 {
            None => PlaceDirection(Some(Dir::North)),
            Some(Dir::North) => PlaceDirection(Some(Dir::East)),
            Some(Dir::East) => PlaceDirection(Some(Dir::South)),
            Some(Dir::South) => PlaceDirection(Some(Dir::West)),
            Some(Dir::West) => PlaceDirection(None),
        }
    }
}

fn mouse_input(
    mut cmd: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    dir: Res<PlaceDirection>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        let Some(pos) = window.cursor_position() else {
            return;
        };
        let Some(dir) = dir.0 else { return };
        info!("Mouse at: {:?}", pos);
        let coords = WorldCoords {
            x: ((-window.width() / 2.0 + pos.x) / 32.0).round() as i32,
            y: ((window.height() / 2.0 - pos.y) / 32.0).round() as i32,
        };
        let entity = cmd.spawn_empty().id();
        cmd.trigger(PlaceBelt {
            entity,
            dir,
            coords,
        });
    }
}

fn change_place_direction(mut direction: ResMut<PlaceDirection>, input: Res<ButtonInput<KeyCode>>) {
    if input.just_pressed(KeyCode::KeyR) {
        direction.0 = direction.next().0;
    }
}
