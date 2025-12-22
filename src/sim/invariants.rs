use crate::core::*;
use crate::sim::{BeltFragment, BeltLane, InLane};
use bevy::prelude::*;
use std::collections::HashSet;

pub struct InvariantsPlugin;

impl Plugin for InvariantsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                check_no_belt_and_fragment,
                check_belts_have_coords,
                check_belt_coords_sync,
                check_no_duplicate_coords,
                check_no_duplicate_belts_in_lane,
                check_lane_ranges_contiguous,
                check_inlane_bidirectional,
                check_adjacent_belts_in_lane_are_connected,
            )
                .chain(),
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
    belts: Query<(&Belt, &WorldCoords)>,
) {
    for (lane_entity, lane) in lanes.iter() {
        if lane.belts.belts.len() <= 1 {
            continue; // Single belt lane is always valid
        }

        for i in 0..lane.belts.belts.len() - 1 {
            let current_entity = lane.belts.belts[i].1;
            let next_entity = lane.belts.belts[i + 1].1;

            let Ok((current_belt, current_coords)) = belts.get(current_entity) else {
                panic!(
                    "INVARIANT VIOLATION: Lane {:?} contains belt {:?} which doesn't have Belt+WorldCoords components",
                    lane_entity, current_entity
                );
            };

            let Ok((next_belt, next_coords)) = belts.get(next_entity) else {
                panic!(
                    "INVARIANT VIOLATION: Lane {:?} contains belt {:?} which doesn't have Belt+WorldCoords components",
                    lane_entity, next_entity
                );
            };

            // Check that next belt (later in vec, upstream) outputs to current belt (earlier in vec, downstream)
            let expected_next_pos = next_coords.step(next_belt.output());
            if expected_next_pos != *current_coords {
                panic!(
                    "INVARIANT VIOLATION: Lane {lane_entity:?} has non-adjacent belts: belt {next_entity:?} at {next_coords:?} facing {next_belt:?} should output to belt {current_entity:?} at {current_coords:?}, but outputs to {expected_next_pos:?} instead",
                );
            }

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
