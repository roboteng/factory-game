use std::{collections::HashMap, f32::consts::PI};

use bevy::prelude::*;

pub const TILE_SIZE: f32 = 32.0;
pub const POSITIONS_PER_TILE: u16 = 256;
pub const POSITIONS_PER_CURVED_TILE: u16 = 201; // POSITIONS_PER_TILE * Pi / 4

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_place_belt);
        app.add_observer(on_place_item);
        app.init_resource::<BeltCoords>();
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
pub struct BeltPlaced {
    entity: Entity,
    belt: Belt,
    coords: WorldCoords,
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
}

fn on_place_belt(trigger: On<PlaceBelt>, mut cmd: Commands, mut belt_coords: ResMut<BeltCoords>) {
    debug!(
        "Placing belt at {:?} facing {:?}",
        trigger.coords, trigger.dir
    );

    if let Some(prev) = belt_coords.get(trigger.coords) {
        cmd.entity(prev.0).despawn();
    }

    let belt = plan_belt_placement(&trigger, &belt_coords);
    cmd.entity(trigger.entity).insert((belt, trigger.coords));
    belt_coords.insert(trigger.coords, trigger.entity, belt);

    let ahead = trigger.coords.step(trigger.dir);
    belt_coords.get(ahead.clone()).map(|(entity, belt)| {
        let place = PlaceBelt {
            entity,
            dir: belt.output(),
            coords: ahead.clone(),
        };
        let belt = plan_belt_placement(&place, &belt_coords);
        cmd.entity(place.entity).insert((belt, place.coords));
        belt_coords.insert(place.coords, place.entity, belt);
    });
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
        let margin = 0.1;
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
        let margin = 0.1;
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
        let margin = 0.1;
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
        let margin = 0.1;
        assert!(
            dist < margin,
            "Distance between\n\t{:?}\nand\n\t{:?}\nwas not within margin",
            actual.translation,
            expected.translation
        );
    }
}
