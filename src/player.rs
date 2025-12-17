use crate::core::*;
use bevy::{prelude::*, window::PrimaryWindow};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlaceDirection>();
        app.init_resource::<DeleteMode>();
        app.add_systems(PreUpdate, (mouse_input, place_item, delete_belt));
        app.add_systems(
            Update,
            (
                change_place_direction,
                toggle_delete_mode,
                update_ghost_preview,
            )
                .chain(),
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

#[derive(Resource, Default)]
struct DeleteMode(bool);

fn mouse_input(
    mut cmd: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    dir: Res<PlaceDirection>,
    delete_mode: Res<DeleteMode>,
) {
    if delete_mode.0 {
        return; // Don't place belts in delete mode
    }
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
    delete_mode: Res<DeleteMode>,
) {
    if delete_mode.0 {
        return; // Don't place items in delete mode
    }
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

fn delete_belt(
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    delete_mode: Res<DeleteMode>,
    belt_coords: Res<BeltCoords>,
    mut cmd: Commands,
) {
    if !delete_mode.0 {
        return; // Only handle clicks in delete mode
    }

    if buttons.just_pressed(MouseButton::Left) {
        let Some(coords) = WorldCoords::from_cursor(&window) else {
            return;
        };

        // Check if there's a belt at the clicked position
        if let Some((entity, _)) = belt_coords.get(coords) {
            debug!("Deleting belt at {:?}", coords);
            cmd.trigger(RemoveBelt { entity });
        }
        // Clicking empty space does nothing (per requirements)
    }
}

fn change_place_direction(
    mut direction: ResMut<PlaceDirection>,
    input: Res<ButtonInput<KeyCode>>,
    delete_mode: Res<DeleteMode>,
) {
    if delete_mode.0 {
        return; // Tab disabled in delete mode
    }
    if input.just_pressed(KeyCode::KeyR) {
        direction.0 = direction.next().0;
    }
}

fn toggle_delete_mode(mut delete_mode: ResMut<DeleteMode>, input: Res<ButtonInput<KeyCode>>) {
    if input.just_pressed(KeyCode::KeyC) {
        delete_mode.0 = !delete_mode.0;
        debug!("Delete mode: {}", delete_mode.0);
    }
}

fn update_ghost_preview(
    mut cmd: Commands,
    window: Single<&Window, With<PrimaryWindow>>,
    dir: Res<PlaceDirection>,
    delete_mode: Res<DeleteMode>,
    ghost: Query<Entity, With<GhostBeltPreview>>,
    belt_coords: Res<BeltCoords>,
    assets: Res<AssetServer>,
) {
    // Get cursor position
    let Some(coords) = WorldCoords::from_cursor(&window) else {
        // No cursor, despawn ghost
        if let Ok(ghost_entity) = ghost.single() {
            cmd.entity(ghost_entity).despawn();
        }
        return;
    };

    // Handle delete mode
    if delete_mode.0 {
        // Check if there's a belt at cursor position
        if let Some((_, belt)) = belt_coords.get(coords) {
            // Show red ghost over the belt
            let ghost_entity = if let Ok(entity) = ghost.single() {
                entity
            } else {
                cmd.spawn(GhostBeltPreview).id()
            };

            let (sprite_path, flip_y, rotation) = match belt {
                Belt::Straight(dir) => {
                    let k = Vec2::from(dir);
                    let angle = Vec2::X.angle_to(k);
                    (
                        "sprites/belt.png",
                        false,
                        Quat::from_axis_angle(Vec3::Z, angle),
                    )
                }
                _ => {
                    let k = Vec2::from(belt.output());
                    let angle = Vec2::X.angle_to(k);
                    let flip_y = belt.input().left() == belt.output();
                    (
                        "sprites/belt_curved.png",
                        flip_y,
                        Quat::from_axis_angle(Vec3::Z, angle),
                    )
                }
            };

            let mut sprite = Sprite::from(assets.load(sprite_path));
            sprite.color = Color::srgba(1.0, 0.0, 0.0, 0.5); // Red with 50% opacity
            sprite.flip_y = flip_y;

            cmd.entity(ghost_entity).insert((
                coords,
                sprite,
                Transform::from_xyz(
                    coords.x as f32 * TILE_SIZE,
                    coords.y as f32 * TILE_SIZE,
                    1.5,
                )
                .with_rotation(rotation),
            ));
        } else {
            // No belt at cursor, hide ghost
            if let Ok(ghost_entity) = ghost.single() {
                cmd.entity(ghost_entity).despawn();
            }
        }
        return;
    }

    // Regular ghost preview logic (when not in delete mode)
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
