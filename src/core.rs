pub use crate::core::lane::*;
use bevy::{math::ops::sin_cos, prelude::*};
use derivative::Derivative;
use std::{collections::HashMap, f32::consts::PI, ops::Range, path::PathBuf};

mod lane;

#[cfg(feature = "invariant-check")]
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
pub const BASE_BELT_SPEED: i32 = 8; // Items move 8 positions per frame
#[allow(unused)]
pub const BASE_ITEM_MOVEMENT: f32 = BLOCK_SIZE * BASE_BELT_SPEED as f32 / POSITIONS_PER_BELT as f32;
pub const POSITIONS_PER_FRAGMENT: i32 =
    (POSITIONS_PER_BELT as f32 * (1.0 - LANE_OFFSET_FACTOR * 2.0) / 2.0).round() as i32;
pub const POSITIONS_PER_INNER_CURVE: i32 =
    ((0.5 - LANE_OFFSET_FACTOR) * POSITIONS_PER_BELT as f32 * PI / 2.0).round() as i32;
pub const POSITIONS_PER_OUTER_CURVE: i32 =
    ((0.5 + LANE_OFFSET_FACTOR) * POSITIONS_PER_BELT as f32 * PI / 2.0).round() as i32;
#[allow(unused)]
pub const ITEMS_PER_BELT: i32 = POSITIONS_PER_BELT / ITEM_SPACING;

pub const SIDES: [LaneSide; 2] = [LaneSide::Left, LaneSide::Right];

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "invariant-check")]
        app.add_plugins(crate::core::invariants::InvariantsPlugin);
        app.add_observer(on_place_block);
        app.add_observer(on_place_item);
        app.add_observer(on_remove_block);

        let mut registry = ItemRegistry::default();
        registry.register(
            Item(0),
            ItemRegEntry {
                name: "Item",
                model_path: PathBuf::from("models/item.glb"),
                model_variants: HashMap::new(),
                placement: PlacementCategory::NotWorldPlacable,
            },
        );
        registry.register(
            Item(1),
            ItemRegEntry {
                name: "Belt",
                model_path: PathBuf::from("models/Untitled.glb"),
                // Blender exports scenes alphabetically
                // We don't have control over the order, but it should be stable between different types
                model_variants: HashMap::from([("curve", 0), ("straight", 1)]),
                placement: PlacementCategory::Belt,
            },
        );
        registry.register(
            Item(2),
            ItemRegEntry {
                name: "Splitter",
                model_path: PathBuf::from("models/splitter.glb"),
                model_variants: HashMap::new(),
                placement: PlacementCategory::AffectsBelts,
            },
        );
        registry.register(
            Item(3),
            ItemRegEntry {
                name: "Source",
                model_path: PathBuf::from("models/item.glb"),
                model_variants: HashMap::new(),
                placement: PlacementCategory::Independant,
            },
        );
        registry.register(
            Item(4),
            ItemRegEntry {
                name: "Sink",
                model_path: PathBuf::from("models/item.glb"),
                model_variants: HashMap::new(),
                placement: PlacementCategory::Independant,
            },
        );
        app.insert_resource(registry);

        app.init_resource::<WorldPlacements>();
        app.init_resource::<BlockEvents>();
        app.init_resource::<BeltChanges>();

        app.add_systems(Update, (block_changes).chain());
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

#[derive(Debug, Clone, Copy)]
pub struct PlaceBelt {
    pub entity: Entity,
    pub item: Item,
    pub coords: WorldCoords,
    pub dir: HDir,
}

#[derive(EntityEvent, Derivative)]
#[derivative(Debug)]
pub struct PlaceItem {
    pub entity: Entity,
    pub item: Item,
    pub belt: Entity,
    pub lane: LaneSide,
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
#[derive(Component, Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub struct Item(pub u32);

#[derive(Debug, Component)]
pub struct LaneConnection {
    pub target: Entity,
    pub offset: Sided<i32>,
    pub target_side: LaneSide,
}
#[derive(Debug, Component)]
pub struct LaneLoopConnection {
    pub offset: Sided<i32>,
}

#[derive(Component)]
pub struct InLane {
    pub lane: Entity,
}

/// Entry in the world grid
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridEntry {
    Belt(BeltShape),
    #[expect(unused)]
    BeltAdjacent(BeltAdjacent),
    /// Entities that are placed in the world, but never affect belts directly
    #[expect(unused)]
    Machine(Item),
}

/// Entities that affect belt curving, but don't join belt lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(unused)]
pub enum BeltAdjacent {
    /// Only has an input for belt connections
    Input(HDir),
    /// Only has an output for belt connections
    Output(HDir),
    /// Has an input and output for belt connections
    InputAndOutput { input: HDir, output: HDir },
}

#[derive(Resource, Default)]
pub struct WorldPlacements(HashMap<WorldCoords, (Entity, GridEntry)>);

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

#[derive(Component, Debug, PartialEq, Eq, Clone, Default)]
pub struct Sided<T> {
    pub left: T,
    pub right: T,
}

#[derive(Resource, Default, Debug)]
pub struct BlockEvents(pub Vec<BlockEvent>);

#[derive(Debug, Clone)]
pub enum BlockEvent {
    Place(PlaceBlock),
    Remove(RemoveBlock),
}

#[derive(Resource, Default)]
pub struct ItemRegistry(HashMap<Item, ItemRegEntry>);

impl ItemRegistry {
    pub fn register(&mut self, item: Item, entry: ItemRegEntry) {
        self.0.insert(item, entry);
    }

    pub fn get(&self, item: &Item) -> Option<&ItemRegEntry> {
        self.0.get(item)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ItemRegEntry {
    pub name: &'static str,
    pub model_path: PathBuf,
    pub model_variants: HashMap<&'static str, usize>,
    pub placement: PlacementCategory,
}

impl ItemRegEntry {
    /// Returns the scene index for a variant, defaulting to 0.
    pub fn scene_index(&self, variant: &str) -> usize {
        self.model_variants.get(variant).copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementCategory {
    Belt,
    /// This item should affect how belts curve, but isn't a belt itself
    AffectsBelts,
    /// This item is placable, but doesn't interact with belts
    #[expect(dead_code)]
    Independant,
    /// This cannot be placed as a block in the world
    NotWorldPlacable,
}

// -------
// Systems
// -------

pub fn block_changes(world: &mut World) {
    world.get_resource_mut::<BeltChanges>().unwrap().0.clear();
    let events = world
        .get_resource_mut::<BlockEvents>()
        .unwrap()
        .0
        .split_off(0);
    debug!("Finished collecting events: {events:?}");

    for event in events {
        match event {
            BlockEvent::Place(event) => event_place_block(world, event),
            BlockEvent::Remove(event) => event_remove_block(world, event),
        }
    }
}

fn event_place_block(world: &mut World, event: PlaceBlock) {
    let placement = world
        .resource::<ItemRegistry>()
        .get(&event.item)
        .unwrap_or_else(|| panic!("Item {:?} not found in registry", event.item))
        .placement;
    assert_eq!(
        placement,
        PlacementCategory::Belt,
        "Only belts are currently supported for placement"
    );
    debug!(
        "Placing belt {:?} at {:?} facing {:?}",
        event.entity, event.coords, event.dir
    );
    let mut changes = BeltChanges::default();

    let belt_coords = world.resource();
    let belt = plan_belt_placement(event.into(), belt_coords);
    let angle = belt.output().angle();

    let old_entity_and_belt = belt_coords.get_belt(event.coords);
    if let Some((e, _)) = old_entity_and_belt {
        debug!("Marking entity {e:?} for deletion (replaced by new belt)");
        world.entity_mut(e).insert(Delete);
    }

    debug!(
        "Adding components to entity {:?}: Transform, Belt, BeltShape, WorldCoords",
        event.entity
    );
    world.entity_mut(event.entity).insert((
        Transform::from_translation(Vec3::from(event.coords))
            .with_rotation(Quat::from_rotation_y(angle)),
        Belt,
        belt,
        event.item,
        event.coords,
    ));
    debug!(
        "Updating BeltCoords resource: inserting {:?} at {:?}",
        event.entity, event.coords
    );
    world.resource_mut::<WorldPlacements>().insert(
        event.coords,
        event.entity,
        GridEntry::Belt(belt),
    );

    if let Some((old_entity, old_belt)) = old_entity_and_belt {
        debug!("Found existing belt: {old_entity:?}. Replacing it");
        changes.push(ReplacedBelt {
            entity: event.entity,
            old_entity: Some(old_entity),
            old_belt,
            new_belt: belt,
            coords: event.coords,
        });
        debug!("Marking entity {old_entity:?} for deletion (replaced)");
        world.entity_mut(old_entity).insert(Delete);
    } else {
        changes.push(NewBelt {
            entity: event.entity,
            coords: event.coords,
            belt,
        });
    }

    // Check if placing this belt should curve the belt ahead
    let ahead = event.coords.step(belt.output());
    if let Some((entity, ahead_belt)) = world.resource::<WorldPlacements>().get_belt(ahead) {
        let place = PlaceBlock {
            entity,
            item: Item(1),
            dir: ahead_belt.output(),
            coords: ahead,
        };
        let new_belt = plan_belt_placement(place.into(), world.resource::<WorldPlacements>());
        if ahead_belt != new_belt {
            debug!(
                "Placing belt {:?} affected {entity:?}, updating that belt",
                event.entity
            );
            let angle = new_belt.output().angle();
            debug!(
                "Adding components to entity {:?}: BeltShape, Transform",
                place.entity
            );
            world.entity_mut(place.entity).insert((
                new_belt,
                Transform::from_translation(Vec3::from(place.coords))
                    .with_rotation(Quat::from_rotation_y(angle)),
            ));
            debug!(
                "Updating BeltCoords resource: inserting {:?} at {:?}",
                place.entity, place.coords
            );
            world.resource_mut::<WorldPlacements>().insert(
                place.coords,
                place.entity,
                GridEntry::Belt(new_belt),
            );
            changes.push(ReplacedBelt {
                entity: place.entity,
                old_entity: None, // Same entity, just changed belt type
                new_belt,
                old_belt: ahead_belt,
                coords: place.coords,
            });
        }
    }
    link_belts(world, changes);
}

fn event_remove_block(world: &mut World, event: RemoveBlock) {
    let Ok((belt, prev_coords)) = world
        .query::<(&BeltShape, &WorldCoords)>()
        .get(world, event.entity)
    else {
        warn!(
            "Attempted to remove belt entity {:?} but it doesn't exist",
            event.entity
        );
        return;
    };
    let belt = belt.clone();
    let prev_coords = prev_coords.clone();

    let mut changes = BeltChanges::default();

    debug!("Removing belt at {:?}", prev_coords);
    debug!(
        "Updating BeltCoords resource: removing entry at {:?}",
        prev_coords
    );
    world.resource_mut::<WorldPlacements>().remove(prev_coords);
    changes.push(RemovedBelt {
        entity: event.entity,
        old_belt: belt,
        coords: prev_coords,
    });
    debug!("Marking entity {:?} for deletion", event.entity);
    world.entity_mut(event.entity).insert(Delete);

    // Check if placing this belt should curve the belt ahead
    let ahead = prev_coords.step(belt.output());
    if let Some((entity, ahead_belt)) = world.resource::<WorldPlacements>().get_belt(ahead) {
        let place = PlaceBlock {
            entity,
            item: Item(1),
            dir: ahead_belt.output(),
            coords: ahead,
        };
        let new_belt = plan_belt_placement(place.into(), world.resource::<WorldPlacements>());
        if ahead_belt != new_belt {
            debug!(
                "Placing belt {:?} affected {entity:?}, updating that belt",
                event.entity
            );
            let angle = new_belt.output().angle();
            debug!(
                "Adding components to entity {:?}: BeltShape, Transform",
                place.entity
            );
            world.entity_mut(place.entity).insert((
                new_belt,
                Transform::from_translation(Vec3::from(place.coords))
                    .with_rotation(Quat::from_rotation_y(angle)),
            ));
            debug!(
                "Updating BeltCoords resource: inserting {:?} at {:?}",
                place.entity, place.coords
            );
            world.resource_mut::<WorldPlacements>().insert(
                place.coords,
                place.entity,
                GridEntry::Belt(new_belt),
            );
        }
    }
    link_belts(world, changes);
}

fn despawn_old_entities(mut cmd: Commands, q: Query<Entity, With<Delete>>) {
    for entity in q {
        cmd.entity(entity).despawn();
    }
}

fn on_place_block(event: On<PlaceBlock>, mut events: ResMut<BlockEvents>) {
    debug!(
        "Placing belt {:?} at {:?} facing {:?}",
        event.entity, event.coords, event.dir
    );
    events.0.push(BlockEvent::Place(event.clone()));
}

fn on_place_item(
    event: On<PlaceItem>,
    belts: Query<(&InLane, &BeltShape, &WorldCoords)>,
    mut lanes: Query<&mut BeltLane>,
    mut commands: Commands,
) {
    let lane_ent = belts
        .get(event.belt)
        .expect("Invariant broken: all_belts_and_frags_claim_to_be_in_a_lane")
        .0
        .lane;
    let mut lane = lanes
        .get_mut(lane_ent)
        .expect("Invariant broken: lane_belts_and_inlane_match");

    match lane.add_item(
        ItemEntry {
            pos: event.position,
            item: event.item,
            entity: event.entity,
        },
        event.lane,
        event.belt,
    ) {
        Ok(()) => {
            let belt = belts.get(event.belt).unwrap();
            commands.entity(event.entity).insert((
                event.item,
                item_position(*belt.1, *belt.2, event.lane, event.position),
            ));
        }
        Err(error) => {
            // Call the error handler provided by the caller
            (event.on_error)(commands, error);
        }
    }
}

fn on_remove_block(event: On<RemoveBlock>, mut events: ResMut<BlockEvents>) {
    events.0.push(BlockEvent::Remove(event.clone()));
}

pub fn link_belts(world: &mut World, changed_belts: BeltChanges) {
    debug!("Updating belts: {:?}", changed_belts.0);

    world
        .resource_mut::<BeltChanges>()
        .0
        .extend(changed_belts.0.iter().cloned());

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
                let items = detach_belt(world, &remaining_entities, removed);
                for ItemEntry { entity, .. } in items.0.iter().chain(items.1.iter()) {
                    debug!("Despawning item entity {entity:?} from removed belt lane");
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

impl WorldPlacements {
    pub fn insert(&mut self, coords: WorldCoords, entity: Entity, entry: GridEntry) {
        self.0.insert(coords, (entity, entry));
    }

    pub fn get(&self, coords: WorldCoords) -> Option<(Entity, GridEntry)> {
        self.0.get(&coords).copied()
    }

    pub fn remove(&mut self, coords: WorldCoords) -> Option<(Entity, GridEntry)> {
        self.0.remove(&coords)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&WorldCoords, &(Entity, GridEntry))> {
        self.0.iter()
    }

    /// Convenience method for belt-specific code
    pub fn get_belt(&self, coords: WorldCoords) -> Option<(Entity, BeltShape)> {
        self.0.get(&coords).and_then(|(entity, entry)| {
            if let GridEntry::Belt(belt) = entry {
                Some((*entity, *belt))
            } else {
                None
            }
        })
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
            Self::Fragment(dir) => *dir,
        }
    }
    pub const fn input(&self) -> HDir {
        match self {
            Self::Straight(dir) => *dir,
            Self::Curve(curve) => curve.input(),
            Self::Fragment(dir) => *dir,
        }
    }

    pub const fn num_pos(&self, side: LaneSide) -> i32 {
        match side {
            LaneSide::Left => self.left_num_pos(),
            LaneSide::Right => self.right_num_pos(),
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
            Self::Fragment(_) => POSITIONS_PER_FRAGMENT,
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
            Self::Fragment(_) => POSITIONS_PER_FRAGMENT,
        }
    }

    pub fn is_fragment(&self) -> bool {
        match self {
            Self::Fragment(_) => true,
            _ => false,
        }
    }
}

impl Curve {
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

    pub const fn inner_lane(&self) -> LaneSide {
        if self.is_clockwise() {
            LaneSide::Right
        } else {
            LaneSide::Left
        }
    }
    #[expect(unused)]
    pub const fn outet_lane(&self) -> LaneSide {
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
        self.0.push(change);
    }
}

impl BeltChange {
    pub const fn entity(&self) -> Entity {
        match self {
            BeltChange::New(NewBelt { entity, .. }) => *entity,
            BeltChange::Removed(RemovedBelt { entity, .. }) => *entity,
            BeltChange::Replaced(ReplacedBelt { entity, .. }) => *entity,
        }
    }

    #[expect(dead_code)]
    pub fn coords(&self) -> WorldCoords {
        match self {
            BeltChange::New(NewBelt { coords, .. }) => *coords,
            BeltChange::Removed(RemovedBelt { coords, .. }) => *coords,
            BeltChange::Replaced(ReplacedBelt { coords, .. }) => *coords,
        }
    }
}

impl BeltAdjacent {
    pub fn output_dir(&self) -> Option<HDir> {
        match self {
            Self::Output(output) | Self::InputAndOutput { output, .. } => Some(*output),
            Self::Input(_) => None,
        }
    }
}

impl GridEntry {
    pub fn output_dir(&self) -> Option<HDir> {
        match self {
            Self::Belt(belt) => Some(belt.output()),
            Self::BeltAdjacent(adj) => adj.output_dir(),
            Self::Machine(_) => None,
        }
    }

    pub fn is_belt(&self) -> Option<&BeltShape> {
        match self {
            Self::Belt(belt) => Some(belt),
            _ => None,
        }
    }
}

// -----------
// Trait impls
// -----------

impl From<PlaceBlock> for PlaceBelt {
    fn from(value: PlaceBlock) -> Self {
        PlaceBelt {
            coords: value.coords,
            dir: value.dir,
            entity: value.entity,
            item: value.item,
        }
    }
}

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

impl<T> std::ops::Index<LaneSide> for Sided<T> {
    type Output = T;

    fn index(&self, index: LaneSide) -> &Self::Output {
        match index {
            LaneSide::Left => &self.left,
            LaneSide::Right => &self.right,
        }
    }
}

impl<T> std::ops::IndexMut<LaneSide> for Sided<T> {
    fn index_mut(&mut self, index: LaneSide) -> &mut Self::Output {
        match index {
            LaneSide::Left => &mut self.left,
            LaneSide::Right => &mut self.right,
        }
    }
}

// --------
// Functions
// ---------

/// For Straight and Curved Belts, a po of 0 will put the item
/// as far as it should go, when at the head
///
/// For Fragments, a pos of 0 will put the item where it waits
/// when sideloaded
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
        BeltShape::Fragment(dir) => item_position(
            BeltShape::Straight(dir),
            coords,
            lane,
            pos + POSITIONS_PER_BELT - POSITIONS_PER_FRAGMENT + ITEM_SPACING / 2,
        ),
    }
}

fn plan_belt_placement(trigger: PlaceBelt, belt_coords: &WorldPlacements) -> BeltShape {
    let left = trigger.coords.step(trigger.dir.left());
    let right = trigger.coords.step(trigger.dir.right());
    let behind = trigger.coords.step(trigger.dir.opposite());
    let fed_from_left = belt_coords
        .get(left)
        .and_then(|(_, entry)| entry.output_dir())
        .map(|dir| dir == trigger.dir.right())
        .unwrap_or(false);
    let fed_from_right = belt_coords
        .get(right)
        .and_then(|(_, entry)| entry.output_dir())
        .map(|dir| dir == trigger.dir.left())
        .unwrap_or(false);
    let fed_from_behind = belt_coords
        .get(behind)
        .and_then(|(_, entry)| entry.output_dir())
        .map(|dir| dir == trigger.dir)
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
    let belt_coords = world.resource::<WorldPlacements>();
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
            let mut lane = BeltLane::from_belt(new.belt, new.coords, new.entity);
            debug!(
                "Adding {} items to left lane, {} items to right lane",
                existing_items.0.len(),
                existing_items.1.len()
            );
            lane.insert_items_at(&existing_items.0, LaneSide::Left);
            lane.insert_items_at(&existing_items.1, LaneSide::Right);
            let lane_ent = world.spawn(lane).id();
            debug!(
                "Spawned new lane entity {lane_ent:?} for belt {:?}",
                new.entity
            );
            debug!(
                "Adding InLane component to entity {:?} pointing to lane {lane_ent:?}",
                new.entity
            );
            world.entity_mut(new.entity).insert(InLane::new(lane_ent));
        }
        (None, Some((behind_ent, _))) => {
            debug!("Adding to head of existing lane");
            let lane_ent = get_lane_entity(world, behind_ent);
            let mut lane = get_lane_mut(world, lane_ent);
            debug!("Adding belt {:?} to head of lane {lane_ent:?}", new.entity);
            lane.add_to_head(new.belt, new.coords, new.entity);
            debug!(
                "Adding {} items to left lane, {} items to right lane",
                existing_items.0.len(),
                existing_items.1.len()
            );
            lane.insert_items_at(&existing_items.0, LaneSide::Left);
            lane.insert_items_at(&existing_items.1, LaneSide::Right);
            debug!("Lane is {:#?}", lane);
            debug!(
                "Adding InLane component to entity {:?} pointing to lane {lane_ent:?}",
                new.entity
            );
            world.entity_mut(new.entity).insert(InLane::new(lane_ent));
        }
        (Some((ahead_ent, _, ConnectionType::Direct)), None) => {
            debug!("Adding to tail of existing lane");
            let lane_ent = get_lane_entity(world, ahead_ent);
            let mut lane = get_lane_mut(world, lane_ent);

            let mut new_lane = BeltLane::from_belt(new.belt, new.coords, new.entity);
            debug!(
                "Adding {} items to left lane, {} items to right lane",
                existing_items.0.len(),
                existing_items.1.len()
            );
            new_lane.insert_items_at(&existing_items.0, LaneSide::Left);
            new_lane.insert_items_at(&existing_items.1, LaneSide::Right);
            debug!("Merging new belt into lane {lane_ent:?}");
            lane.merge(new_lane);
            debug!("Lane is {:?}", lane);
            debug!(
                "Adding InLane component to entity {:?} pointing to lane {lane_ent:?}",
                new.entity
            );
            world.entity_mut(new.entity).insert(InLane::new(lane_ent));
        }
        (Some((ahead_ent, _, ConnectionType::Direct)), Some((behind_ent, _))) => {
            debug!("Merging lanes");
            let behind_lane_ent = get_lane_entity(world, behind_ent);
            let ahead_lane_ent = get_lane_entity(world, ahead_ent);
            let mut behind_lane = get_lane_mut(world, behind_lane_ent);
            debug!(
                "Adding belt {:?} to head of lane {behind_lane_ent:?}",
                new.entity
            );
            behind_lane.add_to_head(new.belt, new.coords, new.entity);
            debug!(
                "Adding {} items to left lane, {} items to right lane",
                existing_items.0.len(),
                existing_items.1.len()
            );
            behind_lane.insert_items_at(&existing_items.0, LaneSide::Left);
            behind_lane.insert_items_at(&existing_items.1, LaneSide::Right);

            if behind_lane_ent == ahead_lane_ent {
                debug!("Belt loop detected");
                debug!(
                    "Adding InLane component to entity {:?} pointing to lane {behind_lane_ent:?}",
                    new.entity
                );
                world
                    .entity_mut(new.entity)
                    .insert(InLane::new(behind_lane_ent));
                let mut lane = get_lane_mut(world, behind_lane_ent);
                lane.belts[0].ranges.left.start -= ITEM_SPACING / 2;
                lane.belts[0].ranges.right.start -= ITEM_SPACING / 2;
                let ranges = lane.ranges();
                debug!("Adding LaneLoopConnection component to lane {behind_lane_ent:?}");
                world
                    .entity_mut(behind_lane_ent)
                    .insert(LaneLoopConnection {
                        offset: Sided {
                            left: ranges.left.end - ranges.left.start,
                            right: ranges.right.end - ranges.right.start,
                        },
                    });
                debug!("spawned loop connection");
            } else {
                let behind_lane = get_lane(world, behind_lane_ent).clone();
                for belt in &behind_lane.belts {
                    let belt_ent = belt.entity;
                    debug!(
                        "Adding InLane component to entity {belt_ent:?} pointing to lane {ahead_lane_ent:?}"
                    );
                    world
                        .entity_mut(belt_ent)
                        .insert(InLane::new(ahead_lane_ent));
                }
                let mut lane = get_lane_mut(world, ahead_lane_ent);
                debug!("Merging behind_lane into lane {ahead_lane_ent:?}");
                lane.merge(behind_lane.clone());
                debug!("Lane is {:#?}", lane);
                debug!("Despawning old lane entity {behind_lane_ent:?}");
                world.entity_mut(behind_lane_ent).despawn();
                debug!(
                    "Adding InLane component to entity {:?} pointing to lane {ahead_lane_ent:?}",
                    new.entity
                );
                world
                    .entity_mut(new.entity)
                    .insert(InLane::new(ahead_lane_ent));
            }
        }
        (Some((side_ent, _, ConnectionType::SideLoad(side))), None) => {
            debug!("sideloading new lane");
            let mut lane = BeltLane::from_belt(new.belt, new.coords, new.entity);
            debug!(
                "Adding {} items to left lane, {} items to right lane",
                existing_items.0.len(),
                existing_items.1.len()
            );
            lane.insert_items_at(&existing_items.0, LaneSide::Left);
            lane.insert_items_at(&existing_items.1, LaneSide::Right);
            let lane_ent = world.spawn(lane).id();
            debug!("Spawned new lane entity {lane_ent:?} that sideloads");

            create_sideload_connection(
                world,
                lane_ent,
                side_ent,
                new.belt.output(),
                new.coords.step(new.belt.output()),
                side,
            );

            debug!(
                "Adding InLane component to entity {:?} pointing to lane {lane_ent:?}",
                new.entity
            );
            world.entity_mut(new.entity).insert(InLane::new(lane_ent));
        }
        (Some((side_ent, _, ConnectionType::SideLoad(side))), Some((behind_ent, _))) => {
            let lane_ent = get_lane_entity(world, behind_ent);

            let mut lane = get_lane_mut(world, lane_ent);
            debug!("Adding belt {:?} to head of lane {lane_ent:?}", new.entity);
            lane.add_to_head(new.belt, new.coords, new.entity);
            debug!(
                "Adding {} items to left lane, {} items to right lane",
                existing_items.0.len(),
                existing_items.1.len()
            );
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

            debug!(
                "Adding InLane component to entity {:?} pointing to lane {lane_ent:?}",
                new.entity
            );
            world.entity_mut(new.entity).insert(InLane::new(lane_ent));
        }
    }
    if new.belt.input() == new.belt.output() {
        new.coords.step(new.belt.output().left());
        let belt_coords = world.resource::<WorldPlacements>();

        let left = new.coords.step(new.belt.output().left());
        if let Some(left_belt) = belt_coords.get(left).filter(|(ent, entry)| {
            !remaining_entities.contains(ent)
                && entry.output_dir() == Some(new.belt.output().right())
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

        let belt_coords = world.resource::<WorldPlacements>();
        let right = new.coords.step(new.belt.output().right());
        if let Some(right_belt) = belt_coords.get(right).filter(|(ent, entry)| {
            !remaining_entities.contains(ent)
                && entry.output_dir() == Some(new.belt.output().left())
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

fn detach_belt(
    world: &mut World,
    remaining_entities: &[Entity],
    removed: &RemovedBelt,
) -> (Vec<ItemEntry>, Vec<ItemEntry>) {
    debug!("Detaching {:?} from any lanes", removed.entity);

    let belt_coords = world.resource::<WorldPlacements>();
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

    if let BeltShape::Straight(dir) = removed.old_belt {
        debug!("Detaching straigt belt, checking for sideloading");
        for side_dir in [dir.left(), dir.right()] {
            let side_coords = removed.coords.step(side_dir);

            let belt_coords = world.resource::<WorldPlacements>();
            let Some(side_belt) = belt_coords
                .get(side_coords)
                .filter(|(ent, _)| !remaining_entities.contains(ent))
                .filter(|(_, entry)| entry.output_dir() == Some(side_dir.opposite()))
            else {
                continue;
            };
            debug!("Found sideloading from {side_dir:?}");
            let side_lane_ent = get_lane_entity(world, side_belt.0);
            let side_lane = get_lane(world, side_lane_ent);

            if let BeltShape::Fragment(dir) = side_lane.belts[0].belt {
                let side_frag = side_lane.belts[0].clone();
                assert_eq!(dir, side_dir.opposite());

                debug!("Removing LaneConnection component from lane {side_lane_ent:?}");
                world.entity_mut(side_lane_ent).remove::<LaneConnection>();
                let mut side_lane = get_lane_mut(world, side_lane_ent);
                debug!("Removing head from lane {side_lane_ent:?}");
                let items = side_lane.remove_head();
                debug!("Despawning frag entity {:?}", side_frag.entity);
                world.entity_mut(side_frag.entity).despawn();
            } else {
                debug!("Sideloading connection not created yet, skipping");
            };
        }
    }

    let lane_ent = get_lane_entity(world, removed.entity);
    match (ahead_belt, behind_belt) {
        (None, None) => {
            debug!("Removing with nothing around it.");
            let lane = get_lane(world, lane_ent);

            // Check if there are other belts in the lane that will be processed later
            // (they're in remaining_entities and won't show up as neighbors)
            if lane.belts.len() == 1 {
                // This is truly the only belt in the lane, safe to despawn
                let left = lane.lanes[LaneSide::Left].clone();
                let right = lane.lanes[LaneSide::Right].clone();
                debug!(
                    "Despawning lane entity {lane_ent:?} (only belt in lane, returning {} left items, {} right items)",
                    left.len(),
                    right.len()
                );
                world.despawn(lane_ent);
                (left, right)
            } else {
                // Other belts exist in the lane (must be in remaining_entities)
                // Determine our position and remove appropriately
                let belt_idx = lane
                    .belts
                    .iter()
                    .position(|b| b.entity == removed.entity)
                    .expect("Belt should be in its lane");

                if belt_idx == 0 {
                    // At the head
                    let mut lane = get_lane_mut(world, lane_ent);
                    debug!("Removing head from lane {lane_ent:?}");
                    lane.remove_head()
                } else if belt_idx == lane.belts.len() - 1 {
                    // At the tail
                    let mut lane = get_lane_mut(world, lane_ent);
                    debug!("Removing tail from lane {lane_ent:?}");
                    lane.remove_tail()
                } else {
                    // In the middle - split the lane
                    let mut lane = get_lane_mut(world, lane_ent);
                    debug!("Splitting lane {lane_ent:?} at entity {:?}", removed.entity);
                    let mut tail_lane = lane.split_at(removed.entity).unwrap();
                    debug!("Removing head from split tail lane");
                    let leftover_items = tail_lane.remove_head();
                    let belts = tail_lane.belts.iter().map(|b| b.entity).collect::<Vec<_>>();

                    let tail_lane_ent = world.spawn(tail_lane).id();
                    debug!("Spawned new lane entity {tail_lane_ent:?} from splitting");
                    for belt in belts.iter() {
                        debug!(
                            "Adding InLane component to entity {belt:?} pointing to lane {tail_lane_ent:?}"
                        );
                        world.entity_mut(*belt).insert(InLane::new(tail_lane_ent));
                    }
                    leftover_items
                }
            }
        }
        (None, Some(_)) => {
            debug!("Removing from the head");
            let mut lane = get_lane_mut(world, lane_ent);
            let items = if lane
                .belts
                .iter()
                .filter(|b| match b.belt {
                    BeltShape::Straight(_) => true,
                    BeltShape::Curve(_) => true,
                    BeltShape::Fragment(_) => false,
                })
                .map(|_| ())
                .collect::<Vec<()>>()
                .len()
                > 1
            {
                debug!(
                    "Removing {:?} from the head of {lane_ent:?}",
                    removed.entity
                );
                lane.remove_head()
            } else {
                let items = (lane.lanes.left.clone(), lane.lanes.right.clone());
                debug!(
                    "Despawning lane entity {lane_ent:?} (returning {} left items, {} right items)",
                    items.0.len(),
                    items.1.len()
                );
                world.entity_mut(lane_ent).despawn();
                items
            };
            items
        }
        (Some((_, _, ConnectionType::Direct)), None) => {
            debug!("Removing from the tail");
            let mut lane = get_lane_mut(world, lane_ent);

            let items = if lane.belts.len() > 1 {
                debug!("Removing tail from lane {lane_ent:?}");
                lane.remove_tail()
            } else {
                let items = (lane.lanes.left.clone(), lane.lanes.right.clone());
                debug!(
                    "Despawning lane entity {lane_ent:?} (returning {} left items, {} right items)",
                    items.0.len(),
                    items.1.len()
                );
                world.entity_mut(lane_ent).despawn();
                items
            };
            items
        }
        (Some((_, _, ConnectionType::Direct)), Some(_)) => {
            debug!("Removing from the middle");
            let mut lane = get_lane_mut(world, lane_ent);
            debug!("Splitting lane {lane_ent:?} at entity {:?}", removed.entity);
            let mut tail_lane = lane.split_at(removed.entity).unwrap();
            debug!("Removing head from split tail lane");
            let leftover_items = tail_lane.remove_head();
            let belts = tail_lane.belts.iter().map(|b| b.entity).collect::<Vec<_>>();

            let tail_lane_ent = world.spawn(tail_lane).id();
            debug!("Spawned new lane entity {tail_lane_ent:?} from splitting");
            for belt in belts.iter() {
                debug!(
                    "Adding InLane component to entity {belt:?} pointing to lane {tail_lane_ent:?}"
                );
                world.entity_mut(*belt).insert(InLane::new(tail_lane_ent));
            }
            leftover_items
        }
        (Some((_, _, ConnectionType::SideLoad(_))), None) => {
            debug!("Despawning sideload lane entity {lane_ent:?}");
            let entities = get_lane(world, lane_ent)
                .belts
                .iter()
                .map(|b| b.entity)
                .collect::<Vec<_>>();
            for b in entities {
                world.entity_mut(b).insert(Delete);
            }
            world.entity_mut(lane_ent).despawn();
            Default::default()
        }
        (Some((_, _, ConnectionType::SideLoad(_))), Some(_)) => {
            debug!("Shortening sideload lane entity {lane_ent:?}");

            let mut lane = get_lane_mut(world, lane_ent);
            assert!(lane.belts[0].belt.is_fragment());
            let frag_ent = lane.belts[0].entity;
            let mut frag_items = lane.remove_head();
            let belt_items = lane.remove_head();
            world.entity_mut(frag_ent).insert(Delete);
            world.entity_mut(lane_ent).remove::<LaneConnection>();

            frag_items.0.extend(belt_items.0);
            frag_items.1.extend(belt_items.1);
            (frag_items.0, frag_items.1)
        }
    }
}

fn replace_belt(world: &mut World, remaining_entities: &[Entity], replaced: &ReplacedBelt) {
    info!("Replacing Belt");
    match replaced.old_entity {
        Some(old) => {
            debug!("Replacing belt {:?} with {:?}", old, replaced.entity);
            let removed = RemovedBelt {
                entity: old,
                old_belt: replaced.old_belt,
                coords: replaced.coords,
            };
            let remaining_items = detach_belt(world, remaining_entities, &removed);
            let new = NewBelt::from(*replaced);
            new_belt(world, remaining_entities, &new, remaining_items);
        }
        None => {
            debug!("Updating {:?} in place", replaced.entity);
            let removed = RemovedBelt::from(*replaced);
            let remaining_items = detach_belt(world, remaining_entities, &removed);
            let new = NewBelt::from(*replaced);
            new_belt(world, remaining_entities, &new, remaining_items);
        }
    }
}

fn ahead_connected_belt(
    belt_coords: &WorldPlacements,
    remaining_entities: &[Entity],
    coords: WorldCoords,
    dir: HDir,
) -> Option<(Entity, BeltShape, ConnectionType)> {
    belt_coords
        .get_belt(coords.step(dir))
        .filter(|(ent, _)| !remaining_entities.contains(ent))
        .and_then(|(entity, ahead)| {
            if ahead.input() == dir {
                Some((entity, ahead, ConnectionType::Direct))
            } else {
                if ahead.input().opposite() == dir {
                    None
                } else {
                    // Determine which side of the target belt this is sideloading into
                    let target_side = determine_sideload_target_side(dir, ahead.input());
                    Some((entity, ahead, ConnectionType::SideLoad(target_side)))
                }
            }
        })
}

/// Determine which side of the target belt a sideloading connection targets
/// based on the direction the source is coming from and the target's input direction
fn determine_sideload_target_side(source_dir: HDir, target_input: HDir) -> LaneSide {
    use HDir::*;
    use LaneSide::*;

    // From the perspective of someone standing on the target belt facing its input direction,
    // determine if the source is approaching from the left or right
    match (target_input, source_dir) {
        // Target inputs from North (x+), determine if source is on left or right
        (North, West) => Right, // Coming from West (z-) is on the right
        (North, East) => Left,  // Coming from East (z+) is on the left

        // Target inputs from South (x-), determine if source is on left or right
        (South, East) => Right, // Coming from East (z+) is on the right
        (South, West) => Left,  // Coming from West (z-) is on the left

        // Target inputs from East (z+), determine if source is on left or right
        (East, North) => Right, // Coming from North (x+) is on the right
        (East, South) => Left,  // Coming from South (x-) is on the left

        // Target inputs from West (z-), determine if source is on left or right
        (West, South) => Right, // Coming from South (x-) is on the right
        (West, North) => Left,  // Coming from North (x+) is on the left

        _ => unreachable!(
            "Invalid sideload: source {:?}, target {:?}",
            source_dir, target_input
        ),
    }
}

fn behind_connected_belt(
    belt_coords: &WorldPlacements,
    remaining_entities: &[Entity],
    coords: WorldCoords,
    dir: HDir,
) -> Option<(Entity, BeltShape)> {
    belt_coords
        .get_belt(coords.step(dir.opposite()))
        .filter(|(ent, _)| !remaining_entities.contains(ent))
        .filter(|behind| behind.1.output() == dir)
}

fn create_sideload_connection(
    world: &mut World,
    source_lane_ent: Entity,
    target_belt_ent: Entity,
    source_dir: HDir,
    intersection_coords: WorldCoords,
    target_side: LaneSide,
) {
    let frag_ent = world
        .spawn((
            BeltShape::Fragment(source_dir),
            intersection_coords,
            InLane::new(source_lane_ent),
        ))
        .id();
    debug!("Spawning BeltFrag: {frag_ent:?} in lane {source_lane_ent:?}");
    let mut source_lane = get_lane_mut(world, source_lane_ent);
    source_lane.prepend_fragment(source_dir, intersection_coords, frag_ent);

    let target_lane_ent = get_lane_entity(world, target_belt_ent);
    let target_lane = get_lane(world, target_lane_ent);

    let ranges = target_lane
        .range_for(target_belt_ent)
        .expect("Invariant broken: lane_belt_data_matches_world");

    let center = (ranges[target_side].start + ranges[target_side].end) / 2;

    let lane_offset = (LANE_OFFSET_FACTOR * POSITIONS_PER_BELT as f32) as i32;

    let (left_offset, right_offset) = match target_side {
        LaneSide::Left => (center - lane_offset, center + lane_offset),
        LaneSide::Right => (center + lane_offset, center - lane_offset),
    };

    world.entity_mut(source_lane_ent).insert(LaneConnection {
        target: target_lane_ent,
        offset: Sided {
            left: left_offset,
            right: right_offset,
        },
        target_side,
    });
}

#[track_caller]
fn get_lane_entity(world: &mut World, belt_ent: Entity) -> Entity {
    world
        .query::<&InLane>()
        .get(world, belt_ent)
        .map(|l| l.lane)
        .expect("Invariant broken: all_belts_and_frags_claim_to_be_in_a_lane")
}

fn get_lane(world: &mut World, lane_ent: Entity) -> &BeltLane {
    world
        .query::<&BeltLane>()
        .get(world, lane_ent)
        .expect("Invariant broken: lane_entity_has_belt_lane_component")
}

fn get_lane_mut(world: &mut World, lane_ent: Entity) -> bevy::ecs::world::Mut<'_, BeltLane> {
    world
        .query::<&mut BeltLane>()
        .get_mut(world, lane_ent)
        .expect("Invariant broken: lane_entity_has_belt_lane_component")
}

fn update_connection_offsets_for_lane(
    world: &mut World,
    lane_ent: Entity,
    side: LaneSide,
    offset: i32,
) {
    // Loops can't change length

    // Find and update all connections pointing to this lane (sideload connections)
    let mut conns_to_update: Vec<Entity> = Vec::new();
    let mut query = world.query::<(Entity, &LaneConnection)>();
    for (source_ent, conn) in query.iter(world) {
        if conn.target == lane_ent {
            conns_to_update.push(source_ent);
        }
    }

    // For sideload connections, update both sides
    for source_ent in conns_to_update {
        if let Some(mut conn) = world.entity_mut(source_ent).get_mut::<LaneConnection>() {
            if side == conn.target_side {
                for side in SIDES {
                    conn.offset[side] += offset;
                }
            }
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
    app.add_plugins(crate::sim::SimPlugin);
    app.init_resource::<PlacementErrors>();
    app
}

#[cfg(test)]
#[derive(Resource, Default)]
pub struct PlacementErrors {
    pub errors: Vec<ItemPlacementError>,
}

#[cfg(test)]
pub trait AppExtension {
    fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: HDir) -> Entity;
    fn add_item(&mut self, belt: Entity, pos: i32, lane: LaneSide) -> Entity;
    fn find_item(&mut self, item: Entity) -> Option<(Item, Transform)>;
    fn find_belt(&mut self, belt: Entity) -> Option<(BeltShape, Transform)>;
    fn remove_belt_at(&mut self, coords: impl Into<WorldCoords>) -> bool;
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
            item: Item(1),
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
        let Some((entity, _)) = self.world_mut().resource::<WorldPlacements>().get(coords) else {
            return false;
        };
        self.world_mut().trigger(RemoveBlock { entity });
        true
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
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    #[should_panic]
    fn panics_on_invarient_failure() {
        let mut app = test_app();
        let world = app.world_mut();
        world.spawn(InLane::new(Entity::from_raw_u32(2).unwrap()));
        app.update();
    }

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
                ranges: Sided {
                    left: 0..POSITIONS_PER_BELT - ITEM_SPACING / 2,
                    right: 0..POSITIONS_PER_BELT - ITEM_SPACING / 2,
                },
                lane_offsets: Sided { left: 0, right: 0 },
            }],
            lanes: Sided {
                left: vec![ItemEntry {
                    pos: 0,
                    item: Item(0),
                    entity: item_ent,
                }],
                right: vec![],
            },
            is_blocked: Sided {
                left: false,
                right: false,
            },
        };
        assert_eq!(*actual, expected);
    }

    #[test]
    fn belt_has_inlane() {
        let mut app = test_app();
        let world = app.world_mut();
        let entity = world.spawn_empty().id();
        world.trigger(PlaceBlock {
            entity,
            item: Item(1),
            dir: HDir::North,
            coords: (0, 0, 0).into(),
        });
        app.update();

        let world = app.world_mut();
        world.query::<&InLane>().single(world).unwrap();
    }

    #[test]
    fn remove_single_belt() {
        let mut app = test_app();
        let entity = app.add_belt((0, 0, 0), HDir::North);
        app.update();

        let world = app.world_mut();
        world.query::<&InLane>().single(world).unwrap();

        app.remove_belt_at((0, 0, 0));
        app.update();
        assert!(app.find_belt(entity).is_none());
    }

    #[test]
    fn remove_belt_at_head() {
        let mut app = test_app();
        let entity = app.add_belt((0, 0, 0), HDir::North);
        app.add_belt((-1, 0, 0), HDir::North);
        app.update();

        app.remove_belt_at((0, 0, 0));
        app.update();

        assert!(app.find_belt(entity).is_none());
    }

    #[test]
    fn remove_belt_at_tail() {
        let mut app = test_app();
        let entity = app.add_belt((0, 0, 0), HDir::North);
        app.add_belt((1, 0, 0), HDir::North);
        app.update();

        app.remove_belt_at((0, 0, 0));
        app.update();

        assert!(app.find_belt(entity).is_none());
    }

    #[test]
    fn remove_belt_in_middle() {
        let mut app = test_app();
        let entity = app.add_belt((0, 0, 0), HDir::North);
        app.add_belt((1, 0, 0), HDir::North);
        app.add_belt((-1, 0, 0), HDir::North);
        app.update();

        app.remove_belt_at((0, 0, 0));
        app.update();

        assert!(app.find_belt(entity).is_none());
    }

    #[test]
    fn remove_belt_in_middle_with_items() {
        let mut app = test_app();
        let middle = app.add_belt((0, 0, 0), HDir::North);
        let head = app.add_belt((1, 0, 0), HDir::North);
        let tail = app.add_belt((-1, 0, 0), HDir::North);
        app.update();

        for i in 0..ITEMS_PER_BELT {
            app.add_item(head, i * ITEM_SPACING, LaneSide::Left);
            app.add_item(middle, i * ITEM_SPACING, LaneSide::Left);
        }
        app.add_item(tail, 0, LaneSide::Left);
        app.update();

        app.remove_belt_at((0, 0, 0));
        app.update();

        assert!(app.find_belt(middle).is_none());
    }

    #[test]
    fn replace_belt_at_head_flipped() {
        let mut app = test_app();

        let first = app.add_belt((0, 0, 0), HDir::East);
        app.update();
        app.add_belt((-1, 0, 0), HDir::North);
        app.update();
        let replaced = app.add_belt((0, 0, 0), HDir::West);
        app.update();

        assert!(app.find_belt(first).is_none());
        assert!(app.find_belt(replaced).is_some());
    }

    #[test]
    fn replace_belt_with_two_neighbors_immediate() {
        let mut app = test_app();

        let first = app.add_belt((0, 0, 0), HDir::West);
        app.add_belt((-1, 0, 0), HDir::North);
        app.add_belt((1, 0, 0), HDir::South);
        let replaced = app.add_belt((0, 0, 0), HDir::North);
        app.update();

        // Verify the belt was replaced
        assert!(app.find_belt(first).is_none());
        assert!(app.find_belt(replaced).is_some());
    }

    #[test]
    fn replace_belt_with_two_neighbors_with_update() {
        let mut app = test_app();

        let first = app.add_belt((0, 0, 0), HDir::West);
        app.add_belt((-1, 0, 0), HDir::North);
        app.add_belt((1, 0, 0), HDir::South);
        app.update();
        let replaced = app.add_belt((0, 0, 0), HDir::North);
        app.update();

        // Verify the belt was replaced
        assert!(app.find_belt(first).is_none());
        assert!(app.find_belt(replaced).is_some());
    }

    #[test]
    fn replace_then_add_neighbor() {
        let mut app = test_app();

        let first = app.add_belt((0, 0, 0), HDir::West);
        let replaced = app.add_belt((0, 0, 0), HDir::North);
        app.add_belt((1, 0, 0), HDir::South);
        app.update();

        assert!(app.find_belt(first).is_none());
        assert!(app.find_belt(replaced).is_some());
    }

    #[test]
    fn replace_same_belt_twice() {
        let mut app = test_app();

        let first = app.add_belt((0, 0, 0), HDir::West);
        app.add_belt((-1, 0, 0), HDir::North);
        app.update();

        let replaced1 = app.add_belt((0, 0, 0), HDir::North);
        let replaced2 = app.add_belt((0, 0, 0), HDir::North);
        app.update();

        // Verify the first belt was replaced
        assert!(app.find_belt(first).is_none());
        // The first replacement should be gone, kept the second one
        assert!(app.find_belt(replaced1).is_none());
        assert!(app.find_belt(replaced2).is_some());
    }

    #[test]
    fn replace_both_belts_in_connected_pair() {
        let mut app = test_app();

        // Place two connected belts
        let belt_a = app.add_belt((0, 0, 0), HDir::West);
        let belt_b = app.add_belt((-1, 0, 0), HDir::North);
        app.update();

        // Replace both belts in the same frame
        let replaced_a = app.add_belt((0, 0, 0), HDir::North);
        let replaced_b = app.add_belt((-1, 0, 0), HDir::North);
        app.update();

        // Both original belts should be gone
        assert!(app.find_belt(belt_a).is_none());
        assert!(app.find_belt(belt_b).is_none());
        // Both replacements should exist
        assert!(app.find_belt(replaced_a).is_some());
        assert!(app.find_belt(replaced_b).is_some());
    }

    #[test]
    fn replace_with_two_neighbors_after_update() {
        let mut app = test_app();

        let first = app.add_belt((0, 0, 0), HDir::West);
        app.add_belt((-1, 0, 0), HDir::North);
        app.update();

        app.add_belt((1, 0, 0), HDir::South);
        let replaced = app.add_belt((0, 0, 0), HDir::North);
        app.update();

        assert!(app.find_belt(first).is_none());
        assert!(app.find_belt(replaced).is_some());
    }

    #[test]
    fn replace_north_belt_with_east_after_item() {
        let mut app = test_app();

        let belt = app.add_belt((-1, 2, 0), HDir::North);
        app.update();
        app.add_item(belt, 0, LaneSide::Left);
        app.update();
        app.add_belt((-1, 2, 0), HDir::East);
        app.update();
    }

    #[test]
    fn replace_belt_with_multiple_neighbors_complex() {
        let mut app = test_app();

        // First frame: place 4 belts
        app.add_belt((0, -3, 0), HDir::West);
        let original = app.add_belt((0, 0, 0), HDir::North);
        app.add_belt((0, -1, 0), HDir::North);
        app.add_belt((0, 1, 0), HDir::North);
        app.update();

        // Second frame: place 2 new belts and replace the one at (0,0,0)
        app.add_belt((1, -3, 0), HDir::South);
        app.add_belt((-1, -2, 0), HDir::North);
        let replaced = app.add_belt((0, 0, 0), HDir::North);
        app.update();

        assert!(app.find_belt(original).is_none());
        assert!(app.find_belt(replaced).is_some());
    }

    #[test]
    fn replace_connected_belt_with_new_neighbor() {
        let mut app = test_app();

        // First frame: place 5 belts, including a connected pair at (-1,-3) and (0,-3)
        app.add_belt((0, -3, 0), HDir::West);
        let original = app.add_belt((-1, -3, 0), HDir::North);
        app.add_belt((0, 0, 0), HDir::North);
        app.add_belt((0, -1, 0), HDir::North);
        app.add_belt((0, 1, 0), HDir::North);
        app.update();

        // Second frame: add new belt, replace (-1,-3), add another new belt
        app.add_belt((1, -3, 0), HDir::South);
        let replaced = app.add_belt((-1, -3, 0), HDir::North);
        app.add_belt((-1, 0, 0), HDir::North);
        app.update();

        assert!(app.find_belt(original).is_none());
        assert!(app.find_belt(replaced).is_some());
    }

    #[test]
    fn replace_connected_belt_with_new_neighbor_minimal_v4() {
        // Same as original but third belt at different location
        let mut app = test_app();

        app.add_belt((0, -3, 0), HDir::West);
        let original = app.add_belt((-1, -3, 0), HDir::North);
        app.add_belt((0, 0, 0), HDir::North);
        app.add_belt((0, -1, 0), HDir::North);
        app.add_belt((0, 1, 0), HDir::North);
        app.update();

        app.add_belt((1, -3, 0), HDir::South);
        let replaced = app.add_belt((-1, -3, 0), HDir::North);
        app.add_belt((5, 5, 0), HDir::North); // Different location
        app.update();

        assert!(app.find_belt(original).is_none());
        assert!(app.find_belt(replaced).is_some());
    }

    #[test]
    fn replace_sideloading_into() {
        let mut app = test_app();

        app.add_belt((0, 0, 0), HDir::West);
        let original = app.add_belt((-1, 0, 0), HDir::North);
        app.update();

        app.add_belt((1, 0, 0), HDir::South);
        let replaced = app.add_belt((-1, 0, 0), HDir::North);
        app.update();

        assert!(app.find_belt(original).is_none());
        assert!(app.find_belt(replaced).is_some());
    }

    #[test]
    fn replace_connected_belt_no_third_belt() {
        // Same setup but NO third new belt - this should pass
        let mut app = test_app();

        app.add_belt((0, -3, 0), HDir::West);
        let original = app.add_belt((-1, -3, 0), HDir::North);
        app.add_belt((0, 0, 0), HDir::North);
        app.add_belt((0, -1, 0), HDir::North);
        app.add_belt((0, 1, 0), HDir::North);
        app.update();

        app.add_belt((1, -3, 0), HDir::South);
        let replaced = app.add_belt((-1, -3, 0), HDir::North);
        // NO third belt
        app.update();

        assert!(app.find_belt(original).is_none());
        assert!(app.find_belt(replaced).is_some());
    }

    #[test]
    fn remove_one_belt() {
        let mut app = test_app();

        let belt = app.add_belt((0, 0, 0), HDir::West);
        app.update();

        app.remove_belt_at((0, 0, 0));
        app.update();

        assert!(app.find_belt(belt).is_none());
    }

    #[test]
    fn remove_sideload_from_long_lane() {
        let mut app = test_app();
        app.add_belt((0, 0, 0), HDir::North);
        app.add_belt((-1, 0, 0), HDir::North);
        let _side_loader = app.add_belt((0, 0, 1), HDir::West);
        app.add_belt((0, 0, 2), HDir::West);
        app.update();

        app.remove_belt_at((0, 0, 1));
        app.update();
    }
}
