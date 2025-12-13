use crate::game::*;
use bevy::log::tracing::instrument;
use bevy::log::{debug, info, warn};
use std::{collections::HashMap, ops::Range};

const ITEM_SPACING: u16 = 64;
const BELT_SPEED: u16 = 8;

pub struct SimPlugin;
impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BeltGroups>();
        app.add_observer(on_belt_created);
        app.add_observer(on_create_item);
        app.add_systems(
            Update,
            (plan_item_movement, move_items.after(plan_item_movement)),
        );
    }
}

#[derive(Component)]
pub struct ExpectedMovement(u16);

#[derive(Debug, PartialEq, Clone)]
struct OrderedBelts {
    belts: Vec<(Range<u16>, Entity)>,
}

impl OrderedBelts {
    fn belt_at_pos(&self, pos: u16) -> Option<&(Range<u16>, Entity)> {
        self.belts.iter().find(|&belt| belt.0.contains(&pos))
    }
}

impl OrderedBelts {
    fn count_positions(&self) -> u16 {
        self.belts.last().expect("Belts should not be empty").0.end
    }
}

#[derive(Default, Debug, PartialEq, Clone)]
struct Lane {
    lane: Vec<(u16, Entity)>,
}

impl Lane {
    fn add_item_at(&mut self, pos: u16, item: Entity) {
        let insert_idx = self
            .lane
            .binary_search_by(|probe| probe.0.cmp(&pos))
            .unwrap_or_else(|idx| idx);
        self.lane.insert(insert_idx, (pos, item));
    }
}

#[derive(Component, Debug, PartialEq, Clone)]
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
    fn add_item_at(&mut self, belt: Entity, position: u16, item_entity: Entity) {
        let slot = self.belts.belts.iter().find(|slot| slot.1 == belt).unwrap();
        let global_position = slot.0.start + position;
        self.lane.add_item_at(global_position, item_entity);
    }
    fn join_from_belt(&mut self, joining_belt: Entity, n_pos: u16, other: Self) {
        self.add_belt_at_tail(joining_belt, n_pos);
        self.join(other);
    }
    pub fn join(&mut self, other: Self) {
        let offset = self.belts.count_positions();
        debug!(
            offset = offset,
            joining_belts = ?other.belts.belts.iter().map(|(_, b)| b).collect::<Vec<_>>(),
            "Joining belt groups"
        );
        let new_belts = other
            .belts
            .belts
            .iter()
            .map(|(range, e)| ((range.start + offset)..(range.end + offset), *e));
        let new_lane = other.lane.lane.iter().map(|(pos, e)| (pos + offset, *e));
        self.belts.belts.extend(new_belts);
        self.lane.lane.extend(new_lane);
    }

    pub fn truncate_by(&mut self, n_pos: u16) -> Lane {
        let last = self
            .belts
            .belts
            .last_mut()
            .expect("Groups should always have belts");
        last.0.end -= n_pos;
        let new_end = last.0.end;
        let at = self.lane.lane.partition_point(|(pos, _)| *pos < new_end);
        let lane = self
            .lane
            .lane
            .split_off(at)
            .iter()
            .map(|a| (a.0 - new_end, a.1))
            .collect();
        Lane { lane }
    }

    pub fn expand_by(&mut self, n_pos: u16) -> u16 {
        let last = self
            .belts
            .belts
            .last_mut()
            .expect("Group should have belts");
        let old_end = last.0.end;
        last.0.end += n_pos;
        old_end
    }

    pub fn split_off_after(&mut self, belt: Entity) -> Option<BeltGroup> {
        // Find the belt in the group
        let belt_idx = self.belts.belts.iter().position(|(_, e)| *e == belt)?;

        debug!(
            belt = ?belt,
            belt_idx = belt_idx,
            total_belts = self.belts.belts.len(),
            "Splitting belt group after belt"
        );

        // Split off all belts after this one
        let remaining_belts = self.belts.belts.split_off(belt_idx + 1);

        if remaining_belts.is_empty() {
            debug!("No belts after split point, returning None");
            return None;
        }

        // Calculate the split point (end of the kept belt)
        let split_point = self.belts.belts.last()?.0.end;

        // Split items at the split point
        let item_split_idx = self
            .lane
            .lane
            .partition_point(|(pos, _)| *pos < split_point);
        let remaining_items = self.lane.lane.split_off(item_split_idx);

        // Adjust positions in the new group to start from 0
        let new_belts = remaining_belts
            .into_iter()
            .map(|(range, e)| ((range.start - split_point)..(range.end - split_point), e))
            .collect();

        let new_items = remaining_items
            .into_iter()
            .map(|(pos, e)| (pos - split_point, e))
            .collect();

        Some(BeltGroup {
            belts: OrderedBelts { belts: new_belts },
            lane: Lane { lane: new_items },
        })
    }

    fn apply_fragment(&mut self, fragment: Lane) {
        // There is a more efficient way to do this
        self.lane.lane.extend(fragment.lane);
        self.lane.lane.sort();
    }
}

/// Give a `Belt`, get its `BeltGroup`
#[derive(Resource, Default, Debug)]
struct BeltGroups(HashMap<Entity, Entity>);

fn on_create_item(
    trigger: On<CreateBeltItem>,
    mut cmd: Commands,
    mut belt_groups: Query<&mut BeltGroup>,
    groups: Res<BeltGroups>,
) {
    let group = groups.0.get(&trigger.belt).unwrap();
    let mut group = belt_groups.get_mut(*group).unwrap();

    let item_entity = trigger.entity;
    group.add_item_at(trigger.belt, trigger.position, item_entity);
    cmd.entity(item_entity).insert(ExpectedMovement(0));
}

#[instrument(skip_all, fields(belt_coords = ?belt_coords.0, trigger = ?trigger, groups = ?groups))]
fn on_belt_created(
    trigger: On<BeltCreated>,
    mut cmd: Commands,
    mut groups: ResMut<BeltGroups>,
    belt_coords: Res<BeltCoords>,
    mut belt_groups_query: Query<&mut BeltGroup>,
) {
    let belt_entity = trigger.entity;
    if let Some(old_belt) = trigger.old_belt {
        replace_belt(
            trigger.clone(),
            belt_coords.into_inner(),
            groups.into_inner(),
            belt_groups_query,
            cmd,
            old_belt,
        );
    } else {
        debug!("Creating new belt group");
        let belt_ahead = belt_coords
            .get(&trigger.coords_ahead())
            .filter(|b| trigger.new_belt.feeds(&b.1));
        let belt_behind = trigger.new_belt_behind;

        assert_eq!(
            belt_behind,
            belt_coords
                .get(&trigger.coords_behind())
                .filter(|b| b.1.feeds(&trigger.new_belt))
        );

        match (belt_ahead, belt_behind) {
            (None, None) => {
                spawn_new_group(trigger.clone(), cmd, &mut groups);
            }
            (Some((belt_ahead, _)), None) => {
                let &group = groups
                    .0
                    .get(&belt_ahead)
                    .expect("Belt should a part of a group");
                let mut belt_group = belt_groups_query
                    .get_mut(group)
                    .expect("Group should be created already");
                belt_group.add_belt_at_tail(belt_entity, trigger.new_belt.number_of_positions());
                groups.0.insert(belt_entity, group);
                info!(
                    belt = ?belt_entity,
                    group = ?group,
                    "Added belt to tail of group"
                );
            }
            (None, Some((belt_behind, belt))) => {
                if belt.feeds(&trigger.new_belt) {
                    {
                        let &group = groups
                            .0
                            .get(&belt_behind)
                            .expect("Belt should be a part of a group");
                        let mut belt_group = belt_groups_query
                            .get_mut(group)
                            .expect("Groupd should ber created already");
                        belt_group
                            .add_belt_at_head(belt_entity, trigger.new_belt.number_of_positions());
                        groups.0.insert(belt_entity, group);
                        info!(
                            belt = ?belt_entity,
                            group = ?group,
                            "Added belt to head of group"
                        );
                    }
                } else {
                    spawn_new_group(trigger.clone(), cmd, &mut groups);
                }
            }
            (Some(belt_ahead), Some(belt_behind)) => {
                match (
                    trigger.new_belt.feeds(&belt_ahead.1),
                    belt_behind.1.feeds(&trigger.new_belt),
                ) {
                    (true, true) => {
                        let &group_behind = groups
                            .0
                            .get(&belt_behind.0)
                            .expect("Belt should be a part of a group");
                        cmd.entity(group_behind).despawn();
                        let group_behind = belt_groups_query
                            .get_mut(group_behind)
                            .expect("Groupd should ber created already")
                            .clone();
                        let &group_ahead_entity = groups
                            .0
                            .get(&belt_ahead.0)
                            .expect("Belt should be a part of a group");
                        let mut group_ahead = belt_groups_query
                            .get_mut(group_ahead_entity)
                            .expect("Groupd should ber created already");
                        group_ahead.join_from_belt(
                            trigger.entity,
                            trigger.new_belt.number_of_positions(),
                            group_behind.clone(),
                        );
                        // We're ssetting all the belts that are already in the group
                        // That is unneccassery, but we can change that if needed
                        for (_, belt) in &group_ahead.belts.belts {
                            groups.0.insert(*belt, group_ahead_entity);
                        }
                    }
                    _ => todo!("Handle additional cases"),
                }
            }
        }
    }
}

#[instrument(skip_all)]
fn replace_belt(
    trigger: BeltCreated,
    belt_coords: &BeltCoords,
    groups: &mut BeltGroups,
    mut belt_groups_query: Query<&mut BeltGroup>,
    mut cmd: Commands,
    old_belt: Belt,
) {
    info!("Replacing belt due to direction change");
    assert_ne!(
        old_belt.input_direction(),
        trigger.new_belt.input_direction(),
        "We assume the input direction of the old belt is different from the new belt"
    );
    // todo: check for input direction
    let old_behind = belt_coords.get(&trigger.coords.step(old_belt.output_direction().opposite()));
    let new_behind = belt_coords.get(&trigger.coords_behind());

    match (old_behind, new_behind) {
        (None, None) => todo!("None, None"),
        (None, Some(new_belt)) => {
            let group = groups.0.get(&new_belt.0).unwrap().clone();
            let mut new_belt_group = belt_groups_query.get(group).unwrap().clone();
            let current_group_ent = groups.0.get(&trigger.entity).unwrap().clone();
            let mut current_group = belt_groups_query.get_mut(current_group_ent).unwrap();
            // Length change
            if trigger.new_belt.number_of_positions() < old_belt.number_of_positions() {
                let frag = current_group.truncate_by(
                    old_belt.number_of_positions() - trigger.new_belt.number_of_positions(),
                );
                new_belt_group.apply_fragment(frag);
            } else {
                todo!("Handle this case");
            }
            current_group.join(new_belt_group);
            reassing_belts_to_group(groups, current_group_ent, &current_group);
            cmd.entity(group).despawn();
        }
        (Some(_), None) => todo!("Some, None"),
        (Some(old), Some(new_fed_from_belt)) => {
            assert_ne!(old.0, new_fed_from_belt.0, "Entities should be different");
            // We are replacing a belt with a new one
            // There are belts feeding into this one from the old belt and new belt

            // split from previous
            let current_group_ent = groups.0.get(&trigger.entity).unwrap();
            let mut current_group = belt_groups_query.get_mut(*current_group_ent).unwrap();
            debug!(current_group = ?&current_group, after = ?trigger.entity, "splitting group");
            let leftover = current_group
                .split_off_after(trigger.entity)
                .expect("We should have been fed from this belt");
            assert!(
                !leftover
                    .belts
                    .belts
                    .iter()
                    .any(|b| b.1 == new_fed_from_belt.0)
            );
            let leftover_ent = cmd.spawn(leftover.clone()).id();
            reassing_belts_to_group(groups, leftover_ent, &leftover);

            // Join to new
            let new_fed_from_group_ent = groups.0.get(&new_fed_from_belt.0).unwrap();
            let new_fed_from_group = belt_groups_query
                .get(*new_fed_from_group_ent)
                .unwrap()
                .clone();
            assert!(
                old_belt.number_of_positions() < trigger.new_belt.number_of_positions(),
                "Belt should be straight"
            );
            let diff = trigger.new_belt.number_of_positions() - old_belt.number_of_positions();
            let current_group_ent = groups.0.get(&trigger.entity).unwrap().clone();
            let mut current_group = belt_groups_query.get_mut(current_group_ent).unwrap();

            current_group.expand_by(diff);
            current_group.join(new_fed_from_group);
            reassing_belts_to_group(groups, current_group_ent, &current_group);
        }
    }
}

fn reassing_belts_to_group(groups: &mut BeltGroups, new_group_entity: Entity, group: &BeltGroup) {
    for (_, belt) in &group.belts.belts {
        groups.0.insert(*belt, new_group_entity);
    }
    info!(
        new_group = ?new_group_entity,
        belts = ?group.belts.belts.iter().map(|(_, b)| b).collect::<Vec<_>>(),
        "Reassigned belts to new group"
    );
}

fn spawn_new_group(trigger: BeltCreated, mut cmd: Commands, groups: &mut BeltGroups) {
    let group = BeltGroup::from_belt(trigger.entity);
    let group_entity = cmd.spawn(group.clone()).id();
    groups.0.insert(trigger.entity, group_entity);
    info!(
        belt = ?trigger.entity,
        group = ?group_entity,
        coords = ?trigger.coords,
        "Spawned new belt group"
    );
}

fn plan_item_movement(
    belt_groups: Query<&mut BeltGroup>,
    mut items: Query<(&BeltItem, &mut ExpectedMovement)>,
) {
    for mut group in belt_groups {
        for (index, item) in group.lane.lane.iter_mut().enumerate() {
            if let Ok((_, mut expected_movement)) = items.get_mut(item.1) {
                let space = item.0 - (index as u16) * ITEM_SPACING;
                expected_movement.0 = space.min(BELT_SPEED);
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

    use crate::game::tests::{TestBuilder, test_app as core_test_app};
    use crate::game::*;

    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use pretty_assertions::{assert_eq, assert_ne};

    const FRAME_TIME: f32 = 1.0 / 60.0;

    fn init_test_tracing() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            // Initialize tracing for tests - use RUST_LOG env var to control level
            // Example: RUST_LOG=debug cargo test
            let _ = bevy::log::tracing_subscriber::fmt()
                .with_test_writer()
                .without_time()
                .with_env_filter(
                    bevy::log::tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive("factory_game=debug".parse().unwrap()),
                )
                .try_init();
        });
    }

    fn test_app() -> App {
        init_test_tracing();
        let mut app = core_test_app();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            FRAME_TIME,
        )));
        app.add_plugins(SimPlugin);
        app
    }

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
        let mut t = TestBuilder::new(test_app());

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

    #[test]
    fn item_on_belt() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords::default(), Direction::East);
        let item = t.with_item_at(0);
        t.app.update();
        let _ = t.get_transform(item);
    }

    #[test]
    fn item_moves_on_belt() {
        let mut t = TestBuilder::new(test_app());
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
        let mut t = TestBuilder::new(test_app());
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
        let mut t = TestBuilder::new(test_app());
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

    // North direction tests
    #[test]
    fn item_moves_on_belt_north() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords::default(), Direction::North);
        let item = t.with_item_at(128);
        t.app.update();
        let initial_transform = t.get_transform(item);
        t.app.update();
        let next_transform = t.get_transform(item);

        assert!(
            next_transform.translation.y > initial_transform.translation.y,
            "Item should have moved along the belt (North direction means increasing Y as it progresses). Initial: {}, Final: {}",
            initial_transform.translation.y,
            next_transform.translation.y
        );
    }

    #[test]
    fn item_doesnt_move_at_end_of_belt_north() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords::default(), Direction::North);
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
    fn item_moves_to_next_belt_north() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::North);
        let item = t.with_item_at(0);
        t.spawn_belt(WorldCoords { x: 0, y: 1 }, Direction::North);

        t.app.update();

        let initial_position = t.get_transform(item).translation;

        t.app.update();

        let final_position = t.get_transform(item).translation;

        assert_ne!(
            initial_position, final_position,
            "Item transform should have changed after update"
        );
    }

    // South direction tests
    #[test]
    fn item_moves_on_belt_south() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords::default(), Direction::South);
        let item = t.with_item_at(128);
        t.app.update();
        let initial_transform = t.get_transform(item);
        t.app.update();
        let next_transform = t.get_transform(item);

        assert!(
            next_transform.translation.y < initial_transform.translation.y,
            "Item should have moved along the belt (South direction means decreasing Y as it progresses). Initial: {}, Final: {}",
            initial_transform.translation.y,
            next_transform.translation.y
        );
    }

    #[test]
    fn item_doesnt_move_at_end_of_belt_south() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords::default(), Direction::South);
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
    fn item_moves_to_next_belt_south() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::South);
        let item = t.with_item_at(0);
        t.spawn_belt(WorldCoords { x: 0, y: -1 }, Direction::South);

        t.app.update();

        let initial_position = t.get_transform(item).translation;

        t.app.update();

        let final_position = t.get_transform(item).translation;

        assert_ne!(
            initial_position, final_position,
            "Item transform should have changed after update"
        );
    }

    // West direction tests
    #[test]
    fn item_moves_on_belt_west() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords::default(), Direction::West);
        let item = t.with_item_at(128);
        t.app.update();
        let initial_transform = t.get_transform(item);
        t.app.update();
        let next_transform = t.get_transform(item);

        assert!(
            next_transform.translation.x < initial_transform.translation.x,
            "Item should have moved along the belt (West direction means decreasing X as it progresses). Initial: {}, Final: {}",
            initial_transform.translation.x,
            next_transform.translation.x
        );
    }

    #[test]
    fn item_doesnt_move_at_end_of_belt_west() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords::default(), Direction::West);
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
    fn item_moves_to_next_belt_west() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::West);
        let item = t.with_item_at(0);
        t.spawn_belt(WorldCoords { x: -1, y: 0 }, Direction::West);

        t.app.update();

        let initial_position = t.get_transform(item).translation;

        t.app.update();

        let final_position = t.get_transform(item).translation;

        assert_ne!(
            initial_position, final_position,
            "Item transform should have changed after update"
        );
    }

    #[test]
    fn belt_is_different_direction_behind() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);
        let item = t.with_item_at(0);
        t.spawn_belt(WorldCoords { x: 0, y: -1 }, Direction::South);

        t.app.update();

        let actual = t.get_transform(item).translation;
        let expected = Vec3::new(16.0, 0.0, 2.0);

        assert_eq!(actual, expected);
    }

    #[test]
    fn place_belts_between_two_others() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);
        let item = t.with_item_at(0);
        t.spawn_belt(WorldCoords { x: 2, y: 0 }, Direction::East);
        t.spawn_belt(WorldCoords { x: 1, y: 0 }, Direction::East);

        for _ in 0..64 {
            t.app.update();
        }

        let actual = t.get_transform(item).translation;
        let expected = Vec3::new(TILE_SIZE * 2.5, 0.0, 2.0);

        assert_eq!(actual, expected);
    }

    #[test]
    fn curved_belt_at_start_ccw() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);
        t.spawn_belt(WorldCoords { x: 1, y: 0 }, Direction::North);
        let item = t.with_item_at(0);

        t.app.update();

        let actual = t.get_transform(item).translation;
        let expected = Vec3::new(TILE_SIZE, TILE_SIZE / 2.0, 2.0);

        assert_eq!(actual, expected);
    }

    #[track_caller]
    fn assert_close(actual: Vec3, expected: Vec3) {
        let dist = actual.distance(expected);

        assert!(
            dist < 0.99,
            "Distance between \n{actual:?}\nand\n{expected:?}\nis {dist}",
        );
    }

    #[test]
    fn curved_belt_halfway_ccw() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);
        t.spawn_belt(WorldCoords { x: 1, y: 0 }, Direction::North);
        let item = t.with_item_at(101);

        t.app.update();

        let actual = t.get_transform(item).translation;
        let expected = Vec3::new(
            TILE_SIZE * (0.5 + 0.5 * (PI / 4.0).cos()),
            TILE_SIZE * (0.5 - 0.5 * (PI / 4.0).sin()),
            2.0,
        );
        assert_close(actual, expected);
    }

    #[test]
    fn curved_belt_at_start_cw() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);
        t.spawn_belt(WorldCoords { x: 1, y: 0 }, Direction::South);
        let item = t.with_item_at(0);

        t.app.update();

        let actual = t.get_transform(item).translation;
        let expected = Vec3::new(TILE_SIZE, -TILE_SIZE / 2.0, 2.0);

        assert_eq!(actual, expected);
    }

    #[test]
    fn curved_belt_halfway_cw() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);
        t.spawn_belt(WorldCoords { x: 1, y: 0 }, Direction::South);
        let item = t.with_item_at(101);

        t.app.update();

        let actual = t.get_transform(item).translation;
        let expected = Vec3::new(
            TILE_SIZE * (0.5 + 0.5 * (PI / 4.0).cos()),
            -TILE_SIZE * (0.5 - 0.5 * (PI / 4.0).sin()),
            2.0,
        );
        assert_close(actual, expected);
    }

    #[test]
    fn place_xxx() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 1, y: 0 }, Direction::North);
        let item = t.with_item_at(255);
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);

        t.app.update();

        let actual = t.get_transform(item).translation;
        let magic_x = 10.25;
        let expected = Vec3::new(magic_x, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_on_curved_belt_in_multi_belt_group() {
        let mut t = TestBuilder::new(test_app());
        let _ = t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::East);
        let item = t.with_item_at(0);
        let _ = t.spawn_belt(WorldCoords { x: 1, y: 0 }, Direction::North);

        t.app.update();

        let actual = t.get_transform(item).translation;
        let expected = Vec3::new(17.0, 0.0, 2.0);
        assert_close(actual, expected);
    }

    #[test]
    fn item_on_north_to_west_curve_at_halfway() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::North);
        t.spawn_belt(WorldCoords { x: 0, y: 1 }, Direction::West);
        let item = t.with_item_at(101);

        t.app.update();

        let actual = t.get_transform(item).translation;
        // At position 101 (halfway through curve), should be at 45 degrees
        let expected = Vec3::new(
            -TILE_SIZE * (0.5 - 0.5 * (PI / 4.0).sin()),
            TILE_SIZE * (0.5 + 0.5 * (PI / 4.0).cos()),
            2.0,
        );

        assert_close(actual, expected);
    }

    #[test]
    fn curved_belt_with_north_input() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::North);
        t.spawn_belt(WorldCoords { x: 0, y: 1 }, Direction::West);
        let item = t.with_item_at(0);

        t.app.update();

        // Item at position 0 on North→West curve
        // Should be at entry point of the curve
        let actual = t.get_transform(item).translation;
        let expected = Vec3::new(-TILE_SIZE / 2.0, TILE_SIZE, 2.0);

        assert_close(actual, expected);
    }

    #[test]
    fn two_belt_point_in() {
        let mut t = TestBuilder::new(test_app());
        t.spawn_belt(WorldCoords { x: 0, y: 0 }, Direction::North);
        t.spawn_belt(WorldCoords { x: 1, y: 0 }, Direction::West);
        t.spawn_belt(WorldCoords { x: 0, y: -1 }, Direction::North);
        let item = t.with_item_at(0);

        t.app.update();

        let actual = t.get_transform(item).translation;
        let expected = Vec3::new(0.0, -TILE_SIZE / 2.0 + 1.0, 2.0);
        println!("Actual: {:?}, Expected: {:?}", actual, expected);
        assert_eq!(actual, expected);
    }
}
