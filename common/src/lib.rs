use bevy::{math::ops::sin_cos, prelude::*, reflect::reflect_trait};
use std::any::TypeId;
use std::collections::HashMap;
use std::f32::consts::PI;

pub mod dir;
pub mod inventory;
pub mod machine;
pub mod player;
pub mod sim;
pub mod world_gen;

pub use player::{HandCrafter, Player, spawn_player};
pub use sim::SimPlugin;
pub use world_gen::{FlatWorldPlugin, PerlinWorldPlugin};
pub mod invariants;

pub use dir::*;

pub use machine::*;

use inventory::{Inventory, Stack};

pub const ITEMS_PER_BELT: i32 = 4;
pub const POSITIONS_PER_BELT: i32 = 256;
pub const BASE_BELT_SPEED: i32 = 8;
/// How far from center each lane is.
pub const LANE_OFFSET: f32 = 0.25;
/// How far from the bottom of the voxel the belt surface is.
pub const BELT_HEIGHT: f32 = 0.25;

pub const ITEM_SIZE: f32 = 1.0 / (ITEMS_PER_BELT as f32);
#[derive(Resource)]
pub struct MinerTicksPerExtract(pub u32);
impl Default for MinerTicksPerExtract {
    fn default() -> Self {
        Self(600)
    }
}

#[derive(Resource)]
pub struct CollectorMoveTicks(pub u32);
impl Default for CollectorMoveTicks {
    fn default() -> Self {
        Self(60)
    }
}
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
        app.add_plugins(crate::invariants::InvariantsPlugin);

        app.register_type::<Corn>()
            .register_type_data::<Corn, ReflectWorldDrop>()
            .register_type::<Furnace>()
            .register_type_data::<Furnace, ReflectWorldDrop>()
            .register_type::<Assembler>()
            .register_type_data::<Assembler, ReflectWorldDrop>();

        app.init_resource::<CoordsMap>();
        app.insert_resource(Recipes::new());
        app.init_resource::<MinerTicksPerExtract>();
        app.init_resource::<CollectorMoveTicks>();
        app.init_resource::<CornGrowthTicks>();

        app.add_observer(on_place_structure);
        app.add_observer(on_place_item);
        app.add_observer(on_remove_block);
        app.add_observer(on_incline);
        app.add_observer(on_load_machine_input);
        app.add_observer(on_unload_machine_output);
        app.add_observer(on_set_assembler_recipe);
        app.add_observer(on_set_source_item);
        app.add_observer(on_player_mine);

        spawn_player(app.world_mut());
    }
}

// ------
// Models
// ------

#[derive(EntityEvent, Debug, Clone, Copy)]
/// `flb` should always be contained in the bounding box, while `brt` never is.
pub struct PlaceStructure {
    pub entity: Entity,
    pub item: Item,
    /// Front Left Bottom, inclusive
    pub flb: WorldCoords,
    /// Back Right Top, exclusive
    pub brt: WorldCoords,
    pub player: Entity,
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

#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct PlaceItem {
    pub entity: Entity,
    pub item: Item,
}

#[derive(EntityEvent, Debug, Clone)]
pub struct RemoveBlock {
    pub entity: Entity,
    /// `Some(entity)` = player-triggered (drops returned, capacity checked first).
    /// `None` = internal removal (drops skipped, block still cleaned up).
    pub player: Option<Entity>,
}

/// Player moved one item from their inventory into a machine's input buffer.
#[derive(Event, Debug, Clone)]
pub struct LoadMachineInput {
    pub player: Entity,
    pub player_inventory_slot: u16,
    pub machine: Entity,
    /// Which machine input slot to target. `None` = first slot whose filter accepts the item.
    pub machine_input_slot: Option<usize>,
}

/// Player collected an item from a machine's output buffer into their inventory.
#[derive(Event, Debug, Clone)]
pub struct UnloadMachineOutput {
    pub player: Entity,
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

#[derive(Event)]
pub struct PlayerMine {
    pub entity: Entity,
    pub player: Entity,
}

#[derive(Component)]
pub struct Belt;

#[derive(Debug, Component, Default)]
pub struct Source {
    pub configured_item: Option<Item>,
}

#[derive(Component)]
pub struct Miner {
    pub ticks: u32,
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

impl Recipe {
    pub fn ticks(&self) -> u32 {
        match self {
            Recipe::FurnaceRecipe(r) => r.ticks,
            Recipe::AssemblerRecipe(r) => r.ticks,
        }
    }

    pub fn inputs(&self) -> Vec<Stack> {
        match self {
            Recipe::FurnaceRecipe(r) => vec![r.input],
            Recipe::AssemblerRecipe(r) => r.input.clone(),
        }
    }

    pub fn outputs(&self) -> Vec<Stack> {
        match self {
            Recipe::FurnaceRecipe(r) => vec![r.output],
            Recipe::AssemblerRecipe(r) => r.output.clone(),
        }
    }
}

#[derive(Resource)]
pub struct Recipes(pub Vec<Recipe>);

impl Recipes {
    pub fn hand_craftable(&self) -> impl Iterator<Item = (usize, &Recipe)> {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r, Recipe::AssemblerRecipe(_)))
    }

    fn new() -> Self {
        let furnace = |input, output, ticks| {
            Recipe::FurnaceRecipe(machine::FurnaceRecipe {
                input,
                output,
                ticks,
            })
        };
        let assembler = |input, output, ticks| {
            Recipe::AssemblerRecipe(machine::AssemblerRecipe {
                input,
                output,
                ticks,
            })
        };
        let s = |item, count| Stack { item, count };
        Self(vec![
            // Smelting — 2 miners saturate 1 furnace (600t ore / 300t smelt = 2:1)
            furnace(s(Item::IronOre, 1), s(Item::IronIngot, 1), 300),
            furnace(s(Item::CopperOre, 1), s(Item::CopperIngot, 1), 300),
            // Basic components — 1 furnace : 1 component assembler
            assembler(
                vec![s(Item::IronIngot, 1)],
                vec![s(Item::IronPlate, 2)],
                300,
            ),
            assembler(vec![s(Item::IronIngot, 1)], vec![s(Item::IronRod, 2)], 300),
            assembler(
                vec![s(Item::CopperIngot, 1)],
                vec![s(Item::CopperWire, 2)],
                300,
            ),
            assembler(vec![s(Item::IronPlate, 2)], vec![s(Item::Gear, 1)], 300),
            // Circuit — 2 circuit assemblers saturate 1 machine assembler (300t × 2 = 600t machine)
            assembler(
                vec![s(Item::IronPlate, 1), s(Item::CopperWire, 2)],
                vec![s(Item::Circuit, 1)],
                300,
            ),
            // Infrastructure
            assembler(
                vec![s(Item::IronPlate, 1), s(Item::IronRod, 1)],
                vec![s(Item::Belt, 2)],
                150,
            ),
            // Machines
            assembler(
                vec![s(Item::IronPlate, 1), s(Item::Gear, 1), s(Item::Circuit, 1)],
                vec![s(Item::Miner, 1)],
                600,
            ),
            assembler(vec![s(Item::Rock, 8)], vec![s(Item::Furnace, 1)], 600),
            assembler(
                vec![s(Item::IronPlate, 2), s(Item::Gear, 1), s(Item::Circuit, 1)],
                vec![s(Item::Assembler, 1)],
                600,
            ),
            assembler(
                vec![s(Item::IronPlate, 1), s(Item::Circuit, 1)],
                vec![s(Item::Collector, 1)],
                300,
            ),
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
    IronPlate,
    IronRod,
    CopperWire,
    Circuit,
    Miner,
    Furnace,
    Assembler,
    Collector,
    CornKernels,
    CornStalk,
    Biomass,
    Gear,
    PickAxe,
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
            IronPlate => "Iron Plate",
            IronRod => "Iron Rod",
            CopperWire => "Copper Wire",
            Circuit => "Circuit",
            Miner => "Miner",
            Furnace => "Furnace",
            Assembler => "Assembler",
            Collector => "Collector",
            CornKernels => "Corn Kernels",
            CornStalk => "Corn Stalk",
            Biomass => "Biomass",
            Gear => "Gear",
            PickAxe => "Pick Axe",
        }
    }

    pub fn stack_size(self) -> u16 {
        match self {
            Item::PickAxe => 1,
            _ => 100,
        }
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
            | Item::IronPlate
            | Item::IronRod
            | Item::CopperWire
            | Item::Circuit
            | Item::CornStalk
            | Item::Biomass
            | Item::PickAxe
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

    /// Item produced when mined by a Miner or the Player. `None` means not minable.
    pub fn mine(self) -> Option<Item> {
        match self {
            Structure::IronOreDeposit => Some(Item::IronOre),
            Structure::CopperOreDeposit => Some(Item::CopperOre),
            Structure::Rock => Some(Item::Rock),
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
            Structure::IronOreDeposit | Structure::CopperOreDeposit => BreakDrop::Unbreakable,
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

    /// Inserts all components for this structure type onto `cmd`, updates `coord_map`,
    /// and inserts the spatial bundle (transform, structure, flb coords).
    /// Does not consume a player inventory — callers are responsible for that.
    pub fn attach_bundle(
        self,
        cmd: &mut EntityCommands,
        coord_map: &mut CoordsMap,
        flb: WorldCoords,
        facing: Option<HDir>,
    ) {
        let size = self.size();
        let rt = size.into_raycast_target(facing.unwrap_or(HDir::North));
        let rotation = facing
            .map(|d| Quat::from_rotation_y(d.angle()))
            .unwrap_or(Quat::IDENTITY);
        let transform = Transform::from_translation(Vec3::from(flb) + size.center_offset())
            .with_rotation(rotation);

        match self {
            Structure::Belt => {
                cmd.insert((Belt, ItemLanes::default(), rt));
            }
            Structure::Source => {
                cmd.insert((
                    Source::default(),
                    OutputBuffer::default(),
                    OutputsToBelt {
                        at: flb.step(facing.unwrap_or(HDir::North)),
                    },
                    rt,
                ));
            }
            Structure::Sink => {
                cmd.insert((Sink, InputBuffer::default(), rt));
            }
            Structure::Miner => {
                let dir = facing.expect("Miner must have a facing direction");
                cmd.insert((
                    Miner { ticks: 0, dir },
                    OutputBuffer::default(),
                    OutputsToBelt {
                        at: flb.step(dir.opposite()),
                    },
                    rt,
                ));
            }
            Structure::Furnace => {
                cmd.insert((
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
                cmd.insert((
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
                cmd.insert((
                    Collector {
                        state: CollectorState::ReadyToPickUp,
                    },
                    rt,
                ));
            }
            Structure::Corn => {
                cmd.insert((Corn::Growing { age: 0 }, rt));
            }
            Structure::Rock
            | Structure::Dirt
            | Structure::IronOreDeposit
            | Structure::CopperOreDeposit => {
                cmd.insert(rt);
            }
        }

        cmd.insert((self, flb, transform));
        if let Some(dir) = facing {
            cmd.insert(dir);
        }

        for c in size.iter_coords(flb) {
            coord_map.0.insert(c, cmd.id());
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
    /// Block cannot be broken; `RemoveBlock` returns early, nothing changes.
    Unbreakable,
    /// Block can be broken and despawns, but nothing is returned to inventory.
    NoDrop,
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

#[derive(Resource)]
pub struct CornGrowthTicks(pub u32);
impl Default for CornGrowthTicks {
    fn default() -> Self {
        Self(3600)
    }
}

/// State of a planted corn block. `Growing` tracks total age in ticks across all stages;
/// there are 3 equal stages (A/B/C), each `total / 3` ticks wide. `FullyGrown` is stage D.
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
    /// `total_ticks` is the full growth duration from `CornGrowthTicks`.
    pub fn visual_stage(&self, total_ticks: u32) -> u8 {
        match self {
            Corn::FullyGrown => 3,
            Corn::Growing { age } => (*age / (total_ticks / 3)).min(2) as u8,
        }
    }
}

#[derive(Component, Debug, PartialEq, Eq, Clone, Default)]
pub struct Sided<T> {
    pub left: T,
    pub right: T,
}

#[derive(Resource, Default)]
pub struct CreativeMode(pub bool);

#[derive(Resource, Default)]
pub struct CoordsMap(pub HashMap<WorldCoords, Entity>);

// -------
// Systems
// -------

fn on_place_structure(
    event: On<PlaceStructure>,
    mut cmd: Commands,
    mut coord_map: ResMut<CoordsMap>,
    belts_q: Query<&ItemLanes, With<Belt>>,
    mut inventories: Query<&mut Inventory>,
) {
    let Some(structure) = event.item.can_place() else {
        cmd.entity(event.entity).despawn();
        return;
    };

    let facing = event.facing();
    debug!(
        "Placing {:?} at {:?} facing {:?}",
        structure, event.flb, facing
    );
    let size = structure.size();

    // Full-height blocks must sit at an even y coordinate. If the ray lands
    // on an odd slot (e.g. top face of a belt), snap down to the nearest even.
    let flb = if size.is_full_block() {
        event.flb.snap_height_even()
    } else {
        event.flb
    };

    // Validate player has the item before touching any state.
    let Ok(mut inv) = inventories.get_mut(event.player) else {
        cmd.entity(event.entity).despawn();
        return;
    };
    if inv.item_count(event.item) == 0 {
        cmd.entity(event.entity).despawn();
        return;
    }

    // Check if any WorldCoords the structure would occupy is already taken.
    let first_conflict = size.iter_coords(flb).find(|c| coord_map.0.contains_key(c));

    if let Some(conflict) = first_conflict {
        if structure == Structure::Belt {
            if let Some(&existing) = coord_map.0.get(&conflict)
                && let Ok(old_lanes) = belts_q.get(existing)
            {
                // Belt-on-belt: consume item, replace the old belt, transfer its items.
                inv.take_items(Stack::from(event.item));
                let transferred = old_lanes.0.clone();
                drop(inv);
                cmd.entity(existing).despawn();
                coord_map.0.remove(&conflict);
                {
                    let mut ec = cmd.entity(event.entity);
                    structure.attach_bundle(&mut ec, &mut coord_map, flb, facing);
                }
                cmd.entity(event.entity).insert(ItemLanes(transferred));
                return;
            }
        }
        // Any other collision: leave inventory untouched, despawn pre-allocated entity.
        cmd.entity(event.entity).despawn();
        return;
    }

    inv.take_items(Stack::from(event.item));
    drop(inv);
    let mut ec = cmd.entity(event.entity);
    structure.attach_bundle(&mut ec, &mut coord_map, flb, facing);
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
    type_registry: Res<AppTypeRegistry>,
    // EntityRef reads all components, so it conflicts with &mut Inventory — use ParamSet.
    mut params: ParamSet<(Query<EntityRef>, Query<&mut Inventory>)>,
    buf_q: Query<(Option<&InputBuffer>, Option<&OutputBuffer>)>,
    mut cmd: Commands,
) {
    debug!("Removing {:?}", event.entity);

    // Must be first: refuse to destroy unbreakable blocks before any state mutation.
    if let Ok(block) = blocks_q.get(event.entity) {
        if matches!(block.break_drop(), BreakDrop::Unbreakable) {
            return;
        }
    }

    // Collect all drops before touching any state.
    let stacks: Vec<Stack> = if let Ok(block) = blocks_q.get(event.entity) {
        let mut s: Vec<Stack> = match block.break_drop() {
            BreakDrop::Unbreakable => unreachable!(),
            BreakDrop::NoDrop => vec![],
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
        if let Ok((input_buf, output_buf)) = buf_q.get(event.entity) {
            if let Some(buf) = input_buf {
                s.extend(buf.slots.iter().cloned());
            }
            if let Some(buf) = output_buf {
                s.extend(buf.slots.iter().cloned());
            }
        }
        s
    } else {
        vec![]
    };

    // For player-triggered removals, verify inventory has room before destroying anything.
    if let Some(player_entity) = event.player {
        let inv_q = params.p1();
        let Ok(inv) = inv_q.get(player_entity) else {
            return;
        };
        if !inv.can_fit_all(&stacks) {
            return;
        }
    }

    // State mutation begins here — all validation has passed.
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

    if let Some(player_entity) = event.player {
        if let Ok(mut inv) = params.p1().get_mut(player_entity) {
            for stack in stacks {
                let _ = inv.insert(stack);
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
    mut inventories: Query<&mut Inventory>,
    mut machine_q: Query<(&mut InputBuffer, Option<&Filter>)>,
) {
    let Ok((mut input_buf, filter)) = machine_q.get_mut(event.machine) else {
        return;
    };
    // Peek at the inventory slot before taking — validate filter first.
    let Ok(inv) = inventories.get(event.player) else {
        return;
    };
    let Some(stack) = inv.get(event.player_inventory_slot) else {
        return;
    };
    if let Some(filter) = filter {
        if !filter.accepts(stack.item) {
            return;
        }
    }
    // Validation passed — take the item and insert it.
    let Ok(mut inv) = inventories.get_mut(event.player) else {
        return;
    };
    let Some(stack) = inv.take_slot(event.player_inventory_slot) else {
        return;
    };
    input_buf.insert(&[stack]);
}

fn on_unload_machine_output(
    event: On<UnloadMachineOutput>,
    mut inventories: Query<&mut Inventory>,
    mut output_bufs: Query<&mut OutputBuffer>,
) {
    let Ok(mut output_buf) = output_bufs.get_mut(event.machine) else {
        return;
    };
    let Some(&stack) = output_buf.buffer.slots.get(event.output_slot) else {
        return;
    };
    // Insert into inventory first; only remove from output on success.
    let Ok(mut inv) = inventories.get_mut(event.player) else {
        return;
    };
    if inv.insert(stack).is_ok() {
        output_buf.buffer.slots.remove(event.output_slot);
    }
}

fn on_set_assembler_recipe(event: On<SetAssemblerRecipe>, mut assemblers: Query<&mut Assembler>) {
    let Ok(mut assembler) = assemblers.get_mut(event.assembler) else {
        return;
    };
    assembler.configured_recipe = event.recipe.clone();
}

fn on_set_source_item(event: On<SetSourceItem>, mut sources: Query<&mut Source>) {
    let Ok(mut source) = sources.get_mut(event.source) else {
        return;
    };
    source.configured_item = event.item;
}

fn on_player_mine(
    event: On<PlayerMine>,
    mut invs: Query<&mut Inventory>,
    structs: Query<&Structure>,
) {
    let Ok(s) = structs.get(event.entity) else {
        return;
    };
    match s.mine() {
        Some(item) => {
            let Ok(mut inv) = invs.get_mut(event.player) else {
                return;
            };
            inv.insert(item.into()).unwrap();
        }
        None => {}
    };
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
        self.world_mut()
            .resource_scope(|world, mut coord_map: Mut<CoordsMap>| {
                let mut cmd = world.commands();
                let mut ec = cmd.entity(entity);
                Structure::Belt.attach_bundle(&mut ec, &mut *coord_map, flb, Some(dir));
            });
        self.world_mut().flush();
        entity
    }

    fn add_world_block(&mut self, coords: impl Into<WorldCoords>, block: Structure) -> Entity {
        let entity = self.world_mut().spawn_empty().id();
        let flb: WorldCoords = coords.into();
        self.world_mut()
            .resource_scope(|world, mut coord_map: Mut<CoordsMap>| {
                let mut cmd = world.commands();
                let mut ec = cmd.entity(entity);
                block.attach_bundle(&mut ec, &mut *coord_map, flb, None);
            });
        self.world_mut().flush();
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
            self.world_mut().trigger(RemoveBlock {
                entity,
                player: None,
            });
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

    fn find_slot(inv: &Inventory, item: Item) -> u16 {
        (0..64)
            .find(|&s| inv.get(s).map(|st| st.item == item).unwrap_or(false))
            .expect("item not found in inventory")
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
                item: Item::Dirt,
                player: Entity::PLACEHOLDER,
                flb: case.flb,
                brt: case.brt,
            };
            let actual = event.facing();
            assert_eq!(actual, case.expected, "{case:#?}");
        }
    }

    #[test]
    fn filter_accepts_item_transfers() {
        let mut app = test_app();

        let player = app.world_mut().spawn(Inventory::new()).id();
        let machine = app
            .world_mut()
            .spawn((InputBuffer::default(), Filter::from_iter([Item::IronOre])))
            .id();

        {
            let mut inv = app.world_mut().get_mut::<Inventory>(player).unwrap();
            inv.insert(Stack::new(Item::IronOre, 1)).unwrap();
        }
        let ore_slot = find_slot(app.world().get::<Inventory>(player).unwrap(), Item::IronOre);

        app.world_mut().trigger(LoadMachineInput {
            player,
            player_inventory_slot: ore_slot,
            machine,
            machine_input_slot: None,
        });

        let buf = app.world().get::<InputBuffer>(machine).unwrap();
        assert!(buf.slots.iter().any(|s| s.item == Item::IronOre));
        assert!(
            app.world()
                .get::<Inventory>(player)
                .unwrap()
                .get(ore_slot)
                .is_none()
        );
    }

    #[test]
    fn filter_rejects_item_no_transfer() {
        let mut app = test_app();

        let player = app.world_mut().spawn(Inventory::new()).id();
        let machine = app
            .world_mut()
            .spawn((InputBuffer::default(), Filter::from_iter([Item::IronOre])))
            .id();

        {
            let mut inv = app.world_mut().get_mut::<Inventory>(player).unwrap();
            inv.insert(Stack::new(Item::CopperOre, 1)).unwrap();
        }
        let copper_slot = find_slot(
            app.world().get::<Inventory>(player).unwrap(),
            Item::CopperOre,
        );

        app.world_mut().trigger(LoadMachineInput {
            player,
            player_inventory_slot: copper_slot,
            machine,
            machine_input_slot: None,
        });

        let buf = app.world().get::<InputBuffer>(machine).unwrap();
        assert!(buf.slots.is_empty());
        assert!(
            app.world()
                .get::<Inventory>(player)
                .unwrap()
                .get(copper_slot)
                .is_some()
        );
    }

    #[test]
    fn no_filter_component_accepts_any_item() {
        let mut app = test_app();

        let player = app.world_mut().spawn(Inventory::new()).id();
        let machine = app.world_mut().spawn(InputBuffer::default()).id();

        {
            let mut inv = app.world_mut().get_mut::<Inventory>(player).unwrap();
            inv.insert(Stack::new(Item::CopperOre, 1)).unwrap();
        }
        let copper_slot = find_slot(
            app.world().get::<Inventory>(player).unwrap(),
            Item::CopperOre,
        );

        app.world_mut().trigger(LoadMachineInput {
            player,
            player_inventory_slot: copper_slot,
            machine,
            machine_input_slot: None,
        });

        let buf = app.world().get::<InputBuffer>(machine).unwrap();
        assert!(buf.slots.iter().any(|s| s.item == Item::CopperOre));
    }
}
