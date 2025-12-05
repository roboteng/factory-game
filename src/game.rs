use std::collections::HashMap;
use std::f32::consts::PI;

use bevy::prelude::*;
pub mod sim;
pub mod ui;

pub const TILE_SIZE: f32 = 32.0;
pub const POSITIONS_PER_TILE: u16 = 256;

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BeltCoords>();

        app.add_observer(on_create_tile);
        app.add_observer(on_create_belt);
        app.add_observer(on_create_belt_item);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Component, Hash)]
pub struct WorldCoords {
    pub x: i32,
    pub y: i32,
}

impl WorldCoords {
    pub fn step(&self, dir: Direction) -> Self {
        match dir {
            Direction::North => Self {
                x: self.x,
                y: self.y + 1,
            },
            Direction::East => Self {
                x: self.x + 1,
                y: self.y,
            },
            Direction::South => Self {
                x: self.x,
                y: self.y - 1,
            },
            Direction::West => Self {
                x: self.x - 1,
                y: self.y,
            },
        }
    }
}

impl From<WorldCoords> for Vec3 {
    fn from(value: WorldCoords) -> Self {
        Vec3::new(value.x as f32 * TILE_SIZE, value.y as f32 * TILE_SIZE, 0.0)
    }
}

/// Give a `WorldCoords`, get its `Belt` and `Direction`
#[derive(Resource, Default)]
struct BeltCoords(HashMap<WorldCoords, (Entity, Direction)>);

#[derive(EntityEvent, Clone)]
pub struct CreateTile {
    pub entity: Entity,
    pub coords: WorldCoords,
}

#[derive(EntityEvent, Clone)]
pub struct CreateBelt {
    pub entity: Entity,
    pub coords: WorldCoords,
    pub dir: Direction,
}

impl CreateBelt {
    pub fn forward(&self) -> WorldCoords {
        let mut coords = self.coords;
        match self.dir {
            Direction::North => coords.y += 1,
            Direction::East => coords.x += 1,
            Direction::South => coords.y -= 1,
            Direction::West => coords.x -= 1,
        };
        coords
    }
    pub fn backward(&self) -> WorldCoords {
        let mut coords = self.coords;
        match self.dir {
            Direction::North => coords.y -= 1,
            Direction::East => coords.x -= 1,
            Direction::South => coords.y += 1,
            Direction::West => coords.x += 1,
        };
        coords
    }
}

#[derive(EntityEvent, Clone)]
pub struct CreateBeltItem {
    pub entity: Entity,
    pub belt: Entity,
    pub position: u16,
}

fn on_create_tile(trigger: On<CreateTile>, mut cmd: Commands) {
    cmd.entity(trigger.entity)
        .insert(Transform::from_translation(Vec3::from(trigger.coords)));
}

fn on_create_belt(trigger: On<CreateBelt>, mut cmd: Commands, mut belt_coords: ResMut<BeltCoords>) {
    let left = belt_coords
        .0
        .get(&trigger.coords.step(trigger.dir.left()))
        .map(|(_, dir)| *dir == trigger.dir.right())
        .unwrap_or(false);

    let right = belt_coords
        .0
        .get(&trigger.coords.step(trigger.dir.right()))
        .map(|(_, dir)| *dir == trigger.dir.left())
        .unwrap_or(false);
    let back = belt_coords
        .0
        .get(&trigger.coords.step(trigger.dir.opposite()))
        .map(|(_, dir)| *dir == trigger.dir)
        .unwrap_or(false);
    belt_coords
        .0
        .insert(trigger.coords, (trigger.entity, trigger.dir));
    match (left, back, right) {
        (false, _, false) | (true, _, true) | (_, true, _) => {
            let rot = match trigger.dir {
                Direction::North => 0.25,
                Direction::East => 0.0,
                Direction::South => 0.75,
                Direction::West => 0.5,
            };
            let mut t = Transform::from_translation(Vec3::from(trigger.coords));
            t.translation.z = 1.0;
            cmd.entity(trigger.entity).insert((
                t.with_rotation(Quat::from_rotation_z(rot * 2.0 * PI)),
                Belt::new(trigger.dir),
                trigger.coords,
            ));
        }
        (true, false, false) => {
            let rot = match trigger.dir {
                Direction::North => 0.25,
                Direction::East => 0.0,
                Direction::South => 0.75,
                Direction::West => 0.5,
            };
            let mut t = Transform::from_translation(Vec3::from(trigger.coords));
            t.translation.z = 1.0;
            cmd.entity(trigger.entity).insert((
                t.with_rotation(Quat::from_rotation_z(rot * 2.0 * PI)),
                Belt::curved(trigger.dir.right(), trigger.dir).unwrap(),
                trigger.coords,
            ));
        }
        (false, false, true) => {
            let rot = match trigger.dir {
                Direction::North => 0.25,
                Direction::East => 0.0,
                Direction::South => 0.75,
                Direction::West => 0.5,
            };
            let mut t = Transform::from_translation(Vec3::from(trigger.coords));
            t.translation.z = 1.0;
            cmd.entity(trigger.entity).insert((
                t.with_rotation(Quat::from_rotation_z(rot * 2.0 * PI)),
                Belt::curved(trigger.dir.left(), trigger.dir).unwrap(),
                trigger.coords,
            ));
        }
    }
}

#[derive(Component)]
pub struct BeltItem;

fn on_create_belt_item(trigger: On<CreateBeltItem>, mut cmd: Commands) {
    cmd.entity(trigger.entity)
        .insert((Transform::from_xyz(0.0, 0.0, 2.0), BeltItem));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

impl Direction {
    pub fn opposite(&self) -> Self {
        match self {
            Direction::North => Direction::South,
            Direction::East => Direction::West,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
        }
    }

    pub fn left(&self) -> Self {
        match self {
            Direction::North => Direction::West,
            Direction::East => Direction::North,
            Direction::South => Direction::East,
            Direction::West => Direction::South,
        }
    }

    pub fn right(&self) -> Self {
        match self {
            Direction::North => Direction::East,
            Direction::East => Direction::South,
            Direction::South => Direction::West,
            Direction::West => Direction::North,
        }
    }
}

impl From<Direction> for Vec2 {
    fn from(value: Direction) -> Self {
        match value {
            Direction::North => Vec2::Y,
            Direction::East => Vec2::X,
            Direction::South => Vec2::NEG_Y,
            Direction::West => Vec2::NEG_X,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Belt {
    input_direction: Direction,
    output_direction: Direction,
}

impl Belt {
    pub fn new(dir: Direction) -> Self {
        Self {
            input_direction: dir,
            output_direction: dir,
        }
    }
    /// Returns `None` if input and output are opposite
    pub fn curved(input: Direction, output: Direction) -> Option<Self> {
        if input.opposite() == output {
            return None;
        }
        Some(Self {
            input_direction: input,
            output_direction: output,
        })
    }

    pub fn item_position(&self, coords: WorldCoords, pos: u16) -> Vec3 {
        match self.shape() {
            BeltShape::Straight(dir) => {
                let start = Vec2::from(dir);
                let diff =
                    (start / 2.0 - start * pos as f32 / POSITIONS_PER_TILE as f32) * TILE_SIZE;
                let mut k = Vec3::from(coords);
                k.x += diff.x;
                k.y += diff.y;
                k.z = 2.0;
                k
            }
            BeltShape::Curve(direction, direction1) => todo!(),
        }
    }

    fn shape(&self) -> BeltShape {
        match (self.input_direction, self.output_direction) {
            (input, output) if input == output => BeltShape::Straight(input),
            (input, output) if input == output.right() => BeltShape::Curve(input, output),
            (input, output) if input == output.left() => BeltShape::Curve(input, output),
            _ => unreachable!("Belt directions should be set correctly in new"),
        }
    }
}

pub enum BeltShape {
    Straight(Direction),
    Curve(Direction, Direction),
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    pub fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(CorePlugin);
        app
    }

    pub struct TestBuilder {
        pub app: App,
        last_belt_created: Option<Entity>,
    }

    impl TestBuilder {
        pub fn new(app: App) -> Self {
            Self {
                app,
                last_belt_created: None,
            }
        }

        pub fn spawn_belt(&mut self, coords: WorldCoords, dir: Direction) -> Entity {
            let world = self.app.world_mut();
            let belt = world.spawn_empty().id();
            world.trigger(CreateBelt {
                entity: belt,
                coords,
                dir,
            });
            self.last_belt_created = Some(belt);
            belt
        }

        pub fn with_item_at(&mut self, position: u16) -> Entity {
            let world = self.app.world_mut();
            let item = world.spawn_empty().id();
            world.trigger(CreateBeltItem {
                entity: item,
                belt: self.last_belt_created.unwrap(),
                position,
            });
            item
        }

        pub fn get_transform(&mut self, entity: Entity) -> Transform {
            let world = self.app.world_mut();
            let mut q = world.query::<&Transform>();
            *q.get(world, entity).unwrap()
        }
    }

    #[test]
    fn belt() {
        let mut t = TestBuilder::new(test_app());
        let belt_entity = t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);

        t.app.update();
        let belt = t.get_transform(belt_entity);
        assert_eq!(belt.translation.x, 0.0);
        assert_eq!(belt.translation.y, 0.0);
    }

    #[test]
    fn item_on_belt() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);
        let item = t.with_item_at(0);

        t.app.update();
        let _ = t.get_transform(item);
    }

    #[test]
    fn placing_belt_in_front_curves_it() {
        let mut t = TestBuilder::new(test_app());
        let _ = t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);
        let new_belt_entity = t.spawn_belt(WorldCoords { x: 1, y: 0 }, Direction::North);

        t.app.update();
        let world = t.app.world_mut();
        let actual = world.get_mut::<Belt>(new_belt_entity).unwrap().clone();
        let expected = Belt {
            input_direction: Direction::East,
            output_direction: Direction::North,
        };
        assert_eq!(actual, expected);
    }
}
