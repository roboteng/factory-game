use std::f32::consts::PI;

use bevy::prelude::*;
pub mod sim;
pub mod ui;

pub const TILE_SIZE: f32 = 32.0;
pub const POSITIONS_PER_TILE: u16 = 256;
pub const POSITION_PER_ITEM: u16 = 64;

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<CreateConveyor>();
        app.add_message::<CreateConveyorItem>();
        app.add_message::<CreateTile>();
        app.add_systems(
            PreUpdate,
            (
                create_conveyor,
                create_tile,
                create_belt_item.after(create_conveyor),
            ),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldCoords {
    pub x: i32,
    pub y: i32,
}

impl From<WorldCoords> for Vec3 {
    fn from(value: WorldCoords) -> Self {
        Vec3::new(value.x as f32 * TILE_SIZE, value.y as f32 * TILE_SIZE, 0.0)
    }
}

#[derive(Message)]
pub struct CreateTile(pub Entity, pub WorldCoords);

#[derive(Message)]
pub struct CreateConveyor {
    pub entity: Entity,
    pub pos: WorldCoords,
    pub dir: Direction,
}

#[derive(Message)]
pub struct CreateConveyorItem {
    pub entity: Entity,
    pub conveyor: Entity,
    pub position: u16,
}

fn create_tile(mut msgs: MessageReader<CreateTile>, mut cmd: Commands) {
    for CreateTile(entity, vec) in msgs.read() {
        cmd.entity(*entity)
            .insert(Transform::from_translation(Vec3::from(*vec)));
    }
}

fn create_conveyor(mut msgs: MessageReader<CreateConveyor>, mut cmd: Commands) {
    for CreateConveyor { entity, pos, dir } in msgs.read() {
        let rot = match dir {
            Direction::North => 0.25,
            Direction::East => 0.0,
            Direction::South => 0.75,
            Direction::West => 0.5,
        };
        let mut t = Transform::from_translation(Vec3::from(*pos));
        t.translation.z = 1.0;
        cmd.entity(*entity).insert((
            t.with_rotation(Quat::from_rotation_z(rot * 2.0 * PI)),
            Conveyor::new(*dir, *pos),
        ));
    }
}

#[derive(Component)]
pub struct BeltItem;

// #[derive(Component)]
// pub struct BeltGroup;
// /// Mapping from Conveyor Entity to Which group it belongs to
// #[derive(Resource)]
// struct BeltGroupMap(HashMap<Entity, Entity>);

fn create_belt_item(
    mut msgs: MessageReader<CreateConveyorItem>,
    mut cmd: Commands,
    mut belts: Query<&mut Conveyor>,
) {
    for item in msgs.read() {
        if let Ok(mut belt) = belts.get_mut(item.conveyor) {
            belt.lane.push((item.position, item.entity));
            belt.lane.sort_by(|a, b| a.0.cmp(&b.0));
        } else {
            panic!("Couldn't find belt {}", item.conveyor);
        }
        cmd.entity(item.entity)
            .insert((Transform::from_xyz(0.0, 0.0, 2.0), BeltItem));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    North,
    East,
    South,
    West,
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

#[derive(Component)]
pub struct Conveyor {
    direction: Direction,
    pos: WorldCoords,
    lane: Vec<(u16, Entity)>,
}

impl Conveyor {
    pub fn new(dir: Direction, pos: WorldCoords) -> Self {
        Self {
            direction: dir,
            pos,
            lane: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const FRAME_TIME: f32 = 1.0 / 60.0;
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(CorePlugin);
        app
    }

    #[test]
    fn conveyor() {
        let mut app = test_app();

        let world = app.world_mut();
        let belt_entity = world.spawn_empty().id();
        world.write_message(CreateConveyor {
            entity: belt_entity,
            pos: WorldCoords { x: 0, y: 0 },
            dir: Direction::East,
        });

        app.update();
        let world = app.world_mut();
        let mut q = world.query::<&Transform>();
        let conveyor = q.get(world, belt_entity).unwrap();
        assert_eq!(conveyor.translation.x, 0.0);
        assert_eq!(conveyor.translation.y, 0.0);
    }

    #[test]
    fn item_on_conveyor() {
        let mut app = test_app();
        let world = app.world_mut();
        let conveyor = world.spawn_empty().id();
        world.write_message(CreateConveyor {
            entity: conveyor,
            pos: WorldCoords { x: 0, y: 0 },
            dir: Direction::East,
        });

        let item = world.spawn_empty().id();
        world.write_message(CreateConveyorItem {
            entity: item,
            conveyor,
            position: 0,
        });
        app.update();
        let world = app.world_mut();
        let mut q = world.query::<&Transform>();
        q.get(world, item).unwrap();
    }
}
