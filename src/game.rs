use std::collections::HashMap;
use std::f32::consts::PI;

use bevy::log::debug;
use bevy::log::tracing::instrument;
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
#[derive(Resource, Default, Debug)]
struct BeltCoords(HashMap<WorldCoords, (Entity, Belt)>);

impl BeltCoords {
    fn insert(&mut self, coords: WorldCoords, entity: Entity, belt: Belt) {
        self.0.insert(coords, (entity, belt));
    }

    fn get(&self, coords: &WorldCoords) -> Option<(Entity, Belt)> {
        self.0.get(coords).copied()
    }

    fn has_belt_with_output(&self, coords: &WorldCoords, direction: Direction) -> bool {
        self.0
            .get(coords)
            .map(|(_, belt)| belt.output_direction() == direction)
            .unwrap_or(false)
    }
}

#[derive(EntityEvent, Clone)]
pub struct CreateTile {
    pub entity: Entity,
    pub coords: WorldCoords,
}

#[derive(EntityEvent, Clone, Debug)]
pub struct CreateBelt {
    pub entity: Entity,
    pub coords: WorldCoords,
    pub dir: Direction,
}

#[derive(EntityEvent, Clone, Debug)]
pub struct BeltCreated {
    pub entity: Entity,
    pub coords: WorldCoords,
    pub new_belt: Belt,
    /// What the belt used to be
    pub old_belt: Option<Belt>,
    /// Belt behind at the time of creation
    pub old_belt_behind: Option<(Entity, Belt)>,
    /// Current belt behind
    pub new_belt_behind: Option<(Entity, Belt)>,
}

impl BeltCreated {
    pub fn coords_ahead(&self) -> WorldCoords {
        self.coords.step(self.new_belt.output_direction())
    }
    pub fn coords_behind(&self) -> WorldCoords {
        self.coords.step(self.new_belt.input_direction().opposite())
    }
}

pub enum Curvature {
    Straight,
    Clockwise,
    Counterclockwise,
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

#[instrument]
fn create_belt(trigger: CreateBelt, belt_coords: &BeltCoords) -> (Belt, BeltCreated) {
    let fed_from_left = belt_coords.has_belt_with_output(
        &trigger.coords.step(trigger.dir.left()),
        trigger.dir.right(),
    );

    let fed_from_right = belt_coords.has_belt_with_output(
        &trigger.coords.step(trigger.dir.right()),
        trigger.dir.left(),
    );

    let fed_from_behind =
        belt_coords.has_belt_with_output(&trigger.coords.step(trigger.dir.opposite()), trigger.dir);

    match (fed_from_left, fed_from_behind, fed_from_right) {
        (false, _, false) | (true, _, true) | (_, true, _) => {
            let belt = Belt::straight(trigger.dir);
            (
                belt,
                BeltCreated {
                    entity: trigger.entity,
                    coords: trigger.coords,
                    new_belt: belt,
                    old_belt: None,
                    new_belt_behind: belt_coords
                        .get(&trigger.coords.step(belt.input_direction().opposite()))
                        .filter(|b| b.1.output_direction() == belt.input_direction()),
                    old_belt_behind: None,
                },
            )
        }
        (true, false, false) => {
            let input = trigger.dir.right();
            let belt = Belt::curved(input, trigger.dir).unwrap();
            (
                belt,
                BeltCreated {
                    entity: trigger.entity,
                    coords: trigger.coords,
                    new_belt: belt,
                    old_belt: None,
                    new_belt_behind: belt_coords
                        .get(&trigger.coords.step(belt.input_direction().opposite()))
                        .filter(|b| b.1.output_direction() == belt.input_direction()),
                    old_belt_behind: None,
                },
            )
        }
        (false, false, true) => {
            let input = trigger.dir.left();
            let belt = Belt::curved(input, trigger.dir).unwrap();
            (
                belt,
                BeltCreated {
                    entity: trigger.entity,
                    coords: trigger.coords,
                    new_belt: belt,
                    old_belt: None,
                    new_belt_behind: belt_coords
                        .get(&trigger.coords.step(belt.input_direction().opposite()))
                        .filter(|b| b.1.output_direction() == belt.input_direction()),
                    old_belt_behind: None,
                },
            )
        }
    }
}

#[instrument(skip_all, fields(belt_coords = ?belt_coords.0, trigger = ?trigger))]
fn on_create_belt(
    trigger: On<CreateBelt>,
    mut cmd: Commands,
    mut belt_coords: ResMut<BeltCoords>,
    belts: Query<&Belt>,
) {
    let (belt, belt_created) = create_belt(trigger.clone(), &belt_coords);

    // Update belt_coords immediately
    belt_coords.insert(trigger.coords, trigger.entity, belt);

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
        belt,
        trigger.coords,
    ));
    cmd.trigger(belt_created);

    let ahead = belt_coords.get(&trigger.coords.step(trigger.dir));
    if let Some((e, belt)) = ahead {
        let k = CreateBelt {
            entity: e,
            coords: trigger.coords.step(trigger.dir),
            dir: belt.output_direction(),
        };
        let (new_belt, mut belt_created) = create_belt(k, &belt_coords);
        let previous_belt = belts.get(e).unwrap();
        if *previous_belt != new_belt {
            cmd.entity(e).insert(new_belt);
            belt_created.old_belt = Some(*previous_belt);

            // Capture belt ahead/behind state before updating belt_coords
            let behind_coords = belt_created
                .coords
                .step(previous_belt.input_direction().opposite());
            belt_created.old_belt_behind = belt_coords.get(&behind_coords);

            belt_coords.insert(belt_created.coords, belt_created.entity, new_belt);
            debug!(
                coords = ?belt_created.coords,
                entity = ?belt_created.entity,
                new_belt = ?new_belt,
                "Updated BeltCoords for belt ahead"
            );

            cmd.trigger(belt_created);
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
        self.left().opposite()
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
pub enum Belt {
    StraightNorth,
    StraightEast,
    StraightSouth,
    StraightWest,
    CurveNorthToEast,
    CurveNorthToWest,
    CurveSouthToEast,
    CurveSouthToWest,
    CurveEastToNorth,
    CurveEastToSouth,
    CurveWestToNorth,
    CurveWestToSouth,
}

impl Belt {
    pub fn straight(dir: Direction) -> Self {
        match dir {
            Direction::North => Self::StraightNorth,
            Direction::East => Self::StraightEast,
            Direction::South => Self::StraightSouth,
            Direction::West => Self::StraightWest,
        }
    }
    /// Returns `None` if input and output are opposite
    pub fn curved(input: Direction, output: Direction) -> Option<Self> {
        if input.opposite() == output {
            return None;
        }
        match (input, output) {
            (Direction::North, Direction::East) => Some(Self::CurveNorthToEast),
            (Direction::North, Direction::West) => Some(Self::CurveNorthToWest),
            (Direction::South, Direction::East) => Some(Self::CurveSouthToEast),
            (Direction::South, Direction::West) => Some(Self::CurveSouthToWest),
            (Direction::East, Direction::North) => Some(Self::CurveEastToNorth),
            (Direction::East, Direction::South) => Some(Self::CurveEastToSouth),
            (Direction::West, Direction::North) => Some(Self::CurveWestToNorth),
            (Direction::West, Direction::South) => Some(Self::CurveWestToSouth),
            _ => None,
        }
    }

    pub fn input_direction(&self) -> Direction {
        match self {
            Self::StraightNorth => Direction::North,
            Self::StraightSouth => Direction::South,
            Self::StraightEast => Direction::East,
            Self::StraightWest => Direction::West,
            Self::CurveNorthToEast => Direction::North,
            Self::CurveNorthToWest => Direction::North,
            Self::CurveSouthToEast => Direction::South,
            Self::CurveSouthToWest => Direction::South,
            Self::CurveEastToNorth => Direction::East,
            Self::CurveEastToSouth => Direction::East,
            Self::CurveWestToNorth => Direction::West,
            Self::CurveWestToSouth => Direction::West,
        }
    }

    pub fn output_direction(&self) -> Direction {
        match self {
            Self::StraightNorth => Direction::North,
            Self::StraightSouth => Direction::South,
            Self::StraightEast => Direction::East,
            Self::StraightWest => Direction::West,
            Self::CurveNorthToEast => Direction::East,
            Self::CurveNorthToWest => Direction::West,
            Self::CurveSouthToEast => Direction::East,
            Self::CurveSouthToWest => Direction::West,
            Self::CurveEastToNorth => Direction::North,
            Self::CurveEastToSouth => Direction::South,
            Self::CurveWestToNorth => Direction::North,
            Self::CurveWestToSouth => Direction::South,
        }
    }

    pub fn item_position(&self, coords: WorldCoords, pos: u16) -> Vec3 {
        match self {
            Self::StraightEast | Self::StraightWest | Self::StraightNorth | Self::StraightSouth => {
                let dir = self.output_direction();
                let start = Vec2::from(dir);
                let diff =
                    (start / 2.0 - start * pos as f32 / POSITIONS_PER_TILE as f32) * TILE_SIZE;
                let mut k = Vec3::from(coords);
                k.x += diff.x;
                k.y += diff.y;
                k.z = 2.0;
                k
            }
            _ => {
                let input = self.input_direction();
                let output = self.output_direction();
                const CURVED_BELT_POSITIONS: u16 = 201; // PI * (256/2) / 2
                let center_of_tile = Vec3::from(coords);
                let curve_center_offset = (Vec2::from(input.opposite()) + Vec2::from(output)) / 2.0;

                let base_angle = match input {
                    Direction::East => 0.0,
                    Direction::North => PI / 2.0,
                    Direction::West => PI,
                    Direction::South => 3.0 * PI / 2.0,
                };
                let angle_delta = (pos as f32 / CURVED_BELT_POSITIONS as f32) * PI / 2.0;
                let angle = if self.input_direction().right() == self.output_direction() {
                    base_angle + angle_delta // Clockwise
                } else {
                    base_angle - angle_delta // Counterclockwise
                };

                center_of_tile
                    + Vec3::new(
                        TILE_SIZE * (curve_center_offset.x + angle.cos() / 2.0),
                        TILE_SIZE * (curve_center_offset.y + angle.sin() / 2.0),
                        2.0,
                    )
            }
        }
    }

    pub fn shape(&self) -> BeltShape {
        match (self.input_direction(), self.output_direction()) {
            (input, output) if input == output => BeltShape::Straight(input),
            (input, output) if input == output.right() => BeltShape::Curve(input, output),
            (input, output) if input == output.left() => BeltShape::Curve(input, output),
            _ => unreachable!("Belt directions should be set correctly in new"),
        }
    }

    pub fn curvature(&self) -> Curvature {
        match (self.input_direction(), self.output_direction()) {
            (input, output) if input == output => Curvature::Straight,
            (input, output) if input == output.right() => Curvature::Clockwise,
            (input, output) if input == output.left() => Curvature::Counterclockwise,
            _ => unreachable!("Belt directions should be set correctly in new"),
        }
    }

    pub fn number_of_positions(&self) -> u16 {
        match self.shape() {
            BeltShape::Straight(_) => POSITIONS_PER_TILE,
            BeltShape::Curve(_, _) => 201,
        }
    }

    pub fn feeds(&self, other: &Self) -> bool {
        self.output_direction() == other.input_direction()
    }
}

pub enum BeltShape {
    #[expect(dead_code)]
    Straight(Direction),
    #[expect(dead_code)]
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
        let expected = Belt::CurveEastToNorth;
        assert_eq!(actual, expected);
    }

    #[test]
    fn placing_belt_behind_curves_it() {
        let mut t = TestBuilder::new(test_app());
        let curved_belt_entity = t.spawn_belt(WorldCoords { x: 1, y: 0 }, Direction::North);
        let _ = t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);

        t.app.update();
        let world = t.app.world_mut();
        let actual = world.get_mut::<Belt>(curved_belt_entity).unwrap().clone();
        let expected = Belt::CurveEastToNorth;
        assert_eq!(actual, expected);
    }
}
