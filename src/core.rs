use std::{collections::HashMap, f32::consts::PI};

use bevy::prelude::*;

pub const TILE_SIZE: f32 = 32.0;
pub const POSITIONS_PER_TILE: u16 = 256;
pub const ITEM_SPACING: u16 = 64;
pub const BASE_BELT_SPEED: u16 = 8;

pub const POSITIONS_PER_FRAGMENT: u16 = POSITIONS_PER_TILE / 2 - ITEM_SPACING / 2;
pub const POSITIONS_PER_CURVED_TILE: u16 = (POSITIONS_PER_TILE as f32 * PI / 4.0).round() as u16;
pub const ITEMS_PER_TILE: u16 = POSITIONS_PER_TILE / ITEM_SPACING;
pub const BASE_ITEM_MOVEMENT: f32 = TILE_SIZE * BASE_BELT_SPEED as f32 / POSITIONS_PER_TILE as f32;
pub const ITEM_SIZE: f32 = TILE_SIZE / ITEMS_PER_TILE as f32;

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_place_belt);
        app.add_observer(on_place_item);
        app.add_observer(on_remove_belt);
        app.init_resource::<BeltCoords>();
        app.init_resource::<BeltChanges>();
        app.add_systems(
            PostUpdate,
            (despawn_old_belt_entities, clear_changed_belts).chain(),
        );
    }
}

#[derive(EntityEvent, Clone, Debug, PartialEq, Eq)]
pub struct PlaceBelt {
    pub entity: Entity,
    pub dir: Dir,
    pub coords: WorldCoords,
}

#[derive(EntityEvent, Clone, Debug, PartialEq, Eq)]
pub struct PlaceItem {
    pub entity: Entity,
    pub belt: Entity,
    pub pos: u16,
    pub item: Item,
}

#[derive(EntityEvent, Clone, Debug, PartialEq, Eq)]
pub struct RemoveBelt {
    pub entity: Entity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    North,
    East,
    South,
    West,
}

impl Dir {
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
        self.left().opposite()
    }
}

impl From<Dir> for Vec2 {
    fn from(dir: Dir) -> Self {
        match dir {
            Dir::North => Vec2::Y,
            Dir::East => Vec2::X,
            Dir::South => Vec2::NEG_Y,
            Dir::West => Vec2::NEG_X,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorldCoords {
    pub x: i32,
    pub y: i32,
}

impl WorldCoords {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn step(&self, dir: Dir) -> Self {
        match dir {
            Dir::North => Self {
                x: self.x,
                y: self.y + 1,
            },
            Dir::East => Self {
                x: self.x + 1,
                y: self.y,
            },
            Dir::South => Self {
                x: self.x,
                y: self.y - 1,
            },
            Dir::West => Self {
                x: self.x - 1,
                y: self.y,
            },
        }
    }
    pub fn from_cursor(window: &Window) -> Option<Self> {
        let pos = window.cursor_position()?;
        Some(Self {
            x: ((-window.width() / 2.0 + pos.x) / TILE_SIZE).round() as i32,
            y: ((window.height() / 2.0 - pos.y) / TILE_SIZE).round() as i32,
        })
    }
}

impl From<(i32, i32)> for WorldCoords {
    fn from((x, y): (i32, i32)) -> Self {
        Self { x, y }
    }
}

impl From<WorldCoords> for Vec2 {
    fn from(coords: WorldCoords) -> Self {
        Vec2::new(coords.x as f32 * TILE_SIZE, coords.y as f32 * TILE_SIZE)
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Belt {
    Straight(Dir),
    CurvedNorthToEast,
    CurvedNorthToWest,
    CurvedEastToSouth,
    CurvedEastToNorth,
    CurvedSouthToWest,
    CurvedSouthToEast,
    CurvedWestToNorth,
    CurvedWestToSouth,
}

impl Belt {
    pub fn input(&self) -> Dir {
        match self {
            Belt::Straight(dir) => *dir,
            Belt::CurvedNorthToEast => Dir::North,
            Belt::CurvedNorthToWest => Dir::North,
            Belt::CurvedEastToSouth => Dir::East,
            Belt::CurvedEastToNorth => Dir::East,
            Belt::CurvedSouthToWest => Dir::South,
            Belt::CurvedSouthToEast => Dir::South,
            Belt::CurvedWestToNorth => Dir::West,
            Belt::CurvedWestToSouth => Dir::West,
        }
    }

    pub fn output(&self) -> Dir {
        match self {
            Belt::Straight(dir) => *dir,
            Belt::CurvedNorthToEast => Dir::East,
            Belt::CurvedNorthToWest => Dir::West,
            Belt::CurvedEastToSouth => Dir::South,
            Belt::CurvedEastToNorth => Dir::North,
            Belt::CurvedSouthToWest => Dir::West,
            Belt::CurvedSouthToEast => Dir::East,
            Belt::CurvedWestToNorth => Dir::North,
            Belt::CurvedWestToSouth => Dir::South,
        }
    }

    pub fn item_transform(&self, pos: u16, coords: WorldCoords) -> Transform {
        let world_offset = Vec2::from(coords);
        match self {
            Self::Straight(_) => {
                let start = Vec2::from(self.output());
                let end = Vec2::from(self.input().opposite());
                let mid = start.lerp(end, pos as f32 / POSITIONS_PER_TILE as f32);
                Item::transform(world_offset + mid * TILE_SIZE / 2.0)
            }
            _ => {
                let center_offset =
                    (Vec2::from(self.input().opposite()) + Vec2::from(self.output())) * TILE_SIZE
                        / 2.0;
                let angle = pos as f32 / POSITIONS_PER_CURVED_TILE as f32 * PI / 2.0;
                let angle_offset = Vec2::X.angle_to(Vec2::from(self.input()));
                debug!(
                    "angle_offset: {}*PI, angle: {}*PI",
                    angle_offset / PI,
                    angle / PI
                );
                let angle = match self.curvature().unwrap() {
                    CurveDir::CW => angle + angle_offset,
                    CurveDir::CCW => -angle + angle_offset,
                };
                Item::transform(
                    Vec2::from_angle(angle) * TILE_SIZE / 2.0 + center_offset + world_offset,
                )
            }
        }
    }

    pub fn curvature(&self) -> Option<CurveDir> {
        match self {
            Self::Straight(_) => None,
            Self::CurvedEastToNorth
            | Self::CurvedNorthToWest
            | Self::CurvedWestToSouth
            | Self::CurvedSouthToEast => Some(CurveDir::CCW),
            Self::CurvedNorthToEast
            | Self::CurvedEastToSouth
            | Self::CurvedSouthToWest
            | Self::CurvedWestToNorth => Some(CurveDir::CW),
        }
    }

    pub fn num_positions(&self) -> u16 {
        match self {
            Self::Straight(_) => POSITIONS_PER_TILE,
            _ => POSITIONS_PER_CURVED_TILE,
        }
    }
}

pub enum CurveDir {
    /// Clockwise
    CW,
    /// Counter-clockwise
    CCW,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item;

impl Item {
    pub fn transform(vec: Vec2) -> Transform {
        Transform::from_xyz(vec.x, vec.y, 2.0)
    }
}

#[derive(Resource, Default)]
pub struct BeltCoords(HashMap<WorldCoords, (Entity, Belt)>);
impl BeltCoords {
    pub fn insert(&mut self, coords: WorldCoords, entity: Entity, belt: Belt) {
        self.0.insert(coords, (entity, belt));
    }
    pub fn get(&self, coords: WorldCoords) -> Option<(Entity, Belt)> {
        self.0.get(&coords).map(|(entity, belt)| (*entity, *belt))
    }
    pub fn remove(&mut self, coords: WorldCoords) -> Option<(Entity, Belt)> {
        self.0.remove(&coords)
    }
}

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
    pub belt: Belt,
    pub coords: WorldCoords,
}
impl From<NewBelt> for BeltChange {
    fn from(new_belt: NewBelt) -> Self {
        BeltChange::New(new_belt)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemovedBelt {
    pub entity: Entity,
    pub old_belt: Belt,
    pub coords: WorldCoords,
}
impl From<RemovedBelt> for BeltChange {
    fn from(removed_belt: RemovedBelt) -> Self {
        BeltChange::Removed(removed_belt)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplacedBelt {
    pub entity: Entity,
    pub old_entity: Option<Entity>,
    pub old_belt: Belt,
    pub new_belt: Belt,
    pub coords: WorldCoords,
}
impl From<ReplacedBelt> for BeltChange {
    fn from(replaced_belt: ReplacedBelt) -> Self {
        BeltChange::Replaced(replaced_belt)
    }
}

impl BeltChanges {
    pub fn push(&mut self, change: impl Into<BeltChange>) {
        let change = change.into();
        // Check if we can collapse this change with an existing one for the same entity
        let entity = change.entity();

        if let Some(existing_idx) = self.0.iter().position(|c| c.entity() == entity) {
            let existing = self.0[existing_idx].clone();
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
                // New + Removed => nothing (belt created and removed in same frame)
                (BeltChange::New(_), BeltChange::Removed(_)) => {
                    self.0.remove(existing_idx);
                    return;
                }
                // Replaced + Replaced => Replaced (with original old_belt and final new_belt)
                (BeltChange::Replaced(old), BeltChange::Replaced(new)) => {
                    self.0[existing_idx] = ReplacedBelt {
                        entity,
                        old_entity: old.old_entity, // Keep original old_entity
                        old_belt: old.old_belt,
                        new_belt: new.new_belt,
                        coords: new.coords,
                    }
                    .into();
                    return;
                }
                // Replaced + Removed => Removed (with original old_belt)
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

fn on_place_belt(
    trigger: On<PlaceBelt>,
    mut cmd: Commands,
    mut belt_coords: ResMut<BeltCoords>,
    mut changes: ResMut<BeltChanges>,
) {
    debug!(
        "Placing belt {:?} at {:?} facing {:?}",
        trigger.entity, trigger.coords, trigger.dir
    );

    // Check if there's an old belt at these coords (don't despawn yet)
    let old_entity_and_belt = belt_coords.get(trigger.coords);

    let belt = plan_belt_placement(&trigger, &belt_coords);
    cmd.entity(trigger.entity).insert((belt, trigger.coords));
    belt_coords.insert(trigger.coords, trigger.entity, belt);

    // Emit Replaced if replacing an existing belt, otherwise New
    if let Some((old_entity, old_belt)) = old_entity_and_belt {
        changes.push(ReplacedBelt {
            entity: trigger.entity,
            old_entity: Some(old_entity),
            old_belt,
            new_belt: belt,
            coords: trigger.coords,
        });
    } else {
        changes.push(NewBelt {
            entity: trigger.entity,
            coords: trigger.coords,
            belt,
        });
    }

    let ahead = trigger.coords.step(trigger.dir);
    if let Some((entity, belt)) = belt_coords.get(ahead) {
        let place = PlaceBelt {
            entity,
            dir: belt.output(),
            coords: ahead,
        };
        let new_belt = plan_belt_placement(&place, &belt_coords);
        if belt != new_belt {
            cmd.entity(place.entity).insert((new_belt, place.coords));
            belt_coords.insert(place.coords, place.entity, new_belt);
            changes.push(ReplacedBelt {
                entity: place.entity,
                old_entity: None, // Same entity, just changed belt type
                new_belt,
                old_belt: belt,
                coords: place.coords,
            });
        }
    }
}

fn plan_belt_placement(trigger: &PlaceBelt, belt_coords: &BeltCoords) -> Belt {
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
        (true, _, true) | (false, _, false) | (_, true, _) => Belt::Straight(trigger.dir),
        (true, false, false) => match trigger.dir {
            Dir::North => Belt::CurvedEastToNorth,
            Dir::East => Belt::CurvedSouthToEast,
            Dir::South => Belt::CurvedWestToSouth,
            Dir::West => Belt::CurvedNorthToWest,
        },
        (false, false, true) => match trigger.dir {
            Dir::North => Belt::CurvedWestToNorth,
            Dir::East => Belt::CurvedNorthToEast,
            Dir::South => Belt::CurvedEastToSouth,
            Dir::West => Belt::CurvedSouthToWest,
        },
    };
    assert_eq!(belt.output(), trigger.dir);
    belt
}

fn on_place_item(trigger: On<PlaceItem>, mut cmd: Commands, belts: Query<(&Belt, &WorldCoords)>) {
    let Ok((belt, coords)) = belts.get(trigger.belt) else {
        warn!("Couldn't find belt when trying to place item");
        return;
    };
    let transform = belt.item_transform(trigger.pos, *coords);
    cmd.entity(trigger.entity).insert((Item, transform));
}

fn on_remove_belt(
    trigger: On<RemoveBelt>,
    mut belt_coords: ResMut<BeltCoords>,
    belts: Query<(&WorldCoords, &Belt)>,
    mut changes: ResMut<BeltChanges>,
) {
    // Query for the belt's coordinates
    let Ok((coords, belt)) = belts.get(trigger.entity) else {
        warn!(
            "Attempted to remove belt entity {:?} but it doesn't exist",
            trigger.entity
        );
        return;
    };

    debug!("Removing belt at {:?}", coords);

    // Remove from resource and despawn belt entity
    belt_coords.remove(*coords);

    changes.push(RemovedBelt {
        entity: trigger.entity,
        old_belt: *belt,
        coords: *coords,
    });
}

fn despawn_old_belt_entities(
    mut cmd: Commands,
    changes: Res<BeltChanges>,
    mut belt_coords: ResMut<BeltCoords>,
) {
    for change in &changes.0 {
        match change {
            BeltChange::Replaced(ReplacedBelt {
                old_entity: Some(entity),
                ..
            }) => {
                cmd.entity(*entity).despawn();
            }
            BeltChange::Removed(RemovedBelt {
                entity,
                coords: removed_coords,
                ..
            }) => {
                cmd.entity(*entity).despawn();
                belt_coords.remove(*removed_coords);
            }
            _ => {}
        }
    }
}

fn clear_changed_belts(mut changes: ResMut<BeltChanges>) {
    changes.clear();
}

#[cfg(test)]
fn init_tracing() {
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
    fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: Dir) -> Entity;
    fn find_belt(&mut self, entity: Entity) -> Option<(Belt, WorldCoords)>;
    fn find_belt_at(&mut self, coords: impl Into<WorldCoords>) -> Option<Belt>;
    fn add_item(&mut self, belt: Entity, index: u16) -> Entity;
    fn find_item(&mut self, item: Entity) -> Option<(Item, Transform)>;
    fn remove_belt_at(&mut self, coords: impl Into<WorldCoords>) -> bool;
}

#[cfg(test)]
impl AppExtension for App {
    fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: Dir) -> Entity {
        let entity = self.world_mut().spawn_empty().id();
        self.world_mut().trigger(PlaceBelt {
            entity,
            dir,
            coords: coords.into(),
        });
        entity
    }
    fn find_belt(&mut self, entity: Entity) -> Option<(Belt, WorldCoords)> {
        self.world_mut()
            .query::<(&Belt, &WorldCoords)>()
            .get(self.world_mut(), entity)
            .map(|(belt, coords)| (belt.clone(), *coords))
            .ok()
    }

    fn find_belt_at(&mut self, coords: impl Into<WorldCoords>) -> Option<Belt> {
        let coords = coords.into();
        self.world_mut()
            .query::<(&Belt, &WorldCoords)>()
            .iter(self.world_mut())
            .find(|(_, coords2)| &&coords == coords2)
            .map(|(belt, _)| belt.clone())
    }
    fn add_item(&mut self, belt: Entity, index: u16) -> Entity {
        let entity = self.world_mut().spawn_empty().id();
        self.world_mut().trigger(PlaceItem {
            entity,
            belt,
            pos: index,
            item: Item,
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

    fn remove_belt_at(&mut self, coords: impl Into<WorldCoords>) -> bool {
        let coords = coords.into();
        let Some((entity, _)) = self.world_mut().resource::<BeltCoords>().get(coords) else {
            return false;
        };
        self.world_mut().trigger(RemoveBelt { entity });
        true
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne, assert_str_eq};

    #[test]
    fn place_single_belt_east() {
        let mut app = test_app();
        let entity = app.add_belt((0, 0), Dir::East);

        app.update();
        let actual = app.find_belt(entity).unwrap();
        let expected = (Belt::Straight(Dir::East), (0, 0).into());
        assert_eq!(actual, expected);
    }

    #[test]
    fn place_single_belt_north() {
        let mut app = test_app();
        let entity = app.add_belt((0, 0), Dir::North);

        app.update();
        let actual = app.find_belt(entity).unwrap();
        let expected = (Belt::Straight(Dir::North), (0, 0).into());
        assert_eq!(actual, expected);
    }

    #[test]
    fn place_single_belt_diff_loc() {
        let mut app = test_app();
        let entity = app.add_belt((1, 2), Dir::West);

        app.update();
        let actual = app.find_belt(entity).unwrap();
        let expected = (Belt::Straight(Dir::West), (1, 2).into());
        assert_eq!(actual, expected);
    }

    #[test]
    fn place_belt_ahead_curves_it() {
        let mut app = test_app();
        app.add_belt((0, 0), Dir::East);
        app.add_belt((1, 0), Dir::North);

        app.update();
        let actual = app.find_belt_at((1, 0)).unwrap();
        let expected = Belt::CurvedEastToNorth;
        assert_eq!(actual, expected);
    }

    #[test]
    fn place_belt_ahead_curves_it_right() {
        let mut app = test_app();
        app.add_belt((0, 0), Dir::East);
        app.add_belt((1, 0), Dir::South);

        app.update();
        let actual = app.find_belt_at((1, 0)).unwrap();
        let expected = Belt::CurvedEastToSouth;
        assert_eq!(actual, expected);
    }

    #[test]
    fn place_belt_two_inputs_straight() {
        let mut app = test_app();
        app.add_belt((1, 0), Dir::West);
        app.add_belt((-1, 0), Dir::East);
        app.add_belt((0, 0), Dir::North);

        app.update();
        let actual = app.find_belt_at((0, 0)).unwrap();
        let expected = Belt::Straight(Dir::North);
        assert_eq!(actual, expected);
    }

    #[test]
    fn place_belt_beside_curves_it() {
        let mut app = test_app();
        app.add_belt((1, 0), Dir::North);
        app.add_belt((0, 0), Dir::East);

        app.update();
        let actual = app.find_belt_at((1, 0)).unwrap();
        let expected = Belt::CurvedEastToNorth;
        assert_eq!(actual, expected);
    }
    #[test]
    fn placing_belt_on_anther_belt_removes_it() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.add_belt((0, 0), Dir::North);

        app.update();
        let actual = app.find_belt_at((0, 0)).unwrap();
        assert_eq!(actual, Belt::Straight(Dir::North));
        assert!(app.find_belt(belt1).is_none());
    }

    #[test]
    fn items_placed_on_belt_east() {
        let mut app = test_app();
        let belt = app.add_belt((0, 0), Dir::East);
        app.update();

        let item = app.add_item(belt, 0);
        app.update();

        assert_eq!(
            app.find_item(item),
            Some((Item, Transform::from_xyz(TILE_SIZE / 2.0, 0.0, 2.0)))
        );
    }

    #[test]
    fn items_placed_on_belt_north() {
        let mut app = test_app();
        let belt = app.add_belt((0, 0), Dir::North);
        app.update();

        let item = app.add_item(belt, 0);
        app.update();

        assert_eq!(
            app.find_item(item),
            Some((Item, Transform::from_xyz(0.0, TILE_SIZE / 2.0, 2.0)))
        );
    }

    #[test]
    fn items_placed_on_belt_diff_coords() {
        let mut app = test_app();
        let belt = app.add_belt((1, 2), Dir::South);
        app.update();

        let item = app.add_item(belt, 0);
        app.update();

        assert_eq!(
            app.find_item(item),
            Some((Item, Transform::from_xyz(TILE_SIZE, TILE_SIZE * 1.5, 2.0)))
        );
    }

    #[test]
    fn items_placed_on_belt_halfway() {
        let mut app = test_app();
        let belt = app.add_belt((0, 0), Dir::North);
        app.update();

        let item = app.add_item(belt, POSITIONS_PER_TILE / 2);
        app.update();

        assert_eq!(
            app.find_item(item),
            Some((Item, Transform::from_xyz(0.0, 0.0, 2.0)))
        );
    }

    #[test]
    fn items_placed_on_curved_belt() {
        let mut app = test_app();
        app.add_belt((-1, 0), Dir::East);
        let belt = app.add_belt((0, 0), Dir::North);
        app.update();

        let item = app.add_item(belt, 0);
        app.update();

        assert_eq!(
            app.find_item(item),
            Some((Item, Transform::from_xyz(0.0, TILE_SIZE / 2.0, 2.0)))
        );
    }

    #[test]
    fn items_placed_on_curved_belt_halfway() {
        let mut app = test_app();
        app.add_belt((-1, 0), Dir::East);
        let belt = app.add_belt((0, 0), Dir::North);
        app.update();

        let item = app.add_item(belt, POSITIONS_PER_CURVED_TILE / 2);
        app.update();
        let actual = app.find_item(item).unwrap().1;
        let expected = Transform::from_xyz(
            -TILE_SIZE / 2.0 + (PI / 4.0).cos() * TILE_SIZE / 2.0,
            TILE_SIZE / 2.0 - (PI / 4.0).cos() * TILE_SIZE / 2.0,
            2.0,
        );
        let dist = actual.translation.distance(expected.translation);
        let margin = TILE_SIZE * 0.1 / 32.0; // Scale tolerance with tile size
        assert!(
            dist < margin,
            "Distance between\n\t{:?}\nand\n\t{:?}\nwas not within margin",
            actual.translation,
            expected.translation
        );
    }

    #[test]
    fn items_placed_on_curved_belt_north_to_west() {
        let mut app = test_app();
        app.add_belt((0, -1), Dir::North);
        let belt = app.add_belt((0, 0), Dir::West);
        app.update();

        let item = app.add_item(belt, 0);
        app.update();
        let actual = app.find_item(item).unwrap().1;
        let expected = Transform::from_xyz(-TILE_SIZE / 2.0, 0.0, 2.0);
        let dist = actual.translation.distance(expected.translation);
        let margin = TILE_SIZE * 0.1 / 32.0; // Scale tolerance with tile size
        assert!(
            dist < margin,
            "Distance between\n\t{:?}\nand\n\t{:?}\nwas not within margin",
            actual.translation,
            expected.translation
        );
    }

    #[test]
    fn items_placed_on_curved_belt_north_to_west_halfway() {
        let mut app = test_app();
        app.add_belt((0, -1), Dir::North);
        let belt = app.add_belt((0, 0), Dir::West);
        app.update();

        let item = app.add_item(belt, POSITIONS_PER_CURVED_TILE / 2);
        app.update();
        let actual = app.find_item(item).unwrap().1;
        let expected = Transform::from_xyz(
            -TILE_SIZE / 2.0 + (PI / 4.0).cos() * TILE_SIZE / 2.0,
            -TILE_SIZE / 2.0 + (PI / 4.0).cos() * TILE_SIZE / 2.0,
            2.0,
        );
        let dist = actual.translation.distance(expected.translation);
        let margin = TILE_SIZE * 0.1 / 32.0; // Scale tolerance with tile size
        assert!(
            dist < margin,
            "Distance between\n\t{:?}\nand\n\t{:?}\nwas not within margin",
            actual.translation,
            expected.translation
        );
    }

    #[test]
    fn items_placed_on_curved_belt_north_to_east_halfway() {
        let mut app = test_app();
        app.add_belt((0, -1), Dir::North);
        let belt = app.add_belt((0, 0), Dir::East);
        app.update();

        let item = app.add_item(belt, POSITIONS_PER_CURVED_TILE / 2);
        app.update();
        let actual = app.find_item(item).unwrap().1;
        let expected = Transform::from_xyz(
            TILE_SIZE / 2.0 - (PI / 4.0).cos() * TILE_SIZE / 2.0,
            -TILE_SIZE / 2.0 + (PI / 4.0).cos() * TILE_SIZE / 2.0,
            2.0,
        );
        let dist = actual.translation.distance(expected.translation);
        let margin = TILE_SIZE * 0.1 / 32.0; // Scale tolerance with tile size
        assert!(
            dist < margin,
            "Distance between\n\t{:?}\nand\n\t{:?}\nwas not within margin",
            actual.translation,
            expected.translation
        );
    }

    #[test]
    fn remove_empty_belt() {
        let mut app = test_app();
        let entity = app.add_belt((0, 0), Dir::East);
        app.update();

        // Verify belt exists
        assert!(app.find_belt(entity).is_some());
        assert!(app.find_belt_at((0, 0)).is_some());

        // Remove belt
        assert!(app.remove_belt_at((0, 0)));
        app.update();

        // Verify belt is gone
        assert!(app.find_belt(entity).is_none());
        assert!(app.find_belt_at((0, 0)).is_none());
    }

    #[test]
    fn remove_belt_with_items() {
        let mut app = test_app();
        let belt = app.add_belt((1, 1), Dir::North);
        app.update();

        let item1 = app.add_item(belt, 0);
        let item2 = app.add_item(belt, ITEM_SPACING);
        app.update();

        // Verify items exist
        assert!(app.find_item(item1).is_some());
        assert!(app.find_item(item2).is_some());

        // Remove belt
        assert!(app.remove_belt_at((1, 1)));
        app.update();

        // Verify belt is gone (items don't need to be checked per requirements)
        assert!(app.find_belt(belt).is_none());
        assert!(app.find_belt_at((1, 1)).is_none());
    }

    #[test]
    fn remove_nonexistent_belt() {
        let mut app = test_app();

        // Try to remove belt that doesn't exist
        assert!(!app.remove_belt_at((5, 5)));
        app.update();

        // Should not crash or cause issues
    }

    #[test]
    fn remove_belt_updates_belt_coords() {
        let mut app = test_app();
        app.add_belt((2, 3), Dir::West);
        app.update();

        // Verify in BeltCoords
        assert!(app.find_belt_at((2, 3)).is_some());

        // Remove belt
        app.remove_belt_at((2, 3));
        app.update();

        // Verify removed from BeltCoords
        assert!(app.find_belt_at((2, 3)).is_none());

        // Can place new belt at same location
        let new_belt = app.add_belt((2, 3), Dir::East);
        app.update();
        assert_eq!(
            app.find_belt(new_belt),
            Some((Belt::Straight(Dir::East), (2, 3).into()))
        );
    }

    #[test]
    fn remove_belt_from_chain() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        let belt2 = app.add_belt((1, 0), Dir::East);
        let belt3 = app.add_belt((2, 0), Dir::East);
        app.update();

        // Remove middle belt
        app.remove_belt_at((1, 0));
        app.update();

        // Belt2 should be gone
        assert!(app.find_belt(belt2).is_none());

        // Belt1 and belt3 should still exist
        assert!(app.find_belt(belt1).is_some());
        assert!(app.find_belt(belt3).is_some());
    }

    mod belt_changes {
        use super::*;
        #[allow(unused_imports)]
        use pretty_assertions::{assert_eq, assert_ne, assert_str_eq};

        #[test]
        fn basic() {
            let mut k = BeltChanges::default();
            let entity = Entity::from_raw_u32(12).unwrap();
            k.push(NewBelt {
                entity,
                belt: Belt::Straight(Dir::East),
                coords: (0, 0).into(),
            });

            let expected = BeltChanges(vec![BeltChange::New(NewBelt {
                entity,
                belt: Belt::Straight(Dir::East),
                coords: (0, 0).into(),
            })]);
            assert_eq!(k, expected);
        }

        #[test]
        fn replace() {
            let mut k = BeltChanges::default();
            let entity = Entity::from_raw_u32(12).unwrap();
            k.push(NewBelt {
                entity,
                belt: Belt::Straight(Dir::East),
                coords: (0, 0).into(),
            });
            k.push(ReplacedBelt {
                entity,
                old_entity: None,
                old_belt: Belt::Straight(Dir::East),
                new_belt: Belt::CurvedNorthToEast,
                coords: (0, 0).into(),
            });

            let expected = BeltChanges(vec![BeltChange::New(NewBelt {
                entity,
                belt: Belt::CurvedNorthToEast,
                coords: (0, 0).into(),
            })]);
            assert_eq!(k, expected);
        }

        #[test]
        fn positioning() {
            let mut k = BeltChanges::default();
            let entity = Entity::from_raw_u32(12).unwrap();
            let entity2 = Entity::from_raw_u32(13).unwrap();
            k.push(BeltChange::New(NewBelt {
                entity,
                belt: Belt::Straight(Dir::East),
                coords: (0, 0).into(),
            }));

            k.push(BeltChange::New(NewBelt {
                entity: entity2,
                belt: Belt::Straight(Dir::East),
                coords: (1, 0).into(),
            }));
            k.push(BeltChange::Replaced(ReplacedBelt {
                entity,
                old_entity: None,
                old_belt: Belt::Straight(Dir::East),
                new_belt: Belt::CurvedNorthToEast,
                coords: (0, 0).into(),
            }));

            let expected = BeltChanges(vec![
                BeltChange::New(NewBelt {
                    entity: entity2,
                    belt: Belt::Straight(Dir::East),
                    coords: (1, 0).into(),
                }),
                BeltChange::New(NewBelt {
                    entity,
                    belt: Belt::CurvedNorthToEast,
                    coords: (0, 0).into(),
                }),
            ]);
            assert_eq!(k, expected);
        }
    }
}
