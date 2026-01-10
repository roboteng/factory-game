use bevy::app::MainScheduleOrder;
use bevy::ecs::schedule::{ExecutorKind, Schedule, ScheduleLabel};

use crate::core::*;
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
                check_belts_have_coords,
                check_belt_coords_sync,
                check_no_duplicate_coords,
                check_no_duplicate_belts_in_lane,
                check_lane_ranges_contiguous,
                check_inlane_bidirectional,
                check_adjacent_belts_in_lane_are_connected,
                (check_item_movement, update_previous_transforms).chain(),
                super::clear_changed_belts,
            )
                .chain(),
        );
    }
}

/// Invariant: All Belt entities must have WorldCoords
fn check_belts_have_coords(query: Query<Entity, (With<Belt>, Without<WorldCoords>)>) {
    for entity in query.iter() {
        panic!(
            "INVARIANT VIOLATION: Belt entity {:?} does not have WorldCoords component",
            entity
        );
    }
}

/// Invariant: BeltCoords resource must match Belt+WorldCoords entities
fn check_belt_coords_sync(
    belt_coords: Res<BeltCoords>,
    belts: Query<(Entity, &BeltShape, &WorldCoords), With<Belt>>,
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

/// Invariant: No two belts should occupy the same WorldCoords
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

/// Invariant: No belt entity should appear multiple times in a single lane
fn check_no_duplicate_belts_in_lane(lanes: Query<(Entity, &BeltLane)>) {
    for (lane_entity, lane) in lanes.iter() {
        let mut seen_belts = HashSet::new();
        for belt_entry in &lane.belts {
            if !seen_belts.insert(belt_entry.entity) {
                panic!(
                    "INVARIANT VIOLATION: Lane {:?} contains belt {:?} multiple times. Full lane: {:?}",
                    lane_entity, belt_entry.entity, lane.belts
                );
            }
        }
    }
}

/// Invariant: Belt ranges in a lane must be contiguous and non-overlapping for both lanes
fn check_lane_ranges_contiguous(lanes: Query<(Entity, &BeltLane)>) {
    for (lane_entity, lane) in lanes.iter() {
        if lane.belts.is_empty() {
            panic!("INVARIANT VIOLATION: Lane {:?} has no belts", lane_entity);
        }

        // Check left lane
        let first = &lane.belts[0];
        if first.ranges.left.start != 0 && !matches!(first.belt, BeltShape::Fragment(_)) {
            panic!(
                "INVARIANT VIOLATION: Lane {:?} first belt left range doesn't start at 0. Range: {:?}",
                lane_entity, first.ranges.left
            );
        }

        for i in 0..lane.belts.len() - 1 {
            let current = &lane.belts[i];
            let next = &lane.belts[i + 1];

            if current.ranges.left.end != next.ranges.left.start {
                panic!(
                    "INVARIANT VIOLATION: Lane {:?} has non-contiguous left ranges: belt {:?} range {:?} followed by belt {:?} range {:?}",
                    lane_entity, current.entity, current.ranges.left, next.entity, next.ranges.left
                );
            }
        }

        // Check right lane
        if first.ranges.right.start != 0 {
            panic!(
                "INVARIANT VIOLATION: Lane {:?} first belt right range doesn't start at 0. Range: {:?}",
                lane_entity, first.ranges.right
            );
        }

        for i in 0..lane.belts.len() - 1 {
            let current = &lane.belts[i];
            let next = &lane.belts[i + 1];

            if current.ranges.right.end != next.ranges.right.start {
                panic!(
                    "INVARIANT VIOLATION: Lane {:?} has non-contiguous right ranges: belt {:?} range {:?} followed by belt {:?} range {:?}",
                    lane_entity,
                    current.entity,
                    current.ranges.right,
                    next.entity,
                    next.ranges.right
                );
            }
        }
    }
}

/// Invariant: Bidirectional relationship between belts and lanes
fn check_inlane_bidirectional(
    lanes: Query<(Entity, &BeltLane)>,
    belts_with_inlane: Query<(Entity, &InLane)>,
) {
    let mut inlane_map = std::collections::HashMap::new();
    for (belt_entity, inlane) in belts_with_inlane.iter() {
        inlane_map.insert(belt_entity, inlane.lane);
    }

    for (lane_entity, lane) in lanes.iter() {
        for belt_entry in &lane.belts {
            match inlane_map.get(&belt_entry.entity) {
                Some(&inlane_lane) => {
                    if inlane_lane != lane_entity {
                        panic!(
                            "INVARIANT VIOLATION: Belt {:?} is in lane {:?} but has InLane pointing to lane {:?}",
                            belt_entry.entity, lane_entity, inlane_lane
                        );
                    }
                }
                None => {
                    panic!(
                        "INVARIANT VIOLATION: Belt {:?} is in lane {:?} but does not have InLane component",
                        belt_entry.entity, lane_entity
                    );
                }
            }
        }
    }

    for (belt_entity, inlane) in belts_with_inlane.iter() {
        let lane_entity = inlane.lane;
        if let Ok((_, lane)) = lanes.get(lane_entity) {
            let found = lane.belts.iter().any(|entry| entry.entity == belt_entity);
            if !found {
                panic!(
                    "INVARIANT VIOLATION: Belt {:?} has InLane pointing to lane {:?}, but is not in that lane's belt list. Lane contains: {:?}",
                    belt_entity, lane_entity, lane.belts
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

/// Invariant: Adjacent belts in a lane must be physically adjacent
fn check_adjacent_belts_in_lane_are_connected(
    lanes: Query<(Entity, &BeltLane)>,
    belts: Query<&BeltShape, With<Belt>>,
) {
    for (lane_entity, lane) in lanes.iter() {
        if lane.belts.len() <= 1 {
            continue;
        }

        for i in 0..lane.belts.len() - 1 {
            let current = &lane.belts[i];
            let next = &lane.belts[i + 1];

            let Ok(current_belt) = belts.get(current.entity) else {
                panic!(
                    "INVARIANT VIOLATION: Lane {:?} contains belt {:?} which doesn't have BeltShape component",
                    lane_entity, current.entity
                );
            };

            let Ok(next_belt) = belts.get(next.entity) else {
                panic!(
                    "INVARIANT VIOLATION: Lane {:?} contains belt {:?} which doesn't have BeltShape component",
                    lane_entity, next.entity
                );
            };

            if current_belt.input() != next_belt.output() {
                panic!(
                    "INVARIANT VIOLATION: Lane {lane_entity:?} has misconnected belts: belt {:?} outputs {:?} but belt {:?} (earlier in vec) inputs {:?}",
                    next.entity,
                    next_belt.output(),
                    current.entity,
                    current_belt.input()
                );
            }
        }
    }
}

/// Invariant: Items should not move more than expected per frame
fn check_item_movement(
    belt_changes: Res<BeltChanges>,
    lanes: Query<(Entity, &BeltLane)>,
    loop_lanes: Query<Entity, With<super::LaneLoopConnection>>,
    items_with_prev: Query<(Entity, &Transform, &PreviousTransform), With<Item>>,
    belts: Query<&WorldCoords, With<Belt>>,
) {
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

    // Build a set of lanes that contain changed belts or are loop lanes
    let mut changed_lanes = std::collections::HashSet::new();
    for (lane_entity, lane) in lanes.iter() {
        for belt_entry in &lane.belts {
            if changed_belts.contains(&belt_entry.entity) {
                changed_lanes.insert(lane_entity);
                break;
            }
        }
    }
    // Also skip lanes with loop connections (items can jump large distances when looping)
    for loop_lane_ent in loop_lanes.iter() {
        changed_lanes.insert(loop_lane_ent);
    }

    let mut item_to_lane = std::collections::HashMap::new();
    for (lane_entity, lane) in lanes.iter() {
        for item_entry in lane.lanes.left.iter().chain(lane.lanes.right.iter()) {
            item_to_lane.insert(item_entry.entity, lane_entity);
        }
    }

    for (item_entity, transform, prev_transform) in items_with_prev.iter() {
        let Some(&lane_ent) = item_to_lane.get(&item_entity) else {
            continue;
        };

        // Skip items in lanes that have changed belts
        if changed_lanes.contains(&lane_ent) {
            continue;
        }

        let current_pos = transform.translation.xy();
        let prev_pos = prev_transform.0.translation.xy();
        let distance = current_pos.distance(prev_pos);

        // 4.0 seems to be how much an item moves on a straight belt.
        // We'll need to update this when we add faster belts.
        // 5.0 gives us a little margin of error, but not much.
        const MAX_MOVEMENT: f32 =
            5.0 * BASE_BELT_SPEED as f32 / POSITIONS_PER_BELT as f32 / BLOCK_SIZE;
        if distance > MAX_MOVEMENT {
            panic!(
                "INVARIANT VIOLATION: Item {:?} moved {:.2} pixels in one frame (max {:.2}). Previous: {:?}, Current: {:?}",
                item_entity, distance, MAX_MOVEMENT, prev_pos, current_pos
            );
        }
    }
}

fn update_previous_transforms(mut cmd: Commands, items: Query<(Entity, &Transform), With<Item>>) {
    for (entity, transform) in items.iter() {
        cmd.entity(entity).insert(PreviousTransform(*transform));
    }
}
