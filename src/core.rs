use crate::core::inventory::{Inventory, Stack};
use bevy::{math::ops::sin_cos, prelude::*};
use derivative::Derivative;
use std::collections::HashMap;
use std::f32::consts::PI;

pub mod inventory;

#[cfg(feature = "invariant-check")]
pub mod invariants;

#[cfg(all(test, feature = "proptests"))]
mod proptest_actions;
#[cfg(all(test, feature = "proptests"))]
mod proptests;

pub const BLOCK_SIZE: f32 = 2.0;
#[allow(unused)]
pub const HALF_BLOCK_SIZE: f32 = BLOCK_SIZE / 2.0;
#[allow(unused)]
pub const ITEM_SIZE: f32 = BLOCK_SIZE / 4.0;
#[allow(unused)]
pub const HALF_ITEM_SIZE: f32 = ITEM_SIZE / 2.0;
/// How far from the bottom of the voxel the belt surface is.
#[allow(unused)]
pub const BELT_HEIGHT: f32 = 0.25 * HALF_BLOCK_SIZE;
#[allow(unused)]
pub const BELT_HEIGHT_FROM_CENTER: f32 = -(HALF_BLOCK_SIZE / 2.0) + BELT_HEIGHT;
/// Amount of a unit voxel of how far a lane is offset from center.
pub const LANE_OFFSET_FACTOR: f32 = 0.25;
/// How far from center each lane is.
#[allow(unused)]
pub const LANE_OFFSET: f32 = LANE_OFFSET_FACTOR * BLOCK_SIZE;

pub const POSITIONS_PER_BELT: i32 = 256;
pub const ITEM_SPACING: i32 = POSITIONS_PER_BELT / 4;
pub const BASE_BELT_SPEED: i32 = 8; // Items move 8 positions per frame
#[allow(unused)]
pub const BASE_ITEM_MOVEMENT: f32 = BLOCK_SIZE * BASE_BELT_SPEED as f32 / POSITIONS_PER_BELT as f32;
#[allow(unused)]
pub const POSITIONS_PER_INNER_CURVE: i32 =
    ((0.5 - LANE_OFFSET_FACTOR) * POSITIONS_PER_BELT as f32 * PI / 2.0).round() as i32;
#[allow(unused)]
pub const POSITIONS_PER_OUTER_CURVE: i32 =
    ((0.5 + LANE_OFFSET_FACTOR) * POSITIONS_PER_BELT as f32 * PI / 2.0).round() as i32;
#[allow(unused)]
pub const ITEMS_PER_BELT: i32 = POSITIONS_PER_BELT / ITEM_SPACING;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Side {
    Left,
    Right,
}
pub const SIDES: [Side; 2] = [Side::Left, Side::Right];

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "invariant-check")]
        app.add_plugins(crate::core::invariants::InvariantsPlugin);

        app.init_resource::<CoordMap>();

        app.add_observer(on_place_block);
        app.add_observer(on_place_item);
        app.add_observer(on_remove_block);

        let mut inv = Inventory::new();
        inv.insert(Stack::new(Item::Belt, 15.try_into().unwrap()))
            .unwrap();
        let player = app.world_mut().spawn(inv).id();
        app.insert_resource(Player(player));

        app.add_systems(
            Update,
            (
                determine_belt_shape,
                move_items_on_belts,
                transfer_items,
                set_item_transforms,
                sources_place,
                sinks_destroy,
                side_loading,
            ),
        );

        app.add_systems(PostUpdate, despawn_old_entities);
    }
}

// ------
// Models
// ------

#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct PlaceBlock {
    pub entity: Entity,
    pub item: Item,
    pub coords: WorldCoords,
    pub dir: HDir,
}

impl PlaceBlock {
    fn to_bundle(&self) -> impl Bundle {
        (
            self.item,
            self.coords,
            self.dir,
            Transform::from_translation(self.coords.into())
                .with_rotation(Quat::from_rotation_y(self.dir.angle())),
        )
    }
}

#[derive(EntityEvent, Derivative)]
#[derivative(Debug)]
pub struct PlaceItem {
    pub entity: Entity,
    pub item: Item,
    pub belt: Entity,
    pub lane: Side,
    pub position: i32,
    #[derivative(Debug = "ignore")]
    pub on_error: Box<dyn Fn(Commands, ItemPlacementError) + Send + Sync + 'static>,
}

#[derive(EntityEvent, Debug, Clone)]
pub struct RemoveBlock {
    pub entity: Entity,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldCoords {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Horizontal direction
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HDir {
    North,
    South,
    East,
    West,
}

#[derive(Component)]
pub struct Belt;

#[derive(Component)]
pub struct Source;

#[derive(Component)]
pub struct Sink;

#[derive(Component)]
pub struct AffectsBelts;

/// Marks an entity as a target for block-placement raycasts.
/// `half_extents` is the AABB half-size on each axis, centred on the entity's
/// `Transform` translation.
#[derive(Component, Clone, Copy)]
pub struct RaycastTarget {
    pub half_extents: Vec3,
}
impl RaycastTarget {
    /// Half-block tall (belts).
    pub const HALF_BLOCK: Self = Self {
        half_extents: Vec3::new(BLOCK_SIZE / 2.0, HALF_BLOCK_SIZE / 2.0, BLOCK_SIZE / 2.0),
    };
    /// Full-block tall (Rock / Dirt / Source / Sink).
    pub const FULL_BLOCK: Self = Self {
        half_extents: Vec3::new(BLOCK_SIZE / 2.0, BLOCK_SIZE / 2.0, BLOCK_SIZE / 2.0),
    };
}

#[derive(Component)]
pub struct OnBelt;

pub type ItemPos = i32;

#[derive(Component, Default)]
pub struct ItemLanes(Sided<Vec<(ItemPos, Entity)>>);

/// Entities with this will get deleted in `PostUpdate'
#[derive(Component)]
pub struct Delete;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeltShape {
    Straight(HDir),
    Curve(Curve),
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

/// Item type.
#[derive(Component, Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub enum Item {
    Belt,
    Source,
    Sink,
    Rock,
    Dirt,
}

impl Item {
    pub fn name(self) -> &'static str {
        match self {
            Item::Belt => "Belt",
            Item::Source => "Source",
            Item::Sink => "Sink",
            Item::Rock => "Rock",
            Item::Dirt => "Dirt",
        }
    }

    pub fn raycast_target(self) -> RaycastTarget {
        match self {
            Item::Belt => RaycastTarget::HALF_BLOCK,
            Item::Rock | Item::Dirt | Item::Source | Item::Sink => RaycastTarget::FULL_BLOCK,
        }
    }
}

#[derive(Component, Debug, PartialEq, Eq, Clone, Default)]
pub struct Sided<T> {
    pub left: T,
    pub right: T,
}

#[derive(Resource)]
pub struct Player(pub Entity);

#[derive(Resource, Default)]
pub struct CoordMap(pub HashMap<WorldCoords, Entity>);

// -------
// Systems
// -------

fn despawn_old_entities(mut cmd: Commands, q: Query<Entity, With<Delete>>) {
    for entity in q {
        cmd.entity(entity).despawn();
    }
}

fn on_place_block(
    event: On<PlaceBlock>,
    mut cmd: Commands,
    mut coord_map: ResMut<CoordMap>,
    belts_q: Query<&ItemLanes, With<Belt>>,
) {
    let rt = event.item.raycast_target();
    let is_full = rt.half_extents.y > HALF_BLOCK_SIZE / 2.0;

    // Full-height blocks must sit at an even y coordinate. If the ray lands
    // on an odd slot (e.g. top face of a belt), snap down to the nearest even.
    let coords = if is_full {
        WorldCoords {
            y: event.coords.y & !1,
            ..event.coords
        }
    } else {
        event.coords
    };

    let place = PlaceBlock {
        coords,
        ..*event.event()
    };

    debug!(
        "Placing {:?} at {coords:?} facing {:?}",
        event.item, event.dir
    );

    // For full-height blocks, also check the top slot.
    if is_full
        && coord_map.0.contains_key(&WorldCoords {
            y: coords.y + 1,
            ..coords
        })
    {
        cmd.entity(event.entity).despawn();
        return;
    }

    // Check for an existing block at this location.
    if let Some(&existing) = coord_map.0.get(&coords) {
        if event.item == Item::Belt {
            if let Ok(old_lanes) = belts_q.get(existing) {
                // Belt-on-belt: replace the old belt and transfer its items to the new one.
                let transferred = old_lanes.0.clone();
                cmd.entity(existing).despawn();
                coord_map.0.remove(&coords);
                cmd.entity(event.entity)
                    .insert((Belt, ItemLanes(transferred), AffectsBelts, rt));
                cmd.entity(event.entity).insert(place.to_bundle());
                coord_map.0.insert(coords, event.entity);
                return;
            }
        }
        // Any other collision: ignore the placement.
        cmd.entity(event.entity).despawn();
        return;
    }

    match event.item {
        Item::Belt => {
            cmd.entity(event.entity)
                .insert((Belt, ItemLanes::default(), AffectsBelts, rt))
        }
        Item::Source => cmd.entity(event.entity).insert((Source, AffectsBelts, rt)),
        Item::Sink => cmd.entity(event.entity).insert((Sink, AffectsBelts, rt)),
        Item::Rock | Item::Dirt => cmd.entity(event.entity).insert(rt),
    };

    cmd.entity(event.entity).insert(place.to_bundle());
    coord_map.0.insert(coords, event.entity);
    // Register the second slot for full-height blocks.
    if is_full {
        coord_map.0.insert(
            WorldCoords {
                y: coords.y + 1,
                ..coords
            },
            event.entity,
        );
    }
}

fn on_place_item(
    event: On<PlaceItem>,
    mut belts: Query<(&BeltShape, &WorldCoords, &mut ItemLanes), With<Belt>>,
    mut cmd: Commands,
) {
    debug!("Placing item {:?} at {:?}", event.entity, event.belt);

    let Ok(mut belt) = belts.get_mut(event.belt) else {
        warn!("Couldn't find belt for the item");
        return;
    };
    cmd.entity(event.entity).insert((
        event.item,
        OnBelt,
        item_position(*belt.0, *belt.1, event.lane, event.position),
    ));
    belt.2.0[event.lane].push((event.position, event.entity));
}

fn on_remove_block(
    event: On<RemoveBlock>,
    coords_q: Query<&WorldCoords>,
    lanes_q: Query<&ItemLanes>,
    mut coord_map: ResMut<CoordMap>,
    mut cmd: Commands,
) {
    debug!("Removing {:?}", event.entity);
    if let Ok(coords) = coords_q.get(event.entity) {
        coord_map.0.remove(coords);
        // Remove the top slot if this entity registered it (full-height blocks).
        let top = WorldCoords {
            y: coords.y + 1,
            ..*coords
        };
        if coord_map.0.get(&top) == Some(&event.entity) {
            coord_map.0.remove(&top);
        }
    }
    if let Ok(lanes) = lanes_q.get(event.entity) {
        for (_, item) in lanes.0.left.iter().chain(lanes.0.right.iter()) {
            cmd.entity(*item).despawn();
        }
    }
    cmd.entity(event.entity).despawn();
}

fn determine_belt_shape(
    belts: Query<(Entity, &WorldCoords, &HDir), With<Belt>>,
    affecters: Query<&HDir, With<AffectsBelts>>,
    coord_map: Res<CoordMap>,
    mut cmd: Commands,
) {
    for (entity, coords, dir) in belts.iter() {
        let fed_from_behind = coord_map
            .0
            .get(&coords.step(dir.opposite()))
            .and_then(|&e| affecters.get(e).ok())
            .is_some_and(|d| *d == *dir);
        let fed_from_left = coord_map
            .0
            .get(&coords.step(dir.right()))
            .and_then(|&e| affecters.get(e).ok())
            .filter(|d| **d == dir.left())
            .copied();
        let fed_from_right = coord_map
            .0
            .get(&coords.step(dir.left()))
            .and_then(|&e| affecters.get(e).ok())
            .filter(|d| **d == dir.right())
            .copied();
        match (fed_from_left, fed_from_behind, fed_from_right) {
            (None, _, None) | (Some(_), _, Some(_)) | (_, true, _) => {
                cmd.entity(entity).insert(BeltShape::Straight(*dir));
            }
            (Some(a), false, None) | (None, false, Some(a)) => {
                let curve = Curve::from_input_output(a, *dir).unwrap();
                assert_eq!(curve.output(), *dir);
                cmd.entity(entity).insert(BeltShape::Curve(curve));
            }
        }
    }
}

fn move_items_on_belts(mut belts: Query<(&mut ItemLanes, &BeltShape)>) {
    for mut belt in belts.iter_mut() {
        for side in SIDES {
            let Some(lead_item) = belt.0.0[side].get_mut(0) else {
                continue;
            };
            lead_item.0 = 0.max(lead_item.0 - BASE_BELT_SPEED);
            for i in 1..belt.0.0[side].len() {
                let first = belt.0.0[side][i - 1];
                let second = &mut belt.0.0[side][i];

                second.0 = (first.0 + ITEM_SPACING).max(second.0 - BASE_BELT_SPEED);
            }
        }
    }
}

fn transfer_items(
    mut invs: Query<(Entity, &mut ItemLanes, &WorldCoords, &HDir, &BeltShape)>,
    coord_map: Res<CoordMap>,
) {
    struct Transfer {
        source: Entity,
        dest: Entity,
        lane: Side,
    }
    let mut transfers = Vec::new();
    for source in invs.iter() {
        let next = source.2.step(*source.3);
        let Some(&dest_entity) = coord_map.0.get(&next) else {
            continue;
        };
        let Ok(dest) = invs.get(dest_entity) else {
            continue;
        };
        for side in SIDES {
            let Some(i) = source.1.0[side].get(0) else {
                continue;
            };
            if i.0 <= 0
                && dest.1.0[side].last().map(|a| a.0).unwrap_or(0) + ITEM_SPACING
                    < dest.4.num_pos(side)
                && source.4.output() == dest.4.input()
            {
                transfers.push(Transfer {
                    source: source.0,
                    dest: dest_entity,
                    lane: side,
                });
            }
        }
    }
    for transfer in transfers {
        let mut source = invs.get_mut(transfer.source).unwrap();
        let slot = source.1.0[transfer.lane].remove(0);
        drop(source);

        let mut dest = invs.get_mut(transfer.dest).unwrap();
        let lane = &mut dest.1.0[transfer.lane];
        lane.push((dest.4.num_pos(transfer.lane), slot.1));
    }
}

fn side_loading(
    mut invs: Query<(Entity, &mut ItemLanes, &WorldCoords, &HDir, &BeltShape)>,
    coord_map: Res<CoordMap>,
) {
    struct Transfer {
        source: Entity,
        dest: Entity,
        source_lane: Side,
        dest_lane: Side,
        position: ItemPos,
    }
    let mut transfers = Vec::new();
    for source in invs.iter() {
        let next = source.2.step(*source.3);
        let Some(&dest_entity) = coord_map.0.get(&next) else {
            continue;
        };
        let Ok(dest) = invs.get(dest_entity) else {
            continue;
        };
        if matches!(dest.4, BeltShape::Straight(_))
            && (source.4.output() == dest.4.input().left()
                || source.4.output() == dest.4.input().right())
        {
            let dest_side = if source.4.output() == dest.4.input().right() {
                Side::Left
            } else {
                Side::Right
            };
            for side in SIDES {
                let Some(item) = source.1.0[side].get(0) else {
                    continue;
                };
                if item.0 <= 0
                    && dest.1.0[dest_side].last().map(|a| a.0).unwrap_or(0) + ITEM_SPACING
                        < dest.4.num_pos(dest_side)
                {
                    const OFFSET: i32 =
                        (POSITIONS_PER_BELT as f32 * LANE_OFFSET_FACTOR).round() as i32;
                    let position = if side == dest_side {
                        POSITIONS_PER_BELT / 2 - OFFSET
                    } else {
                        POSITIONS_PER_BELT / 2 + OFFSET
                    };
                    transfers.push(Transfer {
                        source: source.0,
                        dest: dest_entity,
                        source_lane: side,
                        dest_lane: dest_side,
                        position,
                    });
                }
            }
        }
    }
    for transfer in transfers {
        let mut source = invs.get_mut(transfer.source).unwrap();
        let slot = source.1.0[transfer.source_lane].remove(0);
        drop(source);

        let mut dest = invs.get_mut(transfer.dest).unwrap();
        let lane = &mut dest.1.0[transfer.dest_lane];
        lane.push((transfer.position, slot.1));
    }
}

fn set_item_transforms(
    belts: Query<(&ItemLanes, &BeltShape, &WorldCoords, &HDir)>,
    mut items: Query<&mut Transform, With<OnBelt>>,
) {
    for belt in belts {
        for side in SIDES {
            for slot in belt.0.0[side].iter() {
                let Ok(mut item) = items.get_mut(slot.1) else {
                    continue;
                };
                *item = item_position(*belt.1, *belt.2, side, slot.0);
            }
        }
    }
}

fn sources_place(
    sources: Query<(&WorldCoords, &HDir), With<Source>>,
    belts: Query<(Entity, &ItemLanes), With<Belt>>,
    coord_map: Res<CoordMap>,
    mut cmd: Commands,
) {
    for (source_coords, source_dir) in sources {
        let target = source_coords.step(*source_dir);
        let Some(&belt_entity) = coord_map.0.get(&target) else {
            continue;
        };
        let Ok((belt_entity, lanes)) = belts.get(belt_entity) else {
            continue;
        };
        if lanes.0.left.len() <= ITEMS_PER_BELT as usize {
            let entity = cmd.spawn_empty().id();
            cmd.trigger(PlaceItem {
                entity,
                item: Item::Belt,
                belt: belt_entity,
                lane: Side::Left,
                position: POSITIONS_PER_BELT,
                on_error: Box::new(|_, _| {}),
            });
        }
    }
}

fn sinks_destroy(
    sinks: Query<&WorldCoords, With<Sink>>,
    mut belts: Query<(Entity, &mut ItemLanes, &HDir)>,
    coord_map: Res<CoordMap>,
    mut cmd: Commands,
) {
    for sink_coords in sinks {
        for d in [HDir::North, HDir::South, HDir::East, HDir::West] {
            let neighbor = sink_coords.step(d.opposite());
            let Some(&belt_entity) = coord_map.0.get(&neighbor) else {
                continue;
            };
            let Ok((_, mut lanes, belt_dir)) = belts.get_mut(belt_entity) else {
                continue;
            };
            if *belt_dir != d {
                continue;
            }
            for side in SIDES {
                let Some(lead_item) = lanes.0[side].get(0) else {
                    continue;
                };
                if lead_item.0 != 0 {
                    continue;
                }
                cmd.entity(lead_item.1).despawn();
                lanes.0[side].remove(0);
            }
        }
    }
}

// -----------
// Model impls
// -----------

impl WorldCoords {
    pub const fn step(&self, dir: HDir) -> Self {
        match dir {
            HDir::North => Self {
                x: self.x + 1,
                y: self.y,
                z: self.z,
            },
            HDir::East => Self {
                x: self.x,
                y: self.y,
                z: self.z + 1,
            },
            HDir::South => Self {
                x: self.x - 1,
                y: self.y,
                z: self.z,
            },
            HDir::West => Self {
                x: self.x,
                y: self.y,
                z: self.z - 1,
            },
        }
    }
}

impl HDir {
    pub const fn angle(&self) -> f32 {
        match self {
            Self::North => 0.0,
            Self::East => -PI / 2.0,
            Self::South => PI,
            Self::West => PI / 2.0,
        }
    }

    pub const fn opposite(&self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }

    pub const fn left(&self) -> Self {
        match self {
            Self::North => Self::West,
            Self::East => Self::North,
            Self::South => Self::East,
            Self::West => Self::South,
        }
    }

    pub const fn right(&self) -> Self {
        match self {
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
        }
    }
}

impl BeltShape {
    pub const fn output(&self) -> HDir {
        match self {
            Self::Straight(dir) => *dir,
            Self::Curve(curve) => curve.output(),
        }
    }
    pub const fn input(&self) -> HDir {
        match self {
            Self::Straight(dir) => *dir,
            Self::Curve(curve) => curve.input(),
        }
    }

    pub const fn num_pos(&self, side: Side) -> i32 {
        match side {
            Side::Left => self.left_num_pos(),
            Side::Right => self.right_num_pos(),
        }
    }

    pub const fn left_num_pos(&self) -> i32 {
        match self {
            Self::Straight(_) => POSITIONS_PER_BELT,
            Self::Curve(curve) => {
                if curve.is_clockwise() {
                    POSITIONS_PER_OUTER_CURVE
                } else {
                    POSITIONS_PER_INNER_CURVE
                }
            }
        }
    }
    pub const fn right_num_pos(&self) -> i32 {
        match self {
            Self::Straight(_) => POSITIONS_PER_BELT,
            Self::Curve(curve) => {
                if curve.is_clockwise() {
                    POSITIONS_PER_INNER_CURVE
                } else {
                    POSITIONS_PER_OUTER_CURVE
                }
            }
        }
    }
}

impl Curve {
    pub const fn from_input_output(input: HDir, output: HDir) -> Option<Self> {
        use HDir::*;
        match (input, output) {
            (North, East) => Some(Self::NorthToEast),
            (North, West) => Some(Self::NorthToWest),
            (South, East) => Some(Self::SouthToEast),
            (South, West) => Some(Self::SouthToWest),
            (East, North) => Some(Self::EastToNorth),
            (East, South) => Some(Self::EastToSouth),
            (West, North) => Some(Self::WestToNorth),
            (West, South) => Some(Self::WestToSouth),
            _ => None,
        }
    }
    pub const fn input(&self) -> HDir {
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

    pub const fn output(&self) -> HDir {
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

    pub const fn is_clockwise(&self) -> bool {
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

    pub const fn inner_lane(&self) -> Side {
        if self.is_clockwise() {
            Side::Right
        } else {
            Side::Left
        }
    }
    #[expect(unused)]
    pub const fn outet_lane(&self) -> Side {
        if self.is_clockwise() {
            Side::Left
        } else {
            Side::Right
        }
    }
}

// -----------
// Trait impls
// -----------

impl From<WorldCoords> for Vec3 {
    fn from(coords: WorldCoords) -> Self {
        Vec3::new(
            coords.x as f32 * BLOCK_SIZE,
            coords.y as f32 * HALF_BLOCK_SIZE,
            coords.z as f32 * BLOCK_SIZE,
        )
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

impl<T> std::ops::Index<Side> for Sided<T> {
    type Output = T;

    fn index(&self, index: Side) -> &Self::Output {
        match index {
            Side::Left => &self.left,
            Side::Right => &self.right,
        }
    }
}

impl<T> std::ops::IndexMut<Side> for Sided<T> {
    fn index_mut(&mut self, index: Side) -> &mut Self::Output {
        match index {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
        }
    }
}

// --------
// Functions
// ---------

/// For Straight and Curved Belts, a po of 0 will put the item
/// as far as it should go, when at the head
pub fn item_position(
    belt: BeltShape,
    coords: impl Into<WorldCoords>,
    lane: Side,
    pos: i32,
) -> Transform {
    match belt {
        BeltShape::Straight(dir) => {
            let z = match lane {
                Side::Left => -LANE_OFFSET,
                Side::Right => LANE_OFFSET,
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
    }
}

#[cfg(test)]
pub fn assert_close(left: Vec3, right: Vec3) {
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
    app.init_resource::<PlacementErrors>();
    app
}

#[derive(Debug)]
pub enum ItemPlacementError {
    BeltNotFound,
    #[expect(unused)]
    PositionOutOfBounds,
    PositionOccupied,
}

#[cfg(test)]
#[derive(Resource, Default)]
pub struct PlacementErrors {
    pub errors: Vec<ItemPlacementError>,
}

#[cfg(test)]
pub struct Layout {
    belts: HashMap<(i32, i32), (Entity, HDir)>,
}

#[cfg(test)]
impl Layout {
    pub fn get(&self, x: i32, z: i32) -> Entity {
        self.belts
            .get(&(x, z))
            .map(|&(e, _)| e)
            .unwrap_or_else(|| panic!("No belt at ({x}, {z})"))
    }

    /// Transition to a new layout string, diffing against this one:
    /// - belts present in `s` but not here → `add_belt` called
    /// - belts present in both with the same direction → entity reused, no call
    /// - belts present in both with a different direction → `add_belt` called (replacement)
    /// - belts present here but not in `s` → `remove_belt_at` called
    pub fn update(&self, app: &mut App, s: &str) -> Layout {
        let new_entries = parse_layout(s);
        let new_coords: std::collections::HashSet<(i32, i32)> =
            new_entries.iter().map(|&(x, z, _)| (x, z)).collect();

        for (&(x, z), _) in &self.belts {
            if !new_coords.contains(&(x, z)) {
                app.remove_belt_at((x, 0, z));
            }
        }

        let belts = new_entries
            .into_iter()
            .map(|(x, z, dir)| {
                let e = match self.belts.get(&(x, z)) {
                    Some(&(entity, old_dir)) if old_dir == dir => entity,
                    _ => app.add_belt((x, 0, z), dir),
                };
                ((x, z), (e, dir))
            })
            .collect();

        Layout { belts }
    }
}

#[cfg(test)]
fn parse_layout(s: &str) -> Vec<(i32, i32, HDir)> {
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();

    // Single-belt shorthand: no axes needed, belt placed at (0,0,0)
    let has_axes = lines.iter().any(|l| l.contains('|') || l.contains('-'));
    let (h_row, v_col) = if has_axes {
        let h = lines
            .iter()
            .position(|l| l.contains('-'))
            .expect("layout with '|' also needs a '-' axis row") as i32;
        let v = lines
            .iter()
            .find_map(|l| l.chars().position(|c| c == '|'))
            .expect("layout with '-' also needs a '|' axis column") as i32;
        (h, v)
    } else {
        // Find the single belt char and treat its position as (0,0,0)
        let (row, col) = lines
            .iter()
            .enumerate()
            .find_map(|(r, l)| l.chars().position(|c| ">^<v".contains(c)).map(|c| (r, c)))
            .expect("layout must contain at least one belt character");
        (row as i32, col as i32)
    };

    let mut out = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        let x = h_row - row as i32;
        for (col, ch) in line.chars().enumerate() {
            let dir = match ch {
                '>' => HDir::East,
                '<' => HDir::West,
                '^' => HDir::North,
                'v' => HDir::South,
                _ => continue,
            };
            let z = col as i32 - v_col;
            out.push((x, z, dir));
        }
    }
    out
}

#[cfg(test)]
pub trait AppExtension {
    fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: HDir) -> Entity;
    fn add_item(&mut self, belt: Entity, pos: i32, lane: Side) -> Entity;
    fn find_item(&mut self, item: Entity) -> Option<(Item, Transform)>;
    fn find_belt(&mut self, belt: Entity) -> Option<(BeltShape, Transform)>;
    fn remove_belt_at(&mut self, coords: impl Into<WorldCoords>) -> bool;
    fn layout(&mut self, s: &str) -> Layout;
    #[allow(unused)]
    fn has_placement_errors(&self) -> bool;
    #[allow(unused)]
    fn take_placement_errors(&mut self) -> Vec<ItemPlacementError>;
}

#[cfg(test)]
impl AppExtension for App {
    fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: HDir) -> Entity {
        let entity = self.world_mut().spawn_empty().id();
        self.world_mut().trigger(PlaceBlock {
            entity,
            item: Item::Belt,
            dir,
            coords: coords.into(),
        });
        entity
    }

    fn add_item(&mut self, belt: Entity, pos: i32, lane: Side) -> Entity {
        let entity = self.world_mut().spawn_empty().id();
        self.world_mut().trigger(PlaceItem {
            entity,
            item: Item::Belt,
            belt,
            lane,
            position: pos,
            on_error: Box::new(|mut commands, error| {
                // Record the error in the PlacementErrors resource
                commands.queue(move |world: &mut World| {
                    if let Some(mut errors) = world.get_resource_mut::<PlacementErrors>() {
                        errors.errors.push(error);
                    }
                });
            }),
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

    fn find_belt(&mut self, belt: Entity) -> Option<(BeltShape, Transform)> {
        let world = self.world_mut();
        world
            .query::<(&BeltShape, &Transform)>()
            .get(world, belt)
            .ok()
            .map(|(shape, transform)| (*shape, *transform))
    }

    fn remove_belt_at(&mut self, coords: impl Into<WorldCoords>) -> bool {
        let coords = coords.into();
        todo!();
    }

    fn has_placement_errors(&self) -> bool {
        self.world()
            .get_resource::<PlacementErrors>()
            .map(|e| !e.errors.is_empty())
            .unwrap_or(false)
    }

    fn take_placement_errors(&mut self) -> Vec<ItemPlacementError> {
        self.world_mut()
            .get_resource_mut::<PlacementErrors>()
            .map(|mut e| std::mem::take(&mut e.errors))
            .unwrap_or_default()
    }

    fn layout(&mut self, s: &str) -> Layout {
        let belts = parse_layout(s)
            .into_iter()
            .map(|(x, z, dir)| {
                let e = self.add_belt((x, 0, z), dir);
                ((x, z), (e, dir))
            })
            .collect();
        Layout { belts }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne};
}
