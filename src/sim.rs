use bevy::prelude::*;

use crate::core::*;

pub struct SimPlugin;
impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                // determine_sideload_blocks,
                // determine_double_sideload_blocks,
                // determine_double_sideload_contention,
                transfers, plan_moves, do_moves,
            )
                .chain()
                .after(link_belts),
        );
    }
}

fn plan_moves(mut lanes: Query<&mut BeltLane>) {
    for mut lane in lanes.iter_mut() {
        lane.tick();
    }
}

fn do_moves(mut items: Query<&mut Transform, With<Item>>, lanes: Query<&BeltLane>) {
    for lane in lanes.iter() {
        for side in SIDES {
            for item_entry in &lane.lanes[side] {
                let belt_entry = lane.belt_for(item_entry.pos, side).unwrap();
                let relative_pos = item_entry.pos - belt_entry.ranges[side].start;
                let transform =
                    item_position(belt_entry.belt, belt_entry.coords, side, relative_pos);

                let mut t = items.get_mut(item_entry.entity).unwrap();
                *t = transform;
            }
        }
    }
}

fn transfers(
    conns: Query<(Entity, &LaneConnection)>,
    loop_conns: Query<(Entity, &LaneLoopConnection)>,
    double_conns: Query<(Entity, &DoubleBeltConnection)>,
    mut lanes: Query<&mut BeltLane>,
) {
    // Process DoubleBeltConnections FIRST (handles both sideloading lanes)
    for (first_lane_ent, conn) in double_conns.iter() {
        debug!("processing double connection");

        // Try to transfer from the first lane (the one with DoubleBeltConnection)
        let first_lane = lanes.get(first_lane_ent).unwrap();
        if !first_lane.is_blocked {
            // Check both sides of the first lane
            for side in SIDES {
                if let Some(item_entry) = first_lane.lanes[side].first().copied() {
                    if item_entry.pos < BASE_BELT_SPEED {
                        debug!(
                            "double conn transferring from first lane {:?}, pos: {}",
                            side, item_entry.pos
                        );
                        let mut source_lane = lanes.get_mut(first_lane_ent).unwrap();
                        source_lane.lanes[side].remove(0);
                        let mut target_lane = lanes.get_mut(conn.target).unwrap();
                        target_lane.insert_items_at(
                            &[ItemEntry {
                                pos: conn.offset,
                                ..item_entry
                            }],
                            side,
                        );
                        break; // Only transfer one item per frame
                    }
                }
            }
        }

        // Try to transfer from the second lane (other_lane, has no connection component)
        let second_lane_ent = conn.other_lane;
        let second_lane = lanes.get(second_lane_ent).unwrap();
        if !second_lane.is_blocked {
            for side in SIDES {
                if let Some(item_entry) = second_lane.lanes[side].first().copied() {
                    if item_entry.pos < BASE_BELT_SPEED {
                        debug!(
                            "double conn transferring from second lane {:?}, pos: {}",
                            side, item_entry.pos
                        );
                        let mut source_lane = lanes.get_mut(second_lane_ent).unwrap();
                        source_lane.lanes[side].remove(0);
                        let mut target_lane = lanes.get_mut(conn.target).unwrap();
                        target_lane.insert_items_at(
                            &[ItemEntry {
                                pos: conn.offset,
                                ..item_entry
                            }],
                            side,
                        );
                        break; // Only transfer one item per frame
                    }
                }
            }
        }
    }

    // Process loop connections
    for (lane_ent, loop_conn) in loop_conns.iter() {
        debug!("processing loop connection");
        let mut lane = lanes.get_mut(lane_ent).unwrap();

        // Process left lane
        if let Some(item) = lane.lanes[LaneSide::Left].first_mut() {
            if item.pos < BASE_BELT_SPEED {
                item.pos += loop_conn.left_offset;
                lane.lanes[LaneSide::Left].sort();
            }
        }

        // Process right lane
        if let Some(item) = lane.lanes[LaneSide::Right].first_mut() {
            if item.pos < BASE_BELT_SPEED {
                item.pos += loop_conn.right_offset;
                lane.lanes[LaneSide::Right].sort();
            }
        }
    }

    // Process regular connections
    for (source_ent, conn) in conns.iter().filter(|(ent, c)| *ent != c.target) {
        debug!("processing connection");
        let mut source_lane = lanes.get_mut(source_ent).unwrap();
        if source_lane.is_blocked {
            debug!("connection blocked, skipping transfer");
            continue;
        }

        // Check the specified side for items to transfer
        let side = conn.side;
        if let Some(item_entry) = source_lane.lanes[side].first().copied() {
            debug!("item at pos: {}", item_entry.pos);
            if item_entry.pos < BASE_BELT_SPEED {
                source_lane.lanes[side].remove(0);
                let mut target_lane = lanes.get_mut(conn.target).unwrap();
                target_lane.insert_items_at(
                    &[ItemEntry {
                        pos: conn.offset,
                        ..item_entry
                    }],
                    side,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AppExtension, LaneSide, test_app};

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
            WorldCoords { x: 0, y: 0, z: 1 },
            LaneSide::Left,
            POSITIONS_PER_BELT - BASE_BELT_SPEED,
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
            WorldCoords { x: 0, y: 0, z: 1 },
            LaneSide::Left,
            POSITIONS_PER_BELT - BASE_BELT_SPEED,
        );
        assert_eq!(actual, expected_transform);
    }

    #[test]
    fn item_moves_on_merged_lanes() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0, 0), HDir::East);
        let _belt2 = app.add_belt((0, 0, 2), HDir::East);
        app.update();
        let _belt3 = app.add_belt((0, 0, 1), HDir::East);
        let item = app.add_item(belt1, 0, LaneSide::Left);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();

        // Item should transfer to belt3 (middle belt) and move
        let expected_transform = item_position(
            BeltShape::Straight(HDir::East),
            WorldCoords { x: 0, y: 0, z: 1 },
            LaneSide::Left,
            POSITIONS_PER_BELT - BASE_BELT_SPEED,
        );
        assert_eq!(actual, expected_transform);
    }

    #[test]
    fn handles_items_too_close_together() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0, 0), HDir::East);
        app.update();
        app.add_item(belt1, 0, LaneSide::Left);
        let item = app.add_item(belt1, 1, LaneSide::Left);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();

        // Item at position 1 should be pushed back to maintain ITEM_SPACING from item at 0
        let expected_transform = item_position(
            BeltShape::Straight(HDir::East),
            WorldCoords { x: 0, y: 0, z: 0 },
            LaneSide::Left,
            ITEM_SPACING,
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
        let item = app.add_item(belt, 0, LaneSide::Left);
        app.update();
        let mut prev_pos = app.find_item(item).unwrap().1.translation;
        app.update();
        // Loop through multiple cycles to verify item keeps moving
        for _ in 0..(POSITIONS_PER_CURVED_BELT * 4 / BASE_BELT_SPEED + BASE_BELT_SPEED) {
            let pos = app.find_item(item).unwrap().1.translation;
            assert_ne!(pos, prev_pos, "Item should keep moving in loop");
            prev_pos = pos;
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
        let belt1 = app.add_belt((0, 0, 0), HDir::East);
        let belt2 = app.add_belt((1, 0, 0), HDir::East);
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
        app.add_belt((2, 0, 0), HDir::East);

        app.update();
        let actual = dist(&mut app, first_item, last_item);
        assert_eq!(actual, expected);
    }
}
