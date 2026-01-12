use bevy::app::MainScheduleOrder;
use bevy::ecs::schedule::{ExecutorKind, Schedule, ScheduleLabel};

use crate::core::*;
use std::collections::HashSet;
use std::panic::Location;

#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
struct InvariantChecks;

#[derive(Component, Clone, Copy, Debug)]
pub struct PreviousTransform(pub Transform);

pub struct InvariantsPlugin;

impl Plugin for InvariantsPlugin {
    fn build(&self, app: &mut App) {
        let mut invariant_schedule = Schedule::new(InvariantChecks);
        invariant_schedule.set_executor_kind(ExecutorKind::SingleThreaded);
        app.add_schedule(invariant_schedule);

        let mut main_schedule_order = app.world_mut().resource_mut::<MainScheduleOrder>();
        main_schedule_order.insert_after(PostUpdate, InvariantChecks);

        app.init_resource::<BrokenInvariants>();

        app.add_systems(
            InvariantChecks,
            (
                all_belts_have_coords,
                belt_coords_is_a_correct_cache,
                no_belts_share_coords,
                belt_is_not_in_one_lane_twice,
                lane_belt_data_matches_world,
                all_lanes_have_belts,
                belt_lanes_ranges_contiguous,
                all_belts_and_frags_claim_to_be_in_a_lane,
                lane_belts_and_inlane_match,
                check_adjacent_belts_in_lane_are_connected,
                no_fragments_marked_as_belt,
                all_fragments_at_head_of_lane,
                items_are_within_belt_bounds,
                (items_dont_move_too_far, update_previous_transforms).chain(),
                super::clear_changed_belts,
                |mut b: ResMut<BrokenInvariants>| {
                    b.check();
                    b.clear();
                },
            )
                .chain(),
        );
    }
}

fn all_belts_have_coords(
    query: Query<Entity, (With<Belt>, Without<WorldCoords>)>,
    mut b: ResMut<BrokenInvariants>,
) {
    for entity in query.iter() {
        b.add(format!(
            "Belt entity {:?} does not have WorldCoords component",
            entity
        ));
    }
}

fn belt_coords_is_a_correct_cache(
    belt_coords: Res<BeltCoords>,
    belts: Query<(Entity, &BeltShape, &WorldCoords), With<Belt>>,
    mut b: ResMut<BrokenInvariants>,
) {
    for (entity, belt, coords) in belts.iter() {
        match belt_coords.get(*coords) {
            Some((res_entity, res_belt)) => {
                if res_entity != entity {
                    b.add(format!(
                        "BeltCoords at {:?} points to entity {:?} but entity {:?} exists there",
                        coords, res_entity, entity
                    ));
                }
                if res_belt != *belt {
                    b.add(format!(
                        "BeltCoords at {:?} has belt type {:?} but entity {:?} has {:?}",
                        coords, res_belt, entity, belt
                    ));
                }
            }
            None => {
                b.add(format!(
                    "Belt entity {:?} at {:?} is not in BeltCoords resource",
                    entity, coords
                ));
            }
        }
    }
}

fn no_belts_share_coords(
    belts: Query<(Entity, &WorldCoords), With<Belt>>,
    mut b: ResMut<BrokenInvariants>,
) {
    let mut coords_map = HashMap::new();

    for (entity, coords) in belts.iter() {
        if let Some(existing_entity) = coords_map.insert(*coords, entity) {
            b.add(format!(
                "Multiple belts at {:?}: entities {:?} and {:?}",
                coords, existing_entity, entity
            ));
        }
    }
}

fn belt_is_not_in_one_lane_twice(
    lanes: Query<(Entity, &BeltLane)>,
    mut b: ResMut<BrokenInvariants>,
) {
    for (lane_entity, lane) in lanes.iter() {
        let mut seen_belts = HashSet::new();
        for belt_entry in &lane.belts {
            if !seen_belts.insert(belt_entry.entity) {
                b.add(format!(
                    "Lane {:?} contains belt {:?} multiple times. Full lane: {:?}",
                    lane_entity, belt_entry.entity, lane.belts
                ));
            }
        }
    }
}

fn all_lanes_have_belts(lanes: Query<(Entity, &BeltLane)>, mut b: ResMut<BrokenInvariants>) {
    for (lane_entity, lane) in lanes.iter() {
        if lane.belts.is_empty() {
            b.add(format!("Lane {:?} has no belts", lane_entity));
            continue;
        }
    }
}

fn belt_lanes_ranges_contiguous(
    lanes: Query<(Entity, &BeltLane)>,
    mut b: ResMut<BrokenInvariants>,
) {
    for (lane_entity, lane) in lanes.iter() {
        for belt in lane.belts.iter() {
            if belt.ranges.left.len() as i32 > belt.belt.left_num_pos() {
                b.add(format!(
                    "Lane {:?} has belt {:?} with more left ranges than positions",
                    lane_entity, belt.entity
                ));
            }
            if belt.ranges.right.len() as i32 > belt.belt.right_num_pos() {
                b.add(format!(
                    "Lane {:?} has belt {:?} with more right ranges than positions",
                    lane_entity, belt.entity
                ));
            }
        }
        for belts in lane.belts.windows(2) {
            let first = &belts[0];
            let second = &belts[1];

            if first.ranges.left.end != second.ranges.left.start {
                b.add(format!(
                    "Lane {:?} has non-contiguous left ranges: belt {:?} range {:?} followed by belt {:?} range {:?}",
                    lane_entity, first.entity, first.ranges.left, second.entity, second.ranges.left
                ));
            }

            if first.ranges.right.end != second.ranges.right.start {
                b.add(format!(
                    "Lane {:?} has non-contiguous right ranges: belt {:?} range {:?} followed by belt {:?} range {:?}",
                    lane_entity,
                    first.entity,
                    first.ranges.right,
                    second.entity,
                    second.ranges.right
                ));
            }
        }
    }
}

fn all_belts_and_frags_claim_to_be_in_a_lane(
    belts: Query<(Entity, &BeltShape), Without<InLane>>,
    mut b: ResMut<BrokenInvariants>,
) {
    for (belt_entity, belt_shape) in belts.iter() {
        b.add(format!("Belt {:?} is not in a lane", belt_entity));
    }
}

fn lane_belts_and_inlane_match(
    lanes: Query<(Entity, &BeltLane)>,
    belts_with_inlane: Query<(Entity, &InLane)>,
    mut b: ResMut<BrokenInvariants>,
) {
    for (lane_entity, lane) in lanes.iter() {
        for belt_entry in &lane.belts {
            match belts_with_inlane.get(belt_entry.entity) {
                Ok(inlane_lane) => {
                    if inlane_lane.1.lane != lane_entity {
                        b.add(format!(
                            "Belt {:?} is in lane {:?} but has InLane pointing to lane {:?}",
                            belt_entry.entity, lane_entity, inlane_lane.1.lane
                        ));
                    }
                }
                Err(_) => {
                    b.add(format!(
                        "Belt {:?} is in lane {:?} but does not have InLane component",
                        belt_entry.entity, lane_entity
                    ));
                }
            }
        }
    }

    for (belt_entity, inlane) in belts_with_inlane.iter() {
        let lane_entity = inlane.lane;
        if let Ok((_, lane)) = lanes.get(lane_entity) {
            let found = lane.belts.iter().any(|entry| entry.entity == belt_entity);
            if !found {
                b.add(format!(
                    "Belt {:?} has InLane pointing to lane {:?}, but is not in that lane's belt list. Lane contains: {:?}",
                    belt_entity, lane_entity, lane.belts
                ));
            }
        } else {
            b.add(format!(
                "Belt {:?} has InLane pointing to non-existent lane {:?}",
                belt_entity, lane_entity
            ));
        }
    }
}

fn lane_belt_data_matches_world(
    lanes: Query<(Entity, &BeltLane)>,
    belts: Query<(&BeltShape, &WorldCoords)>,
    mut b: ResMut<BrokenInvariants>,
) {
    for (lane_ent, lane) in lanes.iter() {
        for entry in lane.belts.iter() {
            match belts.get(entry.entity) {
                Ok((shape, coords)) => {
                    if *shape != entry.belt {
                        b.add(format!(
                            "Belt {:?} has shape in world {:?} but lane {:?} has shape {:?}",
                            entry.entity, shape, lane_ent, entry.belt
                        ));
                    }

                    if coords != &entry.coords {
                        b.add(format!(
                            "Belt {:?} has coords in world {:?} but lane {:?} has coords {:?}",
                            entry.entity, coords, lane_ent, entry.coords
                        ));
                    }
                }
                Err(_) => {
                    b.add(format!(
                        "Lane {:?} has belt {:?} which doesn't exist",
                        lane_ent, entry.entity
                    ));
                }
            }
        }
    }
}

fn check_adjacent_belts_in_lane_are_connected(
    lanes: Query<(Entity, &BeltLane)>,
    mut b: ResMut<BrokenInvariants>,
) {
    for (lane_entity, lane) in lanes.iter() {
        for pair in lane.belts.windows(2) {
            let first = &pair[0];
            let second = &pair[1];
            if second.coords.step(second.belt.output()) != first.coords {
                b.add(format!(
                    "Belt {:?} at {:?} is not ahead of Belt {:?} at {:?} with shape {:?}",
                    first.entity, first.coords, second.entity, second.coords, second.belt
                ));
            }
            if first.belt.input() != second.belt.output() {
                b.add(format!(
                    "Belt {:?} is not connected to belt {:?}",
                    first.entity, second.entity
                ));
            }
        }
    }
}

fn no_fragments_marked_as_belt(
    query: Query<(Entity, &BeltShape), With<Belt>>,
    mut b: ResMut<BrokenInvariants>,
) {
    for (entity, shape) in query.iter() {
        if matches!(shape, BeltShape::Fragment(_)) {
            b.add(format!(
                "Fragment entity {:?} incorrectly has Belt component",
                entity
            ));
        }
    }
}

fn all_fragments_at_head_of_lane(
    lanes: Query<(Entity, &BeltLane)>,
    mut b: ResMut<BrokenInvariants>,
) {
    for (lane_entity, lane) in lanes.iter() {
        for (index, belt_entry) in lane.belts.iter().enumerate() {
            if matches!(belt_entry.belt, BeltShape::Fragment(_)) {
                if index != 0 {
                    b.add(format!(
                        "Fragment belt {:?} is at index {} in lane {:?}, but should be at head (index 0)",
                        belt_entry.entity, index, lane_entity
                    ));
                }
            }
        }
    }
}

fn items_are_within_belt_bounds(
    lanes: Query<&BeltLane>,
    items: Query<(Entity, &Transform), With<Item>>,
    belts: Query<&WorldCoords>,
    mut b: ResMut<BrokenInvariants>,
) {
    for lane in lanes.iter() {
        for side in SIDES {
            for item_entry in &lane.lanes[side] {
                if let Some(belt_entry) = lane.belt_for(item_entry.pos, side) {
                    // Get the item's actual world position
                    if let Ok((item_entity, item_transform)) = items.get(item_entry.entity) {
                        // Get the belt's world coordinates
                        if let Ok(belt_coords) = belts.get(belt_entry.entity) {
                            let item_pos = item_transform.translation;
                            let belt_center = Vec3::from(*belt_coords);

                            // Check if item is within belt's bounding box
                            // Each belt occupies a BLOCK_SIZE cube centered at belt_center
                            let min_bound = belt_center - Vec3::splat(HALF_BLOCK_SIZE);
                            let max_bound = belt_center + Vec3::splat(HALF_BLOCK_SIZE);

                            if item_pos.x < min_bound.x || item_pos.x > max_bound.x
                                || item_pos.y < min_bound.y || item_pos.y > max_bound.y
                                || item_pos.z < min_bound.z || item_pos.z > max_bound.z
                            {
                                b.add(format!(
                                    "Item {:?} at position {:?} is outside belt {:?} bounds (min: {:?}, max: {:?})",
                                    item_entity, item_pos, belt_entry.entity, min_bound, max_bound
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn items_dont_move_too_far(
    belt_changes: Res<BeltChanges>,
    lanes: Query<(Entity, &BeltLane)>,
    items_with_prev: Query<(Entity, &Transform, &PreviousTransform), With<Item>>,
    belts: Query<&WorldCoords, With<Belt>>,
    mut broken_invariants: ResMut<BrokenInvariants>,
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

    let mut items_to_ignore = HashSet::new();

    for (lane_entity, lane) in lanes.iter() {
        for belt_entry in &lane.belts {
            if changed_belts.contains(&belt_entry.entity) {
                for side in SIDES {
                    lane.lanes[side]
                        .iter()
                        .filter(|item| belt_entry.ranges[side].contains(&item.pos))
                        .for_each(|i| {
                            items_to_ignore.insert(i.entity);
                        });
                }
            }
        }
    }

    for (item_entity, transform, prev_transform) in items_with_prev
        .iter()
        .filter(|i| !items_to_ignore.contains(&i.0))
    {
        let current_pos = transform.translation.xy();
        let prev_pos = prev_transform.0.translation.xy();
        let distance = current_pos.distance(prev_pos);

        // 4.0 seems to be how much an item moves on a straight belt.
        // We'll need to update this when we add faster belts.
        // 5.0 gives us a little margin of error, but not much.
        const MAX_MOVEMENT: f32 =
            5.0 * BASE_BELT_SPEED as f32 / POSITIONS_PER_BELT as f32 / BLOCK_SIZE;
        if distance > MAX_MOVEMENT {
            broken_invariants.add(format!(
                "Item {:?} moved {:.2} units in one frame (max {:.2}). Previous: {:?}, Current: {:?}",
                item_entity, distance, MAX_MOVEMENT, prev_pos, current_pos
            ));
        }
    }
}

fn update_previous_transforms(mut cmd: Commands, items: Query<(Entity, &Transform), With<Item>>) {
    for (entity, transform) in items.iter() {
        cmd.entity(entity).insert(PreviousTransform(*transform));
    }
}

#[derive(Resource, Default)]
struct BrokenInvariants {
    failures: Vec<String>,
}

impl BrokenInvariants {
    #[track_caller]
    fn add(&mut self, message: impl AsRef<str>) {
        let loc = Location::caller();
        self.failures.push(format!("{} at {loc}", message.as_ref()));
    }
    fn check(&self) {
        if !self.failures.is_empty() {
            eprintln!("Broken Invariants:");
            for f in self.failures.iter() {
                eprintln!("{f}");
            }
            panic!("Found {} failures", self.failures.len());
        }
    }
    fn clear(&mut self) {
        self.failures.clear();
    }
}
