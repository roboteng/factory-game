use bevy::{math::ops::sin_cos, prelude::*, reflect::reflect_trait};
use std::any::TypeId;
use std::collections::HashMap;
use std::f32::consts::PI;

pub mod dir;
pub mod inventory;
pub mod machine;
pub mod world_gen;

pub use world_gen::{FlatWorldPlugin, PerlinWorldPlugin};
#[cfg(feature = "invariant-check")]
pub mod invariants;

// Re-export direction types; explicit `use` for `Curve` to shadow `bevy::prelude::Curve`.
use dir::Curve;
pub use dir::*;

pub use machine::*;

use crate::inventory::{Inventory, Stack};

pub const ITEMS_PER_BELT: i32 = 4;
pub const POSITIONS_PER_BELT: i32 = 256;
pub const BASE_BELT_SPEED: i32 = 8;
/// How far from center each lane is.
pub const LANE_OFFSET: f32 = 0.25;
/// How far from the bottom of the voxel the belt surface is.
pub const BELT_HEIGHT: f32 = 0.25;

pub const ITEM_SIZE: f32 = 1.0 / (ITEMS_PER_BELT as f32);
pub const MINER_TICKS_PER_EXTRACT: u32 = 60;
pub const COLLECTOR_MOVE_TICKS: u32 = 60;
pub const ITEM_SPACING: i32 = POSITIONS_PER_BELT / ITEMS_PER_BELT;
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
        app.add_plugins(crate::invariants::InvariantsPlugin);

        app.register_type::<Corn>()
            .register_type_data::<Corn, ReflectWorldDrop>()
            .register_type::<Furnace>()
            .register_type_data::<Furnace, ReflectWorldDrop>()
            .register_type::<Assembler>()
            .register_type_data::<Assembler, ReflectWorldDrop>();

        app.init_resource::<CoordsMap>();
        app.insert_resource(Recipes::new());

        app.add_observer(on_place_structure);
        app.add_observer(on_place_item);
        app.add_observer(on_remove_block);
        app.add_observer(on_incline);
        app.add_observer(on_load_machine_input);
        app.add_observer(on_unload_machine_output);
        app.add_observer(on_set_assembler_recipe);
        app.add_observer(on_set_source_item);


        app.add_systems(
            Update,
            (
                (determine_belt_shape, ApplyDeferred, determine_belt_shape).chain(),
                move_items_on_belts,
                transfer_items,
                set_item_transforms,
                fill_sources,
                fill_miners,
                push_to_belt,
                (recalculate_filters, pull_from_belt, tick_collectors).chain(),
                process_furnace,
                process_assembler,
                consume_sink_buffer,
                side_loading,
                grow_corn,
            ),
        );

        app.add_systems(PostUpdate, despawn_old_entities);
    }
}

// ------
// Models
// ------

#[derive(EntityEvent, Debug, Clone, Copy)]
/// `flb` should always be contained in the bounding box, while `brt` never is.
pub struct PlaceStructure {
    pub entity: Entity,
    pub structure: Structure,
    /// Front Left Bottom, inclusive
    pub flb: WorldCoords,
    /// Back Right Top, exclusive
    pub brt: WorldCoords,
}

impl PlaceStructure {
    /// Going from back to front.
    /// Returns `None` for non-directional blocks (brt directly above flb, dx==0 && dz==0).
    pub fn facing(&self) -> Option<HDir> {
        let d = self.flb.delta_to(self.brt);
        let (dx, _, dz) = d.xyz();
        if dx == 0 && dz == 0 {
            return None;
        }
        match (dx.signum(), dz.signum()) {
            (-1, 1) => Some(HDir::North),
            (1, -1) => Some(HDir::South),
            (-1, -1) => Some(HDir::East),
            (1, 1) => Some(HDir::West),
            _ => None,
        }
    }
}

#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct Incline {
    pub entity: Entity,
}

impl PlaceStructure {
    fn to_bundle(&self) -> impl Bundle {
        // Mirror the ghost-preview formula so placed blocks appear at the same position.
        // Ghost uses: Vec3::from(flb) + block.size().center_offset()
        let rotation = self
            .facing()
            .map(|d| Quat::from_rotation_y(d.angle()))
            .unwrap_or(Quat::IDENTITY);
        let transform = Transform::from_translation(
            Vec3::from(self.flb) + self.structure.size().center_offset(),
        )
        .with_rotation(rotation);
        (self.structure, self.flb, transform)
    }
}

#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct PlaceItem {
    pub entity: Entity,
    pub item: Item,
}

#[derive(EntityEvent, Debug, Clone)]
pub struct RemoveBlock {
    pub entity: Entity,
}

/// Player moved one item from their inventory into a machine's input buffer.
#[derive(Event, Debug, Clone)]
pub struct LoadMachineInput {
    pub player_inventory_slot: u16,
    pub machine: Entity,
}

/// Player collected an item from a machine's output buffer into their inventory.
#[derive(Event, Debug, Clone)]
pub struct UnloadMachineOutput {
    pub machine: Entity,
    pub output_slot: usize,
}

/// Player selected an item for a source to produce. `None` clears the selection.
#[derive(Event, Debug, Clone)]
pub struct SetSourceItem {
    pub source: Entity,
    pub item: Option<Item>,
}

/// Player set or cleared an assembler's recipe. `None` clears the recipe.
#[derive(Event, Debug, Clone)]
pub struct SetAssemblerRecipe {
    pub assembler: Entity,
    pub recipe: Option<machine::AssemblerRecipe>,
}

#[derive(Component)]
pub struct Belt;

#[derive(Debug, Component, Default)]
pub struct Source {
    pub configured_item: Option<Item>,
}

#[derive(Component)]
pub struct Miner {
    ticks: u32,
    dir: HDir,
}

#[derive(Component)]
pub struct Sink;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum MachineStatus<R> {
    #[default]
    Idle,
    Processing {
        recipe: R,
        elapsed_ticks: u32,
    },
}

#[derive(Debug, Clone)]
pub enum Recipe {
    FurnaceRecipe(machine::FurnaceRecipe),
    AssemblerRecipe(machine::AssemblerRecipe),
}

impl From<machine::AssemblerRecipe> for Recipe {
    fn from(value: machine::AssemblerRecipe) -> Self {
        Recipe::AssemblerRecipe(value)
    }
}

impl From<machine::FurnaceRecipe> for Recipe {
    fn from(value: machine::FurnaceRecipe) -> Self {
        Recipe::FurnaceRecipe(value)
    }
}

#[derive(Resource)]
pub struct Recipes(pub Vec<Recipe>);

impl Recipes {
    fn new() -> Self {
        Self(vec![
            Recipe::FurnaceRecipe(machine::FurnaceRecipe {
                input: Stack {
                    item: Item::IronOre,
                    count: 1,
                },
                output: Stack {
                    item: Item::IronIngot,
                    count: 1,
                },
                ticks: 100,
            }),
            Recipe::FurnaceRecipe(machine::FurnaceRecipe {
                input: Stack {
                    item: Item::CopperOre,
                    count: 1,
                },
                output: Stack {
                    item: Item::CopperIngot,
                    count: 1,
                },
                ticks: 100,
            }),
            Recipe::AssemblerRecipe(AssemblerRecipe {
                input: vec![Stack {
                    item: Item::IronIngot,
                    count: 2,
                }],
                output: vec![Stack {
                    item: Item::Belt,
                    count: 1,
                }],
                ticks: 60,
            }),
            Recipe::AssemblerRecipe(machine::AssemblerRecipe {
                input: vec![
                    Stack {
                        item: Item::IronIngot,
                        count: 1,
                    },
                    Stack {
                        item: Item::CopperIngot,
                        count: 1,
                    },
                ],
                output: vec![Stack {
                    item: Item::Miner,
                    count: 1,
                }],
                ticks: 120,
            }),
            Recipe::AssemblerRecipe(machine::AssemblerRecipe {
                input: vec![
                    Stack {
                        item: Item::IronIngot,
                        count: 2,
                    },
                    Stack {
                        item: Item::CopperIngot,
                        count: 1,
                    },
                ],
                output: vec![Stack {
                    item: Item::Furnace,
                    count: 1,
                }],
                ticks: 150,
            }),
            Recipe::AssemblerRecipe(AssemblerRecipe {
                input: vec![Stack {
                    item: Item::IronIngot,
                    count: 1,
                }],
                output: vec![Stack {
                    item: Item::Gear,
                    count: 1,
                }],
                ticks: 360,
            }),
        ])
    }
}

#[derive(Component)]
pub struct OutputsToBelt {
    at: WorldCoords,
}

#[derive(Component)]
struct DirtyBelt;

/// Marks an entity as a target for block-placement raycasts.
/// `half_extents` is the AABB half-size on each axis, centred on the entity's
/// `Transform` translation.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
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

#[derive(Debug, Component, Default, PartialEq)]
pub struct ItemLanes(pub Sided<Vec<(ItemPos, Entity)>>);

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
    Assembler,
    Collector,
    CornKernels,
    CornStalk,
    Biomass,
    Gear,
}

impl Item {
    pub fn name(self) -> &'static str {
        use Item::*;
        match self {
            Belt => "Belt",
            Source => "Source",
            Sink => "Sink",
            Rock => "Rock",
            Dirt => "Dirt",
            IronOre => "Iron Ore",
            CopperOre => "Copper Ore",
            IronIngot => "Iron Ingot",
            CopperIngot => "Copper Ingot",
            Miner => "Miner",
            Furnace => "Furnace",
            Assembler => "Assembler",
            Collector => "Collector",
            CornKernels => "Corn Kernels",
            CornStalk => "Corn Stalk",
            Biomass => "Biomass",
            Gear => "Gear",
        }
    }

    pub fn stack_size(self) -> u16 {
        100
    }

    /// Returns the world block this item places, or `None` if the item cannot be placed.
    pub fn can_place(self) -> Option<Structure> {
        match self {
            Item::Belt => Some(Structure::Belt),
            Item::Source => Some(Structure::Source),
            Item::Sink => Some(Structure::Sink),
            Item::Rock => Some(Structure::Rock),
            Item::Dirt => Some(Structure::Dirt),
            Item::Miner => Some(Structure::Miner),
            Item::Furnace => Some(Structure::Furnace),
            Item::Assembler => Some(Structure::Assembler),
            Item::Collector => Some(Structure::Collector),
            Item::CornKernels => Some(Structure::Corn),
            Item::IronOre
            | Item::CopperOre
            | Item::IronIngot
            | Item::CopperIngot
            | Item::CornStalk
            | Item::Biomass
            | Item::Gear => None,
        }
    }
}

/// World block type — everything that occupies a position in the world, whether placed by the
/// player or spawned by world generation. Not all world blocks have a corresponding item.
#[derive(Component, Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub enum Structure {
    Belt,
    Source,
    Sink,
    Rock,
    Dirt,
    IronOreDeposit,
    CopperOreDeposit,
    Miner,
    Furnace,
    Assembler,
    Collector,
    Corn,
}

impl Structure {
    pub fn name(self) -> &'static str {
        match self {
            Structure::Belt => "Belt",
            Structure::Source => "Source",
            Structure::Sink => "Sink",
            Structure::Rock => "Rock",
            Structure::Dirt => "Dirt",
            Structure::IronOreDeposit => "Iron Ore Deposit",
            Structure::CopperOreDeposit => "Copper Ore Deposit",
            Structure::Miner => "Miner",
            Structure::Furnace => "Furnace",
            Structure::Assembler => "Assembler",
            Structure::Collector => "Collector",
            Structure::Corn => "Corn",
        }
    }

    /// Item produced when a miner harvests this block. `None` means not minable.
    pub fn mine(self) -> Option<Item> {
        match self {
            Structure::IronOreDeposit => Some(Item::IronOre),
            Structure::CopperOreDeposit => Some(Item::CopperOre),
            _ => None,
        }
    }

    /// What is dropped when a player breaks this block.
    pub fn break_drop(self) -> BreakDrop {
        match self {
            Structure::Belt => BreakDrop::Item(Item::Belt),
            Structure::Source => BreakDrop::Item(Item::Source),
            Structure::Sink => BreakDrop::Item(Item::Sink),
            Structure::Rock => BreakDrop::Item(Item::Rock),
            Structure::Dirt => BreakDrop::Item(Item::Dirt),
            Structure::Miner => BreakDrop::Item(Item::Miner),
            Structure::Furnace => BreakDrop::Custom(TypeId::of::<Furnace>()),
            Structure::Assembler => BreakDrop::Custom(TypeId::of::<Assembler>()),
            Structure::Collector => BreakDrop::Item(Item::Collector),
            Structure::IronOreDeposit | Structure::CopperOreDeposit => BreakDrop::None,
            Structure::Corn => BreakDrop::Custom(TypeId::of::<Corn>()),
        }
    }

    /// Compute `brt` for a `PlaceBlock` event given `flb` and the facing direction.
    /// For non-directional blocks pass `None`; `brt` will be directly above `flb` (dx==dz==0).
    /// For directional blocks the footprint extends left and backward relative to `facing`.
    pub fn brt_for(self, flb: WorldCoords, facing: Option<HDir>) -> WorldCoords {
        let size = self.size();
        let delta = match facing {
            None => WorldCoordsDelta::ZERO.height(size.height as i32),
            Some(dir) => WorldCoordsDelta::ZERO
                .height(size.height as i32)
                .dir(dir.left(), size.width as usize)
                .dir(dir.opposite(), size.depth as usize),
        };
        flb.step(delta)
    }

    pub fn size(self) -> StructureSize {
        match self {
            Structure::Belt => StructureSize {
                height: 1,
                width: 1,
                depth: 1,
            },
            Structure::Furnace => StructureSize {
                height: 6,
                width: 2,
                depth: 2,
            },
            Structure::Assembler => StructureSize {
                height: 4,
                width: 3,
                depth: 2,
            },
            _ => StructureSize {
                height: 2,
                width: 1,
                depth: 1,
            },
        }
    }
}

pub struct StructureSize {
    /// In voxels
    pub height: u8,
    pub width: u8,
    pub depth: u8,
}

impl StructureSize {
    pub fn into_raycast_target(&self, dir: HDir) -> RaycastTarget {
        let d = WorldCoordsDelta::ZERO
            .height(self.height.into())
            .dir(dir, self.depth.into())
            .dir(dir.left(), self.width.into());
        let (x, y, z) = d.xyz();
        fn f(a: i32) -> f32 {
            (a as f32).abs() / 2.0
        }
        let (x, y, z) = (f(x), f(y) / 2.0, f(z));
        RaycastTarget {
            half_extents: Vec3::new(x, y, z),
        }
    }

    /// World-space (x, z) offset from the placement coordinate to the entity's
    /// visual centre. Always extends in the +x / +z direction; the model
    /// rotation handles facing. Returns `Vec3::ZERO` for 1×1 blocks.
    pub fn center_offset(&self) -> Vec3 {
        Vec3::new(
            (self.width - 1) as f32 * 0.5,
            0.0,
            (self.depth - 1) as f32 * 0.5,
        )
    }

    pub fn is_full_block(&self) -> bool {
        self.height % 2 == 0
    }

    /// Returns all voxels occupied by a structure of this size
    /// placed at `origin`. The footprint always extends East and South from
    /// the origin corner.
    pub fn iter_coords(&self, origin: WorldCoords) -> impl Iterator<Item = WorldCoords> + '_ {
        let (w, h, d) = (self.width as i32, self.height as i32, self.depth as i32);
        (0..w).flat_map(move |dx| {
            (0..d).flat_map(move |dz| {
                (0..h).map(move |dy| {
                    origin.step(WorldCoordsDelta::ZERO.east(dx).south(dz).height(dy))
                })
            })
        })
    }
}

/// What is dropped when a player breaks a block.
pub enum BreakDrop {
    /// Block cannot be broken or drops nothing.
    None,
    /// Drops a single static item.
    Item(Item),
    /// Drops are determined by the `WorldDrop` reflect-trait implemented on the component
    /// identified by the given `TypeId`. That component must be registered with both
    /// `ReflectComponent` and `ReflectWorldDrop`.
    Custom(TypeId),
}

/// Reflect-trait for components that produce variable item drops when their block is broken.
/// Implement this on the stateful component (e.g. `Corn`) and register via
/// `app.register_type_data::<T, ReflectWorldDrop>()`.
#[reflect_trait]
pub trait WorldDrop {
    fn drop_items(&self) -> Vec<Stack>;
}

pub const CORN_TICKS_PER_STAGE: u32 = 120;

/// State of a planted corn block. `Growing` tracks total age in ticks across all stages;
/// stages A/B/C are each `CORN_TICKS_PER_STAGE` ticks wide. `FullyGrown` is stage D.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub enum Corn {
    Growing { age: u32 },
    FullyGrown,
}

impl WorldDrop for Corn {
    fn drop_items(&self) -> Vec<Stack> {
        match self {
            Corn::FullyGrown => vec![Stack::new(Item::CornStalk, 1), Stack::new(Item::Biomass, 1)],
            Corn::Growing { .. } => vec![Stack::new(Item::CornKernels, 1)],
        }
    }
}

impl Corn {
    /// Returns 0..=3 mapping to stages A–D, for model selection.
    pub fn visual_stage(&self) -> u8 {
        match self {
            Corn::FullyGrown => 3,
            Corn::Growing { age } => (*age / CORN_TICKS_PER_STAGE).min(2) as u8,
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
pub struct CreativeMode(pub bool);

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

fn on_place_structure(
    event: On<PlaceStructure>,
    mut cmd: Commands,
    mut coord_map: ResMut<CoordsMap>,
    belts_q: Query<&ItemLanes, With<Belt>>,
) {
    let facing = event.facing();
    debug!(
        "Placing {:?} at {:?} facing {:?}",
        event.flb, event.structure, facing
    );
    let size = event.structure.size();

    // Full-height blocks must sit at an even y coordinate. If the ray lands
    // on an odd slot (e.g. top face of a belt), snap down to the nearest even.
    let (flb, brt) = if size.is_full_block() {
        let snapped = event.flb.snap_height_even();
        let dy = snapped.y - event.flb.y;
        let brt_adj = event.brt.step(WorldCoordsDelta::ZERO.height(dy));
        (snapped, brt_adj)
    } else {
        (event.flb, event.brt)
    };

    let place = PlaceStructure {
        flb,
        brt,
        ..*event.event()
    };

    // Direction for raycast sizing; defaults to North for non-directional (symmetric) blocks.
    let rt = size.into_raycast_target(facing.unwrap_or(HDir::North));

    // Check if any WorldCoords the structure would occupy is already taken.
    let first_conflict = size.iter_coords(flb).find(|c| coord_map.0.contains_key(c));

    if let Some(conflict) = first_conflict {
        if event.structure == Structure::Belt {
            if let Some(&existing) = coord_map.0.get(&conflict)
                && let Ok(old_lanes) = belts_q.get(existing)
            {
                // Belt-on-belt: replace the old belt and transfer its items to the new one.
                let transferred = old_lanes.0.clone();
                cmd.entity(existing).despawn();
                coord_map.0.remove(&conflict);
                cmd.entity(event.entity)
                    .insert((Belt, ItemLanes(transferred), rt));
                cmd.entity(event.entity).insert(place.to_bundle());
                coord_map.0.insert(flb, event.entity);
                return;
            }
        }
        // Any other collision: ignore the placement.
        cmd.entity(event.entity).despawn();
        return;
    }

    match event.structure {
        Structure::Belt => {
            if let Some(dir) = facing {
                cmd.entity(event.entity)
                    .insert((Belt, ItemLanes::default(), rt, dir));
            } else {
                cmd.entity(event.entity)
                    .insert((Belt, ItemLanes::default(), rt));
            }
        }
        Structure::Source => {
            cmd.entity(event.entity).insert((
                Source::default(),
                OutputBuffer::default(),
                OutputsToBelt {
                    at: flb.step(facing.unwrap_or(HDir::North)),
                },
                rt,
            ));
        }
        Structure::Sink => {
            cmd.entity(event.entity)
                .insert((Sink, InputBuffer::default(), rt));
        }
        Structure::Miner => {
            let dir = facing.expect("Miner must have a facing direction");
            cmd.entity(event.entity).insert((
                Miner { ticks: 0, dir },
                OutputBuffer::default(),
                OutputsToBelt {
                    at: flb.step(dir.opposite()),
                },
                rt,
                dir,
            ));
        }
        Structure::Furnace => {
            cmd.entity(event.entity).insert((
                Furnace::default(),
                InputBuffer::default(),
                Filter::none(),
                OutputBuffer::default(),
                OutputsToBelt {
                    at: flb.step(facing.unwrap_or(HDir::North)),
                },
                rt,
            ));
        }
        Structure::Assembler => {
            cmd.entity(event.entity).insert((
                Assembler::default(),
                InputBuffer::default(),
                Filter::none(),
                OutputBuffer::default(),
                OutputsToBelt {
                    at: flb.step(facing.unwrap_or(HDir::North)),
                },
                rt,
            ));
        }
        Structure::Collector => {
            cmd.entity(event.entity).insert((
                Collector {
                    state: CollectorState::ReadyToPickUp,
                },
                rt,
            ));
        }
        Structure::Corn => {
            cmd.entity(event.entity)
                .insert((Corn::Growing { age: 0 }, rt));
        }
        Structure::Rock
        | Structure::Dirt
        | Structure::IronOreDeposit
        | Structure::CopperOreDeposit => {
            cmd.entity(event.entity).insert(rt);
        }
    };

    if let Some(dir) = facing {
        cmd.entity(event.entity)
            .insert(place.to_bundle())
            .insert(dir);
    } else {
        cmd.entity(event.entity).insert(place.to_bundle());
    }
    // Register every WorldCoords the structure occupies.
    for c in size.iter_coords(flb) {
        coord_map.0.insert(c, event.entity);
    }
}

fn on_place_item(event: On<PlaceItem>, mut cmd: Commands, transforms: Query<&Transform>) {
    if transforms.contains(event.entity) {
        cmd.entity(event.entity).insert(event.item);
    } else {
        cmd.entity(event.entity)
            .insert((event.item, Transform::default()));
    }
}

fn on_remove_block(
    event: On<RemoveBlock>,
    outputs_to_belts: Query<Option<&OutputsToBelt>>,
    coords_q: Query<&WorldCoords>,
    lanes_q: Query<&ItemLanes>,
    blocks_q: Query<&Structure>,
    mut coord_map: ResMut<CoordsMap>,
    player: Res<Player>,
    type_registry: Res<AppTypeRegistry>,
    // EntityRef reads all components, so it conflicts with &mut Inventory — use ParamSet.
    mut params: ParamSet<(Query<EntityRef>, Query<&mut Inventory>)>,
    buf_q: Query<(Option<&InputBuffer>, Option<&OutputBuffer>)>,
    mut cmd: Commands,
) {
    debug!("Removing {:?}", event.entity);
    if let Ok(c) = outputs_to_belts.get(event.entity)
        && let Some(c) = c
        && let Some(&other) = coord_map.0.get(&c.at)
    {
        cmd.entity(other).insert(DirtyBelt);
    }
    if let Ok(coords) = coords_q.get(event.entity) {
        if let Ok(&block) = blocks_q.get(event.entity) {
            for c in block.size().iter_coords(*coords) {
                if coord_map.0.get(&c) == Some(&event.entity) {
                    coord_map.0.remove(&c);
                }
            }
        } else {
            coord_map.0.remove(coords);
        }
    }
    if let Ok(lanes) = lanes_q.get(event.entity) {
        for (_, item) in lanes.0.left.iter().chain(lanes.0.right.iter()) {
            cmd.entity(*item).despawn();
        }
    }
    // Return the block's drops to the player's inventory.
    if let Ok(block) = blocks_q.get(event.entity) {
        let mut stacks: Vec<Stack> = match block.break_drop() {
            BreakDrop::None => return,
            BreakDrop::Item(item) => vec![Stack::from(item)],
            BreakDrop::Custom(type_id) => {
                let registry = type_registry.read();
                let entities = params.p0();
                let entity_ref = entities.get(event.entity).unwrap();
                registry
                    .get_type_data::<ReflectComponent>(type_id)
                    .and_then(|rc| rc.reflect(entity_ref))
                    .and_then(|reflect_val| {
                        registry
                            .get_type_data::<ReflectWorldDrop>(type_id)
                            .and_then(|rwd| rwd.get(reflect_val))
                            .map(|world_drop| world_drop.drop_items())
                    })
                    .unwrap()
            }
        };
        // Drain input/output buffers — general mechanism for all machines.
        if let Ok((input_buf, output_buf)) = buf_q.get(event.entity) {
            if let Some(buf) = input_buf {
                stacks.extend(buf.slots.iter().cloned());
            }
            if let Some(buf) = output_buf {
                stacks.extend(buf.slots.iter().cloned());
            }
        }
        if let Ok(mut inv) = params.p1().get_mut(player.0) {
            for stack in stacks {
                if let Err(e) = inv.insert(stack) {
                    warn!("Could not add {:?} to player inventory: {e}", stack.item);
                }
            }
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
    mut cmd: Commands,
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
    let shape = belt.into_inner();

    cmd.entity(event.entity).insert(OutputsToBelt {
        at: coords.step(*shape),
    });

    let maybe_belt = coords_map.0.get(&coords.step(*shape));
    if let Some(&maybe_belt) = maybe_belt
        && belts.contains(maybe_belt)
    {
        cmd.entity(maybe_belt).insert(DirtyBelt);
    }
}

fn on_load_machine_input(
    event: On<LoadMachineInput>,
    player: Res<Player>,
    mut inventories: Query<&mut Inventory>,
    mut machine_q: Query<(&mut InputBuffer, Option<&Filter>)>,
) {
    let Ok(mut inv) = inventories.get_mut(player.0) else {
        return;
    };
    let Ok((mut input_buf, filter)) = machine_q.get_mut(event.machine) else {
        return;
    };
    let Some(stack) = inv.take_slot(event.player_inventory_slot) else {
        return;
    };
    input_buf.insert(&[stack]);
}

fn on_unload_machine_output(
    event: On<UnloadMachineOutput>,
    player: Res<Player>,
    mut inventories: Query<&mut Inventory>,
    mut output_bufs: Query<&mut OutputBuffer>,
) {
    let Ok(mut inv) = inventories.get_mut(player.0) else {
        return;
    };
    let Ok(mut output_buf) = output_bufs.get_mut(event.machine) else {
        return;
    };
    if event.output_slot < output_buf.buffer.slots.len() {
        let item = output_buf.buffer.slots.remove(event.output_slot);
        let _ = inv.insert(item);
    }
}

fn on_set_assembler_recipe(event: On<SetAssemblerRecipe>, mut assemblers: Query<&mut Assembler>) {
    let Ok(mut assembler) = assemblers.get_mut(event.assembler) else {
        return;
    };
    assembler.configured_recipe = event.recipe.clone();
}

fn determine_belt_shape(
    mut belts: Query<
        (Entity, &WorldCoords, &HDir, Option<&mut BeltShape>),
        (With<Belt>, Or<(Added<Belt>, With<DirtyBelt>)>),
    >,
    belts_check: Query<(), With<Belt>>,
    affecters: Query<&OutputsToBelt>,
    coord_map: Res<CoordsMap>,
    mut cmd: Commands,
) {
    for (entity, coords, &dir, current_shape) in belts.iter_mut() {
        if let Ok(a) = affecters.get(entity)
            && let Some(&a) = coord_map.0.get(&a.at)
            && let Ok(()) = belts_check.get(a)
        {
            continue;
        }
        let a_feeds_b = |a: WorldCoords, b: WorldCoords| {
            coord_map
                .0
                .get(&a)
                .and_then(|a| affecters.get(*a).ok())
                .is_some_and(|a| a.at == b)
        };
        let fed_from_side = |loc: WorldCoords, fed_from: HDir| {
            let middle_fed = loc.step(fed_from);
            a_feeds_b(middle_fed, loc)
                || a_feeds_b(middle_fed.step(Dir::Up), loc)
                || a_feeds_b(middle_fed.step(Dir::Down), loc)
        };

        let fed_from_left = fed_from_side(*coords, dir.left());
        let fed_from_right = fed_from_side(*coords, dir.right());
        let fed_from_behind = fed_from_side(*coords, dir.opposite());
        debug!(
            "Placing with: {:?}",
            (fed_from_left, fed_from_behind, fed_from_right)
        );
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
        let output = coords.step(desired);
        match current_shape {
            Some(mut shape) => {
                if *shape != desired {
                    shape.set_if_neq(desired);
                    if let Some(&e) = coord_map.0.get(&output) {
                        cmd.entity(e).insert(DirtyBelt);
                    }
                }
            }
            None => {
                if let Some(&e) = coord_map.0.get(&output) {
                    cmd.entity(e).insert(DirtyBelt);
                }
                cmd.entity(entity).insert(desired);
            }
        }
        cmd.entity(entity).insert(OutputsToBelt { at: output });
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
        side: Side,
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
                    side,
                });
            }
        }
    }
    for transfer in transfers {
        let mut source = invs.get_mut(transfer.source).unwrap();
        let slot = source.1.0[transfer.side].remove(0);
        drop(source);

        let mut dest = invs.get_mut(transfer.dest).unwrap();
        let lane = &mut dest.1.0[transfer.side];
        lane.push((dest.3.num_pos(transfer.side), slot.1));
    }
}

fn side_loading(
    mut invs: Query<(Entity, &mut ItemLanes, &WorldCoords, &BeltShape)>,
    coord_map: Res<CoordsMap>,
) {
    struct Transfer {
        source: Entity,
        dest: Entity,
        source_side: Side,
        dest_side: Side,
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
                        source_side: side,
                        dest_side,
                        position,
                    });
                }
            }
        }
    }
    for transfer in transfers {
        let mut source = invs.get_mut(transfer.source).unwrap();
        let slot = source.1.0[transfer.source_side].remove(0);
        drop(source);

        let mut dest = invs.get_mut(transfer.dest).unwrap();
        let lane = &mut dest.1.0[transfer.dest_side];
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

fn fill_sources(mut sources: Query<(&Source, &mut OutputBuffer)>) {
    for (source, mut buffer) in &mut sources {
        if let Some(item) = source.configured_item {
            buffer.insert(&[item.into()]);
        }
    }
}

fn on_set_source_item(event: On<SetSourceItem>, mut sources: Query<&mut Source>) {
    let Ok(mut source) = sources.get_mut(event.source) else {
        return;
    };
    source.configured_item = event.item;
}

fn grow_corn(mut corns: Query<&mut Corn>) {
    for mut corn in &mut corns {
        let age = match corn.bypass_change_detection() {
            Corn::Growing { age } => *age,
            Corn::FullyGrown => continue,
        };

        let new_age = age + 1;
        if new_age >= CORN_TICKS_PER_STAGE * 3 {
            // Transition to fully grown — triggers Changed<Corn> for visual update
            *corn = Corn::FullyGrown;
        } else if new_age % CORN_TICKS_PER_STAGE == 0 {
            // Stage boundary crossed — trigger Changed<Corn> for visual update
            match &mut *corn {
                Corn::Growing { age } => *age = new_age,
                Corn::FullyGrown => {}
            }
        } else {
            // Mid-stage tick — silent update, no visual change needed
            match corn.bypass_change_detection() {
                Corn::Growing { age } => *age = new_age,
                Corn::FullyGrown => {}
            }
        }
    }
}

fn fill_miners(
    mut miners: Query<(&WorldCoords, &mut Miner, &mut OutputBuffer)>,
    world_blocks: Query<&Structure>,
    coord_map: Res<CoordsMap>,
) {
    for (miner_coords, mut miner, mut buffer) in &mut miners {
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
        let stack = item.into();
        if !buffer.would_overflow(&[stack]) {
            buffer.insert(&[stack]);
        }
    }
}

fn push_to_belt(
    mut pushers: Query<(&mut OutputBuffer, &WorldCoords, &OutputsToBelt)>,
    mut belts: Query<(Entity, &mut ItemLanes), With<Belt>>,
    coord_map: Res<CoordsMap>,
    mut cmd: Commands,
) {
    for (mut buffer, _coords, output_dir) in &mut pushers {
        let target = output_dir.at;
        let Some(&belt_entity) = coord_map.0.get(&target) else {
            continue;
        };
        let Ok((belt_entity, mut lanes)) = belts.get_mut(belt_entity) else {
            continue;
        };
        if lanes.0.left.len() >= ITEMS_PER_BELT as usize {
            continue;
        }
        let Some(item) = buffer.remove_any() else {
            continue;
        };
        let entity = cmd.spawn(OnBelt).id();
        lanes.0.left.push((POSITIONS_PER_BELT, entity));
        cmd.trigger(PlaceItem {
            entity,
            item: item.item,
        });
    }
}

fn pull_from_belt(
    mut sinks: Query<(&mut InputBuffer, &WorldCoords, Option<&Filter>)>,
    mut belts: Query<(&mut ItemLanes, &HDir)>,
    items: Query<&Item, With<OnBelt>>,
    coord_map: Res<CoordsMap>,
    mut cmd: Commands,
) {
    for (mut buffer, sink_coords, filter) in &mut sinks {
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
                if !filter.map_or(true, |f| f.accepts(item)) {
                    continue;
                }
                lanes.0[side].remove(0);
                cmd.entity(item_entity).despawn();
                buffer.insert(&[item.into()]);
                break;
            }
        }
    }
}

fn tick_collectors(
    mut collectors: Query<(&mut Collector, &WorldCoords, &HDir)>,
    mut belts: Query<(&mut ItemLanes, &BeltShape), With<Belt>>,
    items: Query<&Item, With<OnBelt>>,
    mut output_buffers: Query<&mut OutputBuffer>,
    mut input_buffers: Query<(&mut InputBuffer, Option<&Filter>)>,
    mut visual_transforms: Query<&mut Transform>,
    coord_map: Res<CoordsMap>,
    mut cmd: Commands,
) {
    for (mut collector, &coords, &dir) in &mut collectors {
        let forward = coords.step(dir);
        let backward = coords.step(dir.opposite());
        let forward_ent = coord_map.0.get(&forward).copied();
        let backward_ent = coord_map.0.get(&backward).copied();

        let pickup_pos = Vec3::from(backward) + Vec3::new(0.0, BELT_HEIGHT + 0.5, 0.0);
        let dropoff_pos = Vec3::from(forward) + Vec3::new(0.0, BELT_HEIGHT + 0.5, 0.0);

        let state = collector.state;
        match state {
            CollectorState::ReadyToPickUp => {
                let Some(bwd) = backward_ent else { continue };

                let forward_filter: Option<Filter> = forward_ent
                    .and_then(|fwd_ent| input_buffers.get(fwd_ent).ok())
                    .and_then(|(_, filter)| filter.cloned());

                let maybe_item: Option<Item> = if let Ok((mut lanes, _)) = belts.get_mut(bwd) {
                    let mut taken = None;
                    // todo: remove from the closest side first
                    for side in SIDES {
                        if let Some(&(pos, item_ent)) = lanes.0[side].get(0) {
                            if pos == 0 {
                                if let Ok(&item) = items.get(item_ent) {
                                    if forward_filter.as_ref().map_or(true, |f| f.accepts(item)) {
                                        lanes.0[side].remove(0);
                                        cmd.entity(item_ent).despawn();
                                        taken = Some(item);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    taken
                } else if let Ok(mut out_buf) = output_buffers.get_mut(bwd) {
                    let peeked = out_buf.slots.get(0).map(|s| s.item);
                    match peeked {
                        Some(item) if forward_filter.as_ref().map_or(true, |f| f.accepts(item)) => {
                            out_buf.remove_any().map(|s| s.item)
                        }
                        _ => None,
                    }
                } else {
                    None
                };

                if let Some(item) = maybe_item {
                    // todo: set translation based on pickup,items on belts should move smoothly
                    let visual = cmd.spawn(Transform::from_translation(pickup_pos)).id();
                    collector.state = CollectorState::MovingItem {
                        item,
                        visual,
                        start: pickup_pos,
                        end: dropoff_pos,
                        ticks: 1,
                        needs_place_item: true,
                    };
                }
            }
            CollectorState::MovingItem {
                item,
                visual,
                start,
                end,
                ticks,
                needs_place_item,
            } => {
                // todo: move this to when the entity is first created
                if needs_place_item {
                    cmd.trigger(PlaceItem {
                        entity: visual,
                        item,
                    });
                }
                let t = (ticks as f32 / COLLECTOR_MOVE_TICKS as f32).min(1.0);
                let pos = start.lerp(end, t);
                if let Ok(mut transform) = visual_transforms.get_mut(visual) {
                    transform.translation = pos;
                }
                if ticks >= COLLECTOR_MOVE_TICKS {
                    collector.state = CollectorState::ReadyToDropOff { item, visual };
                } else {
                    collector.state = CollectorState::MovingItem {
                        item,
                        visual,
                        start,
                        end,
                        ticks: ticks + 1,
                        needs_place_item: false,
                    };
                }
            }
            CollectorState::ReadyToDropOff { item, visual } => {
                let Some(fwd) = forward_ent else { continue };

                let deposited = if let Ok((mut in_buf, filter)) = input_buffers.get_mut(fwd) {
                    if filter.map_or(true, |f| f.accepts(item)) {
                        in_buf.insert(&[item.into()]);
                        cmd.entity(visual).despawn();
                        true
                    } else {
                        false
                    }
                } else if let Ok((mut lanes, shape)) = belts.get_mut(fwd) {
                    let mut placed = false;
                    for side in SIDES {
                        let last_pos = lanes.0[side].last().map(|&(p, _)| p).unwrap_or(0);
                        let n_pos = shape.num_pos(side);
                        if (lanes.0[side].len() as i32) < ITEMS_PER_BELT
                            && last_pos + ITEM_SPACING <= n_pos
                        {
                            let new_ent = cmd.spawn(OnBelt).id();
                            lanes.0[side].push((n_pos, new_ent));
                            cmd.trigger(PlaceItem {
                                entity: new_ent,
                                item,
                            });
                            cmd.entity(visual).despawn();
                            placed = true;
                            break;
                        }
                    }
                    placed
                } else {
                    false
                };

                if deposited {
                    collector.state = CollectorState::MovingToStart {
                        ticks: COLLECTOR_MOVE_TICKS,
                    };
                }
            }
            CollectorState::MovingToStart { ticks } => {
                if ticks == 0 {
                    collector.state = CollectorState::ReadyToPickUp;
                } else {
                    collector.state = CollectorState::MovingToStart { ticks: ticks - 1 };
                }
            }
        }
    }
}

fn recalculate_filters(
    mut furnaces: Query<(&Furnace, &InputBuffer, &mut Filter), Without<Assembler>>,
    mut assemblers: Query<(&Assembler, &InputBuffer, &mut Filter), Without<Furnace>>,
    recipes: Res<Recipes>,
) {
    let furnace_recipes: Vec<machine::FurnaceRecipe> = recipes
        .0
        .iter()
        .filter_map(|r| {
            if let Recipe::FurnaceRecipe(fr) = r {
                Some(*fr)
            } else {
                None
            }
        })
        .collect();

    for (furnace, input, mut filter) in &mut furnaces {
        *filter = furnace.allowed_items(input, &furnace_recipes);
    }

    for (assembler, input, mut filter) in &mut assemblers {
        *filter = assembler.allowed_items(input);
    }
}

fn process_furnace(
    mut furnaces: Query<(&mut Furnace, &mut InputBuffer, &mut OutputBuffer)>,
    recipes: Res<Recipes>,
) {
    let furnace_recipes: Vec<FurnaceRecipe> = recipes
        .0
        .iter()
        .filter_map(|r| match r {
            Recipe::FurnaceRecipe(fr) => Some(*fr),
            _ => None,
        })
        .collect();

    for (mut furnace, mut input, mut output) in &mut furnaces {
        furnace.tick(&mut input, &mut output, &furnace_recipes);
    }
}

fn process_assembler(mut assemblers: Query<(&mut Assembler, &mut InputBuffer, &mut OutputBuffer)>) {
    for (mut assembler, mut input, mut output) in &mut assemblers {
        assembler.tick(&mut input, &mut output);
    }
}

fn consume_sink_buffer(mut sinks: Query<&mut InputBuffer, With<Sink>>) {
    for mut buffer in &mut sinks {
        for slot in &mut buffer.slots {
            slot.count = 0;
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
    side: Side,
    pos: i32,
) -> Transform {
    match belt {
        BeltShape::Straight(dir) => {
            let x = match side {
                Side::Left => LANE_OFFSET,
                Side::Right => -LANE_OFFSET,
            };
            let start = Vec3::new(x, BELT_HEIGHT, 0.5);
            let end = Vec3::new(x, BELT_HEIGHT, -0.5);

            let t = (pos + ITEM_SPACING / 2) as f32 / POSITIONS_PER_BELT as f32;
            let angle = dir.angle();
            Transform::from_translation(
                start.lerp(end, t).rotate_y(angle) + Vec3::from(coords.into()),
            )
        }
        BeltShape::Curve(curve) => {
            let center_offset =
                (Vec2::from(belt.input().opposite()) + Vec2::from(belt.output())) / 2.0;
            let n_pos = if curve.inner_lane() == side {
                POSITIONS_PER_INNER_CURVE
            } else {
                POSITIONS_PER_OUTER_CURVE
            };
            let lane_offset = if curve.inner_lane() == side {
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
                Vec3::new(local_offset.y, BELT_HEIGHT, local_offset.x) + Vec3::from(coords.into()),
            )
            .with_rotation(Quat::from_rotation_y(angle + PI / 2.0))
        }
        BeltShape::RampUp(dir) => {
            let coords = coords.into();
            let mut lower = item_position(BeltShape::Straight(dir), coords, side, pos);
            let upper = item_position(BeltShape::Straight(dir), coords.step(Dir::Up), side, pos);
            let t = (POSITIONS_PER_BELT - pos) as f32 / POSITIONS_PER_BELT as f32;
            let translation = lower.translation * (1.0 - t) + upper.translation * t;
            lower.translation = translation;
            lower
        }
        BeltShape::RampDown(dir) => {
            let coords = coords.into();
            let mut upper = item_position(BeltShape::Straight(dir), coords, side, pos);
            let lower = item_position(BeltShape::Straight(dir), coords.step(Dir::Down), side, pos);
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
    fn add_world_block(&mut self, coords: impl Into<WorldCoords>, block: Structure) -> Entity;
    fn add_item(&mut self, belt: Entity, pos: i32, side: Side) -> Entity;
    fn find_item(&mut self, item: Entity) -> Option<(Item, Transform)>;
    fn find_belt(&mut self, belt: Entity) -> Option<(BeltShape, Transform)>;
    fn item_count_on_belt(&mut self, belt: Entity) -> usize;
    fn remove_belt_at(&mut self, coords: impl Into<WorldCoords>) -> bool;
    fn layout(&mut self, s: &str) -> Layout;
    #[allow(unused)]
    fn has_placement_errors(&self) -> bool;
    #[allow(unused)]
    fn take_placement_errors(&mut self) -> Vec<ItemPlacementError>;
    /// Returns the player entity set up by CorePlugin.
    fn spawn_player(&self) -> Entity;
}

#[cfg(test)]
impl AppExtension for App {
    fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: HDir) -> Entity {
        let entity = self.world_mut().spawn_empty().id();
        let flb: WorldCoords = coords.into();
        let brt = Structure::Belt.brt_for(flb, Some(dir));
        self.world_mut().trigger(PlaceStructure {
            entity,
            structure: Structure::Belt,
            flb,
            brt,
        });
        entity
    }

    fn add_world_block(&mut self, coords: impl Into<WorldCoords>, block: Structure) -> Entity {
        let entity = self.world_mut().spawn_empty().id();
        let flb: WorldCoords = coords.into();
        let brt = block.brt_for(flb, None);
        self.world_mut().trigger(PlaceStructure {
            entity,
            structure: block,
            flb,
            brt,
        });
        entity
    }

    fn add_item(&mut self, belt: Entity, pos: i32, side: Side) -> Entity {
        let entity = self.world_mut().spawn(OnBelt).id();
        if let Some(mut lanes) = self.world_mut().get_mut::<ItemLanes>(belt) {
            lanes.0[side].push((pos, entity));
        }
        self.world_mut().trigger(PlaceItem {
            entity,
            item: Item::Belt,
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

    fn spawn_player(&self) -> Entity {
        self.world().resource::<Player>().0
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
            Structure::IronOreDeposit,
        );

        let miner = app.world_mut().spawn_empty().id();
        let flb = o;
        app.world_mut().trigger(PlaceStructure {
            entity: miner,
            structure: Structure::Miner,
            brt: Structure::Miner.brt_for(flb, Some(HDir::South)),
            flb,
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
        app.add_world_block(o.step(HDir::South), Structure::IronOreDeposit);

        // Place miner at origin facing the ore to the south.
        let miner = app.world_mut().spawn_empty().id();
        let flb = o;
        app.world_mut().trigger(PlaceStructure {
            entity: miner,
            structure: Structure::Miner,
            brt: Structure::Miner.brt_for(flb, Some(HDir::South)),
            flb,
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
            (Structure::IronOreDeposit, Item::IronOre),
            (Structure::CopperOreDeposit, Item::CopperOre),
        ] {
            let mut app = test_app();
            let o = WorldCoords::ORIGIN;

            app.add_world_block(o.step(HDir::South), deposit);

            let miner = app.world_mut().spawn_empty().id();
            let flb = o;
            app.world_mut().trigger(PlaceStructure {
                entity: miner,
                structure: Structure::Miner,
                brt: Structure::Miner.brt_for(flb, Some(HDir::South)),
                flb,
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

    #[test]
    fn belt_ramping_down_curves_belt_in_front_on_incline() {
        let mut app = test_app();
        let ramp = app.add_belt(WorldCoords::ORIGIN.step(Dir::Up), HDir::North);
        let curve = app.add_belt(WorldCoords::ORIGIN.step(Dir::North), HDir::East);
        app.update();

        app.world_mut().trigger(Incline { entity: ramp });
        app.update();

        let (c, _) = app.find_belt(curve).unwrap();
        assert_eq!(c, BeltShape::Curve(Curve::NorthToEast));
    }

    #[test]
    fn belt_ramping_down_curves_belt_in_front_on_place() {
        let mut app = test_app();
        let ramp = app.add_belt(WorldCoords::ORIGIN.step(Dir::South), HDir::North);
        app.update();
        app.world_mut().trigger(Incline { entity: ramp });
        app.update();

        let curve = app.add_belt(WorldCoords::ORIGIN.step(Dir::Up), HDir::East);
        app.update();

        let (c, _) = app.find_belt(curve).unwrap();
        assert_eq!(c, BeltShape::Curve(Curve::NorthToEast));
    }

    #[test]
    fn belt_curves_belt_in_front() {
        let mut app = test_app();
        let curve = app.add_belt(WorldCoords::ORIGIN, HDir::East);
        app.update();

        app.add_belt(WorldCoords::ORIGIN.step(Dir::South), HDir::North);
        app.update();

        let (c, _) = app.find_belt(curve).unwrap();
        assert_eq!(c, BeltShape::Curve(Curve::NorthToEast));
    }

    #[test]
    fn belt_two_side_load_is_straight() {
        let mut app = test_app();
        app.add_belt(WorldCoords::ORIGIN.step(HDir::West), HDir::East);
        app.add_belt(WorldCoords::ORIGIN.step(HDir::East), HDir::West);
        app.update();

        let belt = app.add_belt(WorldCoords::ORIGIN, HDir::North);
        app.update();

        let (c, _) = app.find_belt(belt).unwrap();
        assert_eq!(c, BeltShape::Straight(HDir::North));
    }

    #[test]
    fn belt_side_load_and_back_load_is_straight() {
        let mut app = test_app();
        app.add_belt(WorldCoords::ORIGIN.step(HDir::South), HDir::North);
        app.add_belt(WorldCoords::ORIGIN.step(HDir::East), HDir::West);
        app.update();

        let belt = app.add_belt(WorldCoords::ORIGIN, HDir::North);
        app.update();

        let (c, _) = app.find_belt(belt).unwrap();
        assert_eq!(c, BeltShape::Straight(HDir::North));
    }

    #[test]
    fn belt_remp_doesnt_update_when_pointing_at_belt() {
        let mut app = test_app();
        app.add_belt(
            WorldCoords::ORIGIN.step(Dir::Up).step(HDir::North),
            HDir::North,
        );
        app.add_belt(WorldCoords::ORIGIN.step(HDir::North), HDir::North);
        app.update();
        let ramp = app.add_belt(WorldCoords::ORIGIN, HDir::North);
        app.update();

        app.world_mut().trigger(Incline { entity: ramp });
        app.update();
        app.add_belt(WorldCoords::ORIGIN.step(HDir::South), HDir::North);
        app.update();

        let (c, _) = app.find_belt(ramp).unwrap();
        assert_eq!(c, BeltShape::RampUp(HDir::North));
    }

    #[test]
    fn collector_deposits_item_into_furnace() {
        let mut app = test_app();

        // Layout (top = North = -z):
        //   furnace at (0, 0, -2)  →  occupies z=-2 and z=-1
        //   collector at (0, 0, 0) facing North
        //     forward  = (0,0,-1) → inside furnace footprint
        //     backward = (0,0, 1) → belt
        //   belt at (0, 0, 1) facing North (exits toward collector)

        let furnace = {
            let e = app.world_mut().spawn_empty().id();
            let flb: WorldCoords = (0i32, 0i32, -2i32).into();
            app.world_mut().trigger(PlaceStructure {
                entity: e,
                structure: Structure::Furnace,
                brt: Structure::Furnace.brt_for(flb, Some(HDir::North)),
                flb,
            });
            e
        };
        let _collector = {
            let e = app.world_mut().spawn_empty().id();
            let flb: WorldCoords = (0i32, 0i32, 0i32).into();
            app.world_mut().trigger(PlaceStructure {
                entity: e,
                structure: Structure::Collector,
                brt: Structure::Collector.brt_for(flb, Some(HDir::North)),
                flb,
            });
            e
        };
        let belt = app.add_belt((0i32, 0i32, 1i32), HDir::North);
        app.update(); // flush deferred component inserts

        // Manually add an IronOre item at position 0 on the belt (head = exit end)
        let item_entity = app
            .world_mut()
            .spawn((OnBelt, Item::IronOre, Transform::default()))
            .id();
        app.world_mut().get_mut::<ItemLanes>(belt).unwrap().0[Side::Left].push((0, item_entity));

        // Check CoordsMap has the expected entries
        {
            let coord_map = app.world().resource::<CoordsMap>();
            let furnace_cell: WorldCoords = (0i32, 0i32, -1i32).into();
            let belt_cell: WorldCoords = (0i32, 0i32, 1i32).into();
            assert!(
                coord_map.0.contains_key(&furnace_cell),
                "CoordsMap should have furnace at (0,0,-1)"
            );
            assert!(
                coord_map.0.contains_key(&belt_cell),
                "CoordsMap should have belt at (0,0,1)"
            );
        }

        // Tick 1: collector should pick up the item
        app.update();
        {
            let world = app.world_mut();
            let collector_state = world.query::<&Collector>().single(world).unwrap().state;
            assert!(
                matches!(collector_state, CollectorState::MovingItem { .. }),
                "after tick 1, collector should be MovingItem, got: {:?}",
                collector_state
            );
        }

        // Run COLLECTOR_MOVE_TICKS more ticks
        for _ in 0..COLLECTOR_MOVE_TICKS {
            app.update();
        }

        {
            let world = app.world_mut();
            let collector_state = world.query::<&Collector>().single(world).unwrap().state;
            assert!(
                matches!(collector_state, CollectorState::ReadyToDropOff { .. })
                    || matches!(collector_state, CollectorState::MovingToStart { .. }),
                "after move ticks, collector should be ReadyToDropOff or MovingToStart, got: {:?}",
                collector_state
            );
        }

        // A few more ticks to ensure deposit happens
        for _ in 0..5 {
            app.update();
        }

        // The furnace consumes the IronOre from InputBuffer immediately when it starts
        // processing. So check that the furnace is now processing (or has produced output).
        let furnace_component = app.world().get::<Furnace>(furnace).unwrap();
        let input_buf = app.world().get::<InputBuffer>(furnace).unwrap();
        let output_buf = app.world().get::<OutputBuffer>(furnace).unwrap();
        assert!(
            matches!(furnace_component.status, MachineStatus::Processing { .. })
                || output_buf.slots.iter().any(|s| s.item == Item::IronIngot)
                || input_buf.slots.iter().any(|s| s.item == Item::IronOre),
            "expected furnace to be processing iron ore (collector delivered it), \
             got status: {:?}, input: {:?}, output: {:?}",
            furnace_component.status,
            input_buf.slots,
            output_buf.slots,
        );
    }

    #[test]
    fn load_machine_input_moves_ore_to_furnace_input_buffer() {
        let mut app = test_app();

        let player = app.spawn_player();
        let furnace = app.add_world_block(WorldCoords::ORIGIN, Structure::Furnace);
        app.update();

        // Give the player 2 iron ore and find which slot they land in.
        let ore_slot = {
            let mut inv = app.world_mut().get_mut::<Inventory>(player).unwrap();
            inv.insert(Stack::new(Item::IronOre, 2)).unwrap();

            (0..64)
                .find(|&s| {
                    inv.get(s)
                        .map(|st| st.item == Item::IronOre)
                        .unwrap_or(false)
                })
                .expect("player should have iron ore")
        };

        app.world_mut().trigger(LoadMachineInput {
            player_inventory_slot: ore_slot,
            machine: furnace,
        });
        app.update();

        let input_buf = app.world().get::<InputBuffer>(furnace).unwrap();
        assert!(
            input_buf
                .slots
                .iter()
                .any(|s| s.item == Item::IronOre && s.count >= 1),
            "expected at least 1 iron ore in furnace input buffer, got: {:?}",
            input_buf.slots,
        );
    }

    #[test]
    fn into_raycast() {
        let i = StructureSize {
            height: 2,
            width: 1,
            depth: 1,
        };
        let actual = i.into_raycast_target(HDir::South);
        let expected = RaycastTarget {
            half_extents: Vec3::new(0.5, 0.5, 0.5),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn place_block_facing() {
        use Dir::*;
        #[derive(Debug)]
        struct TestCase {
            flb: WorldCoords,
            brt: WorldCoords,
            expected: Option<HDir>,
        }
        let o = WorldCoords::ORIGIN;
        let cases = vec![
            TestCase {
                flb: o,
                brt: o.step(South).step(West).step(Up),
                expected: Some(HDir::North),
            },
            TestCase {
                flb: o,
                brt: o.step(North).step(East).step(Up),
                expected: Some(HDir::South),
            },
            TestCase {
                flb: o,
                brt: o.step(North).step(West).step(Up),
                expected: Some(HDir::East),
            },
            TestCase {
                flb: o,
                brt: o.step(South).step(East).step(Up),
                expected: Some(HDir::West),
            },
            // Non-directional: brt directly above, dx==0 && dz==0
            TestCase {
                flb: o,
                brt: o.step(Up).step(Up),
                expected: None,
            },
            // Partial offset (one axis zero) → None
            TestCase {
                flb: o,
                brt: o.step(South).step(Up),
                expected: None,
            },
            TestCase {
                flb: o,
                brt: o.step(West).step(Up),
                expected: None,
            },
        ];
        for case in cases {
            let event = PlaceStructure {
                entity: Entity::PLACEHOLDER,
                structure: Structure::Dirt,
                flb: case.flb,
                brt: case.brt,
            };
            let actual = event.facing();
            assert_eq!(actual, case.expected, "{case:#?}");
        }
    }
}
