use bevy::app::MainScheduleOrder;
use bevy::ecs::schedule::{ExecutorKind, Schedule, ScheduleLabel};

use crate::sim::*;
use std::collections::HashSet;

#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
struct InvariantChecks;

#[derive(Component, Clone, Copy, Debug)]
pub struct PreviousTransform(pub Transform);

pub struct InvariantsPlugin;

impl Plugin for InvariantsPlugin {
    fn build(&self, app: &mut App) {
        // Create custom schedule
        let mut invariant_schedule = Schedule::new(InvariantChecks);
        invariant_schedule.set_executor_kind(ExecutorKind::SingleThreaded);
        app.add_schedule(invariant_schedule);

        // Order it to run after PostUpdate
        let mut main_schedule_order = app.world_mut().resource_mut::<MainScheduleOrder>();
        main_schedule_order.insert_after(PostUpdate, InvariantChecks);

        // Add systems to the custom schedule
        app.add_systems(
            InvariantChecks,
            (
                check_no_belt_and_fragment,
                check_belts_have_coords,
                check_belt_coords_sync,
                check_no_duplicate_coords,
                check_no_duplicate_belts_in_lane,
                check_lane_ranges_contiguous,
                check_inlane_bidirectional,
                check_adjacent_belts_in_lane_are_connected,
                (check_item_movement, update_previous_transforms).chain(),
                // TODO: all_fragments_at_head_of_lane
            ),
        );
    }
}

/// Invariant 1: No entity should have both Belt and BeltFragment
fn check_no_belt_and_fragment(query: Query<Entity, (With<Belt>, With<BeltFragment>)>) {
    for entity in query.iter() {
        panic!(
            "INVARIANT VIOLATION: Entity {:?} has both Belt and BeltFragment components",
            entity
        );
    }
}

/// Invariant 5: All Belt entities must have WorldCoords
fn check_belts_have_coords(query: Query<Entity, (With<Belt>, Without<WorldCoords>)>) {
    for entity in query.iter() {
        panic!(
            "INVARIANT VIOLATION: Belt entity {:?} does not have WorldCoords component",
            entity
        );
    }
}

/// Invariant 6: BeltCoords resource must match Belt+WorldCoords entities
fn check_belt_coords_sync(
    belt_coords: Res<BeltCoords>,
    belts: Query<(Entity, &Belt, &WorldCoords)>,
) {
    for (entity, belt, coords) in belts.iter() {
        match belt_coords.get(*coords) {
            Some((res_entity, res_belt)) => {
                if res_entity != entity {
                    panic!(
                        "INVARIANT VIOLATION: BeltCoords at {:?} points to entity {:?} but entity {:?} exists there",
                        coords, res_entity, entity
                    );
                }
                if res_belt != *belt {
                    panic!(
                        "INVARIANT VIOLATION: BeltCoords at {:?} has belt type {:?} but entity {:?} has {:?}",
                        coords, res_belt, entity, belt
                    );
                }
            }
            None => {
                panic!(
                    "INVARIANT VIOLATION: Belt entity {:?} at {:?} is not in BeltCoords resource",
                    entity, coords
                );
            }
        }
    }
}

/// Invariant 7: No two belts should occupy the same WorldCoords
fn check_no_duplicate_coords(belts: Query<(Entity, &WorldCoords), With<Belt>>) {
    let mut coords_map = std::collections::HashMap::new();

    for (entity, coords) in belts.iter() {
        if let Some(existing_entity) = coords_map.insert(*coords, entity) {
            panic!(
                "INVARIANT VIOLATION: Multiple belts at {:?}: entities {:?} and {:?}",
                coords, existing_entity, entity
            );
        }
    }
}

/// Invariant 8: No belt entity should appear multiple times in a single lane
fn check_no_duplicate_belts_in_lane(lanes: Query<(Entity, &BeltLane)>) {
    for (lane_entity, lane) in lanes.iter() {
        let mut seen_belts = HashSet::new();
        for (_, belt_entity) in &lane.belts.belts {
            if !seen_belts.insert(belt_entity) {
                panic!(
                    "INVARIANT VIOLATION: Lane {:?} contains belt {:?} multiple times. Full lane: {:?}",
                    lane_entity, belt_entity, lane.belts.belts
                );
            }
        }
    }
}

/// Invariant 9: Belt ranges in a lane must be contiguous and non-overlapping
fn check_lane_ranges_contiguous(lanes: Query<(Entity, &BeltLane)>) {
    for (lane_entity, lane) in lanes.iter() {
        if lane.belts.belts.is_empty() {
            panic!("INVARIANT VIOLATION: Lane {:?} has no belts", lane_entity);
        }

        // First belt must start at 0
        let first = &lane.belts.belts[0];
        if first.0.start != 0 {
            panic!(
                "INVARIANT VIOLATION: Lane {:?} first belt range doesn't start at 0. Range: {:?}",
                lane_entity, first.0
            );
        }

        // Check each consecutive pair
        for i in 0..lane.belts.belts.len() - 1 {
            let current = &lane.belts.belts[i];
            let next = &lane.belts.belts[i + 1];

            if current.0.end != next.0.start {
                panic!(
                    "INVARIANT VIOLATION: Lane {:?} has non-contiguous ranges: belt {:?} range {:?} followed by belt {:?} range {:?}",
                    lane_entity, current.1, current.0, next.1, next.0
                );
            }
        }
    }
}

/// Invariant 10: Bidirectional relationship between belts and lanes
/// - If a belt has InLane component, it must exist in that lane
/// - If a belt is in a lane's belt list, it must have InLane pointing to that lane
fn check_inlane_bidirectional(
    lanes: Query<(Entity, &BeltLane)>,
    belts_with_inlane: Query<(Entity, &InLane)>,
) {
    // Build a map of belt -> lane from the InLane components
    let mut inlane_map = std::collections::HashMap::new();
    for (belt_entity, inlane) in belts_with_inlane.iter() {
        inlane_map.insert(belt_entity, inlane.lane);
    }

    // Check each lane
    for (lane_entity, lane) in lanes.iter() {
        for (_, belt_entity) in &lane.belts.belts {
            // Belt in lane should have InLane component pointing to this lane
            match inlane_map.get(belt_entity) {
                Some(&inlane_lane) => {
                    if inlane_lane != lane_entity {
                        panic!(
                            "INVARIANT VIOLATION: Belt {:?} is in lane {:?} but has InLane pointing to lane {:?}",
                            belt_entity, lane_entity, inlane_lane
                        );
                    }
                }
                None => {
                    panic!(
                        "INVARIANT VIOLATION: Belt {:?} is in lane {:?} but does not have InLane component",
                        belt_entity, lane_entity
                    );
                }
            }
        }
    }

    // Check reverse: all belts with InLane should exist in their referenced lane
    for (belt_entity, inlane) in belts_with_inlane.iter() {
        let lane_entity = inlane.lane;
        if let Ok((_, lane)) = lanes.get(lane_entity) {
            let found = lane.belts.belts.iter().any(|(_, ent)| ent == &belt_entity);
            if !found {
                panic!(
                    "INVARIANT VIOLATION: Belt {:?} has InLane pointing to lane {:?}, but is not in that lane's belt list. Lane contains: {:?}",
                    belt_entity, lane_entity, lane.belts.belts
                );
            }
        } else {
            panic!(
                "INVARIANT VIOLATION: Belt {:?} has InLane pointing to non-existent lane {:?}",
                belt_entity, lane_entity
            );
        }
    }
}

/// Invariant 11: Adjacent belts in a lane must be physically connected in the world
/// - Each belt in the lane (except the last) must output to the next belt's input
/// - The world coordinates must be adjacent
fn check_adjacent_belts_in_lane_are_connected(
    lanes: Query<(Entity, &BeltLane)>,
    belts: Query<(AnyOf<(&Belt, &BeltFragment)>, &WorldCoords)>,
) {
    for (lane_entity, lane) in lanes.iter() {
        if lane.belts.belts.len() <= 1 {
            continue; // Single belt lane is always valid
        }

        for i in 0..lane.belts.belts.len() - 1 {
            let current_entity = lane.belts.belts[i].1;
            let next_entity = lane.belts.belts[i + 1].1;

            let Ok((current_belt, _)) = belts.get(current_entity) else {
                panic!(
                    "INVARIANT VIOLATION: Lane {:?} contains belt {:?} which doesn't have Belt+WorldCoords components",
                    lane_entity, current_entity
                );
            };
            let current_belt = BeltLike::new(current_belt);

            let Ok((next_belt, _)) = belts.get(next_entity) else {
                panic!(
                    "INVARIANT VIOLATION: Lane {:?} contains belt {:?} which doesn't have Belt+WorldCoords components",
                    lane_entity, next_entity
                );
            };
            let next_belt = BeltLike::new(next_belt);

            // Check that next belt's output connects to current belt's input
            if current_belt.input() != next_belt.output() {
                panic!(
                    "INVARIANT VIOLATION: Lane {lane_entity:?} has misconnected belts: belt {next_entity:?}  outputs {:?} but belt {current_entity:?} (earlier in vec) inputs {:?}",
                    next_belt.output(),
                    current_belt.input()
                );
            }
        }
    }
}

/// Invariant 12: Items should not move more than 1.1 pixels per frame
/// (excludes items on belts that were updated this frame)
fn check_item_movement(
    belt_changes: Res<BeltChanges>,
    lanes: Query<(Entity, &BeltLane)>,
    items_with_prev: Query<(Entity, &Transform, &PreviousTransform), With<Item>>,
    belts: Query<&WorldCoords, Or<(With<Belt>, With<BeltFragment>)>>,
) {
    // Build set of changed belt entities (including old entities from replaced belts)
    let changed_belts: Vec<Entity> = belt_changes
        .0
        .iter()
        .flat_map(|change| match change {
            BeltChange::New(new_belt) => vec![new_belt.entity],
            BeltChange::Removed(removed_belt) => vec![removed_belt.entity],
            BeltChange::Replaced(ReplacedBelt {
                entity,
                old_entity: Some(old_entity),
                ..
            }) => vec![*entity, *old_entity],
            BeltChange::Replaced(replaced) => vec![replaced.entity],
        })
        .collect();

    // Build map of item -> belt entity
    let mut item_to_belt = std::collections::HashMap::new();
    for (_lane_entity, lane) in lanes.iter() {
        for (_pos, item_ent) in &lane.items.items {
            // Find which belt this item is on
            let belt_ent = lane.belt_for(*_pos).unwrap();
            item_to_belt.insert(*item_ent, belt_ent);
        }
    }

    // Check each item's movement
    for (item_entity, transform, prev_transform) in items_with_prev.iter() {
        // Skip items on belts that were changed this frame
        let &belt_ent = item_to_belt.get(&item_entity).unwrap();
        let expected_coords = belts.get(belt_ent).unwrap();
        let belt_pos = Vec2::from(*expected_coords);
        if changed_belts.contains(&belt_ent) {
            continue;
        }

        let current_pos = transform.translation.xy();
        let prev_pos = prev_transform.0.translation.xy();
        let distance = current_pos.distance(prev_pos);

        assert!(
            belt_pos.distance(current_pos) <= TILE_SIZE / 2.0,
            "Item {item_entity:?} was not on belt {belt_ent:?}"
        );

        const MAX_MOVEMENT: f32 = 1.1;
        if distance > MAX_MOVEMENT {
            panic!(
                "INVARIANT VIOLATION: Item {:?} moved {:.2} pixels in one frame (max {:.2}). Previous: {:?}, Current: {:?}",
                item_entity, distance, MAX_MOVEMENT, prev_pos, current_pos
            );
        }
    }
}

/// Update previous transforms for next frame's movement check
fn update_previous_transforms(mut cmd: Commands, items: Query<(Entity, &Transform), With<Item>>) {
    for (entity, transform) in items.iter() {
        cmd.entity(entity).insert(PreviousTransform(*transform));
    }
}
