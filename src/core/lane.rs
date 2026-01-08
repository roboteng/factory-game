use std::ops::{Index, IndexMut};

use super::*;

// ------
// Models
// ------

#[derive(Component, Debug, PartialEq, Eq, Clone)]
pub struct BeltLane {
    pub belts: Vec<BeltEntry>,
    pub lanes: Lanes,
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
pub struct BeltEntry {
    pub belt: BeltShape,
    pub coords: WorldCoords,
    pub entity: Entity,
    pub ranges: Ranges,
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
                    left: 0..belt.left_num_pos(),
                    right: 0..belt.right_num_pos(),
                },
            }],
            lanes: default(),
        }
    }

    pub fn add_to_tail(&mut self, shape: BeltShape, coords: WorldCoords, entity: Entity) {
        let last = self.belts.last().unwrap();
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
        });
    }

    pub fn add_to_head(&mut self, belt: BeltShape, coords: WorldCoords, entity: Entity) {
        let left_offset = belt.left_num_pos();
        let right_offset = belt.right_num_pos();
        self.add_offsets_to_head(left_offset, right_offset);
        self.belts.insert(
            0,
            BeltEntry {
                belt,
                coords,
                entity,
                ranges: Ranges {
                    left: 0..left_offset,
                    right: 0..right_offset,
                },
            },
        );
    }

    pub fn merge(&mut self, mut other: BeltLane) {
        let (left, right) = self.lengths();
        other.add_offsets_to_head(left, right);
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
    pub fn lengths(&self) -> (i32, i32) {
        let left = self.belts.last().unwrap().ranges.left.end;
        let right = self.belts.last().unwrap().ranges.right.end;
        (left, right)
    }

    fn add_offsets_to_head(&mut self, left_offset: i32, right_offset: i32) {
        for belt in self.belts.iter_mut() {
            belt.ranges.left.start += left_offset;
            belt.ranges.right.start += right_offset;
        }
        for items in self.lanes.left.iter_mut() {
            items.pos += left_offset;
        }
        for items in self.lanes.right.iter_mut() {
            items.pos += right_offset;
        }
    }

    /// The pos in the `ItemEntry` is relative to the start of the belt, not the lane
    pub fn add_item(&mut self, item: ItemEntry, lane: LaneSide, belt: Entity) -> Result<(), ()> {
        let entry = self.belts.iter().find(|b| b.entity == belt).ok_or(())?;
        let offset = entry.ranges[lane].start;
        self.lanes[lane].push(ItemEntry {
            pos: offset + item.pos,
            ..item
        });
        Ok(())
    }

    pub fn item_iter<'a>(
        &'a self,
    ) -> impl Iterator<Item = (Item, i32, BeltShape, LaneSide, WorldCoords)> + 'a {
        let belt_entry = &self.belts[0];
        let belt = belt_entry.belt;
        let coords = belt_entry.coords;

        let left_items = self.lanes[Left]
            .iter()
            .map(move |entry| (entry.item, entry.pos, belt, LaneSide::Left, coords));

        let right_items = self.lanes[Right]
            .iter()
            .map(move |entry| (entry.item, entry.pos, belt, LaneSide::Right, coords));

        left_items.chain(right_items)
    }

    pub fn range_for(&self, belt: Entity) -> Option<Ranges> {
        todo!()
    }

    pub fn insert_item_at(&mut self, pos: i32, item: Entity, lane: LaneSide) {
        todo!()
    }

    pub fn insert_items_at(&mut self, items: &[ItemEntry], side: LaneSide) {
        for item in items {
            self.lanes[side].push(*item);
        }
        self.lanes[side].sort();
    }

    pub fn belt_for(&self, pos: i32, lane: LaneSide) -> Option<Entity> {
        todo!()
    }

    pub fn num_positions(&self, lane: LaneSide) -> i32 {
        todo!()
    }

    pub fn relative_pos(&self, pos: i32, lane: LaneSide) -> i32 {
        todo!()
    }

    pub fn prepend_fragment(&mut self, belt: BeltShape, coords: WorldCoords, entity: Entity) {
        todo!()
    }

    fn shorten_by(&mut self, left_len: i32, right_len: i32) {
        todo!()
    }

    pub fn remove_head(&mut self) -> (Vec<ItemEntry>, Vec<ItemEntry>) {
        todo!()
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

    pub fn is_blocked_at(&self, offset: i32, lane: LaneSide) -> bool {
        todo!()
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
                        left: 0..POSITIONS_PER_BELT,
                        right: 0..POSITIONS_PER_BELT,
                    },
                },
                BeltEntry {
                    belt: BeltShape::Straight(HDir::North),
                    coords: (-1, 0, 0).into(),
                    entity: tail,
                    ranges: Ranges {
                        left: POSITIONS_PER_BELT..(2 * POSITIONS_PER_BELT),
                        right: POSITIONS_PER_BELT..(2 * POSITIONS_PER_BELT),
                    },
                },
            ],
            lanes: Lanes {
                left: vec![],
                right: vec![],
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
}
