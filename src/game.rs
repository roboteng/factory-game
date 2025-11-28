use std::f32::consts::PI;

use bevy::prelude::*;
pub mod ui;

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<CreateTile>();
        app.add_message::<CreateWorldItem>();
        app.add_message::<CreateConveyor>();
        app.add_systems(Update, (create_tile, create_item, create_conveyor));
    }
}

pub type WorldCoords = Vec2;

#[derive(Message)]
pub struct CreateTile(pub Entity, pub WorldCoords);

#[derive(Message)]
pub struct CreateWorldItem(pub Entity, pub WorldCoords);

#[derive(Message)]
pub struct CreateConveyor(pub Entity, pub WorldCoords, pub Direction);

fn create_tile(mut msgs: MessageReader<CreateTile>, mut cmd: Commands) {
    for CreateTile(entity, vec) in msgs.read() {
        cmd.entity(*entity)
            .insert(Transform::from_xyz(vec.x * 32.0, vec.y * 32.0, 0.0));
    }
}

fn create_item(mut msgs: MessageReader<CreateWorldItem>, mut cmd: Commands) {
    for CreateWorldItem(entity, vec) in msgs.read() {
        cmd.entity(*entity).insert((
            Transform::from_xyz(vec.x * 32.0, vec.y * 32.0, 2.0),
            WorldItem,
        ));
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
        cmd.entity(*entity).insert((
            Transform::from_xyz(vec.x * 32.0, vec.y * 32.0, 1.0)
                .with_rotation(Quat::from_rotation_z(rot * 2.0 * PI)),
            Conveyor::new(*dir),
        ));
    }
}

pub struct SimPlugin;
impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, conveyor_moves_items);
    }
}

#[derive(Component, PartialEq, Eq, Debug)]
pub struct WorldItem;

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
}

impl Conveyor {
    pub fn new(dir: Direction) -> Self {
        Self { direction: dir }
    }
}

fn conveyor_moves_items(
    conveyors: Query<(&Conveyor, &Transform)>,
    items: Query<&mut Transform, (With<WorldItem>, Without<Conveyor>)>,
) {
    for mut item in items {
        for (conveyor, con_trans) in conveyors {
            let rel_x = item.translation.x - con_trans.translation.x;
            if rel_x.abs() < 16.0 {
                match conveyor.direction {
                    Direction::North => {
                        item.translation.y += 1.0;
                    }
                    Direction::East => {
                        item.translation.x += 1.0;
                    }
                    _ => todo!(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(CorePlugin);
        app
    }

    #[test]
    fn item() {
        let mut app = test_app();
        app.add_plugins(SimPlugin);

        let world = app.world_mut();
        let entity = world.spawn_empty().id();
        world.write_message(CreateWorldItem(entity, Vec2 { x: 0.0, y: 0.0 }));

        app.update();

        let mut query = app.world_mut().query::<(&WorldItem, &Transform)>();
        let items = query.iter(app.world()).collect::<Vec<_>>();
        let mut expected_trans = Transform::default();
        expected_trans.translation.z = 2.0;
        assert_eq!(items, vec![(&WorldItem, &expected_trans)]);
    }

    #[test]
    fn conveyor_moves_item() {
        let mut app = test_app();

        let world = app.world_mut();
        let item_entity = world.spawn_empty().id();
        world.write_message(CreateWorldItem(item_entity, Vec2 { x: 0.0, y: 0.0 }));
        let conveyor_entity = world.spawn_empty().id();
        world.write_message(CreateConveyor(
            conveyor_entity,
            Vec2 { x: 0.0, y: 0.0 },
            Direction::East,
        ));
        app.update();

        app.add_plugins(SimPlugin);
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
        world.write_message(CreateWorldItem(item_entity, Vec2 { x: 0.0, y: 0.0 }));
        let conveyor_entity = world.spawn_empty().id();
        world.write_message(CreateConveyor(
            conveyor_entity,
            Vec2 { x: 0.0, y: 0.0 },
            Direction::North,
        ));
        app.update();

        app.add_plugins(SimPlugin);
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
        world.write_message(CreateWorldItem(item_entity, WorldCoords { x: 1.0, y: 0.0 }));
        let conveyor_entity = world.spawn_empty().id();
        world.write_message(CreateConveyor(
            conveyor_entity,
            Vec2 { x: 0.0, y: 0.0 },
            Direction::North,
        ));
        app.update();

        app.add_plugins(SimPlugin);
        app.update();

        let mut query = app.world_mut().query::<(&WorldItem, &Transform)>();
        let items = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(1, items.len());
        let actual_y = items[0].1.translation.y;
        assert_eq!(actual_y, 0.0);
    }
}
