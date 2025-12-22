use std::ops::Range;

use crate::core::*;
use bevy::{platform::collections::HashMap, prelude::*};

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

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct BeltLane {
    pub(crate) belts: Belts,
    pub(crate) items: Items,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Belts {
    pub(crate) belts: Vec<(Range<u16>, Entity)>,
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

    fn add_to_head(&mut self, belt: Belt, entity: Entity) {
        let len = belt.num_positions();
        self.offset_by(len);
        self.belts.belts.insert(0, (0..len, entity));
    }

    fn offset_by(&mut self, len: u16) {
        self.belts.belts.iter_mut().for_each(|(range, _)| {
            range.start += len;
            range.end += len;
        });
        self.items.items.iter_mut().for_each(|(pos, _)| {
            *pos += len;
        });
    }
    fn add_to_tail(&mut self, belt: Belt, entity: Entity) {
        let len = belt.num_positions();
        let curr_len = self.num_positions();
        self.belts.belts.push((curr_len..curr_len + len, entity));
    }

    fn num_positions(&self) -> u16 {
        self.belts
            .belts
            .last()
            .map(|(r, _)| r.end)
            .unwrap_or_default()
    }
    fn merge(&mut self, mut tail: BeltLane) {
        let len = self.num_positions();
        tail.offset_by(len);
        self.belts.belts.extend_from_slice(&tail.belts.belts);
        self.items.items.extend_from_slice(&tail.items.items);
    }

    fn relative_pos(&self, pos: u16) -> u16 {
        pos - self
            .belts
            .belts
            .iter()
            .find(|(range, _)| range.contains(&pos))
            .unwrap()
            .0
            .start
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct InLane {
    pub(crate) lane: Entity,
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

#[derive(Debug)]
enum ConnectionType {
    Direct,
    SideLoad,
}

fn calculate_belt_connections(world: &mut World) {
    let changed_belts = world.resource::<BeltChanges>().clone();
    if changed_belts.0.is_empty() {
        return;
    }
    debug!("Updating belts: {:?}", changed_belts.0);

    let num_changes = changed_belts.0.len();
    for i in 0..num_changes {
        let change = &changed_belts.0[i];
        let remaining_entities = changed_belts.0[(i + 1)..]
            .iter()
            .map(|change| change.entity())
            .collect::<Vec<_>>();
        debug!("Remaining entities: {:?}", remaining_entities);
        match change {
            BeltChange::New(new) => {
                let belt_coords = world.resource::<BeltCoords>();
                let ahead_belt = belt_coords
                    .get(new.coords.step(new.belt.output()))
                    .filter(|(ent, _)| !remaining_entities.contains(ent))
                    .and_then(|(entity, ahead)| {
                        if ahead.input() == new.belt.output() {
                            Some((entity, ahead, ConnectionType::Direct))
                        } else {
                            if ahead.input().opposite() == new.belt.output() {
                                None
                            } else {
                                Some((entity, ahead, ConnectionType::SideLoad))
                            }
                        }
                    });
                let behind_belt = belt_coords
                    .get(new.coords.step(new.belt.input().opposite()))
                    .filter(|(ent, _)| !remaining_entities.contains(ent))
                    .filter(|behind| behind.1.output() == new.belt.input());
                debug!("Behind belt: {:?}", behind_belt);
                debug!("ahead belt: {:?}", ahead_belt);

                match (ahead_belt, behind_belt) {
                    (None, None) => {
                        debug!("Creating new lane");
                        let lane = BeltLane::from_belt(new.entity, new.belt);
                        let lane_ent = world.spawn(lane).id();
                        world.entity_mut(new.entity).insert(InLane::new(lane_ent));
                    }
                    (None, Some((behind_ent, _))) => {
                        debug!("Adding to head of existing lane");
                        let lane_ent = world
                            .query::<&InLane>()
                            .get(world, behind_ent)
                            .map(|l| l.lane)
                            .unwrap();
                        let mut lane = world
                            .query::<&mut BeltLane>()
                            .get_mut(world, lane_ent)
                            .unwrap();
                        lane.add_to_head(new.belt, new.entity);
                        debug!("Lane is {:?}", lane);
                        world.entity_mut(new.entity).insert(InLane::new(lane_ent));
                    }
                    (Some((ahead_ent, _, ConnectionType::Direct)), None) => {
                        debug!("Adding to tail of existing lane");
                        let lane_ent = world
                            .query::<&InLane>()
                            .get(world, ahead_ent)
                            .map(|l| l.lane)
                            .unwrap();
                        let mut lane = world
                            .query::<&mut BeltLane>()
                            .get_mut(world, lane_ent)
                            .unwrap();
                        lane.add_to_tail(new.belt, new.entity);
                        debug!("Lane is {:?}", lane);
                        world.entity_mut(new.entity).insert(InLane::new(lane_ent));
                    }
                    (
                        Some((ahead_ent, ahead_belt, ConnectionType::Direct)),
                        Some((behind_ent, behind_belt)),
                    ) => {
                        debug!("Merging lanes");
                        let behind_lane_ent = world
                            .query::<&InLane>()
                            .get(world, behind_ent)
                            .map(|l| l.lane)
                            .unwrap();
                        let mut behind_lane = world
                            .query::<&BeltLane>()
                            .get(world, behind_lane_ent)
                            .unwrap()
                            .clone();
                        behind_lane.add_to_head(new.belt, new.entity);
                        let ahead_lane_ent = world
                            .query::<&InLane>()
                            .get(world, ahead_ent)
                            .map(|l| l.lane)
                            .unwrap();
                        for (_, belt_ent) in &behind_lane.belts.belts {
                            world
                                .entity_mut(*belt_ent)
                                .insert(InLane::new(ahead_lane_ent));
                        }
                        let mut lane = world
                            .query::<&mut BeltLane>()
                            .get_mut(world, ahead_lane_ent)
                            .unwrap();
                        lane.merge(behind_lane);
                        debug!("Lane is {:?}", lane);
                        world.entity_mut(behind_lane_ent).despawn();
                        world
                            .entity_mut(new.entity)
                            .insert(InLane::new(ahead_lane_ent));
                    }
                    (Some((ahead_ent, ahead_belt, ConnectionType::SideLoad)), Some(_)) => {
                        todo!("Side loading")
                    }
                    (Some((ahead_ent, ahead_belt, ConnectionType::SideLoad)), None) => {
                        todo!("Side loading")
                    }
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
        for (i, (pos, _)) in lane.items.items.iter_mut().enumerate() {
            let furthest = i as u16 * ITEM_SPACING;
            let k = pos.saturating_sub(furthest);
            *pos = k.saturating_sub(BASE_BELT_SPEED) + furthest;
            debug!("Setting item at {}", pos);
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
                let transform = belt.item_transform(lane.relative_pos(*pos), *coords);
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
    fn item_moves_on_merged_lanes() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        let belt2 = app.add_belt((2, 0), Dir::East);
        app.update();
        let belt3 = app.add_belt((1, 0), Dir::East);
        // app.add_item(belt1, 0);
        // let item = app.add_item(belt1, 0);
        app.update();
        // let (_, actual) = app.find_item(item).unwrap();
        // let expected = Transform::from_xyz(TILE_SIZE / 2.0 + BASE_ITEM_MOVEMENT, 0.0, 2.0);
        // assert_eq!(actual, expected);
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
            TILE_SIZE / 2.0 - TILE_SIZE * ITEM_SPACING as f32 / POSITIONS_PER_TILE as f32,
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
