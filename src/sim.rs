use std::ops::Range;

use crate::core::*;
use bevy::prelude::*;

#[cfg(feature = "invariant-ckeck")]
mod invariants;

#[cfg(all(test, feature = "proptests"))]
mod proptest_actions;
#[cfg(all(test, feature = "proptests"))]
mod proptests;

pub struct SimPlugin;
impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "invariant-ckeck")]
        app.add_plugins(invariants::InvariantsPlugin);
        app.add_observer(on_place_item);
        app.add_systems(
            Update,
            (
                calculate_belt_connections,
                ApplyDeferred,
                plan_moves,
                do_moves,
            )
                .chain(),
        );
    }
}

#[derive(Component, Default, Clone)]
pub struct BeltInventory {
    item: Vec<(u16, Entity)>,
}

impl BeltInventory {
    pub fn add(&mut self, pos: u16, entity: Entity) {
        self.item.push((pos, entity));
    }

    pub fn item_at_head(&self) -> Option<(u16, Entity)> {
        self.item.first().copied()
    }

    pub fn has_space_at_tail(&self, n_pos: u16) -> bool {
        self.item
            .last()
            .is_none_or(|&(pos, _)| pos < n_pos - ITEM_SPACING)
    }

    pub fn remove_first(&mut self) {
        self.item.remove(0);
    }

    pub fn sort(&mut self) {
        self.item.sort();
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
struct BeltLane {
    belts: Belts,
    items: Items,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Belts {
    belts: Vec<(Range<u16>, Entity)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Items {
    items: Vec<(u16, Entity)>,
}

impl BeltLane {
    fn from_belt(entity: Entity, belt: Belt) -> Self {
        let len = belt.num_positions();
        let belts = Belts {
            belts: vec![(0..len, entity)],
        };
        let items = Items { items: vec![] };
        Self { belts, items }
    }

    fn range_for(&self, belt: Entity) -> Option<Range<u16>> {
        self.belts
            .belts
            .iter()
            .find(|(_, id)| *id == belt)
            .map(|(range, _)| range.clone())
    }

    fn insert_item_at(&mut self, pos: u16, item: Entity) {
        self.items.items.push((pos, item));
        self.items.items.sort();
    }

    fn belt_for(&self, pos: u16) -> Option<Entity> {
        self.belts
            .belts
            .iter()
            .find(|(range, _)| range.contains(&pos))
            .map(|(_, id)| *id)
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
struct InLane {
    lane: Entity,
}

impl InLane {
    fn new(lane: Entity) -> Self {
        Self { lane }
    }
}

fn on_place_item(
    trigger: On<PlaceItem>,
    belts: Query<(Entity, &Belt, &InLane)>,
    mut lanes: Query<&mut BeltLane>,
) {
    let belt = belts.get(trigger.belt).unwrap();
    let mut lane = lanes.get_mut(belt.2.lane).unwrap();
    let start = lane.range_for(belt.0).unwrap().start;
    lane.insert_item_at(start + trigger.pos, trigger.entity);
}

enum ConnectionType {
    Direct,
    SideLoad,
}

fn calculate_belt_connections(
    mut cmd: Commands,
    belt_coords: Res<BeltCoords>,
    changed_belts: Res<BeltChanges>,
    query: Query<&InLane>,
    mut lanes: Query<&mut BeltLane>,
) {
    debug!("Updating belts: {:?}", changed_belts.0);
    for change in &changed_belts.0 {
        match change {
            BeltChange::New(new) => {
                let ahead_belt =
                    belt_coords
                        .get(new.coords.step(new.belt.output()))
                        .map(|(entity, ahead)| {
                            if ahead.input() == new.belt.output() {
                                (entity, ahead, Some(ConnectionType::Direct))
                            } else {
                                if ahead.input().opposite() == new.belt.output() {
                                    (entity, ahead, None)
                                } else {
                                    (entity, ahead, Some(ConnectionType::SideLoad))
                                }
                            }
                        });
                let behind_belt = belt_coords
                    .get(new.coords.step(new.belt.input().opposite()))
                    .filter(|behind| behind.1.output() == new.belt.input());

                match (ahead_belt, behind_belt) {
                    (None, None) => {
                        let lane = BeltLane::from_belt(new.entity, new.belt);
                        let lane_ent = cmd.spawn(lane).id();
                        cmd.entity(new.entity).insert(InLane::new(lane_ent));
                    }
                    (None, Some(behind)) => todo!(),
                    (Some(_), None) => todo!(),
                    (Some(_), Some(_)) => todo!(),
                }
            }
            BeltChange::Removed(removed) => {
                todo!();
            }
            BeltChange::Replaced(replaced) => {
                if let Some(old_entity) = replaced.old_entity {
                    todo!();
                }
                todo!();
            }
        }
    }
}

#[derive(Component, Clone, Copy)]
struct BeltFragment {
    dir: Dir,
}

impl BeltFragment {
    fn input(&self) -> Dir {
        self.dir
    }
    fn num_positions(&self) -> u16 {
        POSITIONS_PER_FRAGMENT
    }
    fn item_transform(&self, pos: u16, coords: WorldCoords) -> Transform {
        debug!("Transforming fragment at {:?} with pos {}", coords, pos);
        let world_offset = Vec2::from(coords);
        let start = Vec2::default();
        let end = Vec2::from(self.input().opposite()) * TILE_SIZE / 2.0;
        let t = pos as f32 / POSITIONS_PER_FRAGMENT as f32;
        let mid = start.lerp(end, t);
        Item::transform(world_offset + mid)
    }
}

enum BeltLike {
    Belt(Belt),
    Fragment(BeltFragment),
}

impl BeltLike {
    fn item_transform(&self, pos: u16, coords: WorldCoords) -> Transform {
        match self {
            BeltLike::Belt(belt) => belt.item_transform(pos, coords),
            BeltLike::Fragment(fragment) => fragment.item_transform(pos, coords),
        }
    }
}

impl From<(Option<&'_ Belt>, Option<&'_ BeltFragment>)> for BeltLike {
    fn from((belt, fragment): (Option<&'_ Belt>, Option<&'_ BeltFragment>)) -> Self {
        match (belt, fragment) {
            (None, None) => panic!("Both belt and fragment are None"),
            (Some(_), Some(_)) => panic!("Both belt and fragment are Some(...)"),
            (Some(belt), _) => BeltLike::Belt(belt.clone()),
            (_, Some(fragment)) => BeltLike::Fragment(fragment.clone()),
        }
    }
}

fn plan_moves(mut lanes: Query<&mut BeltLane>) {
    for mut lane in lanes.iter_mut() {
        for (pos, _) in lane.items.items.iter_mut() {
            *pos -= BASE_BELT_SPEED;
        }
    }
}

fn do_moves(
    mut items: Query<&mut Transform, With<Item>>,
    belts: Query<(&Belt, &WorldCoords)>,
    lanes: Query<&BeltLane>,
) {
    for lane in lanes {
        for (pos, item_ent) in lane.items.items.iter() {
            let belt = lane.belt_for(*pos);
            if let Some(belt) = belt {
                let (belt, coords) = belts.get(belt).unwrap();
                let transform = belt.item_transform(*pos, *coords);
                let mut t = items.get_mut(*item_ent).unwrap();
                *t = transform;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne, assert_str_eq};

    fn test_app() -> App {
        let mut app = crate::core::test_app();
        app.add_plugins(SimPlugin);
        app
    }

    #[test]
    fn item_moves_on_belt() {
        let mut app = test_app();
        let belt = app.add_belt((0, 0), Dir::East);
        app.update();
        let item = app.add_item(belt, POSITIONS_PER_TILE / 2);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(BASE_ITEM_MOVEMENT, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_moves_on_belt_north() {
        let mut app = test_app();
        let belt = app.add_belt((0, 0), Dir::North);
        app.update();
        let item = app.add_item(belt, POSITIONS_PER_TILE / 2);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(0.0, BASE_ITEM_MOVEMENT, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_doesnt_move_on_belt_end() {
        let mut app = test_app();
        let belt = app.add_belt((0, 0), Dir::East);
        app.update();
        let item = app.add_item(belt, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_moves_onto_next_belt() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.add_belt((1, 0), Dir::East);
        app.update();
        let item = app.add_item(belt1, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 + BASE_ITEM_MOVEMENT, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_moves_onto_next_belt_other_order() {
        let mut app = test_app();
        app.add_belt((1, 0), Dir::East);
        app.update();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.update();
        let item = app.add_item(belt1, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 + BASE_ITEM_MOVEMENT, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_dont_get_too_close() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.update();
        app.add_item(belt1, 0);
        let item = app.add_item(belt1, ITEM_SPACING);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(
            TILE_SIZE / 2.0 - ITEM_SPACING as f32 / POSITIONS_PER_TILE as f32 * TILE_SIZE,
            0.0,
            2.0,
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_moves_to_next_belt_with_item() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        let belt2 = app.add_belt((1, 0), Dir::East);
        app.update();
        app.add_item(belt2, 0);
        let item = app.add_item(belt1, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 + BASE_ITEM_MOVEMENT, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn handles_items_too_close_together() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.update();
        app.add_item(belt1, 0);
        let item = app.add_item(belt1, 1);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(
            TILE_SIZE / 2.0 - TILE_SIZE / POSITIONS_PER_TILE as f32,
            0.0,
            2.0,
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_moves_towards_side_loading_belt() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.add_belt((1, 0), Dir::North);
        app.add_belt((1, -1), Dir::North);
        app.update();

        let item = app.add_item(belt1, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 + BASE_ITEM_MOVEMENT, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_moves_towards_side_loading_belt_other_order() {
        let mut app = test_app();
        app.add_belt((1, 0), Dir::North);
        app.add_belt((1, -1), Dir::North);
        app.update();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.update();

        let item = app.add_item(belt1, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 + BASE_ITEM_MOVEMENT, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_moves_onto_side_loaded_belt() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.add_belt((1, 0), Dir::North);
        app.add_belt((1, -1), Dir::North);
        app.update();

        let item = app.add_item(belt1, 0);
        for _ in 0..(POSITIONS_PER_FRAGMENT / BASE_BELT_SPEED + 1) {
            app.update();
        }
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE, BASE_ITEM_MOVEMENT, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    #[ignore = "todo"]
    fn item_moves_onto_side_loaded_belt_unless_full() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        let belt2 = app.add_belt((1, 0), Dir::North);
        app.add_belt((1, -1), Dir::North);
        app.update();

        let item = app.add_item(belt1, POSITIONS_PER_TILE / 2);
        app.add_item(belt2, 0);
        app.add_item(belt2, ITEM_SPACING);
        for _ in 0..(POSITIONS_PER_TILE / 2 / BASE_BELT_SPEED + 1) {
            app.update();
        }
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 - ITEM_SIZE, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn replace_belt_under_item() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        let item = app.add_item(belt1, POSITIONS_PER_FRAGMENT);
        let init_pos = app.find_item(item);
        app.add_belt((0, 0), Dir::East);
        app.update();
        let actual = app.find_item(item);
        assert_ne!(actual, init_pos);
    }

    #[test]
    #[ignore = "todo, probably needs belt groups"]
    fn items_move_together() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        let belt2 = app.add_belt((1, 0), Dir::East);
        app.update();

        let first_item = app.add_item(belt2, 0);
        for i in 1..ITEMS_PER_TILE {
            app.add_item(belt2, ITEM_SPACING * i);
        }
        let last_item = app.add_item(belt1, 0);
        app.update();
        fn dist(app: &mut App, lead_item: Entity, follow_item: Entity) -> f32 {
            let lead_pos = app.find_item(lead_item).unwrap().1.translation;
            let follow_pos = app.find_item(follow_item).unwrap().1.translation;
            lead_pos.distance(follow_pos)
        }
        let expected = dist(&mut app, first_item, last_item);
        app.add_belt((2, 0), Dir::East);

        app.update();
        let actual = dist(&mut app, first_item, last_item);
        assert_eq!(actual, expected);
    }
}
