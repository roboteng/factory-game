use std::{collections::HashMap, ops::Range};

use bevy::{ecs::system::SystemState, prelude::*};

use crate::game::{Belt, BeltItem, CreateBelt, CreateBeltItem, WorldCoords};

pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BeltGroups>();
        app.init_resource::<BeltCoords>();
        app.add_systems(PreUpdate, (create_belt, create_item).chain());
        app.add_systems(
            Update,
            (plan_item_movement, move_items.after(plan_item_movement)),
        );
    }
}

#[derive(Component)]
pub struct ExpectedMovement(u16);

#[derive(Debug, PartialEq)]
struct OrderedBelts {
    belts: Vec<(Range<u16>, Entity)>,
}

impl OrderedBelts {
    fn belt_at_pos(&self, pos: u16) -> Option<&(Range<u16>, Entity)> {
        self.belts.iter().find(|&belt| belt.0.contains(&pos))
    }
}

#[derive(Default, Debug, PartialEq)]
struct Lane {
    lane: Vec<(u16, Entity)>,
}

impl Lane {
    fn add_item_at(&mut self, pos: u16, item: Entity) {
        // Binary search for the correct insertion position
        // Vec is sorted in descending order by position
        let insert_idx = self
            .lane
            .binary_search_by(|probe| probe.0.cmp(&pos).reverse())
            .unwrap_or_else(|idx| idx);
        self.lane.insert(insert_idx, (pos, item));
    }
}

#[derive(Component, Debug, PartialEq)]
struct BeltGroup {
    belts: OrderedBelts,
    lane: Lane,
}

impl BeltGroup {
    fn from_belt(belt: Entity) -> Self {
        Self {
            belts: OrderedBelts {
                belts: vec![(0..256, belt)],
            },
            lane: Default::default(),
        }
    }
    fn add_belt_at_tail(&mut self, belt: Entity, n_pos: u16) {
        if self.belts.belts.iter().any(|b| b.1 == belt) {
            panic!("adding a belt twice");
        }
        let start = self.belts.belts.last().map(|b| b.0.end).unwrap_or(0);
        let end = start + n_pos;
        self.belts.belts.push((start..end, belt));
    }
    fn add_belt_at_head(&mut self, belt: Entity, n_pos: u16) {
        if self.belts.belts.iter().any(|b| b.1 == belt) {
            panic!("adding a belt twice");
        }
        for slot in self.belts.belts.iter_mut() {
            slot.0.end += n_pos;
            slot.0.start += n_pos;
        }
        for item in self.lane.lane.iter_mut() {
            item.0 += n_pos;
        }
        self.belts.belts.insert(0, (0..n_pos, belt));
    }
    fn add_item_at(&mut self, item: &CreateBeltItem) {
        let slot = self
            .belts
            .belts
            .iter()
            .find(|slot| slot.1 == item.belt)
            .unwrap();
        let start = slot.0.start;
        let position = start + item.position;
        self.lane.add_item_at(position, item.entity);
    }
}

/// Give a `Belt`, get its `BeltGroup`
#[derive(Resource, Default)]
struct BeltGroups(HashMap<Entity, Entity>);

/// Give a `WorldCoords`, get its `Belt`
#[derive(Resource, Default)]
struct BeltCoords(HashMap<WorldCoords, Entity>);

fn create_item(
    mut msgs: MessageReader<CreateBeltItem>,
    mut cmd: Commands,
    mut belt_groups: Query<&mut BeltGroup>,
    groups: Res<BeltGroups>,
) {
    for item in msgs.read() {
        let group = groups.0.get(&item.belt).unwrap();
        let mut group = belt_groups.get_mut(*group).unwrap();
        group.add_item_at(item);
        cmd.entity(item.entity).insert(ExpectedMovement(0));
    }
}

fn create_belt(
    world: &mut World,
    state: &mut SystemState<(
        MessageReader<CreateBelt>,
        ResMut<BeltGroups>,
        ResMut<BeltCoords>,
    )>,
) {
    let (mut msgs, _, _) = state.get_mut(world);
    let messages: Vec<_> = msgs.read().cloned().collect();
    state.apply(world);

    for belt in messages {
        let mut system_state: SystemState<(ResMut<BeltGroups>, ResMut<BeltCoords>)> =
            SystemState::new(world);
        let (groups_cache, mut belt_coords) = system_state.get_mut(world);

        belt_coords.0.insert(belt.coords, belt.entity);
        let belt_ahead = belt_coords.0.get(&belt.forward()).copied();
        let belt_behind = belt_coords.0.get(&belt.backward()).copied();

        match (belt_ahead, belt_behind) {
            (None, None) => {
                let group = BeltGroup::from_belt(belt.entity);
                system_state.apply(world);
                let g = world.spawn(group).id();
                let mut groups_cache = world.resource_mut::<BeltGroups>();
                groups_cache.0.insert(belt.entity, g);
            }
            (Some(belt_ahead), None) => {
                let group = *groups_cache
                    .0
                    .get(&belt_ahead)
                    .expect("all belts should be in cache");
                system_state.apply(world);
                world
                    .entity_mut(group)
                    .get_mut::<BeltGroup>()
                    .expect("the group should exist")
                    .add_belt_at_tail(belt.entity, 256);
                let mut groups_cache = world.resource_mut::<BeltGroups>();
                groups_cache.0.insert(belt.entity, group);
            }
            (None, Some(belt_behind)) => {
                let group = *groups_cache
                    .0
                    .get(&belt_behind)
                    .expect("all belts should be in cache");
                system_state.apply(world);
                world
                    .entity_mut(group)
                    .get_mut::<BeltGroup>()
                    .expect("the group should exist")
                    .add_belt_at_head(belt.entity, 256);
                let mut groups_cache = world.resource_mut::<BeltGroups>();
                groups_cache.0.insert(belt.entity, group);
            }
            (Some(_belt_ahead), Some(_belt_behind)) => {
                system_state.apply(world);
            }
        }
    }
}

fn plan_item_movement(
    belt_groups: Query<&mut BeltGroup>,
    mut items: Query<(&BeltItem, &mut ExpectedMovement)>,
) {
    for mut group in belt_groups {
        for (index, item) in group.lane.lane.iter_mut().enumerate() {
            if let Ok((_, mut expected_movement)) = items.get_mut(item.1) {
                let space = item.0 - (index as u16) * 64;
                expected_movement.0 = space.min(8);
            }
        }
    }
}

fn move_items(
    mut items: Query<(Mut<Transform>, &ExpectedMovement), Without<Belt>>,
    mut belt_groups: Query<&mut BeltGroup>,
    belts_q: Query<(&Belt, &WorldCoords)>,
) {
    for group in belt_groups.iter_mut() {
        let BeltGroup { belts, lane } = group.into_inner();
        for (pos, item_entity) in lane.lane.iter_mut() {
            if let Ok((mut transform, expected_movement)) = items.get_mut(*item_entity) {
                *pos -= expected_movement.0;
                let (range, belt_entity) = belts.belt_at_pos(*pos).cloned().unwrap();
                let (belt, &coords) = belts_q.get(belt_entity).unwrap();
                let local_pos = *pos - range.start;

                transform.translation = belt.item_position(coords, local_pos);
            } else {
                warn!("Couldn't find lane: {item_entity}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::game::*;

    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn test_from_belt() {
        let belt = Entity::from_bits(1);
        let actual = BeltGroup::from_belt(belt);
        let expected = BeltGroup {
            belts: OrderedBelts {
                belts: vec![(0..256, belt)],
            },
            lane: Lane { lane: vec![] },
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_add_belt_at_tail() {
        let belt1 = Entity::from_bits(1);
        let belt2 = Entity::from_bits(2);
        let belt3 = Entity::from_bits(3);

        let mut actual = BeltGroup::from_belt(belt1);
        actual.add_belt_at_tail(belt2, 256);
        actual.add_belt_at_tail(belt3, 256);

        let expected = BeltGroup {
            belts: OrderedBelts {
                belts: vec![(0..256, belt1), (256..512, belt2), (512..768, belt3)],
            },
            lane: Lane::default(),
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_add_belt_at_head() {
        let belt1 = Entity::from_bits(1);
        let belt2 = Entity::from_bits(2);

        let mut actual = BeltGroup::from_belt(belt1);
        actual.add_belt_at_head(belt2, 256);

        let expected = BeltGroup {
            belts: OrderedBelts {
                belts: vec![(0..256, belt2), (256..512, belt1)],
            },
            lane: Lane::default(),
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_add_belt_at_head_shifts_items() {
        let belt1 = Entity::from_bits(1);
        let belt2 = Entity::from_bits(2);
        let item = Entity::from_bits(100);

        let mut actual = BeltGroup::from_belt(belt1);
        actual.lane.add_item_at(128, item);
        actual.add_belt_at_head(belt2, 256);

        let expected = BeltGroup {
            belts: OrderedBelts {
                belts: vec![(0..256, belt2), (256..512, belt1)],
            },
            lane: Lane {
                lane: vec![(384, item)], // 128 + 256
            },
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_belt_ranges_match_world_coords() {
        let mut t = TestBuilder::new();

        // Create 5 belts at x=0,1,2,3,4 (like in main.rs)
        let belt0 = t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);
        let belt1 = t.spawn_belt(WorldCoords { x: 1, y: 0 }, Direction::East);
        let belt2 = t.spawn_belt(WorldCoords { x: 2, y: 0 }, Direction::East);
        let belt3 = t.spawn_belt(WorldCoords { x: 3, y: 0 }, Direction::East);
        let belt4 = t.spawn_belt(WorldCoords { x: 4, y: 0 }, Direction::East);

        t.app.update();

        // Get the actual belt group
        let world = t.app.world_mut();
        let groups = world.resource::<BeltGroups>();
        let group_entity = *groups.0.get(&belt0).expect("belt0 should have group");

        let mut group_query = world.query::<&BeltGroup>();
        let actual = group_query
            .get(world, group_entity)
            .expect("group should exist");

        // Expected: belts should be in order from x=0 to x=4
        // with contiguous ranges [0..256), [256..512), etc.
        let expected = BeltGroup {
            belts: OrderedBelts {
                belts: vec![
                    (0..256, belt4),
                    (256..512, belt3),
                    (512..768, belt2),
                    (768..1024, belt1),
                    (1024..1280, belt0),
                ],
            },
            lane: Lane::default(),
        };

        assert_eq!(*actual, expected);
    }

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
                coords,
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

    #[test]
    fn item_moves_to_next_belt() {
        let mut t = TestBuilder::new();
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);
        let item = t.with_item_at(0);
        t.spawn_belt(WorldCoords { x: 1, y: 0 }, Direction::East);

        t.app.update();

        let initial_position = t.get_transform(item).translation;

        t.app.update();

        let final_position = t.get_transform(item).translation;

        assert_ne!(
            initial_position, final_position,
            "Item transform should have changed after update"
        );
    }
}
