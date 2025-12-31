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
            (link_belts, transfers, plan_moves, do_moves).chain(),
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
    pub(crate) belts: Vec<(Range<i32>, Entity)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Items {
    pub(crate) items: Vec<(i32, Entity)>,
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

    fn range_for(&self, belt: Entity) -> Option<Range<i32>> {
        self.belts
            .belts
            .iter()
            .find(|(_, id)| *id == belt)
            .map(|(range, _)| range.clone())
    }

    fn insert_item_at(&mut self, pos: i32, item: Entity) {
        self.items.items.push((pos, item));
        self.items.items.sort();
    }

    fn insert_items_at(&mut self, items: &[(i32, Entity)]) {
        for k in items {
            self.items.items.push(*k);
        }
        self.items.items.sort();
    }

    fn belt_for(&self, pos: i32) -> Option<Entity> {
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

    fn offset_by(&mut self, len: i32) {
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

    fn num_positions(&self) -> i32 {
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

    fn relative_pos(&self, pos: i32) -> i32 {
        pos - self
            .belts
            .belts
            .iter()
            .find(|(range, _)| range.contains(&pos))
            .unwrap()
            .0
            .start
    }
    fn prepend_fragment(&mut self, fragment: BeltFragment, entity: Entity) {
        let len = fragment.num_positions();
        self.offset_by(len);
        self.belts.belts.insert(0, (0..len, entity));
    }

    fn shorten_by(&mut self, len: i32) {
        self.items.items.iter_mut().for_each(|(pos, _)| *pos -= len);
        self.belts.belts.iter_mut().for_each(|(range, _)| {
            range.start -= len;
            range.end -= len;
        });
    }

    /// Returns the items that were on the head of the belt
    fn remove_head(&mut self) -> Vec<(i32, Entity)> {
        let head = self.belts.belts.remove(0);
        let part = self
            .items
            .items
            .partition_point(|(p, _)| head.0.contains(p));
        let (head_items, keep_items) = self.items.items.split_at_mut(part);
        let keep = Vec::from(keep_items);
        let ret = Vec::from_iter(head_items.iter().map(|(pos, e)| (*pos, *e)));
        self.items.items = keep;
        self.shorten_by(head.0.end);
        ret
    }

    fn remove_tail(&mut self) -> Vec<(i32, Entity)> {
        let last = self.belts.belts.len();
        let tail = self.belts.belts.remove(last - 1);
        let part = self
            .items
            .items
            .partition_point(|(p, _)| tail.0.contains(p));
        let (keep_items, tail_items) = self.items.items.split_at_mut(part);
        let keep = Vec::from(keep_items);
        let ret = Vec::from_iter(tail_items.iter().map(|(pos, e)| (*pos, *e)));
        self.items.items = keep;
        ret
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

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct BeltConnection {
    /// Lane Entity
    pub(crate) source: Entity,
    /// Lane Entity
    pub(crate) target: Entity,
    pub(crate) offset: i32,
}

#[derive(Debug)]
enum ConnectionType {
    Direct,
    SideLoad,
}

fn link_belts(world: &mut World) {
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
            BeltChange::New(new) => new_belt(world, &remaining_entities, new, Vec::new()),
            BeltChange::Removed(removed) => {
                let items = remove_belt(world, &remaining_entities, removed);
                for (_, e) in items {
                    world.despawn(e);
                }
            }
            BeltChange::Replaced(replaced) => replace_belt(world, &remaining_entities, replaced),
        }
    }
}

fn new_belt(
    world: &mut World,
    remaining_entities: &[Entity],
    new: &NewBelt,
    existing_items: Vec<(i32, Entity)>,
) {
    let belt_coords = world.resource::<BeltCoords>();
    let ahead_belt = ahead_connected_belt(
        &belt_coords,
        remaining_entities,
        new.coords,
        new.belt.output(),
    );
    let behind_belt = behind_connected_belt(
        &belt_coords,
        remaining_entities,
        new.coords,
        new.belt.input(),
    );
    debug!("Behind belt: {:?}", behind_belt);
    debug!("ahead belt: {:?}", ahead_belt);

    match (ahead_belt, behind_belt) {
        (None, None) => {
            debug!("Creating new lane");
            let mut lane = BeltLane::from_belt(new.entity, new.belt);
            lane.insert_items_at(&existing_items);
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
            lane.insert_items_at(&existing_items);
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
            let offset = lane.num_positions();
            let items = existing_items
                .iter()
                .map(|(pos, e)| (pos + offset, *e))
                .collect::<Vec<_>>();
            lane.add_to_tail(new.belt, new.entity);
            lane.insert_items_at(&items);
            debug!("Lane is {:?}", lane);
            world.entity_mut(new.entity).insert(InLane::new(lane_ent));
        }
        (Some((ahead_ent, _, ConnectionType::Direct)), Some((behind_ent, _))) => {
            debug!("Merging lanes");
            let behind_lane_ent = world
                .query::<&InLane>()
                .get(world, behind_ent)
                .map(|l| l.lane)
                .unwrap();
            let ahead_lane_ent = world
                .query::<&InLane>()
                .get(world, ahead_ent)
                .map(|l| l.lane)
                .unwrap();
            let mut behind_lane = world
                .query::<&mut BeltLane>()
                .get_mut(world, behind_lane_ent)
                .unwrap();
            behind_lane.add_to_head(new.belt, new.entity);
            behind_lane.insert_items_at(&existing_items);

            if behind_lane_ent == ahead_lane_ent {
                debug!("Belt loop");
                let offset = behind_lane.num_positions();
                world
                    .entity_mut(new.entity)
                    .insert(InLane::new(behind_lane_ent));
                world.spawn(BeltConnection {
                    source: behind_lane_ent,
                    target: behind_lane_ent,
                    offset,
                });
                debug!("spawned loop connection");
            } else {
                let behind_lane = behind_lane.clone();
                for (_, belt_ent) in &behind_lane.belts.belts {
                    world
                        .entity_mut(*belt_ent)
                        .insert(InLane::new(ahead_lane_ent));
                    debug!("loop lane is {:?}", behind_lane);
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
        }
        (Some((side_ent, _, ConnectionType::SideLoad)), None) => {
            debug!("sideloading new lane");
            let mut lane = BeltLane::from_belt(new.entity, new.belt);
            lane.insert_items_at(&existing_items);
            let lane_ent = world.spawn(lane).id();

            create_sideload_connection(
                world,
                lane_ent,
                side_ent,
                new.belt.output(),
                new.coords.step(new.belt.output()),
            );

            world.entity_mut(new.entity).insert(InLane::new(lane_ent));
        }
        (Some((side_ent, _, ConnectionType::SideLoad)), Some((behind_ent, _))) => {
            let lane_ent = world
                .query::<&InLane>()
                .get(world, behind_ent)
                .map(|l| l.lane)
                .unwrap();

            create_sideload_connection(
                world,
                lane_ent,
                side_ent,
                new.belt.output(),
                new.coords.step(new.belt.output()),
            );

            let mut lane = world
                .query::<&mut BeltLane>()
                .get_mut(world, lane_ent)
                .unwrap();
            lane.add_to_head(new.belt, new.entity);
            lane.insert_items_at(&existing_items);

            world.entity_mut(new.entity).insert(InLane::new(lane_ent));
        }
    }
    if new.belt.input() == new.belt.output() {
        new.coords.step(new.belt.output().left());
        let belt_coords = world.resource::<BeltCoords>();

        let left = new.coords.step(new.belt.output().left());
        if let Some(left_belt) = belt_coords.get(left).filter(|(ent, belt)| {
            !remaining_entities.contains(ent) && belt.output() == new.belt.output().right()
        }) {
            let left_lane_ent = world
                .query::<&InLane>()
                .get(world, left_belt.0)
                .unwrap()
                .lane;

            create_sideload_connection(
                world,
                left_lane_ent,
                new.entity,
                new.belt.output().right(),
                new.coords,
            );
        }

        let belt_coords = world.resource::<BeltCoords>();
        let right = new.coords.step(new.belt.output().right());
        if let Some(right_belt) = belt_coords.get(right).filter(|(ent, belt)| {
            !remaining_entities.contains(ent) && belt.output() == new.belt.output().left()
        }) {
            let right_lane_ent = world
                .query::<&InLane>()
                .get(world, right_belt.0)
                .unwrap()
                .lane;

            create_sideload_connection(
                world,
                right_lane_ent,
                new.entity,
                new.belt.output().left(),
                new.coords,
            );
        }
    }
}

/// This decouples the belt from the world, but doesn't actualy remove it
fn remove_belt(
    world: &mut World,
    remaining_entities: &[Entity],
    removed: &RemovedBelt,
) -> Vec<(i32, Entity)> {
    let belt_coords = world.resource::<BeltCoords>();
    let ahead_belt = ahead_connected_belt(
        &belt_coords,
        remaining_entities,
        removed.coords,
        removed.old_belt.output(),
    );
    let behind_belt = behind_connected_belt(
        &belt_coords,
        remaining_entities,
        removed.coords,
        removed.old_belt.input(),
    );
    debug!("Behind belt: {:?}", behind_belt);
    debug!("ahead belt: {:?}", ahead_belt);
    let lane_ent = world
        .query::<&InLane>()
        .get(world, removed.entity)
        .unwrap()
        .lane;
    match (ahead_belt, behind_belt) {
        (None, None) => {
            let items = world
                .query::<&BeltLane>()
                .get_mut(world, lane_ent)
                .unwrap()
                .items
                .items
                .clone();
            world.despawn(lane_ent);
            items
        }
        (None, Some(_)) => {
            let mut lane = world
                .query::<&mut BeltLane>()
                .get_mut(world, lane_ent)
                .unwrap();
            lane.remove_head()
        }
        (Some((_, _, ConnectionType::Direct)), None) => {
            let mut lane = world
                .query::<&mut BeltLane>()
                .get_mut(world, lane_ent)
                .unwrap();
            lane.remove_tail()
        }
        (Some((_, _, ConnectionType::Direct)), Some(_)) => {
            // Removing from middle of lane - would need to split lane
            todo!("remove belt from middle of lane")
        }
        (Some((_, _, ConnectionType::SideLoad)), None) => todo!(),
        (Some((_, _, ConnectionType::SideLoad)), Some(_)) => todo!(),
    }
}
fn replace_belt(world: &mut World, remaining_entities: &[Entity], replaced: &ReplacedBelt) {
    match replaced.old_entity {
        Some(old) => {
            let removed = RemovedBelt {
                entity: old,
                old_belt: replaced.old_belt,
                coords: replaced.coords,
            };
            let remaining_items = remove_belt(world, remaining_entities, &removed);
            let new = NewBelt::from(*replaced);
            new_belt(world, remaining_entities, &new, remaining_items);
        }
        None => {
            let removed = RemovedBelt::from(*replaced);
            let remaining_items = remove_belt(world, remaining_entities, &removed);
            let new = NewBelt::from(*replaced);
            new_belt(world, remaining_entities, &new, remaining_items);
        }
    }
}

fn ahead_connected_belt(
    belt_coords: &BeltCoords,
    remaining_entities: &[Entity],
    coords: WorldCoords,
    dir: Dir,
) -> Option<(Entity, Belt, ConnectionType)> {
    belt_coords
        .get(coords.step(dir))
        .filter(|(ent, _)| !remaining_entities.contains(ent))
        .and_then(|(entity, ahead)| {
            if ahead.input() == dir {
                Some((entity, ahead, ConnectionType::Direct))
            } else {
                if ahead.input().opposite() == dir {
                    None
                } else {
                    Some((entity, ahead, ConnectionType::SideLoad))
                }
            }
        })
}

fn create_sideload_connection(
    world: &mut World,
    source_lane_ent: Entity,
    target_belt_ent: Entity,
    source_dir: Dir,
    intersection_coords: WorldCoords,
) {
    let target_lane_ent = world
        .query::<&InLane>()
        .get(world, target_belt_ent)
        .unwrap()
        .lane;
    let target_lane = world
        .query::<&BeltLane>()
        .get(world, target_lane_ent)
        .unwrap();
    let range = target_lane.range_for(target_belt_ent).unwrap();

    world.spawn(BeltConnection {
        source: source_lane_ent,
        target: target_lane_ent,
        offset: (range.start + range.end) / 2,
    });

    let fragment = BeltFragment::new(source_dir);
    let frag_ent = world
        .spawn((fragment, intersection_coords, InLane::new(source_lane_ent)))
        .id();

    let mut source_lane = world
        .query::<&mut BeltLane>()
        .get_mut(world, source_lane_ent)
        .unwrap();
    source_lane.prepend_fragment(fragment, frag_ent);
}

fn behind_connected_belt(
    belt_coords: &BeltCoords,
    remaining_entities: &[Entity],
    coords: WorldCoords,
    dir: Dir,
) -> Option<(Entity, Belt)> {
    belt_coords
        .get(coords.step(dir.opposite()))
        .filter(|(ent, _)| !remaining_entities.contains(ent))
        .filter(|behind| behind.1.output() == dir)
}

#[derive(Component, Clone, Copy, Debug)]
struct BeltFragment {
    dir: Dir,
}

impl BeltFragment {
    fn new(dir: Dir) -> Self {
        Self { dir }
    }
    fn input(&self) -> Dir {
        self.dir
    }
    fn output(&self) -> Dir {
        self.dir
    }
    fn num_positions(&self) -> i32 {
        POSITIONS_PER_FRAGMENT
    }
    fn item_transform(&self, pos: i32, coords: WorldCoords) -> Transform {
        debug!("Transforming fragment at {:?} with pos {}", coords, pos);
        let world_offset = Vec2::from(coords);

        let end = Vec2::from(self.input().opposite()) * TILE_SIZE / 2.0;
        let delta = TILE_SIZE * POSITIONS_PER_FRAGMENT as f32 / POSITIONS_PER_TILE as f32
            * Vec2::from(self.dir.opposite());
        let start = end - delta;
        let t = (pos + ITEM_SPACING / 2) as f32 / POSITIONS_PER_FRAGMENT as f32;
        let mid = start.lerp(end, t);
        Item::transform(world_offset + mid)
    }
}

#[derive(Debug)]
enum BeltLike {
    Belt(Belt),
    Fragment(BeltFragment),
}

impl BeltLike {
    fn new(value: (Option<&Belt>, Option<&BeltFragment>)) -> Self {
        match value {
            (Some(belt), None) => Self::Belt(belt.clone()),
            (None, Some(fragment)) => Self::Fragment(fragment.clone()),
            _ => panic!("Invalid BeltLike value"),
        }
    }
    fn item_transform(&self, pos: i32, coords: WorldCoords) -> Transform {
        match self {
            Self::Belt(belt) => belt.item_transform(pos, coords),
            Self::Fragment(fragment) => fragment.item_transform(pos, coords),
        }
    }
    fn input(&self) -> Dir {
        match self {
            Self::Belt(belt) => belt.input(),
            Self::Fragment(fragment) => fragment.input(),
        }
    }
    fn output(&self) -> Dir {
        match self {
            Self::Belt(belt) => belt.output(),
            Self::Fragment(fragment) => fragment.output(),
        }
    }
}

fn transfers(conns: Query<&BeltConnection>, mut lanes: Query<&mut BeltLane>) {
    for conn in conns {
        debug!("processing connection");
        if conn.source == conn.target {
            debug!("loop connection");
            let mut lane = lanes.get_mut(conn.source).unwrap();
            debug!("init  items: {:?}", lane.items.items);
            let Some((pos, _)) = lane.items.items.first_mut() else {
                debug!("skipping transfer");
                continue;
            };
            if *pos < BASE_BELT_SPEED {
                *pos += conn.offset;
                lane.items.items.sort();
            }
            debug!("final items: {:?}", lane.items.items);
        } else {
            debug!("non-loop connection");
            let mut source_lane = lanes.get_mut(conn.source).unwrap();
            let Some((pos, item_ent)) = source_lane.items.items.first().copied() else {
                continue;
            };
            debug!("pos: {pos:?}");
            if pos < BASE_BELT_SPEED {
                source_lane.items.items.remove(0);
                let mut target_lane = lanes.get_mut(conn.target).unwrap();
                target_lane.insert_item_at(conn.offset - ITEM_SPACING / 2, item_ent);
            }
        }
    }
}

fn plan_moves(mut lanes: Query<&mut BeltLane>) {
    for mut lane in lanes.iter_mut() {
        for (i, (pos, _)) in lane.items.items.iter_mut().enumerate() {
            let furthest = i as i32 * ITEM_SPACING;
            let k = (*pos - furthest).max(0);
            *pos = (k - BASE_BELT_SPEED).max(0) + furthest;
            debug!("Setting item at {}", pos);
        }
    }
}

fn do_moves(
    mut items: Query<&mut Transform, With<Item>>,
    belts: Query<(AnyOf<(&Belt, &BeltFragment)>, &WorldCoords)>,
    lanes: Query<&BeltLane>,
) {
    for lane in lanes {
        for (pos, item_ent) in lane.items.items.iter() {
            let belt = lane.belt_for(*pos);
            if let Some(belt) = belt {
                let (belt, coords) = belts.get(belt).unwrap();
                let belt = BeltLike::new(belt);
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
        let expected = Transform::from_xyz(BASE_ITEM_MOVEMENT - ITEM_SIZE / 2.0, 0.0, 2.0);
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
        let expected = Transform::from_xyz(0.0, BASE_ITEM_MOVEMENT - ITEM_SIZE / 2.0, 2.0);
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
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 - ITEM_SIZE / 2.0, 0.0, 2.0);
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
        let expected = Transform::from_xyz(
            TILE_SIZE / 2.0 + BASE_ITEM_MOVEMENT - ITEM_SIZE / 2.0,
            0.0,
            2.0,
        );
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
        let expected = Transform::from_xyz(
            TILE_SIZE / 2.0 + BASE_ITEM_MOVEMENT - ITEM_SIZE / 2.0,
            0.0,
            2.0,
        );
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
            TILE_SIZE / 2.0
                - ITEM_SPACING as f32 / POSITIONS_PER_TILE as f32 * TILE_SIZE
                - ITEM_SIZE / 2.0,
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
        let expected = Transform::from_xyz(
            TILE_SIZE / 2.0 + BASE_ITEM_MOVEMENT - ITEM_SIZE / 2.0,
            0.0,
            2.0,
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_moves_on_merged_lanes() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        let _belt2 = app.add_belt((2, 0), Dir::East);
        app.update();
        let _belt3 = app.add_belt((1, 0), Dir::East);
        let item = app.add_item(belt1, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(
            TILE_SIZE / 2.0 + BASE_ITEM_MOVEMENT - ITEM_SIZE / 2.0,
            0.0,
            2.0,
        );
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
            TILE_SIZE / 2.0
                - TILE_SIZE * ITEM_SPACING as f32 / POSITIONS_PER_TILE as f32
                - ITEM_SIZE / 2.0,
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
        let expected = Transform::from_xyz(
            TILE_SIZE / 2.0 + BASE_ITEM_MOVEMENT - ITEM_SIZE / 2.0,
            0.0,
            2.0,
        );
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
        let expected = Transform::from_xyz(
            TILE_SIZE / 2.0 + BASE_ITEM_MOVEMENT - ITEM_SIZE / 2.0,
            0.0,
            2.0,
        );
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
        app.update();
        let item = app.add_item(belt1, POSITIONS_PER_FRAGMENT);
        app.update();
        let init_pos = app.find_item(item).unwrap();
        app.add_belt((0, 0), Dir::East);
        app.update();
        let actual = app.find_item(item).unwrap();
        assert_ne!(actual, init_pos);
    }

    #[test]
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

    #[test]
    fn belt_loop() {
        let _example = "
            v<
            v^ v
             ^ v
            v^ v
            >^
        ";
        let mut app = test_app();
        let _ = app.add_belt((0, 1), Dir::South);
        let _ = app.add_belt((0, 0), Dir::East);
        let _ = app.add_belt((1, 0), Dir::North);
        let _ = app.add_belt((1, 1), Dir::North);
        let _ = app.add_belt((1, 2), Dir::North);
        let _ = app.add_belt((1, 3), Dir::North);
        let _ = app.add_belt((1, 4), Dir::West);
        let _ = app.add_belt((0, 4), Dir::South);
        let a = app.add_belt((0, 3), Dir::South);

        let b = app.add_belt((3, 3), Dir::South);
        let _ = app.add_belt((3, 2), Dir::South);
        let _ = app.add_belt((3, 1), Dir::South);
        app.update();
        let _ = app.add_belt((0, 2), Dir::South);
        app.update();
        let test_item = app.add_item(a, 0);
        let ref_item = app.add_item(b, 0);
        app.update();
        for _ in 0..((TILE_SIZE / BASE_ITEM_MOVEMENT) as usize + 2) {
            let ref_pos = app.find_item(ref_item).unwrap().1.translation;
            let actual_pos = app.find_item(test_item).unwrap().1.translation;
            assert_eq!(actual_pos, ref_pos - Vec3::X * TILE_SIZE * 3.0);
            app.update();
        }
    }

    #[test]
    fn small_belt_loop() {
        let mut app = test_app();
        app.add_belt((0, 0), Dir::East);
        app.add_belt((1, 0), Dir::North);
        app.add_belt((1, 1), Dir::West);
        let belt = app.add_belt((0, 1), Dir::South);
        app.update();
        let item = app.add_item(belt, 0);
        app.update();
        let mut prev_pos = app.find_item(item).unwrap().1.translation;
        app.update();
        for _ in 0..(POSITIONS_PER_CURVED_TILE * 4 / BASE_BELT_SPEED + BASE_BELT_SPEED) {
            let pos = app.find_item(item).unwrap().1.translation;
            assert_ne!(pos, prev_pos);
            prev_pos = pos;
            app.update();
        }
    }

    #[test]
    fn remove_single_belt() {
        let mut app = test_app();
        app.add_belt((0, 0), Dir::East);
        app.update();
        app.remove_belt_at((0, 0));
        app.update();
    }
}
