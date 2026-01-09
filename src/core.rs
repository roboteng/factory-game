pub use crate::core::lane::*;
use bevy::{math::ops::sin_cos, prelude::*};
use std::{collections::HashMap, f32::consts::PI, ops::Range};

mod lane;

#[cfg(feature = "invariant-ckeck")]
pub mod invariants;

#[cfg(all(test, feature = "proptests"))]
mod proptest_actions;
#[cfg(all(test, feature = "proptests"))]
mod proptests;

pub const BLOCK_SIZE: f32 = 2.0;
pub const HALF_BLOCK_SIZE: f32 = BLOCK_SIZE / 2.0;
pub const ITEM_SIZE: f32 = BLOCK_SIZE / 4.0;
pub const HALF_ITEM_SIZE: f32 = ITEM_SIZE / 2.0;
/// How far from the bottom of the voxel the belt surface is.
pub const BELT_HEIGHT: f32 = 0.25 * BLOCK_SIZE;
pub const BELT_HEIGHT_FROM_CENTER: f32 = -HALF_BLOCK_SIZE + BELT_HEIGHT;
/// Amount of a unit voxel of how far a lane is offset from center.
pub const LANE_OFFSET_FACTOR: f32 = 0.25;
/// How far from center each lane is.
pub const LANE_OFFSET: f32 = LANE_OFFSET_FACTOR * BLOCK_SIZE;

pub const POSITIONS_PER_BELT: i32 = 256;
pub const ITEM_SPACING: i32 = POSITIONS_PER_BELT / 4;
pub const POSITIONS_PER_FRAGMENT: i32 =
    (POSITIONS_PER_BELT as f32 * (1.0 - LANE_OFFSET_FACTOR * 2.0) / 2.0).round() as i32;
pub const POSITIONS_PER_INNER_CURVE: i32 =
    ((0.5 - LANE_OFFSET_FACTOR) * POSITIONS_PER_BELT as f32 * PI / 2.0).round() as i32;
pub const POSITIONS_PER_OUTER_CURVE: i32 =
    ((0.5 + LANE_OFFSET_FACTOR) * POSITIONS_PER_BELT as f32 * PI / 2.0).round() as i32;

pub const SIDES: [LaneSide; 2] = [LaneSide::Left, LaneSide::Right];

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_place_belt);
        app.add_observer(on_place_item);
        app.add_observer(on_remove_belt);

        app.init_resource::<BeltCoords>();
        app.init_resource::<BeltChanges>();

        app.add_systems(Update, (link_belts, replace_items).chain());
        app.add_systems(
            PostUpdate,
            (despawn_old_entities, clear_changed_belts).chain(),
        );
    }
}

// ------
// Models
// ------

#[derive(EntityEvent)]
pub struct PlaceBelt {
    pub entity: Entity,
    pub coords: WorldCoords,
    pub dir: HDir,
}

#[derive(EntityEvent)]
pub struct PlaceItem {
    pub entity: Entity,
    pub item: Item,
    pub belt: Entity,
    pub lane: LaneSide,
    pub position: i32,
}

#[derive(EntityEvent)]
pub struct RemoveBelt {
    pub entity: Entity,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldCoords {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Horizon direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HDir {
    North,
    South,
    East,
    West,
}

#[derive(Component)]
pub struct Belt;

/// Entities with this will get deleted in `PostUpdate'
#[derive(Component)]
pub struct Delete;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeltShape {
    Straight(HDir),
    Curve(Curve),
    Fragment(HDir),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curve {
    NorthToEast,
    EastToSouth,
    SouthToWest,
    WestToNorth,
    NorthToWest,
    WestToSouth,
    SouthToEast,
    EastToNorth,
}

/// Item ID
#[derive(Component, Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct Item(pub u32);

#[expect(unused)]
#[derive(Debug, Component)]
pub struct LaneConnection {
    pub target: Entity,
    pub offset: i32,
    pub side: LaneSide,
}
#[derive(Debug, Component)]
pub struct LaneLoopConnection {
    pub target: Entity,
    pub left_offset: i32,
    pub right_offset: i32,
}

#[derive(Component)]
pub struct InLane {
    pub lane: Entity,
}

#[derive(Resource, Default)]
pub struct BeltCoords(HashMap<WorldCoords, (Entity, BeltShape)>);

#[derive(Resource, Default, Debug, PartialEq, Eq, Clone)]
pub struct BeltChanges(pub Vec<BeltChange>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeltChange {
    New(NewBelt),
    Removed(RemovedBelt),
    Replaced(ReplacedBelt),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewBelt {
    pub entity: Entity,
    pub belt: BeltShape,
    pub coords: WorldCoords,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemovedBelt {
    pub entity: Entity,
    pub old_belt: BeltShape,
    pub coords: WorldCoords,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplacedBelt {
    pub entity: Entity,
    pub old_entity: Option<Entity>,
    pub old_belt: BeltShape,
    pub new_belt: BeltShape,
    pub coords: WorldCoords,
}

// -------
// Systems
// -------

fn despawn_old_entities(mut cmd: Commands, q: Query<Entity, With<Delete>>) {
    for entity in q {
        cmd.entity(entity).despawn();
    }
}

fn on_place_belt(
    event: On<PlaceBelt>,
    mut cmd: Commands,
    mut belt_coords: ResMut<BeltCoords>,
    mut changes: ResMut<BeltChanges>,
) {
    debug!(
        "Placing belt {:?} at {:?} facing {:?}",
        event.entity, event.coords, event.dir
    );

    let belt = plan_belt_placement(&event, &belt_coords);
    let angle = belt.output().angle();

    let old_entity_and_belt = belt_coords.get(event.coords);

    cmd.entity(event.entity).insert((
        Transform::from_translation(Vec3::from(event.coords))
            .with_rotation(Quat::from_rotation_y(angle)),
        Belt,
        belt,
        event.coords,
    ));
    belt_coords.insert(event.coords, event.entity, belt);

    if let Some((old_entity, old_belt)) = old_entity_and_belt {
        changes.push(ReplacedBelt {
            entity: event.entity,
            old_entity: Some(old_entity),
            old_belt,
            new_belt: belt,
            coords: event.coords,
        });
        cmd.entity(old_entity).insert(Delete);
    } else {
        changes.push(NewBelt {
            entity: event.entity,
            coords: event.coords,
            belt,
        });
    }

    // Check if placing this belt should curve the belt ahead
    let ahead = event.coords.step(belt.output());
    if let Some((entity, ahead_belt)) = belt_coords.get(ahead) {
        let place = PlaceBelt {
            entity,
            dir: ahead_belt.output(),
            coords: ahead,
        };
        let new_belt = plan_belt_placement(&place, &belt_coords);
        if ahead_belt != new_belt {
            let angle = new_belt.output().angle();
            // cmd.entity(place.entity)
            //     .get_mut::<Transform>()
            //     .unwrap()
            //     .rotation = Quat::from_rotation_y(angle);
            belt_coords.insert(place.coords, place.entity, new_belt);
            changes.push(ReplacedBelt {
                entity: place.entity,
                old_entity: None, // Same entity, just changed belt type
                new_belt,
                old_belt: ahead_belt,
                coords: place.coords,
            });
        }
    }
}

fn on_place_item(event: On<PlaceItem>, belts: Query<&InLane>, mut lanes: Query<&mut BeltLane>) {
    let lane_ent = belts.get(event.belt).unwrap().lane;
    let mut lane = lanes.get_mut(lane_ent).unwrap();
    lane.add_item(
        ItemEntry {
            pos: event.position,
            item: event.item,
            entity: event.entity,
        },
        event.lane,
        event.belt,
    )
    .expect("Invarient broken");
}

fn on_remove_belt(
    event: On<RemoveBelt>,
    belts: Query<(&BeltShape, &WorldCoords), With<Belt>>,
    mut changes: ResMut<BeltChanges>,
    mut belt_coords: ResMut<BeltCoords>,
) {
    let Ok((belt, coords)) = belts.get(event.entity) else {
        warn!(
            "Attempted to remove belt entity {:?} but it doesn't exist",
            event.entity
        );
        return;
    };

    debug!("Removing belt at {:?}", coords);

    belt_coords.remove(*coords);
    changes.push(RemovedBelt {
        entity: event.entity,
        old_belt: *belt,
        coords: *coords,
    });
}

fn replace_items(lanes: Query<&BeltLane>, mut items: Query<(&mut Item, &mut Transform)>) {
    for ((item, pos, belt, lane, coords), mut b) in Iterator::zip(
        lanes.iter().map(|l| l.item_iter()).flatten(),
        items.iter_mut(),
    ) {
        let transform = item_position(belt, coords, lane, pos);
        *b.0 = item;
        *b.1 = transform;
    }
}

fn clear_changed_belts(mut changes: ResMut<BeltChanges>) {
    changes.clear();
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
            BeltChange::New(new) => {
                new_belt(world, &remaining_entities, new, (Vec::new(), Vec::new()))
            }
            BeltChange::Removed(removed) => {
                let items = remove_belt(world, &remaining_entities, removed);
                for ItemEntry { entity, .. } in items.0.iter().chain(items.1.iter()) {
                    world.despawn(*entity);
                }
            }
            BeltChange::Replaced(replaced) => replace_belt(world, &remaining_entities, replaced),
        }
    }
}

#[derive(Debug)]
enum ConnectionType {
    Direct,
    SideLoad(LaneSide),
}

// -----------
// Model impls
// -----------

impl InLane {
    pub fn new(lane: Entity) -> Self {
        Self { lane }
    }
}

impl WorldCoords {
    pub fn step(&self, dir: HDir) -> Self {
        match dir {
            HDir::North => Self {
                x: self.x,
                y: self.y,
                z: self.z + 1,
            },
            HDir::East => Self {
                x: self.x + 1,
                y: self.y,
                z: self.z,
            },
            HDir::South => Self {
                x: self.x,
                y: self.y,
                z: self.z - 1,
            },
            HDir::West => Self {
                x: self.x - 1,
                y: self.y,
                z: self.z,
            },
        }
    }
}

impl BeltCoords {
    pub fn insert(&mut self, coords: WorldCoords, entity: Entity, belt: BeltShape) {
        self.0.insert(coords, (entity, belt));
    }

    pub fn get(&self, coords: WorldCoords) -> Option<(Entity, BeltShape)> {
        self.0.get(&coords).copied()
    }

    pub fn remove(&mut self, coords: WorldCoords) -> Option<(Entity, BeltShape)> {
        self.0.remove(&coords)
    }
}

impl HDir {
    pub fn angle(&self) -> f32 {
        match self {
            Self::North => 0.0,
            Self::East => -PI / 2.0,
            Self::South => PI,
            Self::West => PI / 2.0,
        }
    }

    pub fn opposite(&self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }

    pub fn left(&self) -> Self {
        match self {
            Self::North => Self::West,
            Self::East => Self::North,
            Self::South => Self::East,
            Self::West => Self::South,
        }
    }

    pub fn right(&self) -> Self {
        match self {
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
        }
    }
}

impl BeltShape {
    pub fn output(&self) -> HDir {
        match self {
            Self::Straight(dir) => *dir,
            Self::Curve(curve) => curve.output(),
            Self::Fragment(dir) => *dir,
        }
    }
    pub fn input(&self) -> HDir {
        match self {
            Self::Straight(dir) => *dir,
            Self::Curve(curve) => curve.input(),
            Self::Fragment(dir) => *dir,
        }
    }

    pub fn num_pos(&self, side: LaneSide) -> i32 {
        match side {
            LaneSide::Left => self.left_num_pos(),
            LaneSide::Right => self.right_num_pos(),
        }
    }

    pub fn left_num_pos(&self) -> i32 {
        match self {
            Self::Straight(_) => POSITIONS_PER_BELT,
            Self::Curve(curve) => {
                if curve.is_clockwise() {
                    POSITIONS_PER_OUTER_CURVE
                } else {
                    POSITIONS_PER_INNER_CURVE
                }
            }
            Self::Fragment(_) => POSITIONS_PER_FRAGMENT,
        }
    }
    pub fn right_num_pos(&self) -> i32 {
        match self {
            Self::Straight(_) => POSITIONS_PER_BELT,
            Self::Curve(curve) => {
                if curve.is_clockwise() {
                    POSITIONS_PER_INNER_CURVE
                } else {
                    POSITIONS_PER_OUTER_CURVE
                }
            }
            Self::Fragment(_) => POSITIONS_PER_FRAGMENT,
        }
    }
}

impl Curve {
    pub fn input(&self) -> HDir {
        match self {
            Self::NorthToEast => HDir::North,
            Self::EastToSouth => HDir::East,
            Self::SouthToWest => HDir::South,
            Self::WestToNorth => HDir::West,
            Self::NorthToWest => HDir::North,
            Self::EastToNorth => HDir::East,
            Self::SouthToEast => HDir::South,
            Self::WestToSouth => HDir::West,
        }
    }

    pub fn output(&self) -> HDir {
        match self {
            Self::NorthToEast => HDir::East,
            Self::EastToSouth => HDir::South,
            Self::SouthToWest => HDir::West,
            Self::WestToNorth => HDir::North,
            Self::NorthToWest => HDir::West,
            Self::EastToNorth => HDir::North,
            Self::SouthToEast => HDir::East,
            Self::WestToSouth => HDir::South,
        }
    }

    pub fn is_clockwise(&self) -> bool {
        match self {
            Self::NorthToEast => true,
            Self::EastToSouth => true,
            Self::SouthToWest => true,
            Self::WestToNorth => true,
            Self::NorthToWest => false,
            Self::EastToNorth => false,
            Self::SouthToEast => false,
            Self::WestToSouth => false,
        }
    }

    pub fn inner_lane(&self) -> LaneSide {
        if self.is_clockwise() {
            LaneSide::Right
        } else {
            LaneSide::Left
        }
    }
    #[expect(unused)]
    pub fn outet_lane(&self) -> LaneSide {
        if self.is_clockwise() {
            LaneSide::Left
        } else {
            LaneSide::Right
        }
    }
}

impl BeltChanges {
    pub fn push(&mut self, change: impl Into<BeltChange>) {
        let change = change.into();
        // Check if we can collapse this change with an existing one for the same entity
        let entity = change.entity();

        if let Some(existing_idx) = self.0.iter().position(|c| c.entity() == entity) {
            let existing = self.0[existing_idx];
            assert_eq!(existing.coords(), change.coords());
            match (existing, &change) {
                // New + Replaced => New (with final belt), moved to end
                (BeltChange::New(_), BeltChange::Replaced(replaced)) => {
                    self.0.remove(existing_idx);
                    self.0.push(
                        NewBelt {
                            entity,
                            belt: replaced.new_belt,
                            coords: replaced.coords,
                        }
                        .into(),
                    );
                    return;
                }
                // New + Removed => cancel out completely (belt was never really added)
                (BeltChange::New(_), BeltChange::Removed(_)) => {
                    self.0.remove(existing_idx);
                    return;
                }
                // Replaced + Replaced => update existing Replaced with cumulative change
                (BeltChange::Replaced(old_replaced), BeltChange::Replaced(new_replaced)) => {
                    self.0[existing_idx] = ReplacedBelt {
                        entity,
                        old_entity: old_replaced.old_entity,
                        old_belt: old_replaced.old_belt,
                        new_belt: new_replaced.new_belt,
                        coords: old_replaced.coords,
                    }
                    .into();
                    return;
                }
                // Replaced + Removed => collapse to Removed (using original old_belt)
                (BeltChange::Replaced(replaced), BeltChange::Removed(_)) => {
                    self.0[existing_idx] = RemovedBelt {
                        entity,
                        old_belt: replaced.old_belt,
                        coords: replaced.coords,
                    }
                    .into();
                    return;
                }
                _ => {}
            }
        }

        // No collapsing possible, just add the change
        self.0.push(change);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl BeltChange {
    pub fn entity(&self) -> Entity {
        match self {
            BeltChange::New(NewBelt { entity, .. }) => *entity,
            BeltChange::Removed(RemovedBelt { entity, .. }) => *entity,
            BeltChange::Replaced(ReplacedBelt { entity, .. }) => *entity,
        }
    }

    pub fn coords(&self) -> WorldCoords {
        match self {
            BeltChange::New(NewBelt { coords, .. }) => *coords,
            BeltChange::Removed(RemovedBelt { coords, .. }) => *coords,
            BeltChange::Replaced(ReplacedBelt { coords, .. }) => *coords,
        }
    }
}

// -----------
// Trait impls
// -----------

impl From<WorldCoords> for Vec3 {
    fn from(coords: WorldCoords) -> Self {
        Vec3::new(coords.x as f32, coords.y as f32, coords.z as f32) * BLOCK_SIZE
    }
}

impl From<(i32, i32, i32)> for WorldCoords {
    fn from(coords: (i32, i32, i32)) -> Self {
        WorldCoords {
            x: coords.0,
            y: coords.1,
            z: coords.2,
        }
    }
}

impl From<HDir> for Vec3 {
    fn from(value: HDir) -> Vec3 {
        match value {
            HDir::North => Vec3::X,
            HDir::South => Vec3::NEG_X,
            HDir::East => Vec3::Z,
            HDir::West => Vec3::NEG_Z,
        }
    }
}

impl From<HDir> for Vec2 {
    fn from(value: HDir) -> Self {
        Vec3::from(value).zx()
    }
}

impl From<NewBelt> for BeltChange {
    fn from(new_belt: NewBelt) -> Self {
        BeltChange::New(new_belt)
    }
}

impl From<RemovedBelt> for BeltChange {
    fn from(removed_belt: RemovedBelt) -> Self {
        BeltChange::Removed(removed_belt)
    }
}

impl From<ReplacedBelt> for BeltChange {
    fn from(replaced_belt: ReplacedBelt) -> Self {
        BeltChange::Replaced(replaced_belt)
    }
}

impl From<ReplacedBelt> for NewBelt {
    fn from(value: ReplacedBelt) -> Self {
        NewBelt {
            entity: value.entity,
            belt: value.new_belt,
            coords: value.coords,
        }
    }
}

impl From<ReplacedBelt> for RemovedBelt {
    fn from(value: ReplacedBelt) -> Self {
        RemovedBelt {
            entity: value.entity,
            old_belt: value.old_belt,
            coords: value.coords,
        }
    }
}

// --------
// Functions
// ---------

pub fn item_position(
    belt: BeltShape,
    coords: impl Into<WorldCoords>,
    lane: LaneSide,
    pos: i32,
) -> Transform {
    match belt {
        BeltShape::Straight(dir) => {
            let z = match lane {
                LaneSide::Left => -LANE_OFFSET,
                LaneSide::Right => LANE_OFFSET,
            };
            let start = Vec3::new(HALF_BLOCK_SIZE, BELT_HEIGHT_FROM_CENTER, z);
            let end = Vec3::new(-HALF_BLOCK_SIZE, BELT_HEIGHT_FROM_CENTER, z);

            let t = (pos + ITEM_SPACING / 2) as f32 / POSITIONS_PER_BELT as f32;
            let angle = dir.angle();
            Transform::from_translation(
                start.lerp(end, t).rotate_y(angle) + Vec3::from(coords.into()),
            )
        }
        BeltShape::Curve(curve) => {
            let center_offset =
                (Vec2::from(belt.input().opposite()) + Vec2::from(belt.output())) / 2.0;
            let n_pos = if curve.inner_lane() == lane {
                POSITIONS_PER_INNER_CURVE
            } else {
                POSITIONS_PER_OUTER_CURVE
            };
            let lane_offset = if curve.inner_lane() == lane {
                0.5 - LANE_OFFSET_FACTOR
            } else {
                0.5 + LANE_OFFSET_FACTOR
            };
            let angle_offset = (pos + ITEM_SPACING / 2) as f32 / n_pos as f32 * PI / 2.0;
            let angle_base = curve.input().angle();
            // Positions move the opposite way of items, so this is backwards
            let angle = if curve.is_clockwise() {
                angle_base + angle_offset
            } else {
                angle_base - angle_offset
            };
            debug!(
                "angle: {}*pi, angle_offset: {}*pi, angle_base: {}*pi",
                angle / PI,
                angle_offset / PI,
                angle_base / PI
            );
            let local_offset = center_offset
                + lane_offset * {
                    let (sin, cos) = sin_cos(angle);
                    Vec2 { x: -sin, y: cos }
                };
            debug!(
                "center_offset: {center_offset:?}, lane_offset: {lane_offset}, local_offset: {:?}, ",
                local_offset
            );
            Transform::from_translation(
                Vec3::new(
                    local_offset.y * BLOCK_SIZE,
                    BELT_HEIGHT_FROM_CENTER,
                    local_offset.x * BLOCK_SIZE,
                ) + Vec3::from(coords.into()),
            )
            .with_rotation(Quat::from_rotation_y(angle + PI / 2.0))
        }
        BeltShape::Fragment(dir) => todo!(),
    }
}

fn plan_belt_placement(trigger: &PlaceBelt, belt_coords: &BeltCoords) -> BeltShape {
    let left = trigger.coords.step(trigger.dir.left());
    let right = trigger.coords.step(trigger.dir.right());
    let behind = trigger.coords.step(trigger.dir.opposite());
    let fed_from_left = belt_coords
        .get(left)
        .map(|(_, belt)| belt.output() == trigger.dir.right())
        .unwrap_or(false);
    let fed_from_right = belt_coords
        .get(right)
        .map(|(_, belt)| belt.output() == trigger.dir.left())
        .unwrap_or(false);
    let fed_from_behind = belt_coords
        .get(behind)
        .map(|(_, belt)| belt.output() == trigger.dir)
        .unwrap_or(false);
    let belt = match (fed_from_left, fed_from_behind, fed_from_right) {
        (true, _, true) | (false, _, false) | (_, true, _) => BeltShape::Straight(trigger.dir),
        (true, false, false) => match trigger.dir {
            HDir::North => BeltShape::Curve(Curve::EastToNorth),
            HDir::East => BeltShape::Curve(Curve::SouthToEast),
            HDir::South => BeltShape::Curve(Curve::WestToSouth),
            HDir::West => BeltShape::Curve(Curve::NorthToWest),
        },
        (false, false, true) => match trigger.dir {
            HDir::North => BeltShape::Curve(Curve::WestToNorth),
            HDir::East => BeltShape::Curve(Curve::NorthToEast),
            HDir::South => BeltShape::Curve(Curve::EastToSouth),
            HDir::West => BeltShape::Curve(Curve::SouthToWest),
        },
    };
    assert_eq!(belt.output(), trigger.dir);
    belt
}

fn new_belt(
    world: &mut World,
    remaining_entities: &[Entity],
    new: &NewBelt,
    existing_items: (Vec<ItemEntry>, Vec<ItemEntry>),
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
            let mut lane = BeltLane::from_belt(new.belt, new.coords, new.entity);
            lane.insert_items_at(&existing_items.0, LaneSide::Left);
            lane.insert_items_at(&existing_items.1, LaneSide::Right);
            let lane_ent = world.spawn(lane).id();
            world.entity_mut(new.entity).insert(InLane::new(lane_ent));
        }
        (None, Some((behind_ent, _))) => {
            debug!("Adding to head of existing lane");
            let lane_ent = get_lane_entity(world, behind_ent);
            let mut lane = get_lane_mut(world, lane_ent);
            lane.add_to_head(new.belt, new.coords, new.entity);
            lane.insert_items_at(&existing_items.0, LaneSide::Left);
            lane.insert_items_at(&existing_items.1, LaneSide::Right);
            debug!("Lane is {:?}", lane);
            for side in SIDES {
                let len = new.belt.num_pos(side);
                update_connection_offsets_for_lane(world, lane_ent, side, len);
            }
            world.entity_mut(new.entity).insert(InLane::new(lane_ent));
        }
        (Some((ahead_ent, _, ConnectionType::Direct)), None) => {
            debug!("Adding to tail of existing lane");
            let lane_ent = get_lane_entity(world, ahead_ent);
            let mut lane = get_lane_mut(world, lane_ent);

            let mut new_lane = BeltLane::from_belt(new.belt, new.coords, new.entity);
            new_lane.insert_items_at(&existing_items.0, LaneSide::Left);
            new_lane.insert_items_at(&existing_items.1, LaneSide::Right);
            lane.merge(new_lane);
            debug!("Lane is {:?}", lane);
            world.entity_mut(new.entity).insert(InLane::new(lane_ent));
        }
        (Some((ahead_ent, _, ConnectionType::Direct)), Some((behind_ent, _))) => {
            debug!("Merging lanes");
            let behind_lane_ent = get_lane_entity(world, behind_ent);
            let ahead_lane_ent = get_lane_entity(world, ahead_ent);
            let mut behind_lane = get_lane_mut(world, behind_lane_ent);
            behind_lane.add_to_head(new.belt, new.coords, new.entity);
            behind_lane.insert_items_at(&existing_items.0, LaneSide::Left);
            behind_lane.insert_items_at(&existing_items.1, LaneSide::Right);
            for side in SIDES {
                let len = new.belt.num_pos(side);
                update_connection_offsets_for_lane(world, behind_lane_ent, side, len);
            }

            if behind_lane_ent == ahead_lane_ent {
                debug!("Belt loop");
                world
                    .entity_mut(new.entity)
                    .insert(InLane::new(behind_lane_ent));
                let (left_offset, right_offset) = get_lane(world, behind_lane_ent).lengths();
                world
                    .entity_mut(behind_lane_ent)
                    .insert(LaneLoopConnection {
                        target: behind_lane_ent,
                        left_offset,
                        right_offset,
                    });
                debug!("spawned loop connection");
            } else {
                let behind_lane = get_lane(world, behind_lane_ent).clone();
                for belt in &behind_lane.belts {
                    let belt_ent = belt.entity;
                    world
                        .entity_mut(belt_ent)
                        .insert(InLane::new(ahead_lane_ent));
                    debug!("loop lane is {:?}", behind_lane);
                }
                let mut lane = get_lane_mut(world, ahead_lane_ent);
                lane.merge(behind_lane.clone());
                debug!("Lane is {:?}", lane);
                world.entity_mut(behind_lane_ent).despawn();
                world
                    .entity_mut(new.entity)
                    .insert(InLane::new(ahead_lane_ent));
            }
        }
        (Some((side_ent, _, ConnectionType::SideLoad(side))), None) => {
            debug!("sideloading new lane");
            let mut lane = BeltLane::from_belt(new.belt, new.coords, new.entity);
            lane.insert_items_at(&existing_items.0, LaneSide::Left);
            lane.insert_items_at(&existing_items.1, LaneSide::Right);
            let lane_ent = world.spawn(lane).id();

            create_sideload_connection(
                world,
                lane_ent,
                side_ent,
                new.belt.output(),
                new.coords.step(new.belt.output()),
                side,
            );

            world.entity_mut(new.entity).insert(InLane::new(lane_ent));
        }
        (Some((side_ent, _, ConnectionType::SideLoad(side))), Some((behind_ent, _))) => {
            let lane_ent = get_lane_entity(world, behind_ent);

            let mut lane = get_lane_mut(world, lane_ent);
            lane.add_to_head(new.belt, new.coords, new.entity);
            lane.insert_items_at(&existing_items.0, LaneSide::Left);
            lane.insert_items_at(&existing_items.1, LaneSide::Right);

            create_sideload_connection(
                world,
                lane_ent,
                side_ent,
                new.belt.output(),
                new.coords.step(new.belt.output()),
                side,
            );
            for side in SIDES {
                let len = new.belt.num_pos(side);
                update_connection_offsets_for_lane(world, lane_ent, side, len);
            }

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
            let left_lane_ent = get_lane_entity(world, left_belt.0);

            create_sideload_connection(
                world,
                left_lane_ent,
                new.entity,
                new.belt.output().right(),
                new.coords,
                LaneSide::Left,
            );
        }

        let belt_coords = world.resource::<BeltCoords>();
        let right = new.coords.step(new.belt.output().right());
        if let Some(right_belt) = belt_coords.get(right).filter(|(ent, belt)| {
            !remaining_entities.contains(ent) && belt.output() == new.belt.output().left()
        }) {
            let right_lane_ent = get_lane_entity(world, right_belt.0);

            create_sideload_connection(
                world,
                right_lane_ent,
                new.entity,
                new.belt.output().left(),
                new.coords,
                LaneSide::Right,
            );
        }
    }
}

fn remove_belt(
    world: &mut World,
    remaining_entities: &[Entity],
    removed: &RemovedBelt,
) -> (Vec<ItemEntry>, Vec<ItemEntry>) {
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
    let lane_ent = get_lane_entity(world, removed.entity);
    match (ahead_belt, behind_belt) {
        (None, None) => {
            let lane = get_lane(world, lane_ent);
            let left = lane.lanes[LaneSide::Left].clone();
            let right = lane.lanes[LaneSide::Right].clone();
            world.despawn(lane_ent);
            (left, right)
        }
        (None, Some(_)) => {
            let mut lane = get_lane_mut(world, lane_ent);
            let items = lane.remove_head();
            if lane.belts.is_empty() {
                world.despawn(lane_ent);
            }
            items
        }
        (Some((_, _, ConnectionType::Direct)), None) => {
            let mut lane = get_lane_mut(world, lane_ent);
            let items = lane.remove_tail();
            if lane.belts.is_empty() {
                world.despawn(lane_ent);
            }
            items
        }
        (Some((_, _, ConnectionType::Direct)), Some(_)) => {
            // Removing from middle of lane - would need to split lane
            todo!("remove belt from middle of lane")
        }
        (Some((_, _, ConnectionType::SideLoad(_))), None) => todo!(),
        (Some((_, _, ConnectionType::SideLoad(_))), Some(_)) => todo!(),
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
    dir: HDir,
) -> Option<(Entity, BeltShape, ConnectionType)> {
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
                    let side = LaneSide::Left; // TODO
                    Some((entity, ahead, ConnectionType::SideLoad(side)))
                }
            }
        })
}

fn behind_connected_belt(
    belt_coords: &BeltCoords,
    remaining_entities: &[Entity],
    coords: WorldCoords,
    dir: HDir,
) -> Option<(Entity, BeltShape)> {
    belt_coords
        .get(coords.step(dir.opposite()))
        .filter(|(ent, _)| !remaining_entities.contains(ent))
        .filter(|behind| behind.1.output() == dir)
}

fn create_sideload_connection(
    world: &mut World,
    source_lane_ent: Entity,
    target_belt_ent: Entity,
    source_dir: HDir,
    intersection_coords: WorldCoords,
    side: LaneSide,
) {
    let target_lane_ent = get_lane_entity(world, target_belt_ent);
    let target_lane = get_lane(world, target_lane_ent);

    // Get the ranges for the target belt
    let ranges = target_lane.range_for(target_belt_ent).unwrap();

    let offset = (ranges[side].start + ranges[side].end) / 2;

    // Create the connection
    world.entity_mut(source_lane_ent).insert(LaneConnection {
        target: target_lane_ent,
        offset,
        side,
    });

    // TODO: Add fragment for visual representation
    // For now, skip fragment creation as it's mainly cosmetic
}

fn get_lane_entity(world: &mut World, belt_ent: Entity) -> Entity {
    world
        .query::<&InLane>()
        .get(world, belt_ent)
        .map(|l| l.lane)
        .unwrap()
}

fn get_lane(world: &mut World, lane_ent: Entity) -> &BeltLane {
    world.query::<&BeltLane>().get(world, lane_ent).unwrap()
}

fn get_lane_mut(world: &mut World, lane_ent: Entity) -> bevy::ecs::world::Mut<'_, BeltLane> {
    world
        .query::<&mut BeltLane>()
        .get_mut(world, lane_ent)
        .unwrap()
}

fn update_connection_offsets_for_lane(
    world: &mut World,
    lane_ent: Entity,
    side: LaneSide,
    offset: i32,
) {
    // Update connection on this lane (if present and matches side)
    if let Some(mut conn) = world.entity_mut(lane_ent).get_mut::<LaneConnection>() {
        conn.offset += offset;
    }

    // Update loop connection on this lane (if present)
    if let Some(mut conn) = world.entity_mut(lane_ent).get_mut::<LaneLoopConnection>() {
        conn.left_offset += offset;
        conn.right_offset += offset;
    }

    // Find and update all connections pointing to this lane
    let mut conns_to_update: Vec<Entity> = Vec::new();
    let mut query = world.query::<(Entity, &LaneConnection)>();
    for (source_ent, conn) in query.iter(world) {
        if conn.target == lane_ent {
            conns_to_update.push(source_ent);
        }
    }

    for source_ent in conns_to_update {
        if let Some(mut conn) = world.entity_mut(source_ent).get_mut::<LaneConnection>() {
            conn.offset += offset;
        }
    }
}

fn assert_close(left: Vec3, right: Vec3) {
    let dist = left.distance(right);
    assert!(
        dist < 0.0001,
        "Left:\n\t{left:?}\nand Right:\n\t{right:?}\nare distance of {dist} away"
    );
}

#[cfg(test)]
pub fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("warn,factory_game=debug")),
        )
        .with_target(false)
        .with_test_writer()
        .without_time()
        .try_init();
}

#[cfg(test)]
pub fn test_app() -> App {
    init_tracing();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app
}

#[cfg(test)]
pub trait AppExtension {
    fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: HDir) -> Entity;
    fn add_item(&mut self, belt: Entity, pos: i32, lane: LaneSide) -> Entity;
    fn find_item(&mut self, item: Entity) -> Option<(Item, Transform)>;
}

#[cfg(test)]
impl AppExtension for App {
    fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: HDir) -> Entity {
        let entity = self.world_mut().spawn_empty().id();
        self.world_mut().trigger(PlaceBelt {
            entity,
            dir,
            coords: coords.into(),
        });
        entity
    }

    fn add_item(&mut self, belt: Entity, pos: i32, lane: LaneSide) -> Entity {
        let entity = self.world_mut().spawn_empty().id();
        self.world_mut().trigger(PlaceItem {
            entity,
            item: Item(0),
            belt,
            lane,
            position: pos,
        });
        entity
    }

    fn find_item(&mut self, item: Entity) -> Option<(Item, Transform)> {
        let world = self.world_mut();
        world
            .query::<(&Item, &Transform)>()
            .get(world, item)
            .ok()
            .map(|(item, transform)| (*item, *transform))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn north_betl_placement() {
        let mut app = test_app();

        let entity = app.add_belt((0, 0, 0), HDir::North);
        app.update();

        let world = app.world_mut();
        let &actual = world.query::<&Transform>().get(world, entity).unwrap();
        let expected = Transform::from_translation(Vec3::new(0.0, 0.0, 0.0) * BLOCK_SIZE);
        assert_eq!(actual, expected);
    }

    #[test]
    fn east_betl_placement() {
        let mut app = test_app();

        let entity = app.add_belt((0, 0, 0), HDir::East);
        app.update();

        let world = app.world_mut();
        let &actual = world.query::<&Transform>().get(world, entity).unwrap();
        let expected = Transform::from_translation(Vec3::new(0.0, 0.0, 0.0) * BLOCK_SIZE)
            .with_rotation(Quat::from_rotation_y(-PI / 2.0));
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_positioning_front_boundary() {
        let actual = item_position(
            BeltShape::Straight(HDir::North),
            (0, 0, 0),
            LaneSide::Left,
            -ITEM_SPACING / 2,
        );
        let expected = Transform::from_translation(Vec3::new(
            HALF_BLOCK_SIZE,
            -HALF_BLOCK_SIZE + BELT_HEIGHT,
            -LANE_OFFSET,
        ));
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_positioning_start() {
        let actual = item_position(
            BeltShape::Straight(HDir::North),
            (0, 0, 0),
            LaneSide::Left,
            0,
        );
        let expected = Transform::from_translation(Vec3::new(
            HALF_BLOCK_SIZE - HALF_ITEM_SIZE,
            -HALF_BLOCK_SIZE + BELT_HEIGHT,
            -LANE_OFFSET,
        ));
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_positioning_start_east() {
        let actual = item_position(
            BeltShape::Straight(HDir::East),
            (0, 0, 0),
            LaneSide::Left,
            0,
        );
        let expected = Transform::from_translation(Vec3::new(
            LANE_OFFSET,
            -HALF_BLOCK_SIZE + BELT_HEIGHT,
            HALF_BLOCK_SIZE - HALF_ITEM_SIZE,
        ));
        assert_close(actual.translation, expected.translation);
        assert_eq!(actual.rotation, expected.rotation);
        assert_eq!(actual.scale, expected.scale);
    }

    #[test]
    fn item_positioning_start_right() {
        let actual = item_position(
            BeltShape::Straight(HDir::North),
            (0, 0, 0),
            LaneSide::Right,
            0,
        );
        let expected = Transform::from_translation(Vec3::new(
            HALF_BLOCK_SIZE - HALF_ITEM_SIZE,
            -HALF_BLOCK_SIZE + BELT_HEIGHT,
            LANE_OFFSET,
        ));
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_positioning_start_coords() {
        let actual = item_position(
            BeltShape::Straight(HDir::North),
            (1, 1, 1),
            LaneSide::Left,
            0,
        );
        let expected = Transform::from_translation(Vec3::new(
            HALF_BLOCK_SIZE - HALF_ITEM_SIZE + BLOCK_SIZE,
            -HALF_BLOCK_SIZE + BELT_HEIGHT + BLOCK_SIZE,
            -LANE_OFFSET + BLOCK_SIZE,
        ));
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_positioning_front_curved() {
        init_tracing();
        let actual = item_position(
            BeltShape::Curve(Curve::EastToNorth),
            (0, 0, 0),
            LaneSide::Left,
            -ITEM_SPACING / 2,
        );
        let expected = Transform::from_translation(Vec3::new(
            HALF_BLOCK_SIZE,
            -HALF_BLOCK_SIZE + BELT_HEIGHT,
            -LANE_OFFSET,
        ));
        assert_close(actual.translation, expected.translation);
        assert_eq!(actual.rotation, expected.rotation);
    }

    #[test]
    fn item_positioning_end_curved() {
        init_tracing();
        let actual = item_position(
            BeltShape::Curve(Curve::EastToNorth),
            (0, 0, 0),
            LaneSide::Left,
            POSITIONS_PER_INNER_CURVE - ITEM_SPACING / 2,
        );
        let expected = Transform::from_translation(Vec3::new(
            LANE_OFFSET,
            -HALF_BLOCK_SIZE + BELT_HEIGHT,
            -HALF_BLOCK_SIZE,
        ))
        .with_rotation(Quat::from_axis_angle(Vec3::Y, -PI / 2.0));
        assert_close(actual.translation, expected.translation);
        assert_eq!(actual.rotation, expected.rotation);
    }

    #[test]
    fn item_on_belt() {
        let mut app = test_app();
        let entity = app.add_belt((0, 0, 0), HDir::North);
        app.update();

        let item_ent = app.add_item(entity, 0, LaneSide::Left);

        let world = app.world_mut();
        let mut q = world.query::<&BeltLane>();
        let actual = q.single(world).unwrap();
        let expected = BeltLane {
            belts: vec![BeltEntry {
                belt: BeltShape::Straight(HDir::North),
                coords: (0, 0, 0).into(),
                entity,
                ranges: Ranges {
                    left: 0..POSITIONS_PER_BELT,
                    right: 0..POSITIONS_PER_BELT,
                },
            }],
            lanes: Lanes {
                left: vec![ItemEntry {
                    pos: 0,
                    item: Item(0),
                    entity: item_ent,
                }],
                right: vec![],
            },
        };
        assert_eq!(*actual, expected);
    }
}
