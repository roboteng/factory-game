use crate::core::*;
use bevy::{prelude::*, window::PrimaryWindow};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlaceDirection>();
        app.add_systems(PreUpdate, (mouse_input, place_item));
        app.add_systems(
            Update,
            (change_place_direction, update_ghost_preview).chain(),
        );
    }
}

#[derive(Component)]
pub struct GhostBeltPreview;

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
        let Some(coords) = WorldCoords::from_cursor(&window) else {
            return;
        };
        let Some(dir) = dir.0 else { return };
        let entity = cmd.spawn_empty().id();
        cmd.trigger(PlaceBelt {
            entity,
            dir,
            coords,
        });
    }
}

fn place_item(
    mut cmd: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    dir: Res<PlaceDirection>,
    belts: Res<BeltCoords>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        let Some(coords) = WorldCoords::from_cursor(&window) else {
            debug!("Not placing item because no coords");
            return;
        };
        if dir.0.is_some() {
            debug!("Not placing item because dir is some");
            return;
        }
        let Some((belt_entity, _)) = belts.get(coords) else {
            debug!("Not placing item because couldn't find belt");
            return;
        };
        debug!("placing item at {coords:?}");
        let entity = cmd.spawn_empty().id();
        cmd.trigger(PlaceItem {
            entity,
            belt: belt_entity,
            pos: POSITIONS_PER_TILE / 2,
            item: Item,
        });
    }
}

fn change_place_direction(mut direction: ResMut<PlaceDirection>, input: Res<ButtonInput<KeyCode>>) {
    if input.just_pressed(KeyCode::KeyR) {
        direction.0 = direction.next().0;
    }
}

fn update_ghost_preview(
    mut cmd: Commands,
    window: Single<&Window, With<PrimaryWindow>>,
    dir: Res<PlaceDirection>,
    ghost: Query<Entity, With<GhostBeltPreview>>,
    assets: Res<AssetServer>,
) {
    // Get cursor position
    let Some(coords) = WorldCoords::from_cursor(&window) else {
        // No cursor, despawn ghost if it exists
        if let Ok(ghost_entity) = ghost.single() {
            cmd.entity(ghost_entity).despawn();
        }
        return;
    };

    match dir.0 {
        None => {
            // No direction selected, despawn ghost
            if let Ok(ghost_entity) = ghost.single() {
                cmd.entity(ghost_entity).despawn();
            }
        }
        Some(direction) => {
            // Direction selected, spawn or update ghost
            let ghost_entity = if let Ok(entity) = ghost.single() {
                entity
            } else {
                cmd.spawn(GhostBeltPreview).id()
            };

            // Create sprite immediately
            let k = Vec2::from(direction);
            let angle = Vec2::X.angle_to(k);
            let mut sprite = Sprite::from(assets.load("sprites/belt.png"));
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.25);

            // Update the ghost with all components at once
            cmd.entity(ghost_entity).insert((
                coords,
                sprite,
                Transform::from_xyz(
                    coords.x as f32 * TILE_SIZE,
                    coords.y as f32 * TILE_SIZE,
                    0.5,
                )
                .with_rotation(Quat::from_axis_angle(Vec3::Z, angle)),
            ));
        }
    }
}
