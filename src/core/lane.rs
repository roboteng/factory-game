use super::*;

// ------
// Models
// ------

#[derive(Component, Debug, PartialEq, Eq, Clone)]
pub struct BeltLane {
    pub belts: Vec<BeltEntry>,
    pub lanes: Sided<Vec<ItemEntry>>,
    pub is_blocked: Sided<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemPlacementError {
    BeltNotFound,
    #[expect(unused)]
    PositionOutOfBounds,
    PositionOccupied,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BeltEntry {
    pub belt: BeltShape,
    pub coords: WorldCoords,
    pub entity: Entity,
    pub ranges: Sided<Range<i32>>,
    pub lane_offsets: Sided<i32>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct ItemEntry {
    pub pos: i32,
    pub item: Item,
    pub entity: Entity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneSide {
    Left,
    Right,
}
use LaneSide::{Left, Right};
pub const SIDES: [LaneSide; 2] = [Left, Right];

// -----------
// Model impls
// -----------

impl BeltLane {
    pub fn from_belt(belt: BeltShape, coords: WorldCoords, entity: Entity) -> Self {
        Self {
            belts: vec![BeltEntry {
                belt,
                coords,
                entity,
                ranges: Sided {
                    left: 0..(belt.left_num_pos() - ITEM_SPACING / 2),
                    right: 0..belt.right_num_pos() - ITEM_SPACING / 2,
                },
                lane_offsets: Sided { left: 0, right: 0 },
            }],
            lanes: default(),
            is_blocked: Sided {
                left: false,
                right: false,
            },
        }
    }

    pub fn add_to_head(&mut self, belt: BeltShape, coords: WorldCoords, entity: Entity) {
        let left_offset = belt.left_num_pos();
        let right_offset = belt.right_num_pos();
        let prev_head = &self.belts[0];
        let new_left_start = prev_head.ranges.left.start - left_offset;
        let new_right_start = prev_head.ranges.right.start - right_offset;
        for side in SIDES {
            self.belts[0].ranges[side].start -= ITEM_SPACING / 2;
        }
        self.belts.insert(
            0,
            BeltEntry {
                belt,
                coords,
                entity,
                ranges: Sided {
                    left: (new_left_start)..(self.belts[0].ranges.left.start),
                    right: (new_right_start)..(self.belts[0].ranges.right.start),
                },
                lane_offsets: Sided {
                    left: new_left_start,
                    right: new_right_start,
                },
            },
        );
    }

    pub fn merge(&mut self, mut other: BeltLane) {
        other.belts[0].ranges[Left].start -= ITEM_SPACING / 2;
        other.belts[0].ranges[Right].start -= ITEM_SPACING / 2;
        let ranges = self.ranges();

        other.add_offsets_to_head(
            ranges[Left].end - other.belts[0].ranges[Left].start,
            ranges[Right].end - other.belts[0].ranges[Right].start,
        );
        self.belts.extend(other.belts);
        self.lanes.left.extend(other.lanes.left);
        self.lanes.right.extend(other.lanes.right);
    }

    /// Returns (left, right)
    pub fn ranges(&self) -> Sided<Range<i32>> {
        let left_start = self
            .belts
            .first()
            .expect("Invariant broken: all_lanes_have_belts")
            .ranges
            .left
            .start;
        let left_end = self
            .belts
            .last()
            .expect("Invariant broken: all_lanes_have_belts")
            .ranges
            .left
            .end;
        let right_start = self
            .belts
            .first()
            .expect("Invariant broken: all_lanes_have_belts")
            .ranges
            .right
            .start;
        let right_end = self
            .belts
            .last()
            .expect("Invariant broken: all_lanes_have_belts")
            .ranges
            .right
            .end;
        Sided {
            left: left_start..left_end,
            right: right_start..right_end,
        }
    }

    fn add_offsets_to_head(&mut self, left_offset: i32, right_offset: i32) {
        for belt in self.belts.iter_mut() {
            belt.ranges.left.start += left_offset;
            belt.ranges.left.end += left_offset;
            belt.ranges.right.start += right_offset;
            belt.ranges.right.end += right_offset;
            belt.lane_offsets.left += left_offset;
            belt.lane_offsets.right += right_offset;
        }
        for items in self.lanes.left.iter_mut() {
            items.pos += left_offset;
        }
        for items in self.lanes.right.iter_mut() {
            items.pos += right_offset;
        }
    }

    /// The pos in the `ItemEntry` is relative to the belt, not the lane
    pub fn add_item(
        &mut self,
        item: ItemEntry,
        lane: LaneSide,
        belt: Entity,
    ) -> Result<(), ItemPlacementError> {
        let entry = self
            .belts
            .iter()
            .find(|b| b.entity == belt)
            .ok_or(ItemPlacementError::BeltNotFound)?;
        let offset = entry.lane_offsets[lane];
        let new_pos =
            (offset + item.pos).clamp(entry.ranges[lane].start, entry.ranges[lane].end - 1);

        // Check if position is already occupied
        if self.lanes[lane]
            .iter()
            .any(|existing| (existing.pos - new_pos).abs() < ITEM_SPACING)
        {
            return Err(ItemPlacementError::PositionOccupied);
        }

        self.lanes[lane].push(ItemEntry {
            pos: new_pos,
            ..item
        });
        self.lanes[lane].sort();
        Ok(())
    }

    #[expect(dead_code)]
    pub fn item_iter<'a>(
        &'a self,
    ) -> impl Iterator<Item = (Item, i32, BeltShape, LaneSide, WorldCoords, Entity)> + 'a {
        let left_items = self.lanes[Left].iter().map(move |entry| {
            let belt_entry = self
                .belt_for(entry.pos, LaneSide::Left)
                .expect("Invariant broken: items_are_within_belt_bounds");
            let relative_pos = entry.pos - belt_entry.lane_offsets.left;
            (
                entry.item,
                relative_pos,
                belt_entry.belt,
                LaneSide::Left,
                belt_entry.coords,
                entry.entity,
            )
        });

        let right_items = self.lanes[Right].iter().map(move |entry| {
            let belt_entry = self
                .belt_for(entry.pos, LaneSide::Right)
                .expect("Invariant broken: items_are_within_belt_bounds");
            let relative_pos = entry.pos - belt_entry.lane_offsets.right;
            (
                entry.item,
                relative_pos,
                belt_entry.belt,
                LaneSide::Right,
                belt_entry.coords,
                entry.entity,
            )
        });

        left_items.chain(right_items)
    }

    pub fn range_for(&self, belt: Entity) -> Option<Sided<Range<i32>>> {
        self.find_belt(belt).map(|b| b.1.ranges.clone())
    }

    fn find_belt(&self, belt: Entity) -> Option<(usize, &BeltEntry)> {
        self.belts
            .iter()
            .enumerate()
            .find(|(_, b)| b.entity == belt)
    }

    pub fn insert_items_at(&mut self, items: &[ItemEntry], side: LaneSide) {
        for item in items {
            self.lanes[side].push(*item);
        }
        self.lanes[side].sort();
    }

    pub fn belt_for(&self, pos: i32, lane: LaneSide) -> Option<&BeltEntry> {
        self.belts.iter().find(|b| b.ranges[lane].contains(&pos))
    }

    pub fn prepend_fragment(&mut self, output: HDir, coords: WorldCoords, entity: Entity) {
        for side in SIDES {
            self.belts[0].ranges[side].start -= ITEM_SPACING / 2;
        }

        let head = &self.belts[0];
        let left_start = head.ranges.left.start;
        let right_start = head.ranges.right.start;
        self.belts.insert(
            0,
            BeltEntry {
                belt: BeltShape::Fragment(output),
                coords,
                entity,
                ranges: Sided {
                    left: (left_start - POSITIONS_PER_FRAGMENT)..left_start,
                    right: (right_start - POSITIONS_PER_FRAGMENT)..right_start,
                },
                lane_offsets: Sided {
                    left: left_start - POSITIONS_PER_FRAGMENT + ITEM_SPACING,
                    right: right_start - POSITIONS_PER_FRAGMENT + ITEM_SPACING,
                },
            },
        );
    }

    pub fn remove_head(&mut self) -> (Vec<ItemEntry>, Vec<ItemEntry>) {
        assert!(
            self.belts.len() >= 2,
            "We should check if we need to remove the entier lane"
        );
        let head = self.belts.remove(0);

        // Process left lane
        // TODO: include items that are close to the boundary
        // TODO: change item's pos to be zero based
        let part = self.lanes[Left].partition_point(|item| head.ranges[Left].contains(&item.pos));
        let (head_items, keep_items) = self.lanes[Left].split_at_mut(part);
        let keep = Vec::from(keep_items);
        let left = Vec::from_iter(head_items.iter().cloned());
        self.lanes[Left] = keep;

        // Process right lane
        let part = self.lanes[Right].partition_point(|item| head.ranges[Right].contains(&item.pos));
        let (head_items, keep_items) = self.lanes[Right].split_at_mut(part);
        let keep = Vec::from(keep_items);
        let right = Vec::from_iter(head_items.iter().cloned());
        self.lanes[Right] = keep;

        for side in SIDES {
            self.belts[0].ranges[side].start += ITEM_SPACING / 2;
        }

        (left, right)
    }

    pub fn remove_tail(&mut self) -> (Vec<ItemEntry>, Vec<ItemEntry>) {
        assert!(
            self.belts.len() >= 2,
            "We should check if we need to remove the entier lane"
        );

        let last = self.belts.len();
        let tail = self.belts.remove(last - 1);

        let part = self.lanes[Left].partition_point(|item| !tail.ranges[Left].contains(&item.pos));
        let (keep_items, tail_items) = self.lanes[Left].split_at_mut(part);
        let keep = Vec::from(keep_items);
        let left = Vec::from_iter(tail_items.iter().cloned());
        self.lanes[Left] = keep;

        let part =
            self.lanes[Right].partition_point(|item| !tail.ranges[Right].contains(&item.pos));
        let (keep_items, tail_items) = self.lanes[Right].split_at_mut(part);
        let keep = Vec::from(keep_items);
        let right = Vec::from_iter(tail_items.iter().cloned());
        self.lanes[Right] = keep;

        (left, right)
    }

    pub fn is_blocking_at(&self, offset: i32, lane: LaneSide) -> bool {
        debug!("Checking if lane blocked at {}", offset);
        self.lanes[lane]
            .iter()
            .any(|item| item.pos >= offset - ITEM_SPACING && item.pos < offset)
    }

    pub fn is_blocked_for_side(&self, side: LaneSide) -> bool {
        self.is_blocked[side]
    }

    /// Update item positions for one simulation tick
    pub fn tick(&mut self) {
        for side in SIDES {
            let head = self.belts[0].ranges[side].start;
            let tail = self
                .belts
                .last()
                .expect("Invariant broken: all_lanes_have_belts")
                .ranges[side]
                .end
                - 1;
            let Some(lead_item) = self.lanes[side].get_mut(0) else {
                continue;
            };
            let offset = if self.is_blocked[side] && self.belts[0].belt.is_fragment() {
                ITEM_SPACING
            } else {
                0
            };
            lead_item.pos = (head + offset).max(lead_item.pos - BASE_BELT_SPEED);
            for i in 1..self.lanes[side].len() {
                let first = self.lanes[side][i - 1];
                let second = &mut self.lanes[side][i];

                second.pos =
                    ((first.pos + ITEM_SPACING).max(second.pos - BASE_BELT_SPEED)).min(tail);
            }
        }
    }

    pub fn split_at(&mut self, next_head: Entity) -> Option<Self> {
        let (index, belt) = self.find_belt(next_head)?;
        let belt = belt.clone();
        let Self { belts, .. } = self;
        let new_belts = belts.split_off(index);

        let b = belts.last_mut().unwrap();
        b.ranges.left.end -= ITEM_SPACING / 2;
        b.ranges.right.end -= ITEM_SPACING / 2;

        let left_split = self
            .lanes
            .left
            .partition_point(|i| i.pos < belt.ranges.left.start - ITEM_SPACING / 2);
        let left_items = self.lanes.left.split_off(left_split);

        let right_split = self
            .lanes
            .right
            .partition_point(|i| i.pos < belt.ranges.right.start - ITEM_SPACING / 2);
        let right_items = self.lanes.right.split_off(right_split);

        Some(Self {
            belts: new_belts,
            lanes: Sided {
                left: left_items,
                right: right_items,
            },
            is_blocked: Sided {
                left: false,
                right: false,
            },
        })
    }
}

// -----------
// Trait impls
// -----------

impl From<PlaceItem> for ItemEntry {
    fn from(value: PlaceItem) -> Self {
        Self {
            item: value.item,
            entity: value.entity,
            pos: value.position,
        }
    }
}

// ---------
// Functions
// ---------

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn lane_add_to_tail() {
        init_tracing();
        let entity = Entity::from_raw_u32(0).unwrap();
        let mut lane =
            BeltLane::from_belt(BeltShape::Straight(HDir::North), (0, 0, 0).into(), entity);
        let tail = Entity::from_raw_u32(0).unwrap();
        lane.merge(BeltLane::from_belt(
            BeltShape::Straight(HDir::North),
            (-1, 0, 0).into(),
            tail,
        ));
        let expected = BeltLane {
            belts: vec![
                BeltEntry {
                    belt: BeltShape::Straight(HDir::North),
                    coords: (0, 0, 0).into(),
                    entity,
                    ranges: Sided {
                        left: 0..POSITIONS_PER_BELT - ITEM_SPACING / 2,
                        right: 0..POSITIONS_PER_BELT - ITEM_SPACING / 2,
                    },
                    lane_offsets: Sided { left: 0, right: 0 },
                },
                BeltEntry {
                    belt: BeltShape::Straight(HDir::North),
                    coords: (-1, 0, 0).into(),
                    entity: tail,
                    ranges: Sided {
                        left: POSITIONS_PER_BELT - ITEM_SPACING / 2
                            ..(2 * POSITIONS_PER_BELT) - ITEM_SPACING / 2,
                        right: POSITIONS_PER_BELT - ITEM_SPACING / 2
                            ..(2 * POSITIONS_PER_BELT) - ITEM_SPACING / 2,
                    },
                    lane_offsets: Sided {
                        left: POSITIONS_PER_BELT,
                        right: POSITIONS_PER_BELT,
                    },
                },
            ],
            lanes: default(),
            is_blocked: Sided {
                left: false,
                right: false,
            },
        };
        assert_eq!(lane, expected);
    }

    #[test]
    fn item_on_lane() {
        init_tracing();
        let belt_ent = Entity::from_raw_u32(0).unwrap();
        let mut lane =
            BeltLane::from_belt(BeltShape::Straight(HDir::North), (0, 0, 0).into(), belt_ent);
        lane.add_item(
            ItemEntry {
                pos: 0,
                item: Item::Belt,
                entity: Entity::from_raw_u32(10).unwrap(),
            },
            LaneSide::Left,
            belt_ent,
        )
        .unwrap();
        let actual = lane.lanes[Left][0];
        let expected = ItemEntry {
            pos: 0,
            item: Item::Belt,
            entity: Entity::from_raw_u32(10).unwrap(),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_on_lane_next_belt() {
        init_tracing();
        let belt_ent_1 = Entity::from_raw_u32(0).unwrap();
        let belt_ent_2 = Entity::from_raw_u32(1).unwrap();
        let mut lane = BeltLane::from_belt(
            BeltShape::Straight(HDir::North),
            (0, 0, 0).into(),
            belt_ent_1,
        );
        lane.merge(BeltLane::from_belt(
            BeltShape::Straight(HDir::North),
            (-1, 0, 0).into(),
            belt_ent_2,
        ));

        lane.add_item(
            ItemEntry {
                pos: 0,
                item: Item::Belt,
                entity: Entity::from_raw_u32(10).unwrap(),
            },
            LaneSide::Left,
            belt_ent_2,
        )
        .unwrap();
        let actual = lane.lanes[Left][0];
        let expected = ItemEntry {
            pos: POSITIONS_PER_BELT,
            item: Item::Belt,
            entity: Entity::from_raw_u32(10).unwrap(),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn tick_on_single_belt_at_head() {
        init_tracing();
        let belt_ent = Entity::from_raw_u32(0).unwrap();
        let mut lane =
            BeltLane::from_belt(BeltShape::Straight(HDir::North), (0, 0, 0).into(), belt_ent);
        lane.add_item(
            ItemEntry {
                pos: 0,
                item: Item::Belt,
                entity: Entity::from_raw_u32(10).unwrap(),
            },
            LaneSide::Left,
            belt_ent,
        )
        .unwrap();
        lane.tick();
        let actual = lane.lanes[Left][0];
        let expected = ItemEntry {
            pos: 0,
            item: Item::Belt,
            entity: Entity::from_raw_u32(10).unwrap(),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn tick_on_single_belt_farther_back() {
        init_tracing();
        let belt_ent = Entity::from_raw_u32(0).unwrap();
        let mut lane =
            BeltLane::from_belt(BeltShape::Straight(HDir::North), (0, 0, 0).into(), belt_ent);
        lane.add_item(
            ItemEntry {
                pos: BASE_BELT_SPEED * 2,
                item: Item::Belt,
                entity: Entity::from_raw_u32(10).unwrap(),
            },
            LaneSide::Left,
            belt_ent,
        )
        .unwrap();
        lane.tick();
        let actual = lane.lanes[Left][0];
        let expected = ItemEntry {
            pos: BASE_BELT_SPEED,
            item: Item::Belt,
            entity: Entity::from_raw_u32(10).unwrap(),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn tick_with_head_belt() {
        init_tracing();
        let belt_ent = Entity::from_raw_u32(0).unwrap();
        let mut lane =
            BeltLane::from_belt(BeltShape::Straight(HDir::North), (0, 0, 0).into(), belt_ent);
        lane.add_item(
            ItemEntry {
                pos: 0,
                item: Item::Belt,
                entity: Entity::from_raw_u32(10).unwrap(),
            },
            LaneSide::Left,
            belt_ent,
        )
        .unwrap();
        let head_belt = Entity::from_raw_u32(1).unwrap();
        lane.add_to_head(
            BeltShape::Straight(HDir::North),
            (1, 0, 0).into(),
            head_belt,
        );
        lane.tick();
        let actual = lane.lanes[Left][0];
        let expected = ItemEntry {
            pos: -BASE_BELT_SPEED,
            item: Item::Belt,
            entity: Entity::from_raw_u32(10).unwrap(),
        };
        assert_eq!(actual, expected);
    }
}
