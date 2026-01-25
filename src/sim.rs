use bevy::prelude::*;

use crate::core::*;

pub struct SimPlugin;
impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (determine_sideload_blocks, transfers, plan_moves, do_moves)
                .chain()
                .after(block_changes),
        );
    }
}

pub fn plan_moves(mut lanes: Query<&mut BeltLane>) {
    for mut lane in lanes.iter_mut() {
        lane.tick();
    }
}

pub fn do_moves(mut items: Query<&mut Transform, With<Item>>, lanes: Query<&BeltLane>) {
    for lane in lanes.iter() {
        for side in SIDES {
            for item_entry in &lane.lanes[side] {
                let belt_entry = lane
                    .belt_for(item_entry.pos, side)
                    .expect("Invariant broken: items_are_within_belt_bounds");

                let relative_pos = item_entry.pos - belt_entry.lane_offsets[side];
                debug!(
                    "relative position: {:?}, lane offset: {:?}",
                    relative_pos, belt_entry.lane_offsets[side]
                );
                let transform =
                    item_position(belt_entry.belt, belt_entry.coords, side, relative_pos);

                let mut t = items
                    .get_mut(item_entry.entity)
                    .expect("Invariant broken: all_items_have_transform_component");
                *t = transform;
            }
        }
    }
}

pub fn determine_sideload_blocks(
    conns: Query<(Entity, &LaneConnection)>,
    mut lanes: Query<&mut BeltLane>,
) {
    for (source_ent, conn) in conns.iter() {
        // Skip loop connections (source == target)
        if source_ent == conn.target {
            continue;
        }

        let target_lane = lanes
            .get(conn.target)
            .expect("Invariant broken: lane_connection_target_is_valid_lane");

        // Check blocking for LEFT and RIGHT lanes INDEPENDENTLY
        // Each lane transfers to the same target_side but at different offsets
        let left_blocked = target_lane.is_blocking_at(conn.offset.left, conn.target_side);
        let right_blocked = target_lane.is_blocking_at(conn.offset.right, conn.target_side);

        // Set source lane per-side blocking state
        let mut source_lane = lanes
            .get_mut(source_ent)
            .expect("Invariant broken: lane_connection_source_is_valid_lane");
        source_lane.is_blocked[LaneSide::Left] = left_blocked;
        source_lane.is_blocked[LaneSide::Right] = right_blocked;
    }
}

pub fn transfers(
    conns: Query<(Entity, &LaneConnection)>,
    loop_conns: Query<(Entity, &LaneLoopConnection)>,
    mut lanes: Query<&mut BeltLane>,
) {
    // Process loop connections
    for (lane_ent, loop_conn) in loop_conns.iter() {
        debug!("processing loop connection");
        let mut lane = lanes
            .get_mut(lane_ent)
            .expect("Invariant broken: lane_loop_connection_points_to_existing_lane");

        for side in SIDES {
            let start = lane.belts[0].ranges[side].start;
            if let Some(item) = lane.lanes[side].first_mut() {
                if item.pos - start < BASE_BELT_SPEED {
                    item.pos += loop_conn.offset[side];
                    lane.lanes[side].sort();
                }
            }
        }
    }

    // Process regular connections
    for (source_ent, conn) in conns.iter().filter(|(ent, c)| *ent != c.target) {
        debug!("processing connection");

        for side in SIDES {
            let mut source_lane = lanes
                .get_mut(source_ent)
                .expect("Invariant broken: lane_connection_source_is_valid_lane");

            // Check per-side blocking
            if source_lane.is_blocked_for_side(side) {
                debug!("connection blocked for {:?}, skipping transfer", side);
                continue;
            }

            let start = source_lane.belts[0].ranges[side].start;
            if let Some(item_entry) = source_lane.lanes[side].first().copied() {
                debug!("item at pos: {}", item_entry.pos);
                if item_entry.pos - start < BASE_BELT_SPEED {
                    source_lane.lanes[side].remove(0);
                    let mut target_lane = lanes
                        .get_mut(conn.target)
                        .expect("Invariant broken: lane_connection_target_is_valid_lane");
                    target_lane.insert_items_at(
                        &[ItemEntry {
                            pos: conn.offset[side],
                            ..item_entry
                        }],
                        conn.target_side,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AppExtension, LaneSide, test_app};
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn item_moves_on_belt() {
        let mut app = test_app();
        let belt = app.add_belt((0, 0, 0), HDir::East);
        app.update();
        let item = app.add_item(belt, POSITIONS_PER_BELT / 2, LaneSide::Left);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();

        // Item should have moved BASE_BELT_SPEED positions forward
        // Starting at POSITIONS_PER_BELT/2, moving BASE_BELT_SPEED positions forward
        let expected_transform = item_position(
            BeltShape::Straight(HDir::East),
            WorldCoords { x: 0, y: 0, z: 0 },
            LaneSide::Left,
            POSITIONS_PER_BELT / 2 - BASE_BELT_SPEED,
        );
        assert_eq!(actual, expected_transform);
    }

    #[test]
    fn item_moves_on_belt_north() {
        let mut app = test_app();
        let belt = app.add_belt((0, 0, 0), HDir::North);
        app.update();
        let item = app.add_item(belt, POSITIONS_PER_BELT / 2, LaneSide::Left);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();

        let expected_transform = item_position(
            BeltShape::Straight(HDir::North),
            WorldCoords { x: 0, y: 0, z: 0 },
            LaneSide::Left,
            POSITIONS_PER_BELT / 2 - BASE_BELT_SPEED,
        );
        assert_eq!(actual, expected_transform);
    }

    #[test]
    fn item_doesnt_move_at_belt_end() {
        let mut app = test_app();
        let belt = app.add_belt((0, 0, 0), HDir::East);
        app.update();
        let item = app.add_item(belt, 0, LaneSide::Left);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();

        // Item at position 0 can't move further (end of belt), stays at 0
        let expected_transform = item_position(
            BeltShape::Straight(HDir::East),
            WorldCoords { x: 0, y: 0, z: 0 },
            LaneSide::Left,
            0,
        );
        assert_eq!(actual, expected_transform);
    }

    #[test]
    fn item_moves_onto_next_belt() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0, 0), HDir::East);
        app.add_belt((0, 0, 1), HDir::East);
        app.update();
        let item = app.add_item(belt1, 0, LaneSide::Left);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();

        // Item transfers to next belt and moves BASE_BELT_SPEED positions on it
        let expected_transform = item_position(
            BeltShape::Straight(HDir::East),
            WorldCoords { x: 0, y: 0, z: 0 },
            LaneSide::Left,
            -BASE_BELT_SPEED,
        );
        assert_eq!(actual, expected_transform);
    }

    #[test]
    fn item_dont_get_too_close() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0, 0), HDir::East);
        app.update();
        app.add_item(belt1, 0, LaneSide::Left);
        let item2 = app.add_item(belt1, ITEM_SPACING, LaneSide::Left);
        app.update();
        let (_, actual) = app.find_item(item2).unwrap();

        // Item2 should stay at ITEM_SPACING distance from item1 (which is at 0)
        // So it shouldn't move
        let expected_transform = item_position(
            BeltShape::Straight(HDir::East),
            WorldCoords { x: 0, y: 0, z: 0 },
            LaneSide::Left,
            ITEM_SPACING,
        );
        assert_eq!(actual, expected_transform);
    }

    #[test]
    fn item_moves_to_next_belt_with_item() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0, 0), HDir::East);
        let belt2 = app.add_belt((0, 0, 1), HDir::East);
        app.update();
        app.add_item(belt2, 0, LaneSide::Left);
        let item = app.add_item(belt1, 0, LaneSide::Left);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();

        // Item transfers to next belt and moves BASE_BELT_SPEED positions on it
        let expected_transform = item_position(
            BeltShape::Straight(HDir::East),
            WorldCoords { x: 0, y: 0, z: 0 },
            LaneSide::Left,
            -BASE_BELT_SPEED,
        );
        assert_eq!(actual, expected_transform);
    }

    #[test]
    fn item_moves_on_merged_lanes() {
        let mut app = test_app();
        let tail_belt = app.add_belt((0, 0, 0), HDir::East);
        let _head_belt = app.add_belt((0, 0, 2), HDir::East);
        app.update();
        debug!("head and tail placed");
        let _middle_belt = app.add_belt((0, 0, 1), HDir::East);
        let item = app.add_item(tail_belt, 0, LaneSide::Left);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();

        // Item should transfer to belt3 (middle belt) and move
        let expected_transform = item_position(
            BeltShape::Straight(HDir::East),
            WorldCoords { x: 0, y: 0, z: 0 },
            LaneSide::Left,
            -BASE_BELT_SPEED,
        );
        assert_eq!(actual, expected_transform);
    }

    #[test]
    fn small_belt_loop() {
        let mut app = test_app();
        app.add_belt((0, 0, 0), HDir::East);
        app.add_belt((0, 0, 1), HDir::North);
        app.add_belt((1, 0, 1), HDir::West);
        let belt = app.add_belt((1, 0, 0), HDir::South);
        app.update();
        let item1 = app.add_item(belt, 0, LaneSide::Left);
        let item2 = app.add_item(belt, 0, LaneSide::Right);
        app.update();
        let mut prev_pos1 = app.find_item(item1).unwrap().1.translation;
        let mut prev_pos2 = app.find_item(item2).unwrap().1.translation;
        app.update();
        // Loop through multiple cycles to verify item keeps moving
        for _ in 0..(POSITIONS_PER_OUTER_CURVE * 4 / BASE_BELT_SPEED + BASE_BELT_SPEED) {
            let pos = app.find_item(item1).unwrap().1.translation;
            assert_ne!(pos, prev_pos1, "Item should keep moving in loop");
            prev_pos1 = pos;
            let pos = app.find_item(item2).unwrap().1.translation;
            assert_ne!(pos, prev_pos2, "Item should keep moving in loop");
            prev_pos2 = pos;
            app.update();
        }
    }

    #[test]
    fn remove_single_belt() {
        let mut app = test_app();
        app.add_belt((0, 0, 0), HDir::East);
        app.update();
        app.remove_belt_at((0, 0, 0));
        app.update();
    }

    #[test]
    fn replace_belt() {
        let mut app = test_app();
        app.add_belt((3, 2, 0), HDir::East);
        app.add_belt((3, 1, 0), HDir::North);
        app.add_belt((3, 1, 0), HDir::North);
        app.update();
    }

    #[test]
    fn belt_chain_with_branches() {
        let mut app = test_app();
        app.add_belt((3, 0, 2), HDir::East);
        app.add_belt((3, 0, 1), HDir::North);
        app.add_belt((3, 0, 3), HDir::South);
        app.add_belt((2, 0, 1), HDir::East);
        app.update();
    }

    #[test]
    fn replace_belt_with_item_on_curved_belt() {
        let mut app = test_app();
        app.add_belt((0, 0, 0), HDir::East);
        let belt2 = app.add_belt((1, 0, 0), HDir::North);
        app.update();
        app.add_item(belt2, 0, LaneSide::Left);
        app.update();
        app.add_belt((0, 0, 0), HDir::North);
        app.update();
    }

    #[test]
    fn replace_belt_under_item() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0, 0), HDir::East);
        app.update();
        let item = app.add_item(belt1, POSITIONS_PER_FRAGMENT, LaneSide::Left);
        app.update();
        let init_pos = app.find_item(item).unwrap();
        app.add_belt((0, 0, 0), HDir::East);
        app.update();
        let actual = app.find_item(item).unwrap();
        assert_ne!(actual, init_pos);
    }

    #[test]
    fn items_move_together() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0, 0), HDir::North);
        let belt2 = app.add_belt((1, 0, 0), HDir::North);
        app.update();

        let first_item = app.add_item(belt2, 0, LaneSide::Left);
        for i in 1..ITEMS_PER_BELT {
            app.add_item(belt2, ITEM_SPACING * i, LaneSide::Left);
        }
        let last_item = app.add_item(belt1, 0, LaneSide::Left);
        app.update();
        fn dist(app: &mut App, lead_item: Entity, follow_item: Entity) -> f32 {
            let lead_pos = app.find_item(lead_item).unwrap().1.translation;
            let follow_pos = app.find_item(follow_item).unwrap().1.translation;
            lead_pos.distance(follow_pos)
        }
        let expected = dist(&mut app, first_item, last_item);
        app.add_belt((2, 0, 0), HDir::North);

        app.update();
        let actual = dist(&mut app, first_item, last_item);
        assert_eq!(actual, expected);
    }

    #[test]
    fn side_loading_starts_earlier_lane() {
        let mut app = test_app();
        app.add_belt((0, 0, 0), HDir::North);
        app.add_belt((-1, 0, 0), HDir::North);
        app.update();
        let side_load_belt = app.add_belt((0, 0, 1), HDir::West);
        app.update();
        let item = app.add_item(side_load_belt, 0, LaneSide::Left);
        app.update();
        let init_pos = app.find_item(item).unwrap().1.translation;
        app.update();
        let next_pos = app.find_item(item).unwrap().1.translation;
        assert_ne!(init_pos, next_pos);
    }

    #[test]
    fn side_loading_starts_later_lane() {
        let mut app = test_app();
        app.add_belt((0, 0, 0), HDir::North);
        app.add_belt((-1, 0, 0), HDir::North);
        app.update();
        let side_load_belt = app.add_belt((0, 0, 1), HDir::West);
        app.update();
        let item = app.add_item(side_load_belt, 0, LaneSide::Right);
        app.update();
        let init_pos = app.find_item(item).unwrap().1.translation;
        app.update();
        let next_pos = app.find_item(item).unwrap().1.translation;
        assert_ne!(init_pos, next_pos);
    }

    #[test]
    fn place_item_outside_of_bounds() {
        let mut app = test_app();
        let belt = app.add_belt((3, 1, 0), HDir::North);
        app.update();

        let world = app.world_mut();
        let lane = world.query::<&BeltLane>().single(world).unwrap();
        info!("{lane:#?}");

        app.add_item(belt, 232, LaneSide::Left);
        app.update();
    }

    #[test]
    fn item_moves_towards_side_loading_belt_left_lane() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0, 0), HDir::East);
        app.add_belt((0, 0, 1), HDir::North);
        app.add_belt((-1, 0, 1), HDir::North);
        app.update();

        let item = app.add_item(belt1, POSITIONS_PER_BELT / 2, LaneSide::Left);
        app.update();
        let init_pos = app.find_item(item).unwrap().1.translation;
        app.update();
        let next_pos = app.find_item(item).unwrap().1.translation;

        // Item should have moved (not stuck)
        assert_ne!(init_pos, next_pos);
    }

    #[test]
    fn item_moves_towards_side_loading_belt_right_lane() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0, 0), HDir::East);
        app.add_belt((0, 0, 1), HDir::North);
        app.add_belt((-1, 0, 1), HDir::North);
        app.update();

        let item = app.add_item(belt1, POSITIONS_PER_BELT / 2, LaneSide::Right);
        app.update();
        let init_pos = app.find_item(item).unwrap().1.translation;
        app.update();
        let next_pos = app.find_item(item).unwrap().1.translation;

        // Item should have moved (not stuck)
        assert_ne!(init_pos, next_pos);
    }

    #[test]
    fn item_moves_towards_side_loading_belt_other_order_left_lane() {
        let mut app = test_app();
        app.add_belt((0, 0, 1), HDir::North);
        app.add_belt((-1, 0, 1), HDir::North);
        app.update();
        let belt1 = app.add_belt((0, 0, 0), HDir::East);
        app.update();

        let item = app.add_item(belt1, POSITIONS_PER_BELT / 2, LaneSide::Left);
        app.update();
        let init_pos = app.find_item(item).unwrap().1.translation;
        app.update();
        let next_pos = app.find_item(item).unwrap().1.translation;

        // Item should have moved (not stuck)
        assert_ne!(init_pos, next_pos);
    }

    #[test]
    fn item_moves_towards_side_loading_belt_other_order_right_lane() {
        let mut app = test_app();
        app.add_belt((0, 0, 1), HDir::North);
        app.add_belt((-1, 0, 1), HDir::North);
        app.update();
        let belt1 = app.add_belt((0, 0, 0), HDir::East);
        app.update();

        let item = app.add_item(belt1, POSITIONS_PER_BELT / 2, LaneSide::Right);
        app.update();
        let init_pos = app.find_item(item).unwrap().1.translation;
        app.update();
        let next_pos = app.find_item(item).unwrap().1.translation;

        // Item should have moved (not stuck)
        assert_ne!(init_pos, next_pos);
    }

    #[test]
    fn item_moves_onto_side_loaded_belt_left_lane() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0, 0), HDir::East);
        app.add_belt((0, 0, 1), HDir::North);
        app.add_belt((-1, 0, 1), HDir::North);
        app.update();

        let item = app.add_item(belt1, POSITIONS_PER_BELT / 2, LaneSide::Left);
        app.update();
        let init_pos = app.find_item(item).unwrap().1.translation;

        // Run enough updates for item to traverse and transfer
        for _ in 0..((POSITIONS_PER_BELT / 2 + POSITIONS_PER_FRAGMENT) / BASE_BELT_SPEED + 2) {
            app.update();
        }

        let final_pos = app.find_item(item).unwrap().1.translation;

        // Item should have transferred to the North belt (z coordinate should change from 0 to 1.5)
        assert_ne!(init_pos.z, final_pos.z);
        assert!(
            (final_pos.z - 1.5).abs() < 0.1,
            "Item should be on North belt around z=1.5, got z={}",
            final_pos.z
        );
    }

    #[test]
    fn item_moves_onto_side_loaded_belt_right_lane() {
        let mut app = test_app();
        app.add_belt((-1, 0, 0), HDir::North);
        let side_loading = app.add_belt((0, 0, 1), HDir::West);
        let _side_loaded = app.add_belt((0, 0, 0), HDir::North);
        app.update();

        app.add_item(side_loading, 0, LaneSide::Right);

        for _ in 0..(POSITIONS_PER_FRAGMENT + ITEM_SPACING) / BASE_BELT_SPEED {
            app.update();
        }
    }

    #[test]
    fn item_doesnt_move_onto_side_loaded_belt_when_full_left_lane() {
        let mut app = test_app();
        app.add_belt((-1, 0, 0), HDir::North);
        let side_loading = app.add_belt((0, 0, -1), HDir::East);
        let side_loaded = app.add_belt((0, 0, 0), HDir::North);
        app.update();
        for i in 0..ITEMS_PER_BELT {
            app.add_item(side_loaded, i * ITEM_SPACING, LaneSide::Left);
        }
        let item = app.add_item(side_loading, 0, LaneSide::Left);

        for _ in 0..(POSITIONS_PER_FRAGMENT + ITEM_SPACING) / BASE_BELT_SPEED {
            app.update();
        }
        let expected = item_position(
            BeltShape::Fragment(HDir::East),
            (0, 0, 0),
            LaneSide::Left,
            0,
        );
        let actual = app.find_item(item).unwrap().1;
        assert_close(expected.translation, actual.translation);
    }

    #[test]
    fn item_doesnt_move_onto_side_loaded_belt_when_full_right_lane() {
        let mut app = test_app();
        app.add_belt((-1, 0, 0), HDir::North);
        let side_loading = app.add_belt((0, 0, 1), HDir::West);
        let side_loaded = app.add_belt((0, 0, 0), HDir::North);
        app.update();
        for i in 0..ITEMS_PER_BELT {
            app.add_item(side_loaded, i * ITEM_SPACING, LaneSide::Right);
        }
        let item = app.add_item(side_loading, 0, LaneSide::Right);

        for _ in 0..(POSITIONS_PER_FRAGMENT + ITEM_SPACING) / BASE_BELT_SPEED {
            app.update();
        }
        let expected = item_position(
            BeltShape::Fragment(HDir::West),
            (0, 0, 0),
            LaneSide::Right,
            0,
        );
        let actual = app.find_item(item).unwrap().1;
        assert_close(expected.translation, actual.translation);
    }

    #[test]
    fn two_items_at_positions_0_and_64() {
        let mut app = test_app();
        let belt = app.add_belt((-3, 1, 0), HDir::North);
        app.update();

        // Check lane state before adding any items
        {
            let world = app.world_mut();
            let lane = world.query::<&BeltLane>().single(world).unwrap();
            info!("Lane state BEFORE adding items:");
            info!(
                "  Belts: {:?}",
                lane.belts
                    .iter()
                    .map(|b| (b.ranges.clone(), b.lane_offsets.clone()))
                    .collect::<Vec<_>>()
            );
            info!("  Items left: {:?}", lane.lanes.left);
        }

        let item1 = app.add_item(belt, 0, LaneSide::Left);
        app.update();
        info!(
            "After first update, item1 position: {:?}",
            app.find_item(item1).unwrap().1.translation
        );

        // Check lane state after adding item1
        {
            let world = app.world_mut();
            let lane = world.query::<&BeltLane>().single(world).unwrap();
            info!("Lane state AFTER adding item1:");
            info!(
                "  Belts: {:?}",
                lane.belts
                    .iter()
                    .map(|b| (b.ranges.clone(), b.lane_offsets.clone()))
                    .collect::<Vec<_>>()
            );
            info!("  Items left: {:?}", lane.lanes.left);
        }

        let item2 = app.add_item(belt, 64, LaneSide::Left);

        // Check lane state after adding item2 but before update
        {
            let world = app.world_mut();
            let lane = world.query::<&BeltLane>().single(world).unwrap();
            info!("Lane state AFTER adding item2 (before update):");
            info!(
                "  Belts: {:?}",
                lane.belts
                    .iter()
                    .map(|b| (b.ranges.clone(), b.lane_offsets.clone()))
                    .collect::<Vec<_>>()
            );
            info!("  Items left: {:?}", lane.lanes.left);
        }

        app.update();
        info!(
            "After second update, item1: {:?}, item2: {:?}",
            app.find_item(item1).unwrap().1.translation,
            app.find_item(item2).unwrap().1.translation
        );

        // Check the lane state
        let world = app.world_mut();
        let lane = world.query::<&BeltLane>().single(world).unwrap();
        info!("Lane state AFTER second update:");
        info!(
            "  Belts: {:?}",
            lane.belts
                .iter()
                .map(|b| (b.ranges.clone(), b.lane_offsets.clone()))
                .collect::<Vec<_>>()
        );
        info!("  Items left: {:?}", lane.lanes.left);

        let (_, actual) = app.find_item(item2).unwrap();

        // Item2 should stay at 64 distance from item1 (which is at 0)
        // Since 64 equals ITEM_SPACING, item2 shouldn't move closer
        let expected_transform = item_position(
            BeltShape::Straight(HDir::North),
            WorldCoords { x: -3, y: 1, z: 0 },
            LaneSide::Left,
            64,
        );
        assert_eq!(actual, expected_transform);
    }
}
