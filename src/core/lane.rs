use std::ops::{Index, IndexMut};

use super::*;

// ------
// Models
// ------

#[derive(Component, Debug, PartialEq, Eq, Clone)]
pub struct BeltLane {
    pub belts: Vec<BeltEntry>,
    pub lanes: Lanes,
    pub is_blocked_left: bool,
    pub is_blocked_right: bool,
}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct Lanes {
    pub left: Vec<ItemEntry>,
    pub right: Vec<ItemEntry>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Ranges {
    pub left: Range<i32>,
    pub right: Range<i32>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LaneOffsets {
    pub left: i32,
    pub right: i32,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Offset {
    pub left: i32,
    pub right: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemPlacementError {
    BeltNotFound,
    PositionOutOfBounds,
    PositionOccupied,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BeltEntry {
    pub belt: BeltShape,
    pub coords: WorldCoords,
    pub entity: Entity,
    pub ranges: Ranges,
    pub lane_offsets: LaneOffsets,
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
                ranges: Ranges {
                    left: 0..(belt.left_num_pos() - ITEM_SPACING / 2),
                    right: 0..belt.right_num_pos() - ITEM_SPACING / 2,
                },
                lane_offsets: LaneOffsets { left: 0, right: 0 },
            }],
            lanes: default(),
            is_blocked_left: false,
            is_blocked_right: false,
        }
    }

    pub fn add_to_tail(&mut self, shape: BeltShape, coords: WorldCoords, entity: Entity) {
        let last = self
            .belts
            .last()
            .expect("Invariant broken: all_lanes_have_belts");
        let left_end = last.ranges.left.end;
        let right_end = last.ranges.right.end;
        self.belts.push(BeltEntry {
            belt: shape,
            coords,
            entity,
            ranges: Ranges {
                left: left_end..left_end + shape.left_num_pos(),
                right: right_end..right_end + shape.right_num_pos(),
            },
            lane_offsets: LaneOffsets {
                left: left_end + ITEM_SPACING / 2,
                right: right_end + ITEM_SPACING / 2,
            },
        });
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
                ranges: Ranges {
                    left: (new_left_start)..(self.belts[0].ranges.left.start),
                    right: (new_right_start)..(self.belts[0].ranges.right.start),
                },
                lane_offsets: LaneOffsets {
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

    pub fn replace_belt(&mut self, old: Entity, new: Entity) -> Result<(), ()> {
        let b = self.belts.iter_mut().find(|b| b.entity == old).ok_or(())?;
        b.entity = new;
        Ok(())
    }

    /// Returns (left, right)
    pub fn ranges(&self) -> Ranges {
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
        Ranges {
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
            .any(|existing| existing.pos == new_pos)
        {
            return Err(ItemPlacementError::PositionOccupied);
        }

        self.lanes[lane].push(ItemEntry {
            pos: new_pos,
            ..item
        });
        Ok(())
    }

    pub fn item_iter<'a>(
        &'a self,
    ) -> impl Iterator<Item = (Item, i32, BeltShape, LaneSide, WorldCoords)> + 'a {
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
            )
        });

        left_items.chain(right_items)
    }

    pub fn range_for(&self, belt: Entity) -> Option<Ranges> {
        self.belts
            .iter()
            .find(|b| b.entity == belt)
            .map(|b| b.ranges.clone())
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

    pub fn num_positions(&self, lane: LaneSide) -> i32 {
        self.belts.last().map(|b| b.ranges[lane].end).unwrap_or(0)
    }

    pub fn relative_pos(&self, pos: i32, lane: LaneSide) -> i32 {
        self.belts
            .iter()
            .find(|b| b.ranges[lane].contains(&pos))
            .map(|b| pos - b.ranges[lane].start)
            .expect("Invariant broken: items_are_within_belt_bounds")
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
                ranges: Ranges {
                    left: (left_start - POSITIONS_PER_FRAGMENT)..left_start,
                    right: (right_start - POSITIONS_PER_FRAGMENT)..right_start,
                },
                lane_offsets: LaneOffsets {
                    left: left_start - POSITIONS_PER_FRAGMENT + ITEM_SPACING / 2,
                    right: right_start - POSITIONS_PER_FRAGMENT + ITEM_SPACING / 2,
                },
            },
        );
    }

    fn shorten_by(&mut self, left_len: i32, right_len: i32) {
        self.lanes[Left]
            .iter_mut()
            .for_each(|item| item.pos -= left_len);
        self.lanes[Right]
            .iter_mut()
            .for_each(|item| item.pos -= right_len);
        self.belts.iter_mut().for_each(|belt| {
            belt.ranges.left.start -= left_len;
            belt.ranges.left.end -= left_len;
            belt.ranges.right.start -= right_len;
            belt.ranges.right.end -= right_len;
        });
    }

    pub fn remove_head(&mut self) -> (Vec<ItemEntry>, Vec<ItemEntry>) {
        let head = self.belts.remove(0);

        // Process left lane
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

        self.shorten_by(head.ranges.left.end, head.ranges.right.end);
        (left, right)
    }

    pub fn remove_tail(&mut self) -> (Vec<ItemEntry>, Vec<ItemEntry>) {
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
        match side {
            LaneSide::Left => self.is_blocked_left,
            LaneSide::Right => self.is_blocked_right,
        }
    }

    /// Update item positions for one simulation tick
    pub fn tick(&mut self) {
        for side in SIDES {
            let head = self.belts[0].ranges[side].start;
            let Some(mut lead_item) = self.lanes[side].get_mut(0) else {
                continue;
            };
            lead_item.pos = head.max(lead_item.pos - BASE_BELT_SPEED);
            for i in 1..self.lanes[side].len() {
                let first = self.lanes[side][i - 1];
                let second = &mut self.lanes[side][i];

                second.pos = (first.pos + ITEM_SPACING).max(second.pos - BASE_BELT_SPEED);
            }
        }
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

impl Index<LaneSide> for Lanes {
    type Output = Vec<ItemEntry>;

    fn index(&self, index: LaneSide) -> &Self::Output {
        match index {
            LaneSide::Left => &self.left,
            LaneSide::Right => &self.right,
        }
    }
}

impl IndexMut<LaneSide> for Lanes {
    fn index_mut(&mut self, index: LaneSide) -> &mut Self::Output {
        match index {
            LaneSide::Left => &mut self.left,
            LaneSide::Right => &mut self.right,
        }
    }
}

impl Index<LaneSide> for Ranges {
    type Output = Range<i32>;

    fn index(&self, index: LaneSide) -> &Self::Output {
        match index {
            LaneSide::Left => &self.left,
            LaneSide::Right => &self.right,
        }
    }
}

impl IndexMut<LaneSide> for Ranges {
    fn index_mut(&mut self, index: LaneSide) -> &mut Self::Output {
        match index {
            LaneSide::Left => &mut self.left,
            LaneSide::Right => &mut self.right,
        }
    }
}

impl Index<LaneSide> for LaneOffsets {
    type Output = i32;

    fn index(&self, index: LaneSide) -> &Self::Output {
        match index {
            LaneSide::Left => &self.left,
            LaneSide::Right => &self.right,
        }
    }
}

impl IndexMut<LaneSide> for LaneOffsets {
    fn index_mut(&mut self, index: LaneSide) -> &mut Self::Output {
        match index {
            LaneSide::Left => &mut self.left,
            LaneSide::Right => &mut self.right,
        }
    }
}

impl Index<LaneSide> for Offset {
    type Output = i32;

    fn index(&self, index: LaneSide) -> &Self::Output {
        match index {
            LaneSide::Left => &self.left,
            LaneSide::Right => &self.right,
        }
    }
}

impl IndexMut<LaneSide> for Offset {
    fn index_mut(&mut self, index: LaneSide) -> &mut Self::Output {
        match index {
            LaneSide::Left => &mut self.left,
            LaneSide::Right => &mut self.right,
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
        lane.add_to_tail(BeltShape::Straight(HDir::North), (-1, 0, 0).into(), tail);
        let expected = BeltLane {
            belts: vec![
                BeltEntry {
                    belt: BeltShape::Straight(HDir::North),
                    coords: (0, 0, 0).into(),
                    entity,
                    ranges: Ranges {
                        left: 0..POSITIONS_PER_BELT - ITEM_SPACING / 2,
                        right: 0..POSITIONS_PER_BELT - ITEM_SPACING / 2,
                    },
                    lane_offsets: LaneOffsets { left: 0, right: 0 },
                },
                BeltEntry {
                    belt: BeltShape::Straight(HDir::North),
                    coords: (-1, 0, 0).into(),
                    entity: tail,
                    ranges: Ranges {
                        left: POSITIONS_PER_BELT - ITEM_SPACING / 2
                            ..(2 * POSITIONS_PER_BELT) - ITEM_SPACING / 2,
                        right: POSITIONS_PER_BELT - ITEM_SPACING / 2
                            ..(2 * POSITIONS_PER_BELT) - ITEM_SPACING / 2,
                    },
                    lane_offsets: LaneOffsets {
                        left: POSITIONS_PER_BELT,
                        right: POSITIONS_PER_BELT,
                    },
                },
            ],
            lanes: Lanes {
                left: vec![],
                right: vec![],
            },
            is_blocked_left: false,
            is_blocked_right: false,
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
                item: Item(0),
                entity: Entity::from_raw_u32(10).unwrap(),
            },
            LaneSide::Left,
            belt_ent,
        )
        .unwrap();
        let actual = lane.lanes[Left][0];
        let expected = ItemEntry {
            pos: 0,
            item: Item(0),
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
        lane.add_to_tail(
            BeltShape::Straight(HDir::North),
            (-1, 0, 0).into(),
            belt_ent_2,
        );

        lane.add_item(
            ItemEntry {
                pos: 0,
                item: Item(0),
                entity: Entity::from_raw_u32(10).unwrap(),
            },
            LaneSide::Left,
            belt_ent_2,
        )
        .unwrap();
        let actual = lane.lanes[Left][0];
        let expected = ItemEntry {
            pos: POSITIONS_PER_BELT,
            item: Item(0),
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
                item: Item(0),
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
            item: Item(0),
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
                item: Item(0),
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
            item: Item(0),
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
                item: Item(0),
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
            item: Item(0),
            entity: Entity::from_raw_u32(10).unwrap(),
        };
        assert_eq!(actual, expected);
    }
}
