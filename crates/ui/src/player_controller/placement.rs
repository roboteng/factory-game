use super::{cast_ray, FirstPersonCamera, PlacementDirection, RayTarget};
use crate::hotbar::{Hotbar, PlacementItem};
use crate::visuals::BlockModels;
use crate::{InteractionMode, WorldMode};
use factory_core::{
    inventory::{Inventory, Stack},
    *,
};

use bevy::{prelude::*, window::CursorGrabMode, window::CursorOptions};

/// Marker for all ghost preview entities (belt and single).
#[derive(Component)]
pub(super) struct GhostPreview;

/// Marker for belt-drag ghost entities.
#[derive(Component)]
pub(super) struct BeltGhost;

/// Marker for the single-item ghost entity.
#[derive(Component)]
pub(super) struct SingleGhost;

/// Attached to a ghost entity to request semi-transparent tinting of its children.
/// Removed once tinting is applied; re-added when the color needs to change.
#[derive(Component)]
pub(crate) struct NeedsGhostTint(pub(crate) Color);

/// Computed each frame when the player is in placement mode.
/// Removed when not placing.
#[derive(Resource)]
pub(super) struct PlacementTarget {
    pub(super) item: Item,
    pub(super) block: Structure,
    pub(super) facing: HDir,
    pub(super) raycast_coords: Option<WorldCoords>,
    pub(super) inv_count: u16,
    cam_origin: Vec3,
    cam_forward: Vec3,
}

/// Belt placement state. Present only while placing belts.
#[derive(Resource)]
pub(super) struct BeltPlacement {
    pub(super) item: Item,
    facing: HDir,
    drag_start: Option<WorldCoords>,
    pub(super) line: Vec<WorldCoords>,
    pub(super) valid: bool,
}

fn ghost_color(valid: bool) -> Color {
    if valid {
        Color::srgba(0.4, 0.75, 1.0, 0.7)
    } else {
        Color::srgba(1.0, 0.35, 0.35, 0.7)
    }
}

/// Intersect a ray with a horizontal plane at `plane_y` (world units).
/// Returns the grid-snapped `WorldCoords` at the hit point, preserving `template_y`
/// as the half-block Y coordinate, or `None` if the ray is nearly parallel to the plane
/// or points away from it.
fn cast_ray_to_plane(origin: Vec3, dir: Vec3, template_y: i32) -> Option<WorldCoords> {
    let plane_y = template_y as f32 * 0.5;
    if dir.y.abs() < 1e-9 {
        return None;
    }
    let t = (plane_y - origin.y) / dir.y;
    if t < 0.0 {
        return None;
    }
    let hit_x = origin.x + t * dir.x;
    let hit_z = origin.z + t * dir.z;
    Some(WorldCoords::from((
        hit_x.round() as i32,
        template_y,
        hit_z.round() as i32,
    )))
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

fn resolve_item(mode: &InteractionMode, hotbar: &Hotbar) -> Option<(Item, Structure)> {
    let InteractionMode::InWorld(WorldMode::Placing(tool)) = mode else {
        return None;
    };
    let item = match tool {
        PlacementItem::HotbarSlot(slot) => hotbar.0.get(*slot as usize).and_then(|s| *s),
        PlacementItem::Custom(item) => Some(*item),
    }?;
    let block = item.can_place()?;
    Some((item, block))
}

/// Resolves the current placement item, raycast target, and inventory count
/// into a single resource for downstream systems.
pub(super) fn compute_placement_target(
    mode: Res<InteractionMode>,
    camera_q: Single<(&Transform, &GlobalTransform), With<FirstPersonCamera>>,
    player: Res<Player>,
    hotbar: Res<Hotbar>,
    invs: Query<&Inventory>,
    targets: Query<(&WorldCoords, &Transform, &RaycastTarget), Without<GhostPreview>>,
    placement_dir: Res<PlacementDirection>,
    existing: Option<Res<PlacementTarget>>,
    mut cmd: Commands,
) {
    let Some((item, block)) = resolve_item(&mode, &hotbar) else {
        if existing.is_some() {
            cmd.remove_resource::<PlacementTarget>();
        }
        return;
    };

    let (cam_local, cam_global) = camera_q.into_inner();
    let cam_origin = cam_global.translation();
    let cam_forward = *cam_local.forward();

    let raycast_coords = cast_ray(
        cam_origin,
        cam_forward,
        targets.iter().map(|(c, t, rt)| RayTarget {
            coords: *c,
            center: t.translation,
            half_extents: rt.half_extents,
        }),
    )
    .map(|h| h.place_coords);

    let inv_count = invs
        .get(player.0)
        .map(|inv| inv.item_count(item))
        .unwrap_or(0);

    cmd.insert_resource(PlacementTarget {
        item,
        block,
        facing: placement_dir.0,
        raycast_coords,
        inv_count,
        cam_origin,
        cam_forward,
    });
}

pub fn handle_click_to_place(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor_options: Single<&CursorOptions>,
    target: Option<Res<PlacementTarget>>,
    player: Res<Player>,
    mut invs: Query<&mut Inventory>,
    mut cmd: Commands,
    coord_map: Res<CoordsMap>,
) {
    let Some(target) = target else { return };
    if target.block == Structure::Belt {
        return;
    }

    if !mouse.just_pressed(MouseButton::Left) || cursor_options.grab_mode != CursorGrabMode::Locked
    {
        return;
    }

    let Some(coords) = target.raycast_coords else {
        return;
    };

    let size = target.block.size();
    let flb = if size.is_full_block() {
        coords.snap_height_even()
    } else {
        coords
    };
    if size.iter_coords(flb).any(|c| coord_map.0.contains_key(&c)) {
        return;
    }

    let Ok(mut inv) = invs.get_mut(player.0) else {
        return;
    };
    if inv.item_count(target.item) == 0 {
        return;
    }
    inv.take_items(Stack::from(target.item));
    drop(inv);

    let entity = cmd.spawn_empty().id();
    let flb = coords;
    let event = PlaceStructure {
        entity,
        structure: target.block,
        brt: target.block.brt_for(flb, Some(target.facing)),
        flb,
    };
    debug!("Triggering: {event:?}");
    cmd.trigger(event);
}

/// Computes belt placement state each frame and writes it to `BeltPlacement`.
/// Removes the resource when not in belt-placing mode — `sync_belt_ghosts` will
/// then despawn ghost entities on the same frame.
pub(super) fn update_belt_placement(
    target: Option<Res<PlacementTarget>>,
    cursor_options: Single<&CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
    belt_placement: Option<Res<BeltPlacement>>,
    mut cmd: Commands,
    coord_map: Res<CoordsMap>,
    belts_q: Query<(), With<Belt>>,
) {
    let Some(target) = target else {
        if belt_placement.is_some() {
            cmd.remove_resource::<BeltPlacement>();
        }
        return;
    };

    if target.block != Structure::Belt {
        if belt_placement.is_some() {
            cmd.remove_resource::<BeltPlacement>();
        }
        return;
    }

    if belt_placement
        .as_ref()
        .map(|g| g.item != target.item)
        .unwrap_or(false)
    {
        cmd.remove_resource::<BeltPlacement>();
        return;
    }

    let cursor_locked = cursor_options.grab_mode == CursorGrabMode::Locked;
    let prev_drag_start = belt_placement.as_ref().and_then(|g| g.drag_start);

    // While dragging, intersect the ray with the horizontal plane at the drag-start
    // belt's Y level so the ghost line stays at a consistent height regardless of
    // what terrain the cursor sweeps over.
    let current_coords = if let Some(start) = prev_drag_start {
        cast_ray_to_plane(target.cam_origin, target.cam_forward, start.y)
    } else {
        target.raycast_coords
    };

    let drag_start = if mouse.just_pressed(MouseButton::Left) && cursor_locked {
        current_coords.or(prev_drag_start)
    } else {
        prev_drag_start
    };

    let line: Vec<WorldCoords> = match (drag_start, current_coords) {
        (Some(start), Some(end)) => belt_line_coords(start, end, target.facing),
        (Some(start), None) => vec![start],
        (None, Some(end)) => vec![end],
        (None, None) => vec![],
    };

    let no_hard_conflicts = line
        .iter()
        .all(|c| coord_map.0.get(c).map_or(true, |&e| belts_q.contains(e)));
    let valid = target.inv_count >= line.len() as u16 && no_hard_conflicts;

    cmd.insert_resource(BeltPlacement {
        item: target.item,
        facing: target.facing,
        drag_start,
        line,
        valid,
    });
}

/// Reconciles `BeltGhost` entities to match `BeltPlacement` each frame.
/// Pool grows/shrinks as needed — new entities start `Hidden` so their position
/// can be set next frame without a flash at the origin.
/// When `BeltPlacement` is absent, despawns all belt ghosts.
pub(super) fn sync_belt_ghosts(
    belt_placement: Option<Res<BeltPlacement>>,
    mut belt_ghost_q: Query<
        (Entity, &mut Transform, &mut Visibility),
        (With<BeltGhost>, Without<FirstPersonCamera>),
    >,
    block_models: Res<BlockModels>,
    mut cmd: Commands,
    mut state: Local<(Option<Item>, Option<bool>)>,
) {
    let Some(placement) = belt_placement else {
        for (e, _, _) in belt_ghost_q.iter() {
            cmd.entity(e).despawn();
        }
        *state = (None, None);
        return;
    };

    let item = placement.item;
    let facing = placement.facing;
    let line = &placement.line;
    let valid = placement.valid;
    let color = ghost_color(valid);

    let (prev_item, prev_valid) = &mut *state;
    let needs_retint = *prev_item != Some(item) || *prev_valid != Some(valid);

    // Collect existing pool entities in stable order.
    let mut entities: Vec<Entity> = belt_ghost_q.iter().map(|(e, ..)| e).collect();

    // Shrink: despawn extras from the tail.
    for &e in entities.iter().skip(line.len()) {
        cmd.entity(e).despawn();
    }
    entities.truncate(line.len());

    // Grow: spawn new entities as Hidden. They'll be positioned + shown next frame
    // once they appear in the query — this avoids a flash at the default origin.
    let scene = block_models.ghost_scene(item);
    while entities.len() < line.len() {
        let mut ec = cmd.spawn((
            GhostPreview,
            BeltGhost,
            NeedsGhostTint(color),
            Transform::default(),
            Visibility::Hidden,
        ));
        if let Some(ref s) = scene {
            ec.insert(SceneRoot(s.clone()));
        }
        entities.push(ec.id());
    }

    // Reposition all existing entities. Newly spawned ones aren't in the query yet
    // and will be positioned on the next frame.
    for (&entity, &coord) in entities.iter().zip(line.iter()) {
        if let Ok((_, mut transform, mut vis)) = belt_ghost_q.get_mut(entity) {
            transform.translation = Vec3::from(coord);
            transform.rotation = Quat::from_rotation_y(facing.angle());
            *vis = Visibility::Visible;
        }
    }

    // Retint when validity or item changes.
    if needs_retint {
        for &e in &entities {
            cmd.entity(e).insert(NeedsGhostTint(color));
        }
    }

    *prev_item = Some(item);
    *prev_valid = Some(valid);
}

/// Updates the single-item ghost entity each frame for non-belt placements.
pub(super) fn update_single_ghost(
    target: Option<Res<PlacementTarget>>,
    mut single_ghost: Query<
        (Entity, &mut Transform, &mut Visibility),
        (With<SingleGhost>, Without<FirstPersonCamera>),
    >,
    block_models: Res<BlockModels>,
    mut cmd: Commands,
    mut state: Local<(Option<Item>, Option<bool>)>,
    coord_map: Res<CoordsMap>,
) {
    let Some(target) = target else {
        for (e, _, _) in single_ghost.iter() {
            cmd.entity(e).despawn();
        }
        *state = (None, None);
        return;
    };

    if target.block == Structure::Belt {
        for (e, _, _) in single_ghost.iter() {
            cmd.entity(e).despawn();
        }
        *state = (None, None);
        return;
    }

    let is_full = target.block.size().is_full_block();
    let coords = target
        .raycast_coords
        .map(|c| if is_full { c.snap_height_even() } else { c });
    let occupied = coords.is_some_and(|flb| {
        target
            .block
            .size()
            .iter_coords(flb)
            .any(|c| coord_map.0.contains_key(&c))
    });
    let valid = target.inv_count > 0 && !occupied;

    let (prev_item, prev_valid) = &mut *state;
    let item_changed = *prev_item != Some(target.item);
    let needs_retint = item_changed || *prev_valid != Some(valid);
    *prev_item = Some(target.item);
    *prev_valid = Some(valid);

    let color = ghost_color(valid);

    if item_changed {
        for (e, _, _) in single_ghost.iter() {
            cmd.entity(e).despawn();
        }
    }

    let has_existing_ghost = !single_ghost.is_empty() && !item_changed;

    if has_existing_ghost {
        let Ok((entity, mut transform, mut vis)) = single_ghost.single_mut() else {
            return;
        };
        match coords {
            Some(c) => {
                transform.translation = Vec3::from(c) + target.block.size().center_offset();
                transform.rotation = Quat::from_rotation_y(target.facing.angle());
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
            SingleGhost,
            NeedsGhostTint(color),
            Transform::from_translation(Vec3::from(c) + target.block.size().center_offset())
                .with_rotation(Quat::from_rotation_y(target.facing.angle())),
            Visibility::Visible,
        ));
        if let Some(scene) = block_models.ghost_scene(target.item) {
            ec.insert(SceneRoot(scene));
        }
    }
}

/// Places belts on left-mouse release, then clears `drag_start`.
/// Ghost entities are left in place; `sync_belt_ghosts` will shrink them to a hover ghost.
/// Runs before `update_belt_placement` so it sees the cleared `drag_start` on the same frame.
pub(super) fn commit_belt_placement(
    mouse: Res<ButtonInput<MouseButton>>,
    belt_placement: Option<ResMut<BeltPlacement>>,
    player: Res<Player>,
    mut invs: Query<&mut Inventory>,
    mut cmd: Commands,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(mut placement) = belt_placement else {
        return;
    };
    if placement.drag_start.is_none() {
        return;
    }
    if placement.valid {
        let item = placement.item;
        let facing = placement.facing;
        let line = placement.line.clone();
        if let Ok(mut inv) = invs.get_mut(player.0) {
            for &coord in &line {
                let e = cmd.spawn_empty().id();
                inv.take_items(Stack::from(item));
                cmd.trigger(PlaceStructure {
                    entity: e,
                    structure: Structure::Belt,
                    brt: Structure::Belt.brt_for(coord, Some(facing)),
                    flb: coord,
                });
            }
        }
    }
    placement.drag_start = None;
}
