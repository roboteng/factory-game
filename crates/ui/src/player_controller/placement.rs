use super::{cast_ray, FirstPersonCamera, PlacementDirection, RayTarget};
use crate::hotbar::{Hotbar, PlacementItem};
use crate::visuals::BlockModels;
use crate::{InteractionMode, WorldMode};
use factory_core::{
    inventory::{Inventory, Stack},
    *,
};

use bevy::{prelude::*, window::CursorGrabMode, window::CursorOptions};

/// Marker for the ghost preview entity shown while in placing mode.
#[derive(Component)]
pub(super) struct GhostPreview;

/// Attached to a ghost entity to request semi-transparent tinting of its children.
/// Removed once tinting is applied; re-added when the color needs to change.
#[derive(Component)]
pub(crate) struct NeedsGhostTint(pub(crate) Color);

/// Tracks ghost placement state. Present only while in placing mode.
/// Ghost entities are tagged `GhostPreview`
#[derive(Resource)]
pub(super) enum PlacementGhost {
    /// A single ghost for non-belt items. `coords` is None when looking at sky.
    Single {
        item: Item,
        valid: bool,
        coords: Option<WorldCoords>,
        facing: HDir,
    },
    /// Belt ghost. `drag_start` is None while hovering, Some while mouse is held.
    Belt {
        item: Item,
        valid: bool,
        line: Vec<WorldCoords>,
        facing: HDir,
        drag_start: Option<WorldCoords>,
    },
}

impl PlacementGhost {
    pub(super) fn item(&self) -> Item {
        match self {
            Self::Single { item, .. } | Self::Belt { item, .. } => *item,
        }
    }

    fn valid(&self) -> bool {
        match self {
            Self::Single { valid, .. } | Self::Belt { valid, .. } => *valid,
        }
    }

    fn belt_drag_start(&self) -> Option<WorldCoords> {
        match self {
            Self::Belt { drag_start, .. } => *drag_start,
            Self::Single { .. } => None,
        }
    }
}

fn ghost_color(valid: bool) -> Color {
    if valid {
        Color::srgba(0.4, 0.75, 1.0, 0.7)
    } else {
        Color::srgba(1.0, 0.35, 0.35, 0.7)
    }
}

/// Returns the sequence of `WorldCoords` for a belt drag line from `start` to `end`,
/// constrained to the axis parallel or anti-parallel to `facing`.
fn belt_line_coords(start: WorldCoords, end: WorldCoords, facing: HDir) -> Vec<WorldCoords> {
    let axis = Vec3::from(facing);
    let delta = (Vec3::from(end) - Vec3::from(start)).dot(axis);
    let steps = delta.round() as i32;
    let (dir, count) = if steps >= 0 {
        (facing, (steps + 1) as usize)
    } else {
        (facing.opposite(), (-steps + 1) as usize)
    };
    (0..count)
        .scan(start, |coord, _| {
            let c = *coord;
            *coord = coord.step(dir);
            Some(c)
        })
        .collect()
}

/// Places belts on left-mouse drag release, then despawns ghost entities and removes the resource.
/// Runs before `ApplyDeferred` so `update_ghost_resource` sees the world after placement.
pub(super) fn commit_belt_placement(
    mouse: Res<ButtonInput<MouseButton>>,
    ghost: Option<Res<PlacementGhost>>,
    player: Res<Player>,
    mut invs: Query<&mut Inventory>,
    ghost_entities: Query<Entity, With<GhostPreview>>,
    mut cmd: Commands,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(ghost) = ghost else {
        return;
    };
    let PlacementGhost::Belt {
        item,
        valid,
        ref line,
        facing,
        drag_start: Some(_),
    } = *ghost
    else {
        return;
    };
    if valid {
        if let Ok(mut inv) = invs.get_mut(player.0) {
            for &coord in line {
                let e = cmd.spawn_empty().id();
                inv.take_items(Stack::from(item));
                cmd.trigger(PlaceBlock {
                    entity: e,
                    block: WorldBlock::Belt,
                    coords: coord,
                    dir: facing,
                });
            }
        }
    }
    for entity in ghost_entities.iter() {
        cmd.entity(entity).despawn();
    }
    cmd.remove_resource::<PlacementGhost>();
}

/// Determines what the ghost should look like and writes it to `PlacementGhost`.
/// When leaving placing mode or changing items, despawns ghost entities and removes the resource.
pub(super) fn update_ghost_resource(
    mode: Res<InteractionMode>,
    cursor_options: Single<&CursorOptions>,
    camera_q: Single<(&Transform, &GlobalTransform), With<FirstPersonCamera>>,
    player: Res<Player>,
    hotbar: Res<Hotbar>,
    invs: Query<&Inventory>,
    targets: Query<(&WorldCoords, &Transform, &RaycastTarget), Without<GhostPreview>>,
    placement_dir: Res<PlacementDirection>,
    mouse: Res<ButtonInput<MouseButton>>,
    ghost: Option<Res<PlacementGhost>>,
    ghost_entities: Query<Entity, With<GhostPreview>>,
    mut cmd: Commands,
) {
    let placing = matches!(*mode, InteractionMode::InWorld(WorldMode::Placing(_)))
        && cursor_options.grab_mode == CursorGrabMode::Locked;

    // Determine what item/block we should be placing.
    let item_and_block: Option<(Item, WorldBlock)> = if placing {
        let InteractionMode::InWorld(WorldMode::Placing(tool)) = *mode else {
            return;
        };
        let item = match tool {
            PlacementItem::HotbarSlot(slot) => hotbar.0.get(slot as usize).and_then(|s| *s),
            PlacementItem::Custom(item) => Some(item),
        };
        item.and_then(|i| i.can_place().map(|b| (i, b)))
    } else {
        None
    };

    // If not placing or no valid item, despawn all ghosts and remove resource.
    let Some((item, block)) = item_and_block else {
        for entity in ghost_entities.iter() {
            cmd.entity(entity).despawn();
        }
        if ghost.is_some() {
            cmd.remove_resource::<PlacementGhost>();
        }
        return;
    };

    // If the item changed, clear and let the next frame create a fresh ghost.
    if ghost.as_ref().map(|g| g.item() != item).unwrap_or(false) {
        for entity in ghost_entities.iter() {
            cmd.entity(entity).despawn();
        }
        cmd.remove_resource::<PlacementGhost>();
        return;
    }

    let (cam_local, cam_global) = camera_q.into_inner();
    let current_coords = cast_ray(
        cam_global.translation(),
        *cam_local.forward(),
        targets.iter().map(|(c, t, rt)| RayTarget {
            coords: *c,
            center: t.translation,
            half_extents: rt.half_extents,
        }),
    )
    .map(|h| h.place_coords);

    let facing = placement_dir.0;

    // --- Belt path ---
    if block == WorldBlock::Belt {
        let prev_drag_start = ghost.as_ref().and_then(|g| g.belt_drag_start());

        let drag_start = if mouse.just_pressed(MouseButton::Left)
            && cursor_options.grab_mode == CursorGrabMode::Locked
        {
            current_coords.or(prev_drag_start)
        } else {
            prev_drag_start
        };

        let line: Vec<WorldCoords> = match (drag_start, current_coords) {
            (Some(start), Some(end)) => belt_line_coords(start, end, facing),
            (Some(start), None) => vec![start],
            (None, Some(end)) => vec![end],
            (None, None) => vec![],
        };

        let inv_count = invs
            .get(player.0)
            .map(|inv| inv.item_count(item))
            .unwrap_or(0);
        let valid = inv_count >= line.len() as u16;

        cmd.insert_resource(PlacementGhost::Belt {
            item,
            valid,
            line,
            facing,
            drag_start,
        });
        return;
    }

    // --- Non-belt single-ghost path ---
    let is_full = block.raycast_target().half_extents.y > 0.25;
    let valid = invs
        .get(player.0)
        .map(|inv| inv.item_count(item) > 0)
        .unwrap_or(false);
    let coords = current_coords.map(|c| if is_full { c.snap_height_even() } else { c });

    cmd.insert_resource(PlacementGhost::Single {
        item,
        valid,
        coords,
        facing,
    });
}

/// Reconciles `GhostPreview` entities to match the current `PlacementGhost` state.
/// Only runs when the resource exists (enforced by `run_if` in plugin registration).
pub(super) fn sync_ghost_entities(
    ghost: Res<PlacementGhost>,
    block_models: Res<BlockModels>,
    mut cmd: Commands,
    mut ghost_q: Query<
        (Entity, &mut Transform, &mut Visibility),
        (With<GhostPreview>, Without<FirstPersonCamera>),
    >,
    mut state: Local<(Option<Item>, Option<bool>)>,
) {
    let current_item = ghost.item();
    let current_valid = ghost.valid();
    let (prev_item, prev_valid) = &mut *state;

    let needs_retint = *prev_item != Some(current_item) || *prev_valid != Some(current_valid);
    *prev_item = Some(current_item);
    *prev_valid = Some(current_valid);

    let color = ghost_color(current_valid);

    match &*ghost {
        PlacementGhost::Single {
            item,
            coords,
            facing,
            ..
        } => {
            if let Some((entity, mut transform, mut vis)) = ghost_q.iter_mut().next() {
                match coords {
                    Some(c) => {
                        transform.translation = Vec3::from(*c);
                        transform.rotation = Quat::from_rotation_y(facing.angle());
                        *vis = Visibility::Visible;
                    }
                    None => *vis = Visibility::Hidden,
                }
                if needs_retint {
                    cmd.entity(entity).insert(NeedsGhostTint(color));
                }
            } else if let Some(c) = coords {
                let mut ec = cmd.spawn((
                    GhostPreview,
                    NeedsGhostTint(color),
                    Transform::from_translation(Vec3::from(*c))
                        .with_rotation(Quat::from_rotation_y(facing.angle())),
                    Visibility::Visible,
                ));
                if let Some(scene) = block_models.ghost_scene(*item) {
                    ec.insert(SceneRoot(scene));
                }
            }
        }
        PlacementGhost::Belt {
            item, line, facing, ..
        } => {
            let mut entities: Vec<Entity> = ghost_q.iter().map(|(e, ..)| e).collect();

            // Shrink pool.
            for &entity in entities.iter().skip(line.len()) {
                cmd.entity(entity).despawn();
            }
            entities.truncate(line.len());

            // Grow pool.
            let belt_scene = block_models.ghost_scene(*item);
            while entities.len() < line.len() {
                let mut ec = cmd.spawn((
                    GhostPreview,
                    NeedsGhostTint(color),
                    Transform::default(),
                    Visibility::Hidden,
                ));
                if let Some(ref scene) = belt_scene {
                    ec.insert(SceneRoot(scene.clone()));
                }
                entities.push(ec.id());
            }

            // Reposition existing entities (newly spawned ones aren't in ghost_q yet).
            for (&entity, coord) in entities.iter().zip(line.iter()) {
                if let Ok((_, mut transform, mut vis)) = ghost_q.get_mut(entity) {
                    transform.translation = Vec3::from(*coord);
                    transform.rotation = Quat::from_rotation_y(facing.angle());
                    *vis = Visibility::Visible;
                }
            }

            if needs_retint {
                for &entity in &entities {
                    cmd.entity(entity).insert(NeedsGhostTint(color));
                }
            }
        }
    }
}
