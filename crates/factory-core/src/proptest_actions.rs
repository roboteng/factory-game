use bevy::prelude::*;
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};

/// Actions that can be performed during property testing
#[derive(Debug, Clone)]
pub enum Action {
    PlaceBelt {
        coords: WorldCoords,
        dir: HDir,
    },
    PlaceItem {
        belt_coords: WorldCoords,
        pos: i32,
        lane: LaneSide,
    },
    Update,
}

/// Test state for tracking belt/item positions across frames
#[derive(Default)]
pub struct TestState {
    pub belt_coords: HashSet<WorldCoords>,
    pub belt_entities: HashMap<WorldCoords, Entity>,
    pub previous_item_positions: HashMap<Entity, Vec3>,
    pub skip_movement_check: HashSet<Entity>,
    pub frame_count: usize,
}

impl TestState {
    pub fn place_belt(&mut self, coords: WorldCoords, entity: Entity) {
        self.belt_coords.insert(coords);
        self.belt_entities.insert(coords, entity);
    }

    pub fn has_belt(&self, coords: WorldCoords) -> bool {
        self.belt_coords.contains(&coords)
    }

    pub fn get_belt(&self, coords: WorldCoords) -> Option<Entity> {
        self.belt_entities.get(&coords).copied()
    }

    pub fn capture_item_positions(&mut self, app: &mut App) {
        self.previous_item_positions.clear();

        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &Transform), With<Item>>();
        for (entity, transform) in query.iter(app.world()) {
            self.previous_item_positions
                .insert(entity, transform.translation);
        }

        self.skip_movement_check = self.get_items_on_replaced_belts(app);
    }

    pub fn next_frame(&mut self) {
        self.frame_count += 1;
        self.skip_movement_check.clear();
    }

    fn get_items_on_replaced_belts(&self, app: &mut App) -> HashSet<Entity> {
        let changes = app.world().resource::<BeltChanges>().0.clone();
        let mut affected_belt_entities = HashSet::new();

        for change in &changes {
            match change {
                BeltChange::Replaced(replaced) => {
                    if let Some(old_entity) = replaced.old_entity {
                        affected_belt_entities.insert(old_entity);
                    } else {
                        affected_belt_entities.insert(replaced.entity);
                    }
                }
                BeltChange::Removed(removed) => {
                    affected_belt_entities.insert(removed.entity);
                }
                _ => {}
            }
        }

        if affected_belt_entities.is_empty() {
            return HashSet::new();
        }

        let mut skip_items = HashSet::new();

        let mut lane_query = app.world_mut().query::<&BeltLane>();
        for lane in lane_query.iter(app.world()) {
            let affected_ranges: Vec<_> = lane
                .belts
                .iter()
                .filter(|entry| affected_belt_entities.contains(&entry.entity))
                .flat_map(|entry| vec![entry.ranges.left.clone(), entry.ranges.right.clone()])
                .collect();

            if affected_ranges.is_empty() {
                continue;
            }

            for item_entry in lane.lanes.left.iter().chain(lane.lanes.right.iter()) {
                if affected_ranges
                    .iter()
                    .any(|range| range.contains(&item_entry.pos))
                {
                    skip_items.insert(item_entry.entity);
                }
            }
        }

        skip_items
    }
}

// Proptest strategies

pub fn arb_coords() -> impl Strategy<Value = WorldCoords> {
    (-3..=3i32, -3..=3i32, 0..=0i32).prop_map(|(x, y, z)| WorldCoords::from((x, y, z)))
}

pub fn arb_dir() -> impl Strategy<Value = HDir> {
    prop_oneof![
        Just(HDir::North),
        Just(HDir::East),
        Just(HDir::South),
        Just(HDir::West),
    ]
}

pub fn arb_lane() -> impl Strategy<Value = LaneSide> {
    prop_oneof![Just(LaneSide::Left), Just(LaneSide::Right),]
}

pub fn arb_belt_position() -> impl Strategy<Value = i32> {
    0i32..POSITIONS_PER_BELT
}

pub fn arb_action() -> impl Strategy<Value = Action> {
    prop_oneof![
        5 => Just(Action::Update),
        3 => (arb_coords(), arb_dir()).prop_map(|(coords, dir)| Action::PlaceBelt { coords, dir }),
        2 => (arb_coords(), arb_belt_position(), arb_lane()).prop_map(|(belt_coords, pos, lane)| Action::PlaceItem { belt_coords, pos, lane }),
    ]
}

fn is_valid_action_sequence(actions: &[Action]) -> bool {
    let mut newly_placed_belts = HashSet::new();

    for action in actions {
        match action {
            Action::PlaceBelt { coords, .. } => {
                newly_placed_belts.insert(*coords);
            }
            Action::PlaceItem { belt_coords, .. } => {
                if newly_placed_belts.contains(belt_coords) {
                    return false;
                }
            }
            Action::Update => {
                newly_placed_belts.clear();
            }
        }
    }

    true
}

pub fn arb_action_sequence() -> impl Strategy<Value = Vec<Action>> {
    proptest::collection::vec(arb_action(), 0..100).prop_filter(
        "Cannot place item on belt without Update between PlaceBelt and PlaceItem",
        |actions| is_valid_action_sequence(actions),
    )
}

fn has_duplicate_coords(actions: &[Option<Action>]) -> bool {
    let mut coords_seen = HashSet::new();
    for action in actions {
        if let Some(Action::PlaceBelt { coords, .. }) = action {
            if !coords_seen.insert(coords) {
                return true;
            }
        }
    }
    false
}

pub fn arb_action_vec_for_shuffle() -> impl Strategy<Value = (usize, Vec<Option<Action>>, u64)> {
    use proptest::option;

    (0usize..=50)
        .prop_flat_map(|size| {
            let action_strategy = prop_oneof![
                5 => (arb_coords(), arb_dir()).prop_map(|(coords, dir)|
                    Action::PlaceBelt { coords, dir }
                ),
                2 => Just(Action::Update),
            ];
            let actions = proptest::collection::vec(option::of(action_strategy), size..=size);
            let seed = any::<u64>();
            (Just(size), actions, seed)
        })
        .prop_filter(
            "No duplicate coordinates (belt replacements)",
            |(_, actions, _)| !has_duplicate_coords(actions),
        )
}
