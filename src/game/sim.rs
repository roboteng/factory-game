use bevy::prelude::*;

use crate::game::{
    Belt, BeltItem, CreateBeltItem, Direction, POSITIONS_PER_TILE, TILE_SIZE, WorldCoords,
};

pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, create_item);
        app.add_systems(
            Update,
            (plan_item_movement, move_items.after(plan_item_movement)),
        );
    }
}

#[derive(Component)]
pub struct ExpectedMovement(u16);

fn create_item(mut msgs: MessageReader<CreateBeltItem>, mut cmd: Commands) {
    for item in msgs.read() {
        cmd.entity(item.entity).insert(ExpectedMovement(0));
    }
}

fn plan_item_movement(
    belts: Query<&mut Belt>,
    mut items: Query<(&BeltItem, Mut<ExpectedMovement>)>,
) {
    for mut belt in belts {
        for (index, item) in belt.lane.iter_mut().enumerate() {
            if let Ok((_, mut expected_movement)) = items.get_mut(item.1) {
                let space = item.0 - (index as u16) * 64;
                expected_movement.0 = space.min(8);
            }
        }
    }
}

fn move_items(
    mut items: Query<(Mut<Transform>, &ExpectedMovement), Without<Belt>>,
    mut belts: Query<&mut Belt>,
) {
    for belt in belts.iter_mut() {
        let Belt {
            direction,
            pos: coords,
            lane,
        } = belt.into_inner();
        for (pos, entity) in lane.iter_mut() {
            if let Ok((mut transform, expected_movement)) = items.get_mut(*entity) {
                *pos -= expected_movement.0;
                transform.translation = item_position(*coords, *direction, *pos);
            } else {
                warn!("Couldn't find lane: {entity}");
            }
        }
    }
}

fn item_position(coords: WorldCoords, dir: Direction, pos: u16) -> Vec3 {
    let start = Vec2::from(dir);
    let diff = (start / 2.0 - start * pos as f32 / POSITIONS_PER_TILE as f32) * TILE_SIZE;
    let mut k = Vec3::from(coords);
    k.x += diff.x;
    k.y += diff.y;
    k.z = 2.0;
    k
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::game::*;

    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use pretty_assertions::assert_eq;

    struct TestBuilder {
        app: App,
        last_belt_created: Option<Entity>,
    }
    impl TestBuilder {
        fn new() -> Self {
            Self {
                app: test_app(),
                last_belt_created: None,
            }
        }
        fn spawn_belt(&mut self, coords: WorldCoords, dir: Direction) -> Entity {
            let world = self.app.world_mut();
            let belt = world.spawn_empty().id();
            world.write_message(CreateBelt {
                entity: belt,
                pos: coords,
                dir,
            });
            self.last_belt_created = Some(belt);
            belt
        }
        fn with_item_at(&mut self, position: u16) -> Entity {
            let world = self.app.world_mut();
            let item = world.spawn_empty().id();
            world.write_message(CreateBeltItem {
                entity: item,
                belt: self.last_belt_created.unwrap(),
                position,
            });
            item
        }
        fn get_transform(&mut self, entity: Entity) -> Transform {
            let world = self.app.world_mut();
            let mut q = world.query::<&Transform>();
            *q.get(world, entity).unwrap()
        }
    }

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
    fn item_on_belt() {
        let mut t = TestBuilder::new();
        t.spawn_belt(WorldCoords::default(), Direction::East);
        let item = t.with_item_at(0);
        t.app.update();
        let _ = t.get_transform(item);
    }

    #[test]
    fn item_positions() {
        for (input, expected) in [
            (
                (WorldCoords { x: 0, y: 0 }, Direction::East, 0),
                Vec3::new(16.0, 0.0, 2.0),
            ),
            (
                (WorldCoords { x: 0, y: 0 }, Direction::East, 128),
                Vec3::new(0.0, 0.0, 2.0),
            ),
            (
                (WorldCoords { x: 0, y: 0 }, Direction::East, 256),
                Vec3::new(-16.0, 0.0, 2.0),
            ),
        ] {
            let actual = item_position(input.0, input.1, input.2);
            assert_eq!(
                actual, expected,
                "when passing \n\t{input:?},\nexpected\n\t{expected},\nbut got\n\t{actual}"
            );
        }
    }

    #[test]
    fn item_moves_on_belt() {
        let mut t = TestBuilder::new();
        t.spawn_belt(WorldCoords::default(), Direction::East);
        let item = t.with_item_at(128);
        t.app.update();
        let initial_transform = t.get_transform(item);
        t.app.update();
        let next_transform = t.get_transform(item);

        assert!(
            next_transform.translation.x > initial_transform.translation.x,
            "Item should have moved along the belt (East direction means increasing X as it progresses). Initial: {}, Final: {}",
            initial_transform.translation.x,
            next_transform.translation.x
        );
    }

    #[test]
    fn item_doesnt_move_at_end_of_belt() {
        let mut t = TestBuilder::new();
        t.spawn_belt(WorldCoords::default(), Direction::East);
        let item = t.with_item_at(0);

        t.app.update();

        let initial_position = t.get_transform(item).translation;

        t.app.update();

        let final_position = t.get_transform(item).translation;

        assert_eq!(
            initial_position, final_position,
            "Item transform should not have changed after update"
        );
    }
}
