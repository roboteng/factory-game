use crate::core::inventory::{Inventory, Stack};
use bevy::{math::ops::sin_cos, prelude::*};
use derivative::Derivative;
use std::collections::HashMap;
use std::f32::consts::PI;

pub mod dir;
pub mod inventory;

#[cfg(feature = "invariant-check")]
pub mod invariants;

// Re-export direction types; explicit `use` for `Curve` to shadow `bevy::prelude::Curve`.
use dir::Curve;
pub use dir::*;

#[cfg(all(test, feature = "proptests"))]
mod proptest_actions;
#[cfg(all(test, feature = "proptests"))]
mod proptests;

pub const ITEMS_PER_BELT: i32 = 4;
pub const POSITIONS_PER_BELT: i32 = 256;
pub const BASE_BELT_SPEED: i32 = 8;
/// How far from center each lane is.
pub const LANE_OFFSET: f32 = 0.25;
/// How far from the bottom of the voxel the belt surface is.
pub const BELT_HEIGHT: f32 = 0.25;
pub const BELT_HEIGHT_FROM_CENTER: f32 = BELT_HEIGHT - 0.5;

pub const ITEM_SIZE: f32 = 1.0 / (ITEMS_PER_BELT as f32);
pub const HALF_ITEM_SIZE: f32 = ITEM_SIZE / 2.0;
pub const MINER_TICKS_PER_EXTRACT: u32 = 60;
pub const ITEM_SPACING: i32 = POSITIONS_PER_BELT / ITEMS_PER_BELT;
pub const BASE_ITEM_MOVEMENT: f32 = BASE_BELT_SPEED as f32 / POSITIONS_PER_BELT as f32;
pub const POSITIONS_PER_INNER_CURVE: i32 =
    ((0.5 - LANE_OFFSET) * POSITIONS_PER_BELT as f32 * PI / 2.0).round() as i32;
pub const POSITIONS_PER_OUTER_CURVE: i32 =
    ((0.5 + LANE_OFFSET) * POSITIONS_PER_BELT as f32 * PI / 2.0).round() as i32;

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

        app.init_resource::<CoordsMap>();

        app.add_observer(on_place_block);
        app.add_observer(on_place_item);
        app.add_observer(on_remove_block);
        app.add_observer(on_incline);

        let mut inv = Inventory::new();
        inv.insert(Stack::new(Item::Belt, 15)).unwrap();
        inv.insert(Stack::new(Item::IronOre, 5)).unwrap();
        let player = app.world_mut().spawn(inv).id();
        app.insert_resource(Player(player));

        app.add_systems(
            Update,
            (
                determine_belt_shape,
                move_items_on_belts,
                transfer_items,
                set_item_transforms,
                fill_sources,
                fill_miners,
                push_to_belt,
                pull_from_belt,
                process_furnace,
                consume_sink_buffer,
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
    pub block: WorldBlock,
    pub coords: WorldCoords,
    pub dir: HDir,
}

#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct Incline {
    pub entity: Entity,
}

impl PlaceBlock {
    fn to_bundle(&self) -> impl Bundle {
        (
            self.block,
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

#[derive(Component)]
pub struct Belt;

#[derive(Component)]
pub struct Source;

#[derive(Component)]
pub struct Miner {
    ticks: u32,
    dir: HDir,
}

#[derive(Component)]
pub struct Sink;

#[derive(Component, Default)]
pub struct OutputBuffer {
    pub items: Vec<Item>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ProcessingMethod {
    Furnace,
    Assembler,
}

pub struct Recipe {
    pub method: ProcessingMethod,
    pub inputs: &'static [Item],
    pub outputs: &'static [Item],
    pub ticks: u32,
}

pub const RECIPES: &'static [Recipe] = &[
    Recipe {
        method: ProcessingMethod::Furnace,
        inputs: &[Item::IronOre],
        outputs: &[Item::IronIngot],
        ticks: 100,
    },
    Recipe {
        method: ProcessingMethod::Furnace,
        inputs: &[Item::CopperOre],
        outputs: &[Item::CopperIngot],
        ticks: 100,
    },
];

#[derive(Component, Default)]
pub struct Furnace {
    pub ticks: u32,
}

#[derive(Component)]
pub struct InputBuffer {
    pub recipe: Option<&'static Recipe>,
    pub method: Option<ProcessingMethod>,
    pub slots: Vec<Option<Item>>,
}

impl InputBuffer {
    pub fn all() -> Self {
        Self {
            recipe: None,
            method: None,
            slots: vec![None],
        }
    }

    pub fn for_method(method: ProcessingMethod) -> Self {
        Self {
            recipe: None,
            method: Some(method),
            slots: vec![None],
        }
    }

    pub fn for_recipe(recipe: &'static Recipe) -> Self {
        Self {
            recipe: Some(recipe),
            method: None,
            slots: vec![None; recipe.inputs.len()],
        }
    }

    pub fn accepts(&self, item: Item) -> bool {
        if let Some(recipe) = self.recipe {
            return recipe
                .inputs
                .iter()
                .zip(self.slots.iter())
                .any(|(&expected, slot)| slot.is_none() && expected == item);
        }
        if let Some(method) = self.method {
            return self.slots.iter().any(|s| s.is_none())
                && RECIPES
                    .iter()
                    .filter(|r| r.method == method)
                    .any(|r| r.inputs.contains(&item));
        }
        self.slots.iter().any(|s| s.is_none())
    }

    pub fn fill_slot(&mut self, item: Item) {
        if let Some(r) = self.recipe {
            if let Some((_, slot)) = r
                .inputs
                .iter()
                .zip(self.slots.iter_mut())
                .find(|(expected, slot)| slot.is_none() && **expected == item)
            {
                *slot = Some(item);
            }
            return;
        }
        if let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) {
            *slot = Some(item);
        }
    }

    pub fn is_ready(&self) -> bool {
        self.slots.iter().all(|s| s.is_some())
    }
}

/// Output direction for belt pushers. `None` = try all four horizontal directions.
#[derive(Component)]
pub struct OutputDir(pub Option<HDir>);

#[derive(Component)]
pub struct AffectsBelts;

#[derive(Component)]
struct DirtyBelt;

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
        half_extents: Vec3::new(0.5, BELT_HEIGHT / 2.0, 0.5),
    };
    /// Full-block tall (Rock / Dirt / Source / Sink).
    pub const FULL_BLOCK: Self = Self {
        half_extents: Vec3::splat(0.5),
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

/// Item type — things that exist in the player's inventory or flow on belts.
#[derive(Component, Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub enum Item {
    Belt,
    Source,
    Sink,
    Rock,
    Dirt,
    IronOre,
    CopperOre,
    IronIngot,
    CopperIngot,
    Miner,
    Furnace,
}

impl Item {
    pub fn name(self) -> &'static str {
        match self {
            Item::Belt => "Belt",
            Item::Source => "Source",
            Item::Sink => "Sink",
            Item::Rock => "Rock",
            Item::Dirt => "Dirt",
            Item::IronOre => "Iron Ore",
            Item::CopperOre => "Copper Ore",
            Item::IronIngot => "Iron Ingot",
            Item::CopperIngot => "Copper Ingot",
            Item::Miner => "Miner",
            Item::Furnace => "Furnace",
        }
    }

    /// Returns the world block this item places, or `None` if the item cannot be placed.
    pub fn can_place(self) -> Option<WorldBlock> {
        match self {
            Item::Belt => Some(WorldBlock::Belt),
            Item::Source => Some(WorldBlock::Source),
            Item::Sink => Some(WorldBlock::Sink),
            Item::Rock => Some(WorldBlock::Rock),
            Item::Dirt => Some(WorldBlock::Dirt),
            Item::Miner => Some(WorldBlock::Miner),
            Item::Furnace => Some(WorldBlock::Furnace),
            Item::IronOre | Item::CopperOre | Item::IronIngot | Item::CopperIngot => None,
        }
    }
}

/// World block type — everything that occupies a position in the world, whether placed by the
/// player or spawned by world generation. Not all world blocks have a corresponding item.
#[derive(Component, Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub enum WorldBlock {
    Belt,
    Source,
    Sink,
    Rock,
    Dirt,
    IronOreDeposit,
    CopperOreDeposit,
    Miner,
    Furnace,
}

impl WorldBlock {
    pub fn name(self) -> &'static str {
        match self {
            WorldBlock::Belt => "Belt",
            WorldBlock::Source => "Source",
            WorldBlock::Sink => "Sink",
            WorldBlock::Rock => "Rock",
            WorldBlock::Dirt => "Dirt",
            WorldBlock::IronOreDeposit => "Iron Ore Deposit",
            WorldBlock::CopperOreDeposit => "Copper Ore Deposit",
            WorldBlock::Miner => "Miner",
            WorldBlock::Furnace => "Furnace",
        }
    }

    /// Item produced when a miner harvests this block. `None` means not minable.
    pub fn mine(self) -> Option<Item> {
        match self {
            WorldBlock::IronOreDeposit => Some(Item::IronOre),
            WorldBlock::CopperOreDeposit => Some(Item::CopperOre),
            _ => None,
        }
    }

    /// Item dropped when a player breaks this block. `None` means this block cannot be broken.
    pub fn break_drop(self) -> Option<Item> {
        match self {
            WorldBlock::Belt => Some(Item::Belt),
            WorldBlock::Source => Some(Item::Source),
            WorldBlock::Sink => Some(Item::Sink),
            WorldBlock::Rock => Some(Item::Rock),
            WorldBlock::Dirt => Some(Item::Dirt),
            WorldBlock::Miner => Some(Item::Miner),
            WorldBlock::Furnace => Some(Item::Furnace),
            WorldBlock::IronOreDeposit | WorldBlock::CopperOreDeposit => None,
        }
    }

    pub fn raycast_target(self) -> RaycastTarget {
        match self {
            WorldBlock::Belt => RaycastTarget::HALF_BLOCK,
            _ => RaycastTarget::FULL_BLOCK,
        }
    }

    pub fn size(&self) -> BlockSize {
        match self {
            WorldBlock::Belt => BlockSize::HALF_BLOCK,
            WorldBlock::Source
            | WorldBlock::Sink
            | WorldBlock::Rock
            | WorldBlock::Dirt
            | WorldBlock::IronOreDeposit
            | WorldBlock::CopperOreDeposit
            | WorldBlock::Miner => BlockSize::HALF_BLOCK,
            WorldBlock::Furnace => BlockSize {
                height: 6,
                width: 2,
                depth: 2,
            },
        }
    }
}

pub struct BlockSize {
    /// How many half blocks it takes up
    height: u8,
    width: u8,
    depth: u8,
}

impl BlockSize {
    pub const HALF_BLOCK: Self = BlockSize {
        height: 1,
        width: 1,
        depth: 1,
    };
    pub const FULL_BLOCK: Self = BlockSize {
        height: 2,
        width: 1,
        depth: 1,
    };
}

#[derive(Component, Debug, PartialEq, Eq, Clone, Default)]
pub struct Sided<T> {
    pub left: T,
    pub right: T,
}

#[derive(Resource)]
pub struct Player(pub Entity);

#[derive(Resource, Default)]
pub struct CoordsMap(pub HashMap<WorldCoords, Entity>);

// -------
// Systems
// -------

fn despawn_old_entities(mut cmd: Commands, q: Query<Entity, With<Delete>>) {
    for entity in q {
        cmd.entity(entity).despawn();
    }
}

fn mark_belt_neighbors_dirty(center: WorldCoords, coord_map: &CoordsMap, cmd: &mut Commands) {
    for pos in center.horizontal_neighbors() {
        if let Some(&e) = coord_map.0.get(&pos) {
            cmd.entity(e).insert(DirtyBelt);
        }
    }
}

fn on_place_block(
    event: On<PlaceBlock>,
    mut cmd: Commands,
    mut coord_map: ResMut<CoordsMap>,
    belts_q: Query<&ItemLanes, With<Belt>>,
) {
    let rt = event.block.raycast_target();
    let is_full = rt.half_extents.y > 0.25;

    // Full-height blocks must sit at an even y coordinate. If the ray lands
    // on an odd slot (e.g. top face of a belt), snap down to the nearest even.
    let coords = if is_full {
        event.coords.snap_height_even()
    } else {
        event.coords
    };

    let place = PlaceBlock {
        coords,
        ..*event.event()
    };

    debug!(
        "Placing {:?} at {coords:?} facing {:?}",
        event.block, event.dir
    );

    // For full-height blocks, also check the top slot.
    if is_full && coord_map.0.contains_key(&coords.step(Dir::Up)) {
        cmd.entity(event.entity).despawn();
        return;
    }

    // Check for an existing block at this location.
    if let Some(&existing) = coord_map.0.get(&coords) {
        if event.block == WorldBlock::Belt {
            if let Ok(old_lanes) = belts_q.get(existing) {
                // Belt-on-belt: replace the old belt and transfer its items to the new one.
                let transferred = old_lanes.0.clone();
                cmd.entity(existing).despawn();
                coord_map.0.remove(&coords);
                cmd.entity(event.entity)
                    .insert((Belt, ItemLanes(transferred), AffectsBelts, rt));
                cmd.entity(event.entity).insert(place.to_bundle());
                coord_map.0.insert(coords, event.entity);
                mark_belt_neighbors_dirty(coords, &coord_map, &mut cmd);
                return;
            }
        }
        // Any other collision: ignore the placement.
        cmd.entity(event.entity).despawn();
        return;
    }

    match event.block {
        WorldBlock::Belt => {
            cmd.entity(event.entity)
                .insert((Belt, ItemLanes::default(), AffectsBelts, rt));
        }
        WorldBlock::Source => {
            cmd.entity(event.entity).insert((
                Source,
                OutputBuffer::default(),
                OutputDir(Some(place.dir)),
                AffectsBelts,
                rt,
            ));
        }
        WorldBlock::Sink => {
            cmd.entity(event.entity)
                .insert((Sink, InputBuffer::all(), AffectsBelts, rt));
        }
        WorldBlock::Miner => {
            cmd.entity(event.entity).insert((
                Miner {
                    ticks: 0,
                    dir: event.dir,
                },
                OutputBuffer::default(),
                OutputDir(None),
                AffectsBelts,
                rt,
            ));
        }
        WorldBlock::Furnace => {
            cmd.entity(event.entity).insert((
                Furnace::default(),
                InputBuffer::for_method(ProcessingMethod::Furnace),
                OutputBuffer::default(),
                OutputDir(Some(place.dir)),
                AffectsBelts,
                rt,
            ));
        }
        WorldBlock::Rock
        | WorldBlock::Dirt
        | WorldBlock::IronOreDeposit
        | WorldBlock::CopperOreDeposit => {
            cmd.entity(event.entity).insert(rt);
        }
    };

    cmd.entity(event.entity).insert(place.to_bundle());
    coord_map.0.insert(coords, event.entity);
    // Register the second slot for full-height blocks.
    if is_full {
        coord_map.0.insert(coords.step(Dir::Up), event.entity);
    }
    mark_belt_neighbors_dirty(coords, &coord_map, &mut cmd);
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
    mut coord_map: ResMut<CoordsMap>,
    mut cmd: Commands,
) {
    debug!("Removing {:?}", event.entity);
    if let Ok(coords) = coords_q.get(event.entity) {
        mark_belt_neighbors_dirty(*coords, &coord_map, &mut cmd);
        coord_map.0.remove(coords);
        // Remove the top slot if this entity registered it (full-height blocks).
        let top = coords.step(Dir::Up);
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

/// Whether the geometry permits ramping up: nothing directly above.
fn can_ramp_up(coords: WorldCoords, coord_map: &CoordsMap) -> bool {
    !coord_map.0.contains_key(&coords.step(Dir::Up))
}

/// Whether the geometry permits ramping down: nothing directly below.
fn can_ramp_down(coords: WorldCoords, coord_map: &CoordsMap) -> bool {
    !coord_map.0.contains_key(&coords.step(Dir::Down))
}

/// Whether a belt should automatically become a ramp based on neighboring belts.
fn should_ramp(dir: HDir, coords: WorldCoords, coord_map: &CoordsMap) -> Option<BeltShape> {
    let forward = coords.step(dir);
    let forward_up = forward.step(Dir::Up);
    let forward_down = forward.step(Dir::Down);

    let forward_clear = !coord_map.0.contains_key(&forward);

    if coord_map.0.contains_key(&forward_up) && forward_clear && can_ramp_up(coords, coord_map) {
        Some(BeltShape::RampUp(dir))
    } else if coord_map.0.contains_key(&forward_down)
        && forward_clear
        && can_ramp_down(coords, coord_map)
    {
        Some(BeltShape::RampDown(dir))
    } else {
        None
    }
}

fn on_incline(
    event: On<Incline>,
    mut belts: Query<(&mut BeltShape, &WorldCoords)>,
    coords_map: Res<CoordsMap>,
) {
    let Ok((mut belt, &coords)) = belts.get_mut(event.entity) else {
        return;
    };
    match belt.as_ref().clone() {
        BeltShape::Straight(dir) => {
            if let Some(ramp) = should_ramp(dir, coords, &coords_map) {
                belt.set_if_neq(ramp);
            } else if can_ramp_up(coords, &coords_map) {
                belt.set_if_neq(BeltShape::RampUp(dir));
            } else if can_ramp_down(coords, &coords_map) {
                belt.set_if_neq(BeltShape::RampDown(dir));
            }
        }
        BeltShape::RampUp(dir) => {
            if can_ramp_down(coords, &coords_map) {
                belt.set_if_neq(BeltShape::RampDown(dir));
            } else {
                belt.set_if_neq(BeltShape::Straight(dir));
            }
        }
        BeltShape::RampDown(dir) => {
            belt.set_if_neq(BeltShape::Straight(dir));
        }
        BeltShape::Curve(_) => {}
    };
}

fn determine_belt_shape(
    mut belts: Query<
        (Entity, &WorldCoords, &HDir, Option<&mut BeltShape>),
        (With<Belt>, Or<(Added<Belt>, With<DirtyBelt>)>),
    >,
    affecters: Query<&HDir, With<AffectsBelts>>,
    coord_map: Res<CoordsMap>,
    mut cmd: Commands,
) {
    for (entity, coords, &dir, current_shape) in belts.iter_mut() {
        let feeds_from = |step: WorldCoords, expected: HDir| {
            coord_map
                .0
                .get(&step)
                .and_then(|&e| affecters.get(e).ok())
                .is_some_and(|&d| d == expected)
        };
        let fed_from_left = feeds_from(coords.step(dir.left()), dir.right());
        let fed_from_right = feeds_from(coords.step(dir.right()), dir.left());
        let fed_from_behind = feeds_from(coords.step(dir.opposite()), dir);
        let desired = match (fed_from_left, fed_from_behind, fed_from_right) {
            (true, false, false) => {
                let curve = Curve::from_input_output(dir.right(), dir).unwrap();
                assert_eq!(curve.output(), dir);
                BeltShape::Curve(curve)
            }
            (false, false, true) => {
                let curve = Curve::from_input_output(dir.left(), dir).unwrap();
                assert_eq!(curve.output(), dir);
                BeltShape::Curve(curve)
            }
            (false, false, false) => BeltShape::Straight(dir),
            (_, true, _) => BeltShape::Straight(dir),
            (true, _, true) => BeltShape::Straight(dir),
        };
        let desired = if matches!(desired, BeltShape::Straight(_)) {
            should_ramp(dir, *coords, &coord_map).unwrap_or(desired)
        } else {
            desired
        };
        match current_shape {
            Some(mut shape) => {
                if matches!(coord_map.0.get(&coords.step(shape.clone())), Some(_)) {
                    // I'd like to check if its really a belt here or not
                } else {
                    shape.set_if_neq(desired);
                }
            }
            None => {
                cmd.entity(entity).insert(desired);
            }
        }
        cmd.entity(entity).remove::<DirtyBelt>();
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
    mut invs: Query<(Entity, &mut ItemLanes, &WorldCoords, &BeltShape)>,
    coord_map: Res<CoordsMap>,
) {
    struct Transfer {
        source: Entity,
        dest: Entity,
        lane: Side,
    }
    let mut transfers = Vec::new();
    for source in invs.iter() {
        let next = source.2.step(source.3.belt_output());
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
                    < dest.3.num_pos(side)
                && source.3.output() == dest.3.input()
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
        lane.push((dest.3.num_pos(transfer.lane), slot.1));
    }
}

fn side_loading(
    mut invs: Query<(Entity, &mut ItemLanes, &WorldCoords, &BeltShape)>,
    coord_map: Res<CoordsMap>,
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
        let next = source.2.step(source.3.belt_output());
        let Some(&dest_entity) = coord_map.0.get(&next) else {
            continue;
        };
        let Ok(dest) = invs.get(dest_entity) else {
            continue;
        };
        if matches!(
            dest.3,
            BeltShape::Straight(_) | BeltShape::RampUp(_) | BeltShape::RampDown(_)
        ) && (source.3.output() == dest.3.input().left()
            || source.3.output() == dest.3.input().right())
        {
            let dest_side = if source.3.output() == dest.3.input().right() {
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
                        < dest.3.num_pos(dest_side)
                {
                    const OFFSET: i32 = (POSITIONS_PER_BELT as f32 * LANE_OFFSET).round() as i32;
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

fn fill_sources(mut sources: Query<&mut OutputBuffer, With<Source>>) {
    for mut buffer in &mut sources {
        if buffer.items.is_empty() {
            buffer.items.push(Item::Belt);
        }
    }
}

fn fill_miners(
    mut miners: Query<(&WorldCoords, &mut Miner, &mut OutputBuffer)>,
    world_blocks: Query<&WorldBlock>,
    coord_map: Res<CoordsMap>,
) {
    for (miner_coords, mut miner, mut buffer) in &mut miners {
        if !buffer.items.is_empty() {
            continue;
        }
        miner.ticks += 1;
        if miner.ticks < MINER_TICKS_PER_EXTRACT {
            continue;
        }
        miner.ticks = 0;

        let Some(&item) = coord_map.0.get(&miner_coords.step(miner.dir)) else {
            continue;
        };
        let Ok(block) = world_blocks.get(item) else {
            continue;
        };
        let Some(item) = block.mine() else {
            continue;
        };
        buffer.items.push(item);
    }
}

fn push_to_belt(
    mut pushers: Query<(&mut OutputBuffer, &WorldCoords, &OutputDir)>,
    belts: Query<(Entity, &ItemLanes), With<Belt>>,
    coord_map: Res<CoordsMap>,
    mut cmd: Commands,
) {
    const ALL_DIRS: [HDir; 4] = [HDir::North, HDir::South, HDir::East, HDir::West];
    for (mut buffer, coords, output_dir) in &mut pushers {
        let Some(&item) = buffer.items.first() else {
            continue;
        };
        let dirs: &[HDir] = match &output_dir.0 {
            Some(d) => std::slice::from_ref(d),
            None => &ALL_DIRS,
        };
        for &dir in dirs {
            let target = coords.step(dir);
            let Some(&belt_entity) = coord_map.0.get(&target) else {
                continue;
            };
            let Ok((belt_entity, lanes)) = belts.get(belt_entity) else {
                continue;
            };
            if lanes.0.left.len() >= ITEMS_PER_BELT as usize {
                continue;
            }
            buffer.items.remove(0);
            let entity = cmd.spawn_empty().id();
            cmd.trigger(PlaceItem {
                entity,
                item,
                belt: belt_entity,
                lane: Side::Left,
                position: POSITIONS_PER_BELT,
                on_error: Box::new(|_, _| {}),
            });
            break;
        }
    }
}

fn pull_from_belt(
    mut sinks: Query<(&mut InputBuffer, &WorldCoords)>,
    mut belts: Query<(&mut ItemLanes, &HDir)>,
    items: Query<&Item, With<OnBelt>>,
    coord_map: Res<CoordsMap>,
    mut cmd: Commands,
) {
    for (mut buffer, sink_coords) in &mut sinks {
        if buffer.is_ready() {
            continue;
        }
        for d in [HDir::North, HDir::South, HDir::East, HDir::West] {
            let neighbor = sink_coords.step(d.opposite());
            let Some(&belt_entity) = coord_map.0.get(&neighbor) else {
                continue;
            };
            let Ok((mut lanes, belt_dir)) = belts.get_mut(belt_entity) else {
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
                let item_entity = lead_item.1;
                let Ok(&item) = items.get(item_entity) else {
                    continue;
                };
                if !buffer.accepts(item) {
                    continue;
                }
                lanes.0[side].remove(0);
                cmd.entity(item_entity).despawn();
                buffer.fill_slot(item);
                break;
            }
            if buffer.is_ready() {
                break;
            }
        }
    }
}

fn process_furnace(mut furnaces: Query<(&mut Furnace, &mut InputBuffer, &mut OutputBuffer)>) {
    for (mut furnace, mut input, mut output) in &mut furnaces {
        if !input.is_ready() {
            furnace.ticks = 0;
            continue;
        }
        if !output.items.is_empty() {
            continue;
        }
        let recipe = RECIPES
            .iter()
            .filter(|r| r.method == ProcessingMethod::Furnace)
            .find(|r| {
                r.inputs.len() == input.slots.len()
                    && r.inputs
                        .iter()
                        .zip(&input.slots)
                        .all(|(exp, slot)| slot.as_ref() == Some(exp))
            });
        let Some(recipe) = recipe else {
            continue;
        };
        furnace.ticks += 1;
        if furnace.ticks < recipe.ticks {
            continue;
        }
        furnace.ticks = 0;
        output.items.extend_from_slice(recipe.outputs);
        for slot in &mut input.slots {
            *slot = None;
        }
    }
}

fn consume_sink_buffer(mut sinks: Query<&mut InputBuffer, With<Sink>>) {
    for mut buffer in &mut sinks {
        for slot in &mut buffer.slots {
            *slot = None;
        }
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
            let x = match lane {
                Side::Left => LANE_OFFSET,
                Side::Right => -LANE_OFFSET,
            };
            let start = Vec3::new(x, BELT_HEIGHT_FROM_CENTER, 0.5);
            let end = Vec3::new(x, BELT_HEIGHT_FROM_CENTER, -0.5);

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
                0.5 - LANE_OFFSET
            } else {
                0.5 + LANE_OFFSET
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
                    Vec2 { x: cos, y: sin }
                };
            debug!(
                "center_offset: {center_offset:?}, lane_offset: {lane_offset}, local_offset: {:?}, ",
                local_offset
            );
            Transform::from_translation(
                Vec3::new(local_offset.y, BELT_HEIGHT_FROM_CENTER, local_offset.x)
                    + Vec3::from(coords.into()),
            )
            .with_rotation(Quat::from_rotation_y(angle + PI / 2.0))
        }
        BeltShape::RampUp(dir) => {
            let coords = coords.into();
            let mut lower = item_position(BeltShape::Straight(dir), coords, lane, pos);
            let upper = item_position(BeltShape::Straight(dir), coords.step(Dir::Up), lane, pos);
            let t = (POSITIONS_PER_BELT - pos) as f32 / POSITIONS_PER_BELT as f32;
            let translation = lower.translation * (1.0 - t) + upper.translation * t;
            lower.translation = translation;
            lower
        }
        BeltShape::RampDown(dir) => {
            let coords = coords.into();
            let mut upper = item_position(BeltShape::Straight(dir), coords, lane, pos);
            let lower = item_position(BeltShape::Straight(dir), coords.step(Dir::Down), lane, pos);
            let t = (POSITIONS_PER_BELT - pos) as f32 / POSITIONS_PER_BELT as f32;
            let translation = upper.translation * (1.0 - t) + lower.translation * t;
            upper.translation = translation;
            upper
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
    fn add_world_block(&mut self, coords: impl Into<WorldCoords>, block: WorldBlock) -> Entity;
    fn add_item(&mut self, belt: Entity, pos: i32, lane: Side) -> Entity;
    fn find_item(&mut self, item: Entity) -> Option<(Item, Transform)>;
    fn find_belt(&mut self, belt: Entity) -> Option<(BeltShape, Transform)>;
    fn item_count_on_belt(&mut self, belt: Entity) -> usize;
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
            block: WorldBlock::Belt,
            dir,
            coords: coords.into(),
        });
        entity
    }

    fn add_world_block(&mut self, coords: impl Into<WorldCoords>, block: WorldBlock) -> Entity {
        let entity = self.world_mut().spawn_empty().id();
        self.world_mut().trigger(PlaceBlock {
            entity,
            block,
            dir: HDir::North,
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

    fn item_count_on_belt(&mut self, belt: Entity) -> usize {
        let world = self.world_mut();
        world
            .query::<&ItemLanes>()
            .get(world, belt)
            .map(|lanes| lanes.0.left.len() + lanes.0.right.len())
            .unwrap_or(0)
    }

    fn remove_belt_at(&mut self, coords: impl Into<WorldCoords>) -> bool {
        let coords = coords.into();
        let entity = self.world().resource::<CoordsMap>().0.get(&coords).copied();
        if let Some(entity) = entity {
            self.world_mut().trigger(RemoveBlock { entity });
            true
        } else {
            false
        }
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

    #[test]
    fn single_belt_is_straight_north() {
        single_belt_is_straight(HDir::North);
    }

    #[test]
    fn single_belt_is_straight_south() {
        single_belt_is_straight(HDir::South);
    }

    #[test]
    fn single_belt_is_straight_east() {
        single_belt_is_straight(HDir::East);
    }

    #[test]
    fn single_belt_is_straight_west() {
        single_belt_is_straight(HDir::West);
    }

    fn single_belt_is_straight(dir: HDir) {
        let mut app = test_app();

        let belt = app.add_belt(WorldCoords::ORIGIN, dir);
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::Straight(dir));
    }

    #[test]
    fn flat_belt_curves() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        app.add_belt(o.step(HDir::South), HDir::North);
        app.update();

        let belt = app.add_belt(o, HDir::West);
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::Curve(Curve::NorthToWest));
    }

    #[test]
    fn incline_belt() {
        let mut app = test_app();
        let belt = app.add_belt(WorldCoords::ORIGIN, HDir::North);
        app.update();

        app.world_mut().trigger(Incline { entity: belt });
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::RampUp(HDir::North));
    }

    #[test]
    fn incline_belt_with_belt_in_front() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        let belt = app.add_belt(o, HDir::North);
        app.add_belt(o.step(HDir::North), HDir::North);
        app.update();

        app.world_mut().trigger(Incline { entity: belt });
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::RampUp(HDir::North));
    }

    #[test]
    fn incline_belt_on_placement() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        app.add_belt(o.step(HDir::North).step(Dir::Up), HDir::North);
        app.update();

        let belt = app.add_belt(o, HDir::North);
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::RampUp(HDir::North));
    }

    #[test]
    fn not_incline_belt_on_placement_in_front() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        app.add_belt(o.step(HDir::North).step(Dir::Up), HDir::North);
        app.add_belt(o.step(HDir::North), HDir::North);
        app.update();

        let belt = app.add_belt(o, HDir::North);
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::Straight(HDir::North));
    }

    #[test]
    fn not_incline_belt_on_placement_above() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        app.add_belt(o.step(HDir::North).step(Dir::Up), HDir::North);
        app.add_belt(o.step(Dir::Up), HDir::North);
        app.update();

        let belt = app.add_belt(o, HDir::North);
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::Straight(HDir::North));
    }

    #[test]
    fn incline_with_above_filled_becomes_ramp_down() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        let belt = app.add_belt(o, HDir::North);
        app.add_belt(o.step(Dir::Up), HDir::North);
        app.update();

        app.world_mut().trigger(Incline { entity: belt });
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::RampDown(HDir::North));
    }

    #[test]
    fn incline_ramp_down_becomes_straight() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        let belt = app.add_belt(o, HDir::North);
        app.add_belt(o.step(Dir::Up), HDir::North);
        app.update();

        // First incline: Straight -> RampDown (above filled)
        app.world_mut().trigger(Incline { entity: belt });
        app.update();
        // Second incline: RampDown -> Straight
        app.world_mut().trigger(Incline { entity: belt });
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::Straight(HDir::North));
    }

    #[test]
    fn incline_ramp_up_with_below_filled_becomes_straight() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        let belt = app.add_belt(o.step(Dir::Up), HDir::North);
        app.add_belt(o, HDir::North);
        app.update();

        // First incline: Straight -> RampUp (nothing above)
        app.world_mut().trigger(Incline { entity: belt });
        app.update();
        let b = app.find_belt(belt).unwrap();
        assert_eq!(b.0, BeltShape::RampUp(HDir::North));

        // Second incline: RampUp -> Straight (below filled, can't ramp down)
        app.world_mut().trigger(Incline { entity: belt });
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::Straight(HDir::North));
    }

    #[test]
    fn miner_does_not_extract_without_adjacent_ore() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;

        // Place iron ore deposit two steps away — not adjacent to the miner.
        app.add_world_block(
            o.step(HDir::South).step(HDir::South),
            WorldBlock::IronOreDeposit,
        );

        let miner = app.world_mut().spawn_empty().id();
        app.world_mut().trigger(PlaceBlock {
            entity: miner,
            block: WorldBlock::Miner,
            coords: o,
            dir: HDir::South,
        });

        let belt = app.add_belt(o.step(HDir::North), HDir::North);

        for _ in 0..=MINER_TICKS_PER_EXTRACT {
            app.update();
        }

        assert_eq!(app.item_count_on_belt(belt), 0);
    }

    #[test]
    fn miner_extracts_ore_onto_belt() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;

        // Place iron ore deposit adjacent to the south of the miner position.
        app.add_world_block(o.step(HDir::South), WorldBlock::IronOreDeposit);

        // Place miner at origin facing the ore to the south.
        let miner = app.world_mut().spawn_empty().id();
        app.world_mut().trigger(PlaceBlock {
            entity: miner,
            block: WorldBlock::Miner,
            coords: o,
            dir: HDir::South,
        });

        // Place belt to the north — the miner's OutputDir(None) will find it.
        let belt = app.add_belt(o.step(HDir::North), HDir::North);

        // Tick until the miner has had enough time to extract and push.
        for _ in 0..=MINER_TICKS_PER_EXTRACT {
            app.update();
        }

        assert!(app.item_count_on_belt(belt) > 0);
    }

    #[test]
    fn miner_outputs_correct_ore_for_deposit() {
        for (deposit, expected_ore) in [
            (WorldBlock::IronOreDeposit, Item::IronOre),
            (WorldBlock::CopperOreDeposit, Item::CopperOre),
        ] {
            let mut app = test_app();
            let o = WorldCoords::ORIGIN;

            app.add_world_block(o.step(HDir::South), deposit);

            let miner = app.world_mut().spawn_empty().id();
            app.world_mut().trigger(PlaceBlock {
                entity: miner,
                block: WorldBlock::Miner,
                coords: o,
                dir: HDir::South,
            });

            let belt = app.add_belt(o.step(HDir::North), HDir::North);

            for _ in 0..=MINER_TICKS_PER_EXTRACT {
                app.update();
            }

            let world = app.world_mut();
            let lanes = world.query::<&ItemLanes>().get(world, belt).unwrap();
            let item_entities: Vec<Entity> = lanes
                .0
                .left
                .iter()
                .chain(lanes.0.right.iter())
                .map(|(_, e)| *e)
                .collect();
            assert!(
                !item_entities.is_empty(),
                "expected ore on belt for {deposit:?}"
            );
            for entity in item_entities {
                let item = *world.query::<&Item>().get(world, entity).unwrap();
                assert_eq!(item, expected_ore, "wrong ore for {deposit:?}");
            }
        }
    }
}
