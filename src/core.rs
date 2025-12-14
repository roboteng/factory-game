use std::collections::HashMap;

use bevy::prelude::*;

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_place_belt);
        app.init_resource::<BeltCoords>();
    }
}

#[derive(EntityEvent)]
pub struct PlaceBelt {
    entity: Entity,
    dir: Dir,
    coords: WorldCoords,
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

#[derive(Component, Clone, Debug, PartialEq, Eq, Hash)]
pub struct WorldCoords {
    x: i32,
    y: i32,
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
}

impl From<(i32, i32)> for WorldCoords {
    fn from((x, y): (i32, i32)) -> Self {
        Self { x, y }
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
}

#[derive(Resource, Default)]
struct BeltCoords(HashMap<WorldCoords, (Entity, Belt)>);
impl BeltCoords {
    fn insert(&mut self, coords: WorldCoords, entity: Entity, belt: Belt) {
        self.0.insert(coords, (entity, belt));
    }
    fn get(&self, coords: WorldCoords) -> Option<(Entity, Belt)> {
        self.0.get(&coords).map(|(entity, belt)| (*entity, *belt))
    }
}

fn on_place_belt(trigger: On<PlaceBelt>, mut cmd: Commands, mut belt_coords: ResMut<BeltCoords>) {
    debug!(
        "Placing belt at {:?} facing {:?}",
        trigger.coords, trigger.dir
    );
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
    cmd.entity(trigger.entity)
        .insert((belt, trigger.coords.clone()));
    belt_coords.insert(trigger.coords.clone(), trigger.entity, belt);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne, assert_str_eq};
    use tracing_subscriber::{EnvFilter, fmt};

    fn init_tracing() {
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

    fn test_app() -> App {
        init_tracing();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(CorePlugin);
        app
    }

    trait AppExtension {
        fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: Dir) -> Entity;
        fn find_belt(&mut self, entity: Entity) -> Option<(Belt, WorldCoords)>;
        fn find_belt_at(&mut self, coords: impl Into<WorldCoords>) -> Option<Belt>;
    }

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
                .map(|(belt, coords)| (belt.clone(), coords.clone()))
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
    }

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
}
