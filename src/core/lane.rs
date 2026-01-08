use super::*;

// ------
// Models
// ------

#[derive(Component, Debug, PartialEq, Eq)]
pub struct BeltLane {
    pub belts: Vec<BeltEntry>,
    pub left_items: Vec<ItemEntry>,
    pub right_items: Vec<ItemEntry>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BeltEntry {
    pub belt: BeltShape,
    pub coords: WorldCoords,
    pub entity: Entity,
    pub left_range: Range<i32>,
    pub right_range: Range<i32>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
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

//
// Model impls
//

impl BeltLane {
    pub fn from_belt(belt: BeltShape, coords: WorldCoords, entity: Entity) -> Self {
        Self {
            belts: vec![BeltEntry {
                belt,
                coords,
                entity,
                left_range: 0..belt.left_num_pos(),
                right_range: 0..belt.right_num_pos(),
            }],
            left_items: vec![],
            right_items: vec![],
        }
    }

    pub fn add_to_tail(&mut self, shape: BeltShape, coords: WorldCoords, entity: Entity) {
        let last = self.belts.last().unwrap();
        let left_end = last.left_range.end;
        let right_end = last.right_range.end;
        self.belts.push(BeltEntry {
            belt: shape,
            coords,
            entity,
            left_range: left_end..left_end + shape.left_num_pos(),
            right_range: right_end..right_end + shape.right_num_pos(),
        });
    }

    /// The pos in the `ItemEntry` is relative to the start of the belt, not the lane
    pub fn add_item(&mut self, item: ItemEntry, lane: LaneSide, belt: Entity) -> Option<()> {
        let entry = self.belts.iter().find(|b| b.entity == belt)?;
        match lane {
            LaneSide::Left => {
                let offset = entry.left_range.start;
                self.left_items.push(ItemEntry {
                    pos: offset + item.pos,
                    ..item
                });
                Some(())
            }
            LaneSide::Right => {
                let offset = entry.right_range.start;
                self.right_items.push(ItemEntry {
                    pos: offset + item.pos,
                    ..item
                });
                Some(())
            }
        }
    }

    pub fn item_iter<'a>(
        &'a self,
    ) -> impl Iterator<Item = (Item, i32, BeltShape, LaneSide, WorldCoords)> + 'a {
        let belt_entry = &self.belts[0];
        let belt = belt_entry.belt;
        let coords = belt_entry.coords;

        let left_items = self
            .left_items
            .iter()
            .map(move |entry| (entry.item, entry.pos, belt, LaneSide::Left, coords));

        let right_items = self
            .right_items
            .iter()
            .map(move |entry| (entry.item, entry.pos, belt, LaneSide::Right, coords));

        left_items.chain(right_items)
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
        lane.add_to_tail(BeltShape::Straight(HDir::North), (-1, 0, 0).into(), tail);
        let expected = BeltLane {
            belts: vec![
                BeltEntry {
                    belt: BeltShape::Straight(HDir::North),
                    coords: (0, 0, 0).into(),
                    entity,
                    left_range: 0..POSITIONS_PER_BELT,
                    right_range: 0..POSITIONS_PER_BELT,
                },
                BeltEntry {
                    belt: BeltShape::Straight(HDir::North),
                    coords: (-1, 0, 0).into(),
                    entity: tail,
                    left_range: POSITIONS_PER_BELT..(2 * POSITIONS_PER_BELT),
                    right_range: POSITIONS_PER_BELT..(2 * POSITIONS_PER_BELT),
                },
            ],
            left_items: vec![],
            right_items: vec![],
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
        );
        let actual = lane.left_items[0];
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
        );
        let actual = lane.left_items[0];
        let expected = ItemEntry {
            pos: POSITIONS_PER_BELT,
            item: Item(0),
            entity: Entity::from_raw_u32(10).unwrap(),
        };
        assert_eq!(actual, expected);
    }
}
