use std::f32::consts::PI;

use bevy::prelude::*;
pub mod ui;

const CONVEYOR_BASE_SPEED: f32 = 16.0;
const TILE_SIZE: f32 = 32.0;
const CONVEYOR_CLOCK_SIZE: u8 = 60;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSystemSet {
    MessageWrite,
    MessageRead,
    Simulation,
}

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConveyorClock>();
        app.configure_sets(
            Update,
            (
                GameSystemSet::Simulation,
                GameSystemSet::MessageWrite,
                GameSystemSet::MessageRead,
            )
                .chain(),
        );
        app.add_message::<CreateTile>();
        app.add_message::<CreateWorldItem>();
        app.add_message::<CreateConveyor>();
        app.add_systems(
            Update,
            (create_tile, create_item, create_conveyor).in_set(GameSystemSet::MessageRead),
        );
    }
}

#[derive(Debug, Resource, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConveyorClock(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldCoords {
    pub x: i32,
    pub y: i32,
}

impl From<WorldCoords> for Transform {
    fn from(value: WorldCoords) -> Self {
        Transform::from_xyz(value.x as f32 * TILE_SIZE, value.y as f32 * TILE_SIZE, 0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HalfWorldCoords {
    pub coords: WorldCoords,
    pub conrner: Corner,
}

impl From<HalfWorldCoords> for Transform {
    fn from(value: HalfWorldCoords) -> Self {
        let offset = value.conrner.offset();
        let translation = Vec3 {
            x: offset.x,
            y: offset.y,
            z: 0.0,
        };

        let mut t = Transform::from(value.coords);
        t.translation += translation;
        t
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    NE,
    NW,
    SE,
    SW,
}

impl Corner {
    pub fn offset(&self) -> Vec2 {
        let value = match self {
            Corner::NE => Vec2 { x: 0.25, y: 0.25 },
            Corner::NW => Vec2 { x: -0.25, y: 0.25 },
            Corner::SE => Vec2 { x: 0.25, y: -0.25 },
            Corner::SW => Vec2 { x: -0.25, y: -0.25 },
        };
        value * TILE_SIZE
    }
}

#[derive(Message)]
pub struct CreateTile(pub Entity, pub WorldCoords);

#[derive(Message)]
pub struct CreateWorldItem(pub Entity, pub HalfWorldCoords);

#[derive(Message)]
pub struct CreateConveyor(pub Entity, pub WorldCoords, pub Direction);

fn create_tile(mut msgs: MessageReader<CreateTile>, mut cmd: Commands) {
    for CreateTile(entity, vec) in msgs.read() {
        cmd.entity(*entity).insert(Transform::from(*vec));
    }
}

fn create_item(mut msgs: MessageReader<CreateWorldItem>, mut cmd: Commands) {
    for CreateWorldItem(entity, pos) in msgs.read() {
        let mut t = Transform::from(*pos);
        t.translation.z = 2.0;
        cmd.entity(*entity).insert((t, WorldItem(*pos)));
    }
}

fn create_conveyor(mut msgs: MessageReader<CreateConveyor>, mut cmd: Commands) {
    for CreateConveyor(entity, vec, dir) in msgs.read() {
        let rot = match dir {
            Direction::North => 0.25,
            Direction::East => 0.0,
            Direction::South => 0.75,
            Direction::West => 0.5,
        };
        let mut t = Transform::from(*vec);
        t.translation.z = 1.0;
        cmd.entity(*entity).insert((
            t.with_rotation(Quat::from_rotation_z(rot * 2.0 * PI)),
            Conveyor::new(*dir, *vec),
        ));
    }
}

pub struct SimPlugin;
impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, tick_conveyor_clock);
        app.add_systems(
            Update,
            (
                conveyor_moves_items.before(conveyor_moves_item_visuals),
                conveyor_moves_item_visuals,
            )
                .in_set(GameSystemSet::Simulation),
        );
    }
}

fn tick_conveyor_clock(mut clock: ResMut<ConveyorClock>) {
    clock.0 = (clock.0 + 1) % 60;
}

#[derive(Component, PartialEq, Eq, Debug)]
pub struct WorldItem(HalfWorldCoords);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

#[derive(Component)]
pub struct Conveyor {
    direction: Direction,
    pos: WorldCoords,
}

impl Conveyor {
    pub fn new(dir: Direction, pos: WorldCoords) -> Self {
        Self {
            direction: dir,
            pos,
        }
    }
}

fn conveyor_moves_items(
    conveyors: Query<&Conveyor>,
    items: Query<(&mut Transform, &mut WorldItem)>,
    clock: Res<ConveyorClock>,
) {
    let ConveyorClock(clock) = *clock;
    if clock != 0 {
        return;
    }
    for mut item in items {
        for conveyor in conveyors {
            if conveyor.pos == item.1.0.coords {
                match (conveyor.direction, item.1.0.conrner) {
                    (Direction::East, Corner::NE) => {
                        item.1.0.coords.x += 1;
                        item.1.0.conrner = Corner::NW;
                    }
                    (Direction::East, Corner::NW) => {
                        item.1.0.conrner = Corner::NE;
                    }
                    _ => todo!(),
                }
            }
        }
    }
}

fn conveyor_moves_item_visuals(
    conveyors: Query<&Conveyor>,
    items: Query<(&mut Transform, &WorldItem)>,
    clock: Res<ConveyorClock>,
) {
    for (mut item_trans, item) in items {
        for conveyor in conveyors {
            match conveyor.direction {
                Direction::North => {
                    let mut trans = Transform::from(item.0);
                    trans.translation.y += clock.0 as f32 / 120.0;
                    *item_trans = trans;
                }
                Direction::East => {
                    if item.0.coords == conveyor.pos {
                        let mut trans = Transform::from(item.0);
                        trans.translation.x += clock.0 as f32 / 120.0;
                        *item_trans = trans;
                    }
                }
                _ => todo!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use pretty_assertions::assert_eq;

    const FRAME_TIME: f32 = 1.0 / 60.0;
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            FRAME_TIME,
        )));
        app.add_plugins(CorePlugin);
        app.add_plugins(SimPlugin);
        app
    }

    #[test]
    fn item() {
        let mut app = test_app();

        let world = app.world_mut();
        let entity = world.spawn_empty().id();
        world.write_message(CreateWorldItem(
            entity,
            HalfWorldCoords {
                coords: WorldCoords { x: 0, y: 0 },
                conrner: Corner::NE,
            },
        ));

        app.update();

        let mut query = app.world_mut().query::<(&WorldItem, &Transform)>();
        let items = query.iter(app.world()).collect::<Vec<_>>();
        let expected_trans = Transform::from_xyz(8.0, 8.0, 2.0);
        assert_eq!(
            items,
            vec![(
                &WorldItem(HalfWorldCoords {
                    coords: WorldCoords { x: 0, y: 0 },
                    conrner: Corner::NE,
                },),
                &expected_trans
            )]
        );
    }

    #[test]
    fn conveyor_moves_item() {
        let mut app = test_app();

        let world = app.world_mut();
        let item_entity = world.spawn_empty().id();
        world.write_message(CreateWorldItem(
            item_entity,
            HalfWorldCoords {
                coords: WorldCoords { x: 0, y: 0 },
                conrner: Corner::NE,
            },
        ));
        let conveyor_entity = world.spawn_empty().id();
        world.write_message(CreateConveyor(
            conveyor_entity,
            WorldCoords { x: 0, y: 0 },
            Direction::East,
        ));

        app.update();
        app.insert_resource(ConveyorClock::default());
        app.update();

        let mut query = app.world_mut().query::<(&WorldItem, &Transform)>();
        let items = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(1, items.len());
        let actual_x = items[0].1.translation.x;
        assert!(actual_x > 0.0, "expected {actual_x} to be bigger than 0.0");
    }

    #[test]
    fn conveyor_moves_item_north() {
        let mut app = test_app();

        let world = app.world_mut();
        let item_entity = world.spawn_empty().id();
        world.write_message(CreateWorldItem(
            item_entity,
            HalfWorldCoords {
                coords: WorldCoords { x: 0, y: 0 },
                conrner: Corner::NE,
            },
        ));
        let conveyor_entity = world.spawn_empty().id();
        world.write_message(CreateConveyor(
            conveyor_entity,
            WorldCoords { x: 0, y: 0 },
            Direction::North,
        ));

        app.update();
        app.update();

        let mut query = app.world_mut().query::<(&WorldItem, &Transform)>();
        let items = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(1, items.len());
        let actual_y = items[0].1.translation.y;
        assert!(actual_y > 0.0, "expected {actual_y} to be bigger than 0.0");
    }

    #[test]
    fn conveyor_doesnt_moves_item() {
        let mut app = test_app();

        let world = app.world_mut();
        let item_entity = world.spawn_empty().id();
        world.write_message(CreateWorldItem(
            item_entity,
            HalfWorldCoords {
                coords: WorldCoords { x: 1, y: 0 },
                conrner: Corner::NE,
            },
        ));
        let conveyor_entity = world.spawn_empty().id();
        world.write_message(CreateConveyor(
            conveyor_entity,
            WorldCoords { x: 0, y: 0 },
            Direction::North,
        ));
        app.update();

        let mut query = app.world_mut().query::<(&WorldItem, &Transform)>();
        let items = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(1, items.len());
        let actual_y = items[0].1.translation.y;
        assert_eq!(actual_y, TILE_SIZE / 4.0);
    }

    #[test]
    fn conveyor_speed() {
        let mut app = test_app();

        let world = app.world_mut();
        let item_entity = world.spawn_empty().id();
        world.write_message(CreateWorldItem(
            item_entity,
            HalfWorldCoords {
                coords: WorldCoords { x: 0, y: 0 },
                conrner: Corner::NE,
            },
        ));
        let conveyor_entity = world.spawn_empty().id();
        world.write_message(CreateConveyor(
            conveyor_entity,
            WorldCoords { x: 0, y: 0 },
            Direction::East,
        ));
        app.update();
        app.insert_resource(ConveyorClock::default());
        app.update();

        let mut query = app.world_mut().query::<(&WorldItem, &Transform)>();
        let items = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(1, items.len());
        let actual_x = items[0].1.translation.x;
        assert_eq!(
            actual_x,
            TILE_SIZE / 4.0 + 1.0 / CONVEYOR_CLOCK_SIZE as f32 / 2.0
        );
    }

    #[test]
    fn conveyor_moves_item_to_end() {
        let mut app = test_app();

        let world = app.world_mut();
        let item_entity = world.spawn_empty().id();
        world.write_message(CreateWorldItem(
            item_entity,
            HalfWorldCoords {
                coords: WorldCoords { x: 0, y: 0 },
                conrner: Corner::NE,
            },
        ));
        let conveyor_entity = world.spawn_empty().id();
        world.write_message(CreateConveyor(
            conveyor_entity,
            WorldCoords { x: 0, y: 0 },
            Direction::East,
        ));

        app.update();
        app.insert_resource(ConveyorClock(59));
        app.update();

        let mut query = app.world_mut().query::<(&WorldItem, &Transform)>();
        let items = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(1, items.len());
        let actual_x = items[0].1.translation.x;
        assert_eq!(actual_x, TILE_SIZE / 2.0 + TILE_SIZE / 4.0);
    }

    #[test]
    fn conveyor_doesnt_moves_item_just_before_start() {
        let mut app = test_app();

        let world = app.world_mut();
        let item_entity = world.spawn_empty().id();
        world.write_message(CreateWorldItem(
            item_entity,
            HalfWorldCoords {
                coords: WorldCoords { x: -1, y: 0 },
                conrner: Corner::NE,
            },
        ));
        let conveyor_entity = world.spawn_empty().id();
        world.write_message(CreateConveyor(
            conveyor_entity,
            WorldCoords { x: 0, y: 0 },
            Direction::East,
        ));

        app.update();
        app.insert_resource(ConveyorClock::default());
        app.update();

        let mut query = app.world_mut().query::<(&WorldItem, &Transform)>();
        let items = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(1, items.len());
        let actual_x = items[0].1.translation.x;
        assert_eq!(actual_x, -TILE_SIZE / 2.0 - TILE_SIZE / 4.0);
    }

    #[test]
    fn conveyor_doesnt_moves_item_to_the_side() {
        let mut app = test_app();

        let world = app.world_mut();
        let item_entity = world.spawn_empty().id();
        world.write_message(CreateWorldItem(
            item_entity,
            HalfWorldCoords {
                coords: WorldCoords { x: 0, y: 1 },
                conrner: Corner::SE,
            },
        ));
        let conveyor_entity = world.spawn_empty().id();
        world.write_message(CreateConveyor(
            conveyor_entity,
            WorldCoords { x: 0, y: 0 },
            Direction::East,
        ));

        app.update();
        app.update();

        let mut query = app.world_mut().query::<(&WorldItem, &Transform)>();
        let items = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(1, items.len());
        let actual_x = items[0].1.translation.x;
        assert_eq!(actual_x, TILE_SIZE / 4.0);
    }

    #[test]
    #[ignore = "passing other tests first"]
    fn conveyor_doesnt_moves_item_when_end_is_blocked() {
        let mut app = test_app();

        let world = app.world_mut();
        let blocking_item_entity = world.spawn_empty().id();
        world.write_message(CreateWorldItem(
            blocking_item_entity,
            HalfWorldCoords {
                coords: WorldCoords { x: 1, y: 0 },
                conrner: Corner::NW,
            },
        ));
        let blocked_item_entity = world.spawn_empty().id();
        world.write_message(CreateWorldItem(
            blocked_item_entity,
            HalfWorldCoords {
                coords: WorldCoords { x: 0, y: 0 },
                conrner: Corner::NE,
            },
        ));
        let conveyor_entity = world.spawn_empty().id();
        world.write_message(CreateConveyor(
            conveyor_entity,
            WorldCoords { x: 0, y: 0 },
            Direction::East,
        ));

        app.update();
        app.insert_resource(ConveyorClock::default());
        app.update();

        let mut query = app.world_mut().query::<(&WorldItem, &Transform)>();
        let items = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(2, items.len());
        let mut xs = items.iter().map(|i| i.1.translation.x).collect::<Vec<_>>();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let actual_x = xs[0];
        assert_eq!(actual_x, TILE_SIZE / 4.0);
    }
}
